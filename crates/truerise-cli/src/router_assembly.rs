//! The scripted assembly pieces (offline by default): the ScriptRouter and
//! the audit-chain bridge. Real providers live in provider_* modules and
//! plug in through the SAME ports.

use truerise_governance::HashChain;
use truerise_wire::Severity;

/// A scripted router: answers as the pinned model without any network (the
/// real ModelRouter providers are external projects; the port is the contract).
/// With ``tool_call`` set, the act turn emits the RFC 0002 tool envelope
/// instead of plain text (the governed demo of the enforcement chain).
pub struct ScriptRouter {
    model: String,
    tool_call: bool,
}

impl ScriptRouter {
    /// Plain-text scripted router (no tool envelope).
    #[must_use]
    pub fn plain(model: &str) -> Self {
        Self {
            model: model.into(),
            tool_call: false,
        }
    }

    /// Tool-requesting scripted router (act turns carry the envelope).
    #[must_use]
    pub fn tool_calling(model: &str) -> Self {
        Self {
            model: model.into(),
            tool_call: true,
        }
    }
}

impl truerise_core::ModelRouter for ScriptRouter {
    fn route(
        &self,
        request: &truerise_core::TurnRequest,
    ) -> Result<truerise_core::TurnResponse, truerise_core::RouterError> {
        let payload = if self.tool_call && request.tag == "act" {
            // One deterministic scripted action: write the run report under
            // the sandbox fence. The policy decides whether it happens.
            r#"{"truerise_tool":{"tool":"write_file","args":{"path":"report.md","content":"scripted report body"}}}"#.to_string()
        } else {
            format!("scripted response for: {}", request.payload)
        };
        Ok(truerise_core::TurnResponse {
            model: self.model.clone(),
            payload,
        })
    }
}

/// AuditSink bridge: the governance chain as the loop's audit port (this
/// bridge lives HERE, at the boundary, keeping core free of governance).
pub struct ChainSinkBridge<'a>(pub &'a HashChain);

impl truerise_core::AuditSink for ChainSinkBridge<'_> {
    fn append(
        &self,
        kind: &str,
        severity: &str,
        detail: &serde_json::Value,
    ) -> Result<usize, String> {
        let sev = match severity {
            "warn" => Severity::Warn,
            "error" => Severity::Error,
            "debug" => Severity::Debug,
            _ => Severity::Info,
        };
        self.0
            .append(kind, sev, detail, None)
            .map(|_| self.0.len())
            .map_err(|e| e.to_string())
    }
}
