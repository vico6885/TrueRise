//! Tool execution ports (RFC 0002): the loop stays executor-agnostic and
//! policy-free. `ActionPolicy` decides; `ToolProvider` executes; the audit
//! trail records every hop. Denial is an audited outcome, never a run error.

use serde_json::Value;

/// One tool execution request (parsed from an act payload envelope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRequest {
    /// Tool name (e.g. `write_file`); unknown names are refused by the provider.
    pub tool: String,
    /// Tool arguments (string-valued; paths are fenced by the executor).
    pub args: Value,
}

/// The outcome of one allowed execution. The capability level is the
/// EXECUTOR's value (what actually ran), never a declared one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutcome {
    /// Exit-style code (0 = ok).
    pub code: i32,
    /// Captured output (the observation text fed back into the loop).
    pub stdout: String,
    /// Capability level in force for THIS execution (wire name).
    pub capability: String,
}

/// Errors from the tool surface. Every error is FAIL-CLOSED: the action did
/// not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// The executor refused the action (fence escape, unknown tool, io).
    #[error("tool refused: {0}")]
    Refused(String),
}

/// The loop's decision for one request. Mirrors the three-state governance
/// semantics without importing them (ports rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// Execute.
    Allow,
    /// An approval must be on file first; the request is NOT executed.
    NeedsApproval,
    /// Refused outright.
    Deny,
}

/// The policy port: three-state judgment for one request. Implementations
/// own the approval-on-file lookup (approvals never expire into consent).
pub trait ActionPolicy: Send + Sync {
    /// Decide one request.
    fn decide(&self, request: &ToolRequest) -> GateDecision;
}

/// The execution port: run one ALLOWED request inside whatever executor the
/// boundary assembled (in-process fence today, container/microVM tomorrow).
pub trait ToolProvider: Send + Sync {
    /// Execute one request.
    ///
    /// # Errors
    /// [`ToolError::Refused`] - the action did not happen.
    fn execute(&self, request: &ToolRequest) -> Result<ToolOutcome, ToolError>;
}

/// Parse the act-payload tool envelope (`{"truerise_tool": {...}}`). Returns
/// `None` for anything else - plain text NEVER executes.
#[must_use]
pub fn parse_tool_call(payload: &str) -> Option<ToolRequest> {
    let doc: Value = serde_json::from_str(payload).ok()?;
    let inner = doc.get("truerise_tool")?;
    let tool = inner.get("tool")?.as_str()?.to_string();
    let args = inner
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !args.is_object() {
        return None;
    }
    Some(ToolRequest { tool, args })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn envelope_parses_only_the_exact_shape() {
        let req =
            parse_tool_call(r#"{"truerise_tool":{"tool":"write_file","args":{"path":"a.md"}}}"#)
                .expect("envelope parses");
        assert_eq!(req.tool, "write_file");
        assert_eq!(req.args, json!({"path": "a.md"}));
        // Plain text never parses:
        assert!(parse_tool_call("scripted response for: x").is_none());
        assert!(
            parse_tool_call(r#"{"tool":"write_file"}"#).is_none(),
            "no envelope key"
        );
        assert!(
            parse_tool_call(r#"{"truerise_tool":{"tool":"x","args":[1]}}"#).is_none(),
            "args must be an object"
        );
    }
}
