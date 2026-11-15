//! W4 acceptance tests: a governed run ENDS with a verdict from the judging
//! authority, and the exit code carries it. Tests may unwrap (CONTRIBUTING).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use truerise_cli::{ScriptRouter, grade_exit, run_books_in};
use truerise_core::{ModelRouter, RouterError, TurnRequest, TurnResponse};
use truerise_eval::ratchet::Baseline;
use truerise_eval::{RunFacts, judge_run};
use truerise_governance::{HashChain, PolicyState};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_run_eval_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A router whose plan turn produces an EMPTY payload -> the run ends in
/// AwaitingTakeover (no verdict issued) -> one counted failure.
struct EmptyPlanRouter;

impl ModelRouter for EmptyPlanRouter {
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError> {
        let payload = if request.tag == "plan" {
            String::new()
        } else {
            format!("scripted response for: {}", request.payload)
        };
        Ok(TurnResponse {
            model: "scripted".into(),
            payload,
        })
    }
}

fn baseline_zero() -> Baseline {
    Baseline::from([("run_failures".to_string(), 0.0)])
}

/// One governed run + its judged verdict (the run-path sequence run_commandline
/// performs, driven through the same single-track core).
fn run_and_judge(
    dir: &std::path::Path,
    router: &dyn ModelRouter,
    baseline: &Baseline,
) -> Result<truerise_eval::RunVerdict, truerise_eval::EvalError> {
    let books = run_books_in(dir, router, None, None, "judge me", 1).expect("run completes");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let facts = RunFacts {
        finished: truerise_core::Loop::final_state(&books.transitions)
            == truerise_core::State::Finished,
        rounds_total: 1,
    };
    judge_run(&facts, baseline, &chain)
}

fn chain_rows(chain: &HashChain) -> Vec<(String, serde_json::Value)> {
    chain
        .governing_rows()
        .iter()
        .map(|r| (r.event.kind.clone(), r.event.detail.clone()))
        .collect()
}

#[test]
fn run_emits_verdict_row_from_eval_authority() {
    let dir = temp_dir("authority");
    let verdict = run_and_judge(&dir, &ScriptRouter::plain("scripted"), &baseline_zero())
        .expect("verdict recorded");
    assert!(verdict.is_pass());
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    let verdicts: Vec<_> = rows.iter().filter(|(k, _)| k == "eval.verdict").collect();
    assert_eq!(verdicts.len(), 1, "exactly one verdict row: {rows:?}");
    assert_eq!(verdicts[0].1.get("verdict"), Some(&json!("PASS")));
    assert_eq!(verdicts[0].1.get("failures"), Some(&json!(0)));
    // The authority is a type-system fact: RunVerdict has no public
    // constructor, so NO other crate can produce this row's payload.
    chain.verify().expect("chain intact");
    let _ = PolicyState::Auto; // policy types stay out of the verdict path
}

#[test]
fn score_regression_fails_the_run_with_grade_exit() {
    let dir = temp_dir("regression");
    // The run takes over (empty plan) -> failures 1 > baseline 0 -> FAIL.
    let verdict = run_and_judge(&dir, &EmptyPlanRouter, &baseline_zero()).expect("verdict");
    assert!(!verdict.is_pass(), "a takeover run is a counted failure");
    assert_eq!(verdict.failures(), 1);
    // Exit-grade: the same mapping run_commandline applies (no third code).
    assert_eq!(grade_exit(&verdict), 1);
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        rows.iter()
            .any(|(k, d)| k == "eval.verdict" && d.get("verdict") == Some(&json!("FAIL"))),
        "the FAIL verdict is recorded too (auditable): {rows:?}"
    );
}

#[test]
fn unrecordable_verdict_is_error_not_warning() {
    let dir = temp_dir("unrecordable");
    let verdict = run_and_judge(&dir, &ScriptRouter::plain("scripted"), &baseline_zero())
        .expect("first verdict records");
    assert!(verdict.is_pass());
    // Tamper with the persisted rows, then attempt the run-path recording
    // sequence again (re-open -> judge): the open refuses tampered content,
    // the verdict cannot be recorded, and the caller MUST exit non-zero
    // (run_commandline maps exactly this shape to 1).
    let path = dir.join("run.jsonl");
    let text = std::fs::read_to_string(&path).expect("chain file");
    std::fs::write(&path, text.replacen("eval.verdict", "eval.verdictX", 1)).expect("tamper");
    let recording_outcome = match HashChain::open(&path) {
        Ok(_chain) => match run_and_judge(&dir, &ScriptRouter::plain("scripted"), &baseline_zero())
        {
            Ok(_) => 0,
            Err(_) => 1,
        },
        Err(_) => 1, // unrecordable -> exit-grade (never a warning)
    };
    assert_eq!(recording_outcome, 1, "unrecordable verdict is exit-grade");
}
