//! The sandbox-backed `ToolProvider` adapter (RFC 0002): maps core tool
//! requests onto sandbox actions inside a fenced work directory. The
//! capability level in every outcome is the sandbox's OWN return value.

use std::collections::HashMap;

use truerise_core::{ToolError, ToolOutcome, ToolProvider, ToolRequest};

use crate::inprocess::{InProcessProvider, next_id};
use crate::provider::{SandboxError, SandboxHandle, SandboxProvider};

/// One live, fenced executor over an [`InProcessProvider`] handle. Each run
/// assembles its own instance rooted at the run's work directory.
pub struct SandboxToolProvider {
    provider: InProcessProvider,
    handle: SandboxHandle,
}

impl SandboxToolProvider {
    /// Assemble an executor whose fence is ``workdir`` (created on demand).
    ///
    /// # Errors
    /// [`ToolError::Refused`] when the workdir escapes the provider root or
    /// cannot be created (fail-closed: no executor, no executions).
    pub fn new(provider: InProcessProvider, workdir: &std::path::Path) -> Result<Self, ToolError> {
        let handle = SandboxHandle(next_id());
        provider
            .create(handle.0, workdir)
            .map_err(|e| ToolError::Refused(e.to_string()))?;
        Ok(Self { provider, handle })
    }

    /// Release the sandbox handle (idempotent per handle; files under the
    /// workdir are removed by the provider's destroy semantics).
    ///
    /// # Errors
    /// [`ToolError::Refused`] when the provider refuses the destroy call.
    pub fn destroy(&self) -> Result<(), ToolError> {
        self.provider
            .destroy(self.handle)
            .map_err(|e| ToolError::Refused(e.to_string()))
    }
}

impl ToolProvider for SandboxToolProvider {
    fn execute(&self, request: &ToolRequest) -> Result<ToolOutcome, ToolError> {
        let mut payload: HashMap<String, String> = HashMap::new();
        if let Some(args) = request.args.as_object() {
            for (key, value) in args {
                if let Some(text) = value.as_str() {
                    payload.insert(key.clone(), text.to_string());
                }
            }
        }
        // String-typed args only: the fenced actions (write_file/read_file)
        // are string-shaped by contract; anything else is refused below.
        let well_formed = request
            .args
            .as_object()
            .is_some_and(|args| args.values().all(|v| v.is_string() || v.is_null()));
        if !well_formed {
            return Err(ToolError::Refused("tool args must be string-valued".into()));
        }
        let output = self
            .provider
            .execute(self.handle, &request.tool, &payload)
            .map_err(sandbox_refused)?;
        Ok(ToolOutcome {
            code: output.code,
            stdout: output.stdout,
            capability: output.capability.as_str().to_string(),
        })
    }
}

/// Every sandbox error is a refusal: the action did not happen (fail-closed).
fn sandbox_refused(err: SandboxError) -> ToolError {
    ToolError::Refused(err.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_root() -> std::path::PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "truerise_sbx_tool_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(tool: &str, args: serde_json::Value) -> ToolRequest {
        ToolRequest {
            tool: tool.into(),
            args,
        }
    }

    #[test]
    fn writes_land_inside_the_fence_and_carry_capability() {
        let root = temp_root();
        let provider = InProcessProvider::new(&root);
        let executor = SandboxToolProvider::new(provider, &root.join("work")).unwrap();
        let out = executor
            .execute(&request(
                "write_file",
                json!({"path": "out/report.md", "content": "hi"}),
            ))
            .unwrap();
        assert_eq!(out.code, 0);
        assert_eq!(out.capability, "full", "capability is the sandbox's value");
        assert!(root.join("work/out/report.md").exists());
        executor.destroy().unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn escape_is_refused_and_nothing_is_written() {
        let root = temp_root();
        let provider = InProcessProvider::new(&root);
        let executor = SandboxToolProvider::new(provider, &root.join("work")).unwrap();
        let err = executor
            .execute(&request(
                "write_file",
                json!({"path": "../escape.md", "content": "x"}),
            ))
            .unwrap_err();
        assert!(err.to_string().contains("refused"));
        assert!(
            !root.join("escape.md").exists(),
            "no side effect outside the fence"
        );
        executor.destroy().unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn non_string_args_are_refused() {
        let root = temp_root();
        let provider = InProcessProvider::new(&root);
        let executor = SandboxToolProvider::new(provider, &root.join("work")).unwrap();
        let err = executor
            .execute(&request("write_file", json!({"path": 42})))
            .unwrap_err();
        assert!(err.to_string().contains("string-valued"));
        executor.destroy().unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}
