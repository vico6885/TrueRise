//! W7 / RFC 0003 acceptance tests: ONE generation, gen0 -> gen1, end to
//! end. The closed loop must be verifiable in BOTH directions (a caught
//! regression is machinery success), and no apply may ever happen without
//! a live, hash-bound human approval token.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::evolve_cmd;
use truerise_governance::HashChain;
use truerise_rsi::content_hash;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_evolve_gate_{tag}_{}_{}",
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

fn read_cert(dir: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(dir.join("gen1_certificate.json")).expect("cert exists");
    serde_json::from_str(&text).expect("cert parses")
}

/// The approval token IS the proposal hash (byte-binding).
fn gen1_token() -> String {
    content_hash(evolve_cmd::GEN1_ANSWERS)
}

#[test]
fn evolve_gen0_to_gen1_produces_lineage_certificate() {
    let dir = temp_dir("gen1");
    let code = evolve_cmd::evolve_demo(&dir, Some(&gen1_token()), false, None);
    assert_eq!(code, 0, "the closed loop completed");
    let cert = read_cert(&dir);
    assert_eq!(cert["applied"], serde_json::json!(true));
    assert_eq!(cert["verdict"]["verdict"], serde_json::json!("PASS"));
    assert_eq!(cert["verdict"]["failures"], serde_json::json!(0));
    assert_eq!(
        cert["parent"]["content_hash"],
        serde_json::json!(content_hash(evolve_cmd::GEN0_ANSWERS))
    );
    assert_eq!(
        cert["child"]["content_hash"],
        serde_json::json!(gen1_token())
    );
    assert_eq!(cert["rollback"], serde_json::Value::Null);
    // The lineage is ON THE CHAIN:
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    for kind in [
        "evolve.proposal",
        "evolve.approval",
        "evolve.apply",
        "eval.verdict",
        "evolve.certificate",
    ] {
        assert!(
            rows.iter().any(|(k, _)| k == kind),
            "chain row {kind} present: {:?}",
            rows.iter().map(|(k, _)| k).collect::<Vec<_>>()
        );
    }
    // The skill file IS the child now:
    let skill = std::fs::read_to_string(dir.join("skills").join("skill.json")).expect("skill");
    assert_eq!(skill, evolve_cmd::GEN1_ANSWERS);
}

#[test]
fn apply_without_human_approval_token_is_refused() {
    let dir = temp_dir("no_token");
    let code = evolve_cmd::evolve_demo(&dir, None, false, None);
    assert_eq!(code, 1, "a refusal stops the flow (a stop is a failure)");
    let skill = std::fs::read_to_string(dir.join("skills").join("skill.json")).expect("skill");
    assert_eq!(skill, evolve_cmd::GEN0_ANSWERS, "gen0 content untouched");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        rows.iter().any(|(k, d)| k == "evolve.apply_refused"
            && d.get("reason") == Some(&serde_json::json!("approval_required"))),
        "the refusal itself is audited"
    );
    assert!(
        !rows.iter().any(|(k, _)| k == "evolve.apply"),
        "no apply row without approval"
    );
    assert!(
        !dir.join("gen1_certificate.json").exists(),
        "no certificate for an uncompleted loop"
    );
}

#[test]
fn approval_hash_mismatch_with_proposal_is_refused() {
    let dir = temp_dir("mismatch");
    // Approve the WRONG bytes (the gen0 content, not the proposal):
    let wrong_token = content_hash(evolve_cmd::GEN0_ANSWERS);
    let code = evolve_cmd::evolve_demo(&dir, Some(&wrong_token), false, None);
    assert_eq!(code, 1);
    let skill = std::fs::read_to_string(dir.join("skills").join("skill.json")).expect("skill");
    assert_eq!(skill, evolve_cmd::GEN0_ANSWERS, "approve A cannot apply B");
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        rows.iter().any(|(k, d)| k == "evolve.apply_refused"
            && d.get("reason") == Some(&serde_json::json!("approval_hash_mismatch"))),
        "the mismatch is audited with BOTH hashes"
    );
}

#[test]
fn injected_regression_rolls_back_and_chains_the_reason() {
    let dir = temp_dir("regression");
    // The proposal is a REGRESSION: the ratchet must catch it and undo it.
    let regressed_token = content_hash(evolve_cmd::REGRESSED_ANSWERS);
    let code = evolve_cmd::evolve_demo(&dir, Some(&regressed_token), true, None);
    assert_eq!(
        code, 0,
        "a caught regression is a COMPLETED closed loop (machinery success)"
    );
    let cert = read_cert(&dir);
    assert_eq!(cert["verdict"]["verdict"], serde_json::json!("FAIL"));
    assert!(
        cert["rollback"].is_string(),
        "the rollback reason is IN the certificate: {}",
        cert["rollback"]
    );
    assert!(
        cert["rollback"]
            .as_str()
            .expect("rollback reason")
            .contains("ratchet"),
        "the reason names the ratchet"
    );
    // The skill file is BYTE-EXACT gen0 again:
    let skill = std::fs::read_to_string(dir.join("skills").join("skill.json")).expect("skill");
    assert_eq!(
        skill,
        evolve_cmd::GEN0_ANSWERS,
        "rolled back to the snapshot"
    );
    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    assert!(
        rows.iter()
            .any(|(k, d)| k == "evolve.rollback" && d.get("reason").is_some()),
        "the rollback + reason are on the chain"
    );
    // And the FAIL verdict was recorded too (nothing is hidden):
    assert!(
        rows.iter()
            .any(|(k, d)| k == "eval.verdict"
                && d.get("verdict") == Some(&serde_json::json!("FAIL"))),
        "the FAIL verdict is auditable"
    );
}

#[test]
fn generation_certificate_recomputes_identically_offline() {
    // PASS case:
    let dir = temp_dir("replay_pass");
    assert_eq!(
        evolve_cmd::evolve_demo(&dir, Some(&gen1_token()), false, None),
        0
    );
    let cert_path = dir.join("gen1_certificate.json");
    // Copy out: replay must not depend on the run dir.
    let cert_copy = temp_dir("replay_pass_cert").join("gen1_certificate.json");
    std::fs::copy(&cert_path, &cert_copy).expect("copy cert");
    assert_eq!(
        evolve_cmd::evolve_replay(&cert_copy),
        0,
        "the PASS certificate recomputes offline (no run dir, no network)"
    );
    // FAIL+rollback case: the rollback consistency check is part of replay.
    let dir2 = temp_dir("replay_fail");
    let regressed_token = content_hash(evolve_cmd::REGRESSED_ANSWERS);
    assert_eq!(
        evolve_cmd::evolve_demo(&dir2, Some(&regressed_token), true, None),
        0
    );
    let cert2 = dir2.join("gen1_certificate.json");
    let copy2 = temp_dir("replay_fail_cert").join("gen1_certificate.json");
    std::fs::copy(&cert2, &copy2).expect("copy cert");
    assert_eq!(
        evolve_cmd::evolve_replay(&copy2),
        0,
        "the FAIL+rollback certificate recomputes identically too"
    );
    // Tampering is caught:
    let tampered_text = std::fs::read_to_string(&cert_copy)
        .expect("cert")
        .replace("half-open", "totally-open");
    let tampered_path = temp_dir("tampered").join("gen1_certificate.json");
    std::fs::write(&tampered_path, tampered_text).expect("write tampered");
    assert_eq!(
        evolve_cmd::evolve_replay(&tampered_path),
        1,
        "a tampered certificate does NOT recompute"
    );
}
