//! G44 acceptance tests: a give-up claim is bounded or it is refused. The
//! named behaviours map to the spec_map "放弃复核" row.
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

use common::{chain_guard, temp_dir};
use truerise_cli::cli_main;

fn claim_json(verdict: &str, ledger: Option<&str>) -> String {
    match ledger {
        Some(led) => format!(r#"{{"verdict":"{verdict}","ledger":{led}}}"#),
        None => format!(r#"{{"verdict":"{verdict}"}}"#),
    }
}

const GOOD_LEDGER: &str = r#"{
  "budget_bound": "two bounded runs or 100k tokens, whichever first",
  "probes": [
    {"hypothesis": "retry the naive path with a larger window", "outcome": {"status": "tried", "result_note": "came back negative twice"}},
    {"hypothesis": "cache the folded subtable", "outcome": {"status": "untried", "cheapest_probe": "one bounded run with the cache behind a flag", "cost_estimate": "one bounded run"}}
  ],
  "retry_plan": {"candidate_probes": ["one bounded run with the cache behind a flag"]}
}"#;

fn chain_rows() -> String {
    let dir = std::env::temp_dir().join("truerise_cli_run");
    // A concurrent writer holds the OS byte-range lock - a plain read can
    // transiently fail; retry instead of swallowing it into an empty string.
    for _ in 0..40 {
        match std::fs::read_to_string(dir.join("run.jsonl")) {
            Ok(text) => return text,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    }
    String::new()
}

#[test]
fn abandonment_without_search_ledger_is_rejected() {
    let dir = temp_dir("no_ledger");
    let path = dir.join("claim.json");
    std::fs::write(&path, claim_json("infeasible_within_budget", None)).expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "give-up without a ledger is unattributed abandonment");
}

#[test]
fn bounded_giveup_lands_on_chain() {
    let _guard = chain_guard("bounded");
    let dir = temp_dir("bounded");
    let path = dir.join("claim.json");
    std::fs::write(&path, claim_json("infeasible_within_budget", Some(GOOD_LEDGER)))
        .expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 0, "a bounded, disjoint, costed claim records");
    let rows = chain_rows();
    assert!(
        rows.lines()
            .any(|l| l.contains("\"giveup.claim\"") && l.contains("\"infeasible_within_budget\"")),
        "the claim lands on the run chain"
    );
}

#[test]
fn bounded_retry_is_disjoint_from_tried_set() {
    // The retry plan re-runs a probe the ledger already tried: refused.
    let dir = temp_dir("not_disjoint");
    let path = dir.join("claim.json");
    let ledger = GOOD_LEDGER.replace(
        r#""retry_plan": {"candidate_probes": ["one bounded run with the cache behind a flag"]}"#,
        r#""retry_plan": {"candidate_probes": ["retry the naive path with a larger window"]}"#,
    );
    std::fs::write(&path, claim_json("infeasible_within_budget", Some(&ledger)))
        .expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "re-running a known dead end is not a retry plan");
}

#[test]
fn infeasible_certificate_states_its_budget_bound() {
    let dir = temp_dir("unbounded");
    let path = dir.join("claim.json");
    let ledger = GOOD_LEDGER.replace(
        "two bounded runs or 100k tokens, whichever first",
        "   ",
    );
    std::fs::write(&path, claim_json("infeasible_within_budget", Some(&ledger)))
        .expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "'infeasible' without a sealed bound is not a claim");
}

#[test]
fn three_valued_outcome_cannot_be_collapsed() {
    // "pass" is not a verdict the give-up surface accepts: the parse
    // refusal IS the collapse guard (wire-level test carries the full
    // matrix; this asserts the CLI refuses to record a collapsed verdict).
    let dir = temp_dir("collapsed");
    let path = dir.join("claim.json");
    std::fs::write(&path, claim_json("pass", Some(GOOD_LEDGER))).expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "a collapsed verdict does not parse, so it cannot record");
}

#[test]
fn giveup_claim_binds_to_the_sealed_contract() {
    // BOTH calls touch the shared chain - hold the cross-binary lock across
    // the whole test (an unguarded append races a concurrent reader).
    let _guard = chain_guard("binds");
    let dir = temp_dir("contract");
    let contract_path = dir.join("intent.toml");
    std::fs::write(
        &contract_path,
        r#"
contract_version = "v1"
goal = "bounded give-up against a contract"

[[acceptance_items]]
id = "m0"
text = "the ledger row exists"
kind = "machine"

[[acceptance_items]]
id = "m1"
text = "the verdict recomputes offline"
kind = "machine"

[[acceptance_items]]
id = "m2"
text = "the seal matches"
kind = "machine"
"#,
    )
    .expect("write contract");
    let seal = truerise_cli::intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present")
    .sealed_hash;
    // A claim citing a foreign hash is refused.
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        format!(r#"{{"contract_sealed_hash":"deadbeef","verdict":"infeasible_within_budget","ledger":{GOOD_LEDGER}}}"#),
    )
    .expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        bad.to_string_lossy().into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "the claim must cite the contract it dispositions");
    // A claim citing the right hash records.
    let good = dir.join("good.json");
    std::fs::write(
        &good,
        format!(r#"{{"contract_sealed_hash":"{seal}","verdict":"infeasible_within_budget","ledger":{GOOD_LEDGER}}}"#),
    )
    .expect("write claim");
    let code = cli_main(vec![
        "truerise".into(),
        "giveup".into(),
        "--ledger".into(),
        good.to_string_lossy().into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 0);
}
