//! Provider wiring acceptance: `run --provider anthropic` routes the
//! governed loop through the REAL adapter — the same three books, budget,
//! verdict and audit chain as the scripted assembly, over actual HTTP.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

const REPLY: &str =
    r#"{"model":"test-model","content":[{"type":"text","text":"the governed answer"}]}"#;

/// A canned Messages-API server answering THREE sequential requests
/// (plan / act / observe of one governed round).
fn canned_server(replies: &[&'static str]) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let replies: Vec<&'static str> = replies.to_vec();
    std::thread::spawn(move || {
        for reply in replies {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut headers = String::new();
            let mut byte = [0u8; 1];
            loop {
                let n = stream.read(&mut byte).expect("read");
                if n == 0 {
                    break;
                }
                headers.push(byte[0] as char);
                if headers.ends_with("\r\n\r\n") {
                    break;
                }
            }
            let len: usize = headers
                .to_lowercase()
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0);
            let mut buf = vec![0u8; len];
            stream.read_exact(&mut buf).expect("body");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
            stream.flush().expect("flush");
        }
    });
    addr
}

#[test]
fn run_with_provider_routes_through_the_real_adapter() {
    let addr = canned_server(&[REPLY, REPLY, REPLY]);
    // The key arrives through the ENVIRONMENT of the spawned process (never
    // argv/config - SECURITY.md). The adapter reads it fail-closed.
    let output = Command::new(env!("CARGO_BIN_EXE_truerise"))
        .args([
            "run",
            "--goal",
            "answer through a real provider",
            "--provider",
            "anthropic",
            "--provider-url",
            &format!("http://{addr}"),
            "--model",
            "test-model",
        ])
        .env("ANTHROPIC_API_KEY", "test-key")
        .env_remove("TRUERISE_SIGNING_KEY")
        .output()
        .expect("spawn truerise run");
    assert!(
        output.status.success(),
        "the governed provider run completes: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run complete"), "{stdout}");
    assert!(stdout.contains("verdict"), "{stdout}");

    // The missing-key case fails CLOSED with a clear message (exit 1,
    // nothing executed):
    let output = Command::new(env!("CARGO_BIN_EXE_truerise"))
        .args([
            "run",
            "--goal",
            "x",
            "--provider",
            "anthropic",
            "--provider-url",
            &format!("http://{addr}"),
            "--model",
            "test-model",
        ])
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(1), "missing key = failed run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ANTHROPIC_API_KEY"),
        "the message names the env var: {stderr}"
    );
}

#[test]
fn provider_without_model_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_truerise"))
        .args(["run", "--goal", "x", "--provider", "anthropic"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2), "--model is REQUIRED");
}

#[test]
fn unknown_provider_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_truerise"))
        .args(["run", "--goal", "x", "--provider", "skynet", "--model", "m"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(2));
}
