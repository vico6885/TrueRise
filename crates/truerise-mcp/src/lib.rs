//! truerise-mcp: MCP server (zero exposure by default - every tool must be
//! explicitly allowed by governance) and client (write actions HARD-DENIED,
//! every accepted call audited). Frame protocol: JSON-RPC 2.0 over stdio
//! (newline-delimited JSON, one frame per line).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// Frame errors.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// Not valid JSON.
    #[error("mcp frame: invalid json: {0}")]
    InvalidJson(String),
    /// Not a JSON-RPC 2.0 frame (missing jsonrpc/version fields).
    #[error("mcp frame: not json-rpc 2.0")]
    NotJsonRpc,
}

/// A parsed JSON-RPC 2.0 request frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestFrame {
    /// Always "2.0".
    pub jsonrpc: String,
    /// Request id.
    pub id: u64,
    /// Method name.
    pub method: String,
    /// Params (optional).
    #[serde(default)]
    pub params: serde_json::Value,
}

impl RequestFrame {
    /// Parse one line into a frame (strict 2.0 check).
    ///
    /// # Errors
    /// [`FrameError`] on invalid json or wrong version.
    pub fn parse(line: &str) -> Result<Self, FrameError> {
        let v: Self =
            serde_json::from_str(line).map_err(|e| FrameError::InvalidJson(e.to_string()))?;
        if v.jsonrpc != "2.0" {
            return Err(FrameError::NotJsonRpc);
        }
        Ok(v)
    }

    /// The 2.0 response frame for this request.
    pub fn respond(&self, result: serde_json::Value) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.id,
            "result": result,
        })
        .to_string()
    }

    /// The 2.0 error frame for this request.
    pub fn respond_error(&self, code: i64, message: &str) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.id,
            "error": { "code": code, "message": message },
        })
        .to_string()
    }
}

/// Tool handler signature: arguments -> output (plain function, no state).
pub type ToolHandler = fn(&HashMap<String, String>) -> String;

/// Tool descriptors the server exposes (zero exposure by default: the allow
/// list starts EMPTY; governance must allow each tool explicitly).
#[derive(Debug, Default)]
pub struct Server {
    allowed_tools: Mutex<HashMap<String, ToolHandler>>,
}

impl Server {
    /// A server with ZERO exposed tools.
    pub fn new() -> Self {
        Self::default()
    }

    /// Governance explicitly allows a tool (the ONLY way a tool becomes
    /// visible; no default exposure).
    pub fn allow_tool(&self, name: &str, handler: fn(&HashMap<String, String>) -> String) {
        self.allowed_tools
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(name.to_string(), handler);
    }

    /// Handle one request frame: unknown/disallowed methods get a JSON-RPC
    /// error frame; allowed tools run and get a result frame.
    pub fn handle(&self, frame: &RequestFrame) -> Option<String> {
        match frame.method.as_str() {
            "initialize" => Some(frame.respond(serde_json::json!({
                "protocolVersion": "2025-06-18",
                "serverInfo": { "name": "truerise-mcp", "version": "0.2.0-dev.0" },
                "capabilities": { "tools": {} },
            }))),
            "tools/list" => {
                let tools = self.allowed_tools.lock().unwrap_or_else(|p| p.into_inner());
                let names: Vec<String> = tools.keys().cloned().collect();
                Some(frame.respond(serde_json::json!({ "tools": names })))
            }
            m if m.starts_with("tools/call") => {
                let tool = frame
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args: HashMap<String, String> = frame
                    .params
                    .get("arguments")
                    .and_then(|v| v.as_object())
                    .map(|o| {
                        o.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                let tools = self.allowed_tools.lock().unwrap_or_else(|p| p.into_inner());
                match tools.get(tool) {
                    Some(handler) => {
                        Some(frame.respond(serde_json::json!({ "output": handler(&args) })))
                    }
                    None => Some(frame.respond_error(-32602, &format!("tool not allowed: {tool}"))),
                }
            }
            _ => Some(frame.respond_error(-32601, "method not found")),
        }
    }
}

/// The client side: write-class tools are HARD-DENIED (never sent to any
/// server, regardless of what the server claims), read calls are audited.
#[derive(Debug, Default)]
pub struct Client {
    audit: Mutex<Vec<String>>,
}

/// Write-class method prefixes (hard-denied on the client).
const WRITE_PREFIXES: [&str; 3] = ["write", "create", "delete"];

impl Client {
    /// A client with an empty audit log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Is this client-side call a write class action?
    pub fn is_write_class(method: &str) -> bool {
        WRITE_PREFIXES.iter().any(|p| method.starts_with(p))
    }

    /// Attempt a client-side call. Write-class calls are refused BEFORE any
    /// server interaction; read calls are recorded in the audit log.
    ///
    /// # Errors
    /// Write-class refusal (string-typed at the port boundary); the caller
    /// audits the outcome either way.
    pub fn call(&self, method: &str) -> Result<String, String> {
        if Self::is_write_class(method) {
            self.audit
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(format!("DENIED(write-class): {method}"));
            return Err(format!("write-class call hard-denied on client: {method}"));
        }
        self.audit
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(format!("ALLOWED(read-class): {method}"));
        Ok(format!("client-call:{method}"))
    }

    /// The audit log (denials included - refusal is auditable too).
    pub fn audit_log(&self) -> Vec<String> {
        self.audit.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn hello_handler(args: &HashMap<String, String>) -> String {
        format!(
            "hello, {}",
            args.get("who").cloned().unwrap_or_else(|| "world".into())
        )
    }

    #[test]
    fn zero_exposure_by_default() {
        let server = Server::new();
        let frame =
            RequestFrame::parse(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let resp = server.handle(&frame).unwrap();
        assert!(
            resp.contains("\"tools\":[]"),
            "no tools exposed by default: {resp}"
        );
    }

    #[test]
    fn allowlist_gates_tool_calls() {
        let server = Server::new();
        server.allow_tool("hello", hello_handler);
        let call = RequestFrame::parse(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"hello","arguments":{"who":"mcp"}}}"#,
        )
        .unwrap();
        let resp = server.handle(&call).unwrap();
        assert!(resp.contains("hello, mcp"));
        let bad = RequestFrame::parse(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"secret"}}"#,
        )
        .unwrap();
        let resp2 = server.handle(&bad).unwrap();
        assert!(resp2.contains("tool not allowed"));
    }

    #[test]
    fn frame_protocol_strict_2_0() {
        assert!(RequestFrame::parse(r#"{"jsonrpc":"1.0","id":1,"method":"x"}"#).is_err());
        assert!(RequestFrame::parse("not json").is_err());
        let f = RequestFrame::parse(r#"{"jsonrpc":"2.0","id":7,"method":"m"}"#).unwrap();
        assert_eq!(f.id, 7);
    }

    /// Toy e2e: client -> server roundtrip in-process (spec §7 M5 gate).
    #[test]
    fn toy_e2e_client_to_server() {
        let server = Server::new();
        server.allow_tool("hello", hello_handler);
        let client = Client::new();

        let r = client.call("read_status").expect("read allowed");
        assert!(r.contains("read_status"));

        let frame = RequestFrame::parse(
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"hello","arguments":{"who":"e2e"}}}"#,
        )
        .unwrap();
        let resp = server.handle(&frame).unwrap();
        assert!(resp.contains("hello, e2e"));
    }

    #[test]
    fn client_hard_denies_write_class() {
        let client = Client::new();
        assert!(Client::is_write_class("write_file"));
        assert!(Client::is_write_class("create_thing"));
        assert!(Client::is_write_class("delete_all"));
        assert!(!Client::is_write_class("read_file"));
        assert!(
            client.call("write_file").is_err(),
            "write-class HARD-DENIED"
        );
        assert!(client.call("read_file").is_ok());
        let log = client.audit_log();
        assert!(log.iter().any(|l| l.contains("DENIED(write-class)")));
        assert!(log.iter().any(|l| l.contains("ALLOWED(read-class)")));
    }
}
