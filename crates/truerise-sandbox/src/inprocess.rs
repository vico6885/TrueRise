//! The built-in in-process provider: executes actions as file writes inside a
//! fenced work directory. Deliberately minimal - it exists so the conformance
//! suite has a real provider to probe and so the harness can run without any
//! external container runtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::provider::{
    CapabilityBits, CapabilityLevel, ExecutionOutput, SandboxError, SandboxHandle, SandboxProvider,
};

/// An in-process sandbox: writes are confined to the work directory.
#[derive(Debug)]
pub struct InProcessProvider {
    root: PathBuf,
    live: Mutex<HashMap<u64, PathBuf>>,
}

impl InProcessProvider {
    /// Create a provider rooted at ``root`` (all workdirs live under it).
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            live: Mutex::new(HashMap::new()),
        }
    }

    fn fenced(&self, workdir: &Path, rel: &str) -> Result<PathBuf, SandboxError> {
        // Component-level check (Path::starts_with is purely lexical and does
        // NOT resolve `..` - the conformance escape probe exists precisely to
        // catch that). ParentDir / RootDir / Prefix components are refused.
        let rel_path = Path::new(rel);
        for comp in rel_path.components() {
            if matches!(
                comp,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            ) {
                return Err(SandboxError::Refused(format!(
                    "path escapes the sandbox fence: {rel:?} (workdir {workdir:?})"
                )));
            }
        }
        let target = workdir.join(rel_path);
        if !target.starts_with(workdir) {
            return Err(SandboxError::Refused(format!(
                "path escapes the sandbox fence: {target:?} (workdir {workdir:?})"
            )));
        }
        Ok(target)
    }
}

impl SandboxProvider for InProcessProvider {
    fn create(&self, id: u64, workdir: &Path) -> Result<SandboxHandle, SandboxError> {
        if !workdir.starts_with(&self.root) {
            return Err(SandboxError::Refused(format!(
                "workdir escapes the fence: {workdir:?}"
            )));
        }
        std::fs::create_dir_all(workdir)?;
        self.live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id, workdir.to_path_buf());
        Ok(SandboxHandle(id))
    }

    fn execute(
        &self,
        handle: SandboxHandle,
        action: &str,
        payload: &HashMap<String, String>,
    ) -> Result<ExecutionOutput, SandboxError> {
        let workdir = self
            .live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&handle.0)
            .cloned()
            .ok_or(SandboxError::UnknownHandle(handle.0))?;
        match action {
            "write_file" => {
                let rel = payload
                    .get("path")
                    .ok_or_else(|| SandboxError::Refused("missing path".into()))?;
                let content = payload.get("content").cloned().unwrap_or_default();
                let target = self.fenced(&workdir, rel)?;
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, content.as_bytes())?;
                Ok(ExecutionOutput {
                    code: 0,
                    stdout: format!("wrote {rel}"),
                    stderr: String::new(),
                    capability: CapabilityLevel::Full,
                })
            }
            "read_file" => {
                let rel = payload
                    .get("path")
                    .ok_or_else(|| SandboxError::Refused("missing path".into()))?;
                let target = self.fenced(&workdir, rel)?;
                let content = std::fs::read_to_string(&target)?;
                Ok(ExecutionOutput {
                    code: 0,
                    stdout: content,
                    stderr: String::new(),
                    capability: CapabilityLevel::Full,
                })
            }
            other => Err(SandboxError::Refused(format!("unknown action {other}"))),
        }
    }

    fn destroy(&self, handle: SandboxHandle) -> Result<(), SandboxError> {
        let workdir = self
            .live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&handle.0)
            .ok_or(SandboxError::UnknownHandle(handle.0))?;
        if workdir.exists() {
            std::fs::remove_dir_all(&workdir)?;
        }
        Ok(())
    }

    fn metrics(&self, handle: SandboxHandle) -> Result<HashMap<String, u64>, SandboxError> {
        let live = self.live.lock().unwrap_or_else(|p| p.into_inner());
        let _workdir = live
            .get(&handle.0)
            .ok_or(SandboxError::UnknownHandle(handle.0))?;
        let mut m = HashMap::new();
        m.insert("live_handles".to_string(), live.len() as u64);
        m.insert("handle_id".to_string(), handle.0);
        Ok(m)
    }

    fn audit_handle(&self, handle: SandboxHandle) -> Result<String, SandboxError> {
        Ok(format!("sandbox.inprocess.handle.{}", handle.0))
    }
}

/// Declares this provider's abilities: no snapshot/fork/pause/resume (M2).
pub fn declared_capabilities() -> CapabilityBits {
    CapabilityBits::NONE
}

/// Allocates the next sandbox id (single point, like the lease epoch).
pub fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
