//! G39 acceptance tests: the self-service verification bundle. Coverage is
//! sealed from the contract's machine items, the bundle runs outside the
//! repository, and an inexecutable observation routes to NOT-VERIFIED
//! instead of silence.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::{cli_main, verify_cmd};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_bundle_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

const CONTRACT: &str = r#"
contract_version = "v1"
goal = "self-service acceptance"

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

/// RAII guard: restores the process cwd even if the test panics.
struct CwdGuard {
    previous: PathBuf,
}
impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn bundle_json(contract_hash: &str, base: &std::path::Path, m2_target: &str) -> String {
    use serde_json::json;
    // json! escapes the Windows backslashes in the absolute paths - a raw
    // format! would emit invalid \\a / \\v escapes.
    let verdict = format!("{}/artifacts/verdict.json#verdict", base.display());
    let seal = format!("{}/artifacts/seal.txt", base.display());
    json!({
        "contract_sealed_hash": contract_hash,
        "observations": [
            {"item_id": "m0", "kind": "artifact_field", "target": verdict, "expected": "PASS"},
            {"item_id": "m1", "kind": "file_exists", "target": seal, "expected": ""},
            {"item_id": "m2", "kind": "artifact_field", "target": m2_target, "expected": "42"}
        ]
    })
    .to_string()
}

fn learn_seal(contract_path: &std::path::Path) -> String {
    let seal = truerise_cli::intent_cmd::load_intent(&[
        "run".into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ])
    .expect("intent loads")
    .expect("intent present");
    seal.sealed_hash
}

#[test]
fn bundle_passes_when_all_observations_match() {
    let dir = temp_dir("happy");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    std::fs::create_dir_all(dir.join("artifacts")).expect("artifacts dir");
    std::fs::write(dir.join("artifacts/verdict.json"), r#"{"verdict":"PASS"}"#)
        .expect("write artifact");
    std::fs::write(dir.join("artifacts/seal.txt"), "sha256:deadbeef").expect("write seal");
    std::fs::write(
        dir.join("artifacts/count.json"),
        r#"{"items":{"m2":"42"}}"#,
    )
    .expect("write count");
    let bundle_path = dir.join("bundle.json");
    let m2 = format!("{}/artifacts/count.json#items.m2", dir.display());
    std::fs::write(&bundle_path, bundle_json(&seal, &dir, &m2)).expect("write bundle");
    let report = verify_cmd::run_bundle(&bundle_path, &contract_path).expect("bundle runs");
    assert_eq!(report.exit_code(), 0, "all observations pass: {}", report.render());
    assert!(report.render().contains("3 pass / 0 fail / 0 not-verified"));
}

#[test]
fn bundle_expected_observations_sealed_from_contract_items() {
    // A bundle that skips machine item m1 (cherry-picking the easy ones) is
    // refused - coverage is sealed from the contract, not from the author.
    let dir = temp_dir("coverage");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let bundle_path = dir.join("bundle.json");
    std::fs::write(
        &bundle_path,
        format!(
            r#"{{"contract_sealed_hash":"{seal}","observations":[
                {{"item_id":"m0","kind":"file_exists","target":"a.txt","expected":""}},
                {{"item_id":"m2","kind":"file_exists","target":"b.txt","expected":""}}
            ]}}"#
        ),
    )
    .expect("write bundle");
    let err = verify_cmd::run_bundle(&bundle_path, &contract_path).expect_err("refused");
    assert!(err.contains("no observation for machine item m1"), "names the gap: {err}");
    // CLI level: exit 2.
    let code = cli_main(vec![
        "truerise".into(),
        "verify".into(),
        "--bundle".into(),
        bundle_path.to_string_lossy().into(),
        "--intent".into(),
        contract_path.to_string_lossy().into(),
    ]);
    assert_eq!(code, 2);
}

#[test]
fn unverifiable_item_routes_to_not_verified() {
    // The artifact is MISSING: the observation cannot run, so it must land
    // in the NOT-VERIFIED bucket (exit 1 - a gap is not a pass) with the
    // routing note printed, never silently dropped.
    let dir = temp_dir("unverifiable");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    std::fs::create_dir_all(dir.join("artifacts")).expect("artifacts dir");
    std::fs::write(dir.join("artifacts/verdict.json"), r#"{"verdict":"PASS"}"#)
        .expect("write artifact");
    std::fs::write(dir.join("artifacts/seal.txt"), "x").expect("write seal");
    let bundle_path = dir.join("bundle.json");
    let m2 = format!("{}/artifacts/count.json#items.m2", dir.display());
    std::fs::write(&bundle_path, bundle_json(&seal, &dir, &m2)).expect("write bundle");
    let report = verify_cmd::run_bundle(&bundle_path, &contract_path).expect("bundle runs");
    assert_eq!(report.exit_code(), 1, "not-verified is not a pass");
    let rendered = report.render();
    assert!(rendered.contains("NOT-VERIFIED  m2"), "item routed explicitly: {rendered}");
    assert!(rendered.contains("silence is not coverage"));
}

#[test]
fn bundle_rejects_wrong_contract_hash() {
    let dir = temp_dir("hash");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let bundle_path = dir.join("bundle.json");
    std::fs::write(
        &bundle_path,
        bundle_json("deadbeef", &dir, "x#items.m2"),
    )
    .expect("write bundle");
    let err = verify_cmd::run_bundle(&bundle_path, &contract_path).expect_err("refused");
    assert!(err.contains("bundle refused"), "refusal names the cause: {err}");
}

#[test]
fn bundle_fail_is_a_fail_not_a_skip() {
    // The artifact exists but the field disagrees: FAIL (exit 1), distinct
    // from NOT-VERIFIED.
    let dir = temp_dir("fail");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    std::fs::create_dir_all(dir.join("artifacts")).expect("artifacts dir");
    std::fs::write(dir.join("artifacts/verdict.json"), r#"{"verdict":"FAIL"}"#)
        .expect("write artifact");
    std::fs::write(dir.join("artifacts/seal.txt"), "x").expect("write seal");
    std::fs::write(dir.join("artifacts/count.json"), r#"{"items":{"m2":"42"}}"#)
        .expect("write count");
    let bundle_path = dir.join("bundle.json");
    let m2 = format!("{}/artifacts/count.json#items.m2", dir.display());
    std::fs::write(&bundle_path, bundle_json(&seal, &dir, &m2)).expect("write bundle");
    let report = verify_cmd::run_bundle(&bundle_path, &contract_path).expect("bundle runs");
    assert_eq!(report.exit_code(), 1);
    assert!(report.render().contains("FAIL  m0"), "mismatch is FAIL: {}", report.render());
}

#[test]
fn bundle_runs_without_repo_checkout() {
    // The point of the bundle: run it from a DIFFERENT cwd with absolute
    // paths - no repository, no keys, no network. Relative paths in the
    // bundle resolve against the CALLER's cwd.
    let dir = temp_dir("no_repo");
    let contract_path = dir.join("intent.toml");
    std::fs::write(&contract_path, CONTRACT).expect("write contract");
    let seal = learn_seal(&contract_path);
    let workdir = dir.join("user_machine");
    std::fs::create_dir_all(&workdir).expect("user dir");
    std::fs::write(workdir.join("local.json"), r#"{"items":{"m2":"42"}}"#).expect("local artifact");
    let bundle_path = dir.join("bundle.json");
    std::fs::write(
        &bundle_path,
        format!(
            r#"{{"contract_sealed_hash":"{seal}","observations":[
                {{"item_id":"m0","kind":"artifact_field","target":"local.json#verdict","expected":"PASS"}},
                {{"item_id":"m1","kind":"file_exists","target":"local.json","expected":""}},
                {{"item_id":"m2","kind":"artifact_field","target":"local.json#items.m2","expected":"42"}}
            ]}}"#
        ),
    )
    .expect("write bundle");
    let _cwd_guard = CwdGuard {
        previous: std::env::current_dir().expect("cwd"),
    };
    std::env::set_current_dir(&workdir).expect("chdir");
    let report = verify_cmd::run_bundle(&bundle_path, &contract_path).expect("bundle runs from a foreign cwd");
    // m0 fails (no verdict field in local.json) but the RUN itself is cwd-
    // independent - the point of this test is execution, not the verdict.
    assert!(
        report.render().contains("NOT-VERIFIED  m0"),
        "relative paths resolve against the caller's cwd: {}",
        report.render()
    );
}
