//! W3 acceptance tests: the budget must be able to STOP a live run. A
//! ledger that only passes unit tests is not governance - these tests drive
//! the enforcement through the run path. Tests may unwrap (CONTRIBUTING).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use truerise_cli::{RunParams, ScriptRouter, run_books_in};
use truerise_core::LoopError;
use truerise_governance::HashChain;
use truerise_rsi::RunBudget;

/// The ledger singleton is process-global by design: consuming tests hold
/// this guard so delta arithmetic is never raced (same protocol as the rsi
/// unit tests document).
static BUDGET_GUARD: Mutex<()> = Mutex::new(());

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_budget_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn chain_rows(chain: &HashChain) -> Vec<(String, serde_json::Value)> {
    chain
        .governing_rows()
        .iter()
        .map(|r| (r.event.kind.clone(), r.event.detail.clone()))
        .collect()
}

#[test]
fn budget_exhaustion_halts_run_at_step_n() {
    let _guard = BUDGET_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir("halt");
    let router = ScriptRouter::plain("scripted");
    let budget = RunBudget::new(Some(2)); // quota: 2 units
    // 3 requested rounds: rounds 0 and 1 consume 1 unit each; round 2 must
    // NEVER start (its plan call must not happen).
    let err = run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: Some(("halt-test", &budget)),
        hat_toml: None,
        memory: None,
        goal: "goal",
        rounds: 3,
    })
    .expect_err("over budget stops the run");
    assert!(matches!(err, LoopError::Budget { ref hat } if hat == "halt-test"));

    // The stop is on the chain (a stop nobody can see is a note, not a gate).
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    let stops: Vec<_> = rows
        .iter()
        .filter(|(k, _)| k == "budget.exhausted")
        .collect();
    assert_eq!(stops.len(), 1, "exactly one stop row: {rows:?}");
    assert_eq!(stops[0].1.get("hat"), Some(&json!("halt-test")));

    // The ledger itself stopped at the quota (2 consumed, not 3).
    assert_eq!(budget.ledger.hat_count("halt-test"), 2);
}

#[test]
fn budget_is_shared_across_hats_not_sharded() {
    let _guard = BUDGET_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // The singleton is process-global BY DESIGN: two runs under different
    // hats must move ONE global denominator (delta assertions over unique
    // hat names, same protocol as the rsi unit tests).
    let hat_a = format!("shared-a-{}", std::process::id());
    let hat_b = format!("shared-b-{}", std::process::id());
    let ledger_before = truerise_rsi::BudgetLedger::global().global_count();

    let router = ScriptRouter::plain("scripted");
    let unbounded = RunBudget::new(None);
    let dir_a = temp_dir("shared_a");
    run_books_in(RunParams {
        chain_dir: &dir_a,
        router: &router,
        hooks: None,
        budget: Some((hat_a.as_str(), &unbounded)),
        hat_toml: None,
        memory: None,
        goal: "goal",
        rounds: 2,
    })
    .expect("run a");
    let dir_b = temp_dir("shared_b");
    run_books_in(RunParams {
        chain_dir: &dir_b,
        router: &router,
        hooks: None,
        budget: Some((hat_b.as_str(), &unbounded)),
        hat_toml: None,
        memory: None,
        goal: "goal",
        rounds: 1,
    })
    .expect("run b");

    let ledger = truerise_rsi::BudgetLedger::global();
    let global_delta = ledger.global_count() - ledger_before;
    let per_hat_delta = ledger.hat_count(&hat_a) + ledger.hat_count(&hat_b);
    assert_eq!(
        global_delta, per_hat_delta,
        "sum(per-hat) == global: one denominator, never sharded"
    );
    assert_eq!(ledger.reconcile(), 0, "reconciliation holds across hats");
    assert_eq!(ledger.hat_count(&hat_a), 2, "run a consumed its two rounds");
    assert_eq!(ledger.hat_count(&hat_b), 1, "run b consumed its one round");
}

#[test]
fn run_exit_code_reflects_budget_stop() {
    let _guard = BUDGET_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // Budget 0: the very first round is refused -> exit 1 (a stop is a
    // failure, never a note; same contract as a FAIL verdict).
    let stopped = truerise_cli::cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--goal".into(),
        "budgeted goal".into(),
        "--budget".into(),
        "0".into(),
    ]);
    assert_eq!(stopped, 1, "budget stop must exit non-zero");
    // Positive control: a generous budget completes with exit 0.
    let completed = truerise_cli::cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--goal".into(),
        "budgeted goal".into(),
        "--budget".into(),
        "100".into(),
    ]);
    assert_eq!(completed, 0, "within budget completes with exit 0");
}
