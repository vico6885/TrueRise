//! G32 red-line acceptance (review P0): human items need a token, rejected
//! tokens never certify. SEPARATE binary on purpose - the evolve budget is
//! a per-process singleton (quota 64), and 8 evolve tests in ONE binary
//! would exhaust it (the singleton semantics working as designed).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::evolve_cmd;
use truerise_rsi::content_hash;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_compcert_token_{tag}_{}_{}",
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

fn token_json(seal: &str, h0_decision: &str) -> String {
    let decisions = [
        r#"{"item_id":"m0","decision":"accept"}"#,
        r#"{"item_id":"m1","decision":"accept"}"#,
        r#"{"item_id":"m2","decision":"accept"}"#,
    ];
    format!(
        r#"{{"contract_sealed_hash":"{seal}","decisions":[{},{{"item_id":"h0","decision":"{h0_decision}"}}]}}"#,
        decisions.join(",")
    )
}

#[test]
fn certificate_refuses_human_done_without_token() {
    // G32 red line (review P0): a human-judged item marked done WITHOUT a
    // user token is a silent fabrication - the certificate is refused.
    let dir = temp_dir("human_no_token");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json(&seal, &content_hash(evolve_cmd::GEN1_ANSWERS)).replace(
            r#"{"item_id": "h0", "status": {"status": "not_verified", "why": "needs a human eye", "cost_note": "one review pass"}}"#,
            r#"{"item_id": "h0", "status": {"status": "done", "evidence": "the chart looks fine"}}"#,
        ),
    )
    .expect("write report");
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&binding(&contract_path, Some(&report_path))),
    );
    assert_eq!(code, 1, "human done without token never issues");
    assert!(!dir.join("gen1_certificate.json").exists());
}

#[test]
fn certificate_accepts_human_done_with_token_accept() {
    // The same report WITH a per-item token that ACCEPTS h0: the ledger
    // closes and the certificate issues.
    let dir = temp_dir("human_token");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json(&seal, &content_hash(evolve_cmd::GEN1_ANSWERS)).replace(
            r#"{"item_id": "h0", "status": {"status": "not_verified", "why": "needs a human eye", "cost_note": "one review pass"}}"#,
            r#"{"item_id": "h0", "status": {"status": "done", "evidence": "the chart looks fine"}}"#,
        ),
    )
    .expect("write report");
    let token_path = dir.join("token.json");
    std::fs::write(&token_path, token_json(&seal, "accept")).expect("write token");
    let mut b = binding(&contract_path, Some(&report_path));
    b.token_path = Some(token_path);
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&b),
    );
    assert_eq!(code, 0, "human done WITH a token accept is legitimate");
}

#[test]
fn certificate_refuses_token_with_rejection() {
    // G32 red line: a token carrying a REJECT decision means the contract
    // was not accepted - no certificate may cite it, whatever the report says.
    let dir = temp_dir("rejected");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let report_path = dir.join("report.json");
    std::fs::write(
        &report_path,
        report_json(&seal, &content_hash(evolve_cmd::GEN1_ANSWERS)),
    )
    .expect("write report");
    let token_path = dir.join("token.json");
    std::fs::write(&token_path, token_json(&seal, "reject")).expect("write token");
    let mut b = binding(&contract_path, Some(&report_path));
    b.token_path = Some(token_path);
    let code = evolve_cmd::evolve_demo(
        &dir,
        Some(&content_hash(evolve_cmd::GEN1_ANSWERS)),
        false,
        None,
        Some(&b),
    );
    assert_eq!(code, 1, "a rejected contract is not a completion basis");
}
