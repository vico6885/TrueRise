//! RFC 0002 acceptance tests (W2): the governed tool path must deny with an
//! audit row, require an approval ON FILE before executing, land the
//! executor's own capability level on the chain, and record sandbox
//! refusals as denials. Tests may unwrap/expect (CONTRIBUTING rules).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use truerise_cli::{GovernedPolicy, RunParams, RunTooling, ScriptRouter, run_books_in};
use truerise_core::{
    Loop, LoopConfig, ModelRouter, RouterError, ToolHooks, TurnRequest, TurnResponse,
};
use truerise_governance::{HashChain, PolicyState};

/// Unique temp dir per call (parallel-test safe).
fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_tool_gate_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// (kind, detail) pairs of the governing chain rows.
fn chain_rows(chain: &HashChain) -> Vec<(String, serde_json::Value)> {
    chain
        .governing_rows()
        .iter()
        .map(|r| (r.event.kind.clone(), r.event.detail.clone()))
        .collect()
}

fn has_row(rows: &[(String, serde_json::Value)], kind: &str, key: &str, value: &str) -> bool {
    rows.iter()
        .any(|(k, d)| k == kind && d.get(key) == Some(&json!(value)))
}

/// A router whose act turn requests a write OUTSIDE the fence.
struct EscapeRouter;

impl ModelRouter for EscapeRouter {
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError> {
        let payload = if request.tag == "act" {
            r#"{"truerise_tool":{"tool":"write_file","args":{"path":"../escape.md","content":"x"}}}"#.into()
        } else {
            format!("scripted response for: {}", request.payload)
        };
        Ok(TurnResponse {
            model: "scripted".into(),
            payload,
        })
    }
}

fn run(
    dir: &std::path::Path,
    policy: &GovernedPolicy,
    provider: &RunTooling,
    router: &dyn ModelRouter,
) {
    let hooks = ToolHooks {
        policy,
        provider: &provider.provider,
    };
    run_books_in(RunParams {
        chain_dir: dir,
        router,
        hooks: Some(hooks),
        budget: None,
        hat_toml: None,
        goal: "write the report",
        rounds: 1,
    })
    .expect("run completes");
}

#[test]
fn tool_denied_by_policy_writes_no_side_effect() {
    let dir = temp_dir("deny");
    let tooling = RunTooling::new(&dir).expect("assembly");
    let policy = GovernedPolicy::with_rules(&[("write_file", PolicyState::Deny)]);
    let router = ScriptRouter::tool_calling("scripted");
    run(&dir, &policy, &tooling, &router);

    let target = RunTooling::sandbox_path(&dir, "report.md");
    assert!(!target.exists(), "a Deny must never create the target file");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        has_row(&rows, "tool.denied", "reason", "policy_deny"),
        "the denial itself must be audited: {rows:?}"
    );
    assert!(!has_row(&rows, "tool.executed", "tool", "write_file"));
}

#[test]
fn ask_requires_approval_on_file_before_execution() {
    let dir = temp_dir("ask");
    let tooling = RunTooling::new(&dir).expect("assembly");
    let policy = GovernedPolicy::run_defaults(); // write_file = Ask
    let router = ScriptRouter::tool_calling("scripted");

    // First pass: no approval on file -> request queued, NOT executed.
    run(&dir, &policy, &tooling, &router);
    let target = RunTooling::sandbox_path(&dir, "report.md");
    assert!(!target.exists(), "no approval on file, no execution");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    assert!(has_row(
        &chain_rows(&chain),
        "tool.denied",
        "reason",
        "approval_required"
    ));

    // Second pass: approval recorded on file -> the ONLY transition into
    // ask-allowed -> executed.
    policy.approve("write_file");
    run(&dir, &policy, &tooling, &router);
    assert!(target.exists(), "approval on file, executed");
    // Re-open the chain: the handle from pass 1 is a stale in-memory copy
    // (pass 2 appended through its own handle).
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(has_row(&rows, "tool.executed", "tool", "write_file"));
}

#[test]
fn execution_lands_capability_level_on_chain() {
    let dir = temp_dir("capability");
    let tooling = RunTooling::new(&dir).expect("assembly");
    let policy = GovernedPolicy::with_rules(&[("write_file", PolicyState::Auto)]);
    let router = ScriptRouter::tool_calling("scripted");
    run(&dir, &policy, &tooling, &router);

    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    let executed = rows
        .iter()
        .filter(|(k, _)| k == "tool.executed")
        .collect::<Vec<_>>();
    assert_eq!(executed.len(), 1, "exactly one executed row: {rows:?}");
    // The capability level is the SANDBOX's return value (InProcess = full),
    // never a declared one.
    assert_eq!(executed[0].1.get("capability"), Some(&json!("full")));
}

#[test]
fn sandbox_refused_escape_is_recorded_as_denial() {
    let dir = temp_dir("escape");
    let tooling = RunTooling::new(&dir).expect("assembly");
    let policy = GovernedPolicy::with_rules(&[("write_file", PolicyState::Auto)]);
    run(&dir, &policy, &tooling, &EscapeRouter);

    // Nothing may exist outside the fence:
    assert!(!dir.parent().unwrap().join("escape.md").exists());
    assert!(!RunTooling::sandbox_path(&dir, "escape.md").exists());
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        has_row(&rows, "tool.denied", "reason", "sandbox_refused"),
        "the fence refusal must land as an audited denial: {rows:?}"
    );
    assert!(!has_row(&rows, "tool.executed", "tool", "write_file"));
}

/// The hooks must be OPTIONAL: the execution-free path (no hooks) stays
/// byte-identical to the pre-RFC behaviour (golden lock).
#[test]
fn no_hooks_means_no_execution_surface() {
    let dir = temp_dir("nohooks");
    let router = ScriptRouter::tool_calling("scripted");
    // The envelope is in the act payload, but with no hooks wired it is
    // PLAIN TEXT - it must parse as nothing and execute as nothing.
    let books = truerise_cli::run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        goal: "goal",
        rounds: 1,
    })
    .expect("run");
    assert!(!RunTooling::sandbox_path(&dir, "report.md").exists());
    assert_eq!(books.audit_rows, books.transitions.len(), "no tool rows");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    assert!(
        !chain_rows(&chain)
            .iter()
            .any(|(k, _)| k.starts_with("tool."))
    );
}

/// Silence unused-import lints for names asserted indirectly above.
mod contract {
    use super::*;
    #[test]
    fn loop_config_shape_is_stable() {
        let config = LoopConfig {
            pinned_model: "scripted".into(),
            retry: truerise_core::RetryPolicy::default(),
        };
        assert_eq!(config.pinned_model, "scripted");
        let _ = Loop::final_state(&[]);
    }
}
