//! Exit-code contract (2A): the CLI's exit code must carry the verdict, the
//! same way the Python CLI does (FAIL -> 1). A verdict that cannot turn a CI
//! lane red is a note, not a gate — and this repository sells gates.
//!
//! Relaxed for the unknown-command case only: `cli_main` prints help there,
//! no unwraps are involved.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use truerise_cli::cli_main;

#[test]
fn demo_fail_verdict_exits_nonzero() {
    assert_eq!(
        cli_main(vec!["truerise".into(), "demo".into()]),
        1,
        "a FAIL verdict must exit 1 so CI can gate on it"
    );
}

#[test]
fn demo_honest_verdict_exits_zero() {
    assert_eq!(
        cli_main(vec!["truerise".into(), "demo".into(), "--honest".into()]),
        0,
        "flipping the inputs to honest must clear the exit code"
    );
}

#[test]
fn exit_code_tracks_the_verdict_not_the_flag() {
    // Both calls run the same code path; only the inputs differ. If the exit
    // code ever becomes constant, one of these two assertions breaks.
    let cheating = cli_main(vec!["truerise".into(), "demo".into()]);
    let honest = cli_main(vec!["truerise".into(), "demo".into(), "--honest".into()]);
    assert_ne!(
        cheating, honest,
        "the two verdicts must not share an exit code"
    );
    assert!(cheating > honest, "FAIL must be the non-zero branch");
}

#[test]
fn unknown_command_exits_two() {
    assert_eq!(cli_main(vec!["truerise".into(), "not-a-command".into()]), 2);
}

#[test]
fn grade_exit_maps_three_verdicts_to_three_codes() {
    // G7: INCONCLUSIVE is a FIRST-CLASS verdict - distinct exit code (3),
    // never readable as PASS (0) or FAIL (1).
    let dir = std::env::temp_dir().join(format!(
        "truerise_exit_g7_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let chain = truerise_governance::HashChain::open(&dir.join("c.jsonl")).expect("chain");
    let facts = |finished| truerise_eval::RunFacts {
        finished,
        rounds_total: 1,
        memory_recall: false,
        inconclusive: !finished, // the test simulates a governance stop
    };
    let zero_baseline = truerise_eval::ratchet::Baseline::new();
    let pass = truerise_eval::judge_run(&facts(true), &zero_baseline, &chain)
        .expect("recorded");
    let inconclusive = truerise_eval::judge_run(&facts(false), &zero_baseline, &chain)
        .expect("recorded");
    // inconclusive_exit_code_distinct: the third verdict gets the third
    // code - distinct from PASS (0) and FAIL (1), red in CI all the same.
    assert_eq!(truerise_cli::grade_exit(&pass), 0);
    assert_eq!(truerise_cli::grade_exit(&inconclusive), 3);
    assert_ne!(
        truerise_cli::grade_exit(&inconclusive),
        truerise_cli::grade_exit(&pass),
        "an unfinished run never reads as success"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inconclusive_cannot_be_promoted_to_pass() {
    // The promotion guard is structural: Inconclusive's wire name is its
    // own, is_pass() is false, and the exit code is neither 0 nor 1 - three
    // independent locks on the same door.
    let dir = std::env::temp_dir().join(format!(
        "truerise_g7_promo_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let chain = truerise_governance::HashChain::open(&dir.join("c.jsonl")).expect("chain");
    let facts = truerise_eval::RunFacts {
        finished: false,
        rounds_total: 1,
        memory_recall: false,
        inconclusive: true,
    };
    let v = truerise_eval::judge_run(&facts, &truerise_eval::ratchet::Baseline::new(), &chain)
        .expect("recorded");
    assert_eq!(v.verdict_str(), "INCONCLUSIVE");
    assert!(!v.is_pass());
    assert_eq!(truerise_cli::grade_exit(&v), 3);
    let _ = std::fs::remove_dir_all(&dir);
}
