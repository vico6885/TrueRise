//! G32 acceptance tests: the intent contract is sealed before the run cites
//! it, refuses to anchor an unanchored goal, and requires per-item
//! adjudication. Every named behaviour maps to the spec_map "意图合同" row.
//!
//! The tests share the run subcommand's fixed chain dir, and Windows locks
//! chain files process-wide - so the chain-touching tests serialize on one
//! mutex, and each test uses a UNIQUE goal (a distinct contract hash per
//! test keeps the chain-row lookups unambiguous).
#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

use common::{chain_guard, temp_dir};
use truerise_cli::{cli_main, intent_cmd};

fn contract_toml(goal: &str, human_kind: &str) -> String {
    format!(
        r#"
contract_version = "v1"
goal = "{goal}"

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
text = "the seal matches the recomputed hash"
kind = "machine"

[[acceptance_items]]
id = "h0"
text = "the rendered chart reads correctly"
kind = "{human_kind}"

non_goals = ["no extra tooling"]
exclusions = []
"#
    )
}

fn underfloor_contract_toml(goal: &str) -> String {
    format!(
        r#"
contract_version = "v1"
goal = "{goal}"

[[acceptance_items]]
id = "m0"
text = "first machine item"
kind = "machine"

[[acceptance_items]]
id = "m1"
text = "second machine item"
kind = "machine"

[[acceptance_items]]
id = "h0"
text = "one human item"
kind = "human"
"#
    )
}

fn token_json(sealed_hash: &str, decisions: &[(&str, &str)]) -> String {
    let rows: Vec<String> = decisions
        .iter()
        .map(|(id, decision)| format!(r#"{{"item_id":"{id}","decision":"{decision}"}}"#))
        .collect();
    format!(
        r#"{{"contract_sealed_hash":"{sealed_hash}","decisions":[{}]}}"#,
        rows.join(",")
    )
}

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

fn seal_row(hash: &str) -> String {
    chain_rows()
        .lines()
        .filter(|l| l.contains("\"intent.seal\"") && l.contains(hash))
        .last()
        .unwrap_or_else(|| panic!("seal row for {hash} must land on the run chain"))
        .to_string()
}

#[test]
fn run_seals_intent_contract_and_reports_adjudication() {
    let _guard = chain_guard("seals");
    let dir = temp_dir("happy");
    let contract_path = dir.join("intent.toml");
    std::fs::write(
        &contract_path,
        contract_toml("prove the honest ledger path per-item", "human"),
    )
    .expect("write contract");
    // First load (without token) to learn the seal, then present a full
    // per-item token bound to it.
    let seal = intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present");
    assert_eq!(seal.machine_items, 3);
    assert!(!seal.adjudicated, "no token yet");
    let token_path = dir.join("token.json");
    std::fs::write(
        &token_path,
        token_json(
            &seal.sealed_hash,
            &[
                ("m0", "accept"),
                ("m1", "accept"),
                ("m2", "accept"),
                ("h0", "reject"),
            ],
        ),
    )
    .expect("write token");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--goal".into(),
        "unused - the contract goal wins".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
        "--intent-token".into(),
        token_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 0, "a valid contract + valid token runs green");
    let row = seal_row(&seal.sealed_hash);
    assert!(
        row.contains("\"adjudicated\":true"),
        "the chain row states the adjudication status honestly: {row}"
    );
}

#[test]
fn run_intent_without_token_states_adjudicated_no() {
    let _guard = chain_guard("no_token");
    let dir = temp_dir("no_token");
    let contract_path = dir.join("intent.toml");
    std::fs::write(
        &contract_path,
        contract_toml("an unadjudicated contract still runs", "human"),
    )
    .expect("write contract");
    let seal = intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(
        code, 0,
        "an unadjudicated contract still RUNS (it just cannot certify completion)"
    );
    let row = seal_row(&seal.sealed_hash);
    assert!(row.contains("\"adjudicated\":false"));
}

#[test]
fn missing_acceptance_item_forces_question_not_assumption() {
    let dir = temp_dir("no_items");
    let contract_path = dir.join("intent.toml");
    std::fs::write(
        &contract_path,
        "contract_version = \"v1\"\ngoal = \"just do it\"\nacceptance_items = []\n",
    )
    .expect("write contract");
    // CLI level: exit 2 (a refusal, not a warning).
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "no items means ASK, never assume");
    // Unit level: the refusal names the fix.
    let err = intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect_err("refused");
    assert!(err.contains("no acceptance items"), "refusal names the fix: {err}");
}

#[test]
fn run_rejects_intent_token_mismatch() {
    let dir = temp_dir("token_mismatch");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, contract_toml("token hash mismatch case", "human"))
        .expect("write contract");
    let token_path = dir.join("token.json");
    std::fs::write(
        &token_path,
        token_json(
            "deadbeef",
            &[("m0", "accept"), ("m1", "accept"), ("m2", "accept"), ("h0", "accept")],
        ),
    )
    .expect("write token");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
        "--intent-token".into(),
        token_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "a token bound to another contract hash is a refusal");
}

#[test]
fn run_rejects_intent_token_that_skips_an_item() {
    let dir = temp_dir("token_skip");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, contract_toml("skipped item case", "human"))
        .expect("write contract");
    let seal = intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present");
    let token_path = dir.join("token.json");
    // The blanket case: only the machine items are "confirmed" - the human
    // item is skipped, which must refuse (per-item adjudication required).
    std::fs::write(
        &token_path,
        token_json(&seal.sealed_hash, &[("m0", "accept"), ("m1", "accept"), ("m2", "accept")]),
    )
    .expect("write token");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
        "--intent-token".into(),
        token_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "a token that skips an item is not a confirmation");
}

#[test]
fn run_rejects_intent_contract_with_unknown_kind() {
    let dir = temp_dir("unknown_kind");
    let contract_path = dir.join("intent.toml");
    std::fs::write(
        &contract_path,
        contract_toml("unknown kind case", "vibes"),
    )
    .expect("write contract");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2, "an unknown kind is a parse refusal, not a default");
}

#[test]
fn run_records_machine_floor_note_for_underfloor_contract() {
    let _guard = chain_guard("underfloor");
    let dir = temp_dir("underfloor");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, underfloor_contract_toml("under the floor")).expect("write contract");
    let seal = intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present");
    assert_eq!(seal.machine_items, 2, "two machine items load fine");
    let code = cli_main(vec![
        "truerise".into(),
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 0, "the contract RUNS - it just cannot certify completion");
    let row = seal_row(&seal.sealed_hash);
    assert!(
        row.contains("\"machine_items\":2"),
        "the chain row carries the machine count the floor applies to: {row}"
    );
}
