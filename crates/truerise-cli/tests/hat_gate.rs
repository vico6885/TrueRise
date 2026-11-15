//! W5 acceptance tests: the hat's tool_subset is a REAL filter on the run
//! path, the hat's content hash lands on the chain for each run, and a hat
//! naming an unknown tool is refused at LOAD (not silently ignored).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::{GovernedPolicy, RunParams, RunTooling, ScriptRouter, run_books_in};
use truerise_core::ToolHooks;
use truerise_governance::HashChain;

/// The REAL preset hats (compile-embedded: no drift from hats/*.toml).
const CRITIC: &str = include_str!("../../../hats/critic.toml");
const ENGINEER: &str = include_str!("../../../hats/engineer.toml");

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_hat_gate_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// One governed run under ``hat_toml`` with approvals on file (so ONLY the
/// hat gate can deny the scripted write_file request).
fn run_with_hat(dir: &std::path::Path, hat_toml: &str) -> Result<usize, String> {
    let tooling = RunTooling::new(dir)?;
    let policy = GovernedPolicy::run_defaults();
    policy.approve("write_file");
    let router = ScriptRouter::tool_calling("scripted");
    let hooks = ToolHooks {
        policy: &policy,
        provider: &tooling.provider,
    };
    match run_books_in(RunParams {
        chain_dir: dir,
        router: &router,
        hooks: Some(hooks),
        budget: None,
        hat_toml: Some(hat_toml),
        goal: "write the report",
        rounds: 1,
    }) {
        Ok(books) => Ok(books.audit_rows),
        Err(e) => Err(e.to_string()),
    }
}

#[test]
fn critic_hat_denied_write_file_engineer_hat_allowed() {
    // CRITIC: subset = [read_file] -> the scripted write_file request is
    // denied BEFORE the three-state table is consulted.
    let critic_dir = temp_dir("critic");
    run_with_hat(&critic_dir, CRITIC).expect("critic run completes (denial is an outcome)");
    let critic_file = RunTooling::sandbox_path(&critic_dir, "report.md");
    assert!(!critic_file.exists(), "critic hat cannot write_file");

    // ENGINEER: subset includes write_file -> the SAME request executes.
    let engineer_dir = temp_dir("engineer");
    run_with_hat(&engineer_dir, ENGINEER).expect("engineer run completes");
    let engineer_file = RunTooling::sandbox_path(&engineer_dir, "report.md");
    assert!(engineer_file.exists(), "engineer hat may write_file");

    // Both outcomes are audited on their own chains:
    let critic_chain = HashChain::open(&critic_dir.join("run.jsonl")).expect("chain");
    assert!(
        critic_chain
            .governing_rows()
            .iter()
            .any(|r| r.event.kind == "tool.denied"),
        "the critic's denial is audited"
    );
    let engineer_chain = HashChain::open(&engineer_dir.join("run.jsonl")).expect("chain");
    assert!(
        engineer_chain
            .governing_rows()
            .iter()
            .any(|r| r.event.kind == "tool.executed"),
        "the engineer's execution is audited"
    );
}

#[test]
fn hat_content_hash_lands_on_chain_for_each_run() {
    let dir = temp_dir("hash");
    run_with_hat(&dir, CRITIC).expect("run completes");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    // Expected hash: the same TOML through the hats loader itself.
    let scratch_dir = temp_dir("hash_scratch");
    let scratch_path = scratch_dir.join("s.jsonl");
    let scratch = HashChain::open(&scratch_path).expect("scratch");
    let hat = truerise_hats::load_hat(CRITIC, &scratch).expect("hat parses");
    let rows = chain.governing_rows();
    let registered = rows
        .iter()
        .find(|r| r.event.kind == "hat.registered")
        .expect("hat.registered row on the run chain");
    assert_eq!(
        registered.event.detail.get("content_hash"),
        Some(&serde_json::json!(hat.content_hash())),
        "the run chain carries THIS hat's content hash"
    );
    assert_eq!(
        registered.event.detail.get("name"),
        Some(&serde_json::json!("critic"))
    );
}

#[test]
fn unknown_tool_in_hat_refused_at_load() {
    let dir = temp_dir("unknown");
    let bad_hat = CRITIC.replace("\"read_file\"", "\"read_file\", \"teleport\"");
    let err = run_with_hat(&dir, &bad_hat).expect_err("unknown tool refuses the RUN");
    assert!(
        err.contains("unknown tool"),
        "load-time refusal, not a silent ignore: {err}"
    );
    // Nothing was registered for the refused hat (validation happens BEFORE
    // the chain row) and nothing executed:
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    assert!(
        !chain
            .governing_rows()
            .iter()
            .any(|r| r.event.kind == "hat.registered"),
        "a refused hat never registers"
    );
    assert!(
        !chain
            .governing_rows()
            .iter()
            .any(|r| r.event.kind.starts_with("tool.")),
        "a refused run never reaches a tool"
    );
}
