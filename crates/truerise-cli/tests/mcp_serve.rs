//! W8 acceptance tests: the MCP server REALLY runs as a process over stdio
//! with zero exposure by default, and its activity is auditable.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_governance::HashChain;
use truerise_mcp::Client;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_mcp_serve_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Drive one serve session: spawn the REAL binary, write the frames, read
/// the responses (one per line), close stdin (EOF ends the server).
fn serve_session(chain_dir: &std::path::Path, frames: &[String]) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_truerise"))
        .args(["mcp", "--serve", "--chain"])
        .arg(chain_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn truerise mcp --serve");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for frame in frames {
            writeln!(stdin, "{frame}").expect("write frame");
        }
        // stdin dropped at scope end -> EOF -> the server exits cleanly.
    }
    let output = child.wait_with_output().expect("server exits");
    assert!(output.status.success(), "serve exits 0 on EOF");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn mcp_server_default_empty_allowlist_over_stdio() {
    let dir = temp_dir("stdio");
    let frame = |id: u64, method: &str, params: &str| {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
    };
    let responses = serve_session(
        &dir,
        &[
            frame(1, "initialize", "{}"),
            frame(2, "tools/list", "{}"),
            frame(3, "tools/call", r#"{"name":"write_file","arguments":{}}"#),
        ],
    );
    assert_eq!(responses.len(), 3, "one response per frame: {responses:?}");
    // initialize answers with the server identity:
    assert!(responses[0].contains("truerise-mcp"), "{}", responses[0]);
    // tools/list: the allowlist starts EMPTY (zero exposure by default):
    let list: serde_json::Value = serde_json::from_str(&responses[1]).expect("frame");
    assert_eq!(
        list["result"]["tools"].as_array().map(Vec::len),
        Some(0),
        "empty allowlist by default: {}",
        responses[1]
    );
    // tools/call on a non-allowed tool: a JSON-RPC ERROR frame:
    let call: serde_json::Value = serde_json::from_str(&responses[2]).expect("frame");
    assert!(
        call["error"].is_object(),
        "not-allowed tool errors: {}",
        responses[2]
    );
}

#[test]
fn client_write_class_hard_denied_before_any_server_call() {
    // A client with NO server at all: the write-class denial happens
    // client-side BEFORE any server interaction could exist.
    let client = Client::new();
    let err = client.call("write_file").expect_err("write-class denied");
    assert!(err.contains("hard-denied"), "{err}");
    assert!(
        client
            .audit_log()
            .iter()
            .any(|e| e.contains("DENIED(write-class)")),
        "the denial itself is audited"
    );
    // Read class still goes through (audited):
    assert!(client.call("tools/list").is_ok());
}

#[test]
fn mcp_calls_appear_on_the_chain() {
    let dir = temp_dir("chain");
    let frame = |id: u64, method: &str| {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{{}}}}"#)
    };
    let _ = serve_session(&dir, &[frame(1, "initialize"), frame(2, "tools/list")]);
    let chain_path = dir.join("mcp.jsonl");
    let chain = HashChain::open(&chain_path).expect("the serve session left a chain");
    let methods: Vec<String> = chain
        .governing_rows()
        .iter()
        .filter(|r| r.event.kind == "mcp.call")
        .filter_map(|r| {
            r.event
                .detail
                .get("method")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .collect();
    assert_eq!(
        methods,
        vec!["initialize", "tools/list"],
        "audited in order"
    );
    chain.verify().expect("chain intact");
}
