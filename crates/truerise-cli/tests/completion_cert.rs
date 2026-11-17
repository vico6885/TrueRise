//! G35/G36 certificate-integration acceptance: a generation issued under a
//! sealed intent contract MUST carry a closing completion ledger - the
//! certificate is refused otherwise, and replay recomputes the ledger
//! offline. Named behaviours map to the spec_map "证书完成闭合" row.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::evolve_cmd;
use truerise_rsi::content_hash;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_compcert_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

const CONTRACT: &str = r#"
contract_version = "v1"
goal = "gen1 under an intent contract"

[[acceptance_items]]
id = "m0"
text = "the verdict recomputes offline"
kind = "machine"

[[acceptance_items]]
id = "m1"
text = "the seal matches the recomputed hash"
kind = "machine"

[[acceptance_items]]
id = "m2"
text = "the ledger row exists"
kind = "machine"

[[acceptance_items]]
id = "h0"
text = "the rendered chart reads correctly"
kind = "human"
"#;

fn learn_seal(contract_path: &std::path::Path) -> String {
    truerise_cli::intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present")
    .sealed_hash
}

fn report_json(seal: &str, m1_hash: &str) -> String {
    format!(
        r#"{{
            "contract_sealed_hash": "{seal}",
            "entries": [
                {{"item_id": "m0", "status": {{"status": "done", "evidence": "artifacts/verdict.json#verdict"}}}},
                {{"item_id": "m1", "status": {{"status": "done", "evidence": "{m1_hash}"}}}},
                {{"item_id": "m2", "status": {{"status": "not_done", "reason": "condition_unmet", "note": "the trigger never fired this generation"}}}},
                {{"item_id": "h0", "status": {{"status": "not_verified", "why": "needs a human eye", "cost_note": "one review pass"}}}}
            ]
        }}"#
    )
}

fn binding<'a>(
    contract_path: &'a std::path::Path,
    report_path: Option<&'a std::path::Path>,
) -> truerise_cli::completion_cert::CompletionBinding {
    truerise_cli::completion_cert::CompletionBinding {
        intent_path: contract_path.to_path_buf(),
        report_path: report_path.map(PathBuf::from),
        token_path: None,
    }
}

fn read_cert(dir: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(dir.join("gen1_certificate.json")).expect("cert exists");
    serde_json::from_str(&text).expect("cert parses")
}

#[test]
fn certificate_skips_completion_without_intent() {
    let dir = temp_dir("plain");
    let code = evolve_cmd::evolve_demo(&dir, Some(&content_hash(evolve_cmd::GEN1_ANSWERS)), false, None, None);
    assert_eq!(code, 0);
    let cert = read_cert(&dir);
    assert!(
        cert["completion"].is_null(),
        "a contract-free certificate carries no completion section"
    );
}

#[test]
fn certificate_with_sealed_contract_requires_completion_ledger() {
    let dir = temp_dir("no_ledger");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    // The contract is bound but the ledger is missing: the CERTIFICATE is
    // refused (exit 1), not downgraded.
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&binding(&contract_path, None)),
    );
    assert_eq!(code, 1, "no ledger, no certificate");
    assert!(
        !dir.join("gen1_certificate.json").exists(),
        "the refused certificate never reaches the disk"
    );
}

#[test]
fn certificate_completion_ledger_closes_against_contract() {
    let dir = temp_dir("closed");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json(&seal, &content_hash(evolve_cmd::GEN1_ANSWERS)),
    )
    .expect("write report");
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&binding(&contract_path, Some(&report_path))),
    );
    assert_eq!(code, 0, "a complete three-way ledger closes the certificate");
    let cert = read_cert(&dir);
    assert_eq!(
        cert["completion"]["contract_sealed_hash"],
        serde_json::json!(seal)
    );
    assert_eq!(cert["completion"]["machine_items"], serde_json::json!(3));
    assert_eq!(cert["completion"]["items_total"], serde_json::json!(4));
    // The chain row binds the generation to the seal.
    let chain = truerise_governance::HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows: Vec<String> = chain
        .governing_rows()
        .iter()
        .map(|r| r.event.kind.clone())
        .collect();
    assert!(
        rows.iter().any(|k| k == "intent.seal"),
        "the binding lands on the evolve chain: {rows:?}"
    );
}

#[test]
fn certificate_refuses_report_bound_to_another_contract() {
    let dir = temp_dir("cross");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json("deadbeef", &content_hash(evolve_cmd::GEN1_ANSWERS)),
    )
    .expect("write report");
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&binding(&contract_path, Some(&report_path))),
    );
    assert_eq!(code, 1, "a report citing another hash never issues");
}

#[test]
fn replay_verifies_the_completion_ledger() {
    let dir = temp_dir("replay");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json(&seal, &content_hash(evolve_cmd::GEN1_ANSWERS)),
    )
    .expect("write report");
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&binding(&contract_path, Some(&report_path))),
    );
    assert_eq!(code, 0);
    let cert_path = dir.join("gen1_certificate.json");
    assert_eq!(
        evolve_cmd::evolve_replay(&cert_path),
        0,
        "the completion ledger recomputes offline"
    );
    // Tamper with the report's hash: replay must go red.
    let mut cert = read_cert(&dir);
    cert["completion"]["report"]["contract_sealed_hash"] = serde_json::json!("deadbeef");
    let tampered = dir.join("tampered.json");
    std::fs::write(&tampered, serde_json::to_string_pretty(&cert).expect("serialize"))
        .expect("write tampered");
    assert_eq!(
        evolve_cmd::evolve_replay(&tampered),
        1,
        "a tampered ledger does not recompute"
    );
}

