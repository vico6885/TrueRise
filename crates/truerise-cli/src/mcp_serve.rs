//! `truerise mcp --serve` (W8): the zero-exposure MCP server over stdio.
//! The allowlist starts EMPTY - governance must allow each tool explicitly
//! (no default exposure, ever). Every handled request (and every protocol
//! error) lands on the run's chain: MCP activity is auditable activity.

use std::io::{BufRead, Write};
use std::path::Path;

use truerise_mcp::{RequestFrame, Server};

/// Serve MCP over stdio until EOF (one frame per line; responses one per
/// line). Returns the process exit code.
pub fn serve(chain_dir: &Path) -> i32 {
    let chain_path = chain_dir.join("mcp.jsonl");
    let chain = match truerise_governance::HashChain::open(&chain_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("mcp: chain unavailable: {e}");
            return 1;
        }
    };
    let server = Server::new();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break }; // EOF: parent closed the pipe
        if line.trim().is_empty() {
            continue;
        }
        match RequestFrame::parse(&line) {
            Ok(frame) => {
                let _ = chain.append(
                    "mcp.call",
                    truerise_wire::Severity::Info,
                    &serde_json::json!({ "method": frame.method, "id": frame.id }),
                    None,
                );
                if let Some(response) = server.handle(&frame) {
                    let mut out = stdout.lock();
                    let _ = writeln!(out, "{response}");
                    let _ = out.flush();
                }
            }
            Err(e) => {
                let _ = chain.append(
                    "mcp.protocol_error",
                    truerise_wire::Severity::Warn,
                    &serde_json::json!({ "error": e.to_string() }),
                    None,
                );
                let mut out = stdout.lock();
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::json!({
                        "jsonrpc": "2.0", "id": null,
                        "error": { "code": -32700, "message": "parse error" }
                    })
                );
                let _ = out.flush();
            }
        }
    }
    0
}
