#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Chain gate tests (spec §7 M2 gate): dual-writer competition property
//! (1000 appends, 0 lost updates), tamper drill (exit != 0 + row located),
//! idempotent replay, persistence reload, legacy rows.

use serde_json::json;
use truerise_governance::{ChainError, HashChain};
use truerise_wire::Severity;

fn tmp_chain(name: &str) -> (tempdir::TempDir, HashChain) {
    let dir = tempdir::TempDir::default();
    let path = dir.path().join(name);
    let chain = HashChain::open(&path).expect("fresh chain opens");
    (dir, chain)
}

/// Minimal temp-dir helper (std-only, no external dev-dep).
mod tempdir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    impl TempDir {
        /// Create a unique temp dir.
        pub fn new() -> TempDir {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "truerise_chain_test_{}_{}",
                std::process::id(),
                id
            ));
            std::fs::create_dir_all(&path).expect("temp dir created");
            TempDir { path }
        }
    }

    impl Default for TempDir {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[test]
fn append_verify_and_persistence_reload() {
    let (_dir, chain) = tmp_chain("append_verify.jsonl");
    for i in 0..10 {
        chain
            .append(
                "test.event",
                Severity::Info,
                &json!({"i": i, "key": format!("k{i}")}),
                None,
            )
            .expect("append");
    }
    chain.verify().expect("chain intact after 10 appends");
    assert_eq!(chain.len(), 10);

    // Reload from disk: the persisted rows must verify and match.
    let reloaded =
        HashChain::open(&_dir.path().join("append_verify.jsonl")).expect("reload verifies");
    assert_eq!(reloaded.len(), 10);
    assert_eq!(reloaded.governing_rows().len(), 10);
}

#[test]
fn idempotent_replay_skips_duplicate_key() {
    let (_dir, chain) = tmp_chain("replay.jsonl");
    let first = chain
        .append(
            "claim.verdict",
            Severity::Info,
            &json!({"key": "fp-1", "verdict": "FAIL"}),
            Some("fp-1"),
        )
        .expect("first append");
    let second = chain
        .append(
            "claim.verdict",
            Severity::Info,
            &json!({"key": "fp-1", "verdict": "FAIL"}),
            Some("fp-1"),
        )
        .expect("replay");
    assert_eq!(
        first.event.seq, second.event.seq,
        "replay returns the existing row"
    );
    assert_eq!(chain.len(), 1, "no duplicate row was written");
}

#[test]
fn legacy_rows_stored_but_do_not_govern() {
    let (_dir, chain) = tmp_chain("legacy.jsonl");
    chain
        .append(
            "legacy:old-world",
            Severity::Info,
            &json!({"note": "read-only"}),
            None,
        )
        .expect("legacy append");
    chain
        .append("new.event", Severity::Info, &json!({"key": "k"}), None)
        .expect("governing append");
    assert_eq!(chain.len(), 2);
    assert_eq!(
        chain.governing_rows().len(),
        1,
        "legacy row excluded from governance"
    );
}

/// Spec §7 M2 gate: DUAL-WRITER competition - two HashChain handles over the
/// SAME file, 1000 appends total, ZERO lost updates (every write lands, the
/// seq chain stays continuous, verification passes).
#[test]
fn dual_writer_competition_1000_appends_zero_lost_updates() {
    let (_dir, chain_a) = tmp_chain("dual.jsonl");
    let path = _dir.path().join("dual.jsonl");
    let chain_b = HashChain::open(&path).expect("second handle");

    let work = |chain: &HashChain, tag: &str, n: u32| {
        for i in 0..n {
            chain
                .append(
                    "writer.event",
                    Severity::Info,
                    &json!({"writer": tag, "i": i}),
                    None,
                )
                .expect("append under contention");
        }
    };
    let a = std::thread::spawn(move || work(&chain_a, "a", 500));
    let b = std::thread::spawn(move || work(&chain_b, "b", 500));
    if a.join().is_err() {
        panic!("writer a panicked");
    }
    if b.join().is_err() {
        panic!("writer b panicked");
    }

    let final_chain = HashChain::open(&path).expect("final reload verifies");
    assert_eq!(
        final_chain.len(),
        1000,
        "zero lost updates: every append landed"
    );
    final_chain
        .verify()
        .expect("seq continuity + hash linkage intact");
    let writers: usize = final_chain
        .governing_rows()
        .iter()
        .filter(|r| r.event.detail.get("writer") == Some(&json!("a")))
        .count();
    assert_eq!(writers, 500, "both writers' rows all present");
}

/// Spec §7 M2 gate: TAMPER DRILL - inject one tampered byte into the
/// persisted file, then reopening the chain must FAIL with the offending row
/// located (the process-level `exit != 0` equivalence: the error variant).
#[test]
fn tamper_drill_fails_closed_and_locates_the_row() {
    let (_dir, chain) = tmp_chain("tamper.jsonl");
    for i in 0..5 {
        chain
            .append("audit.event", Severity::Info, &json!({"i": i}), None)
            .expect("append");
    }
    let path = _dir.path().join("tamper.jsonl");

    // Inject tampering: rewrite row 2's kind (content changes, hash stays).
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut row: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    row["kind"] = json!("audit.event-TAMPERED");
    lines[2] = serde_json::to_string(&row).unwrap();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();

    match HashChain::open(&path) {
        Err(ChainError::Tampered { rows, reason }) => {
            assert!(rows.contains(&2), "offending row located, got {rows:?}");
            assert!(reason.contains("content hash mismatch"), "reason: {reason}");
        }
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("tampered chain must NOT open"),
    }
}

/// Prev-hash linkage tampering is also located (chain surgery below row 2).
#[test]
fn prev_hash_tampering_located() {
    let (_dir, chain) = tmp_chain("prevhash.jsonl");
    for i in 0..4 {
        chain
            .append("audit.event", Severity::Info, &json!({"i": i}), None)
            .expect("append");
    }
    let path = _dir.path().join("prevhash.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut row: serde_json::Value = serde_json::from_str(&lines[3]).unwrap();
    row["prev_hash"] = json!("f".repeat(64));
    lines[3] = serde_json::to_string(&row).unwrap();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    match HashChain::open(&path) {
        Err(ChainError::Tampered { rows, .. }) => assert!(rows.contains(&3)),
        other => panic!("expected tamper error, got {other:?}"),
    }
}

/// W17: the canonical-JSON float rule - chain rows carry integers or
/// strings ONLY; a float in the detail tree is refused AT THE WRITE GATE
/// (the 10.0-vs-10 dual-implementation bug class, caught once for real).
#[test]
fn canonical_json_float_rule_test() {
    let (_dir, chain) = tmp_chain("float_rule.jsonl");
    chain
        .append("clean.row", Severity::Info, &json!({"count": 3}), None)
        .expect("integers are legal");
    let err = chain
        .append(
            "floaty.row",
            Severity::Info,
            &json!({"ratio": 0.5, "nested": {"a": [1, 2.5]}}),
            None,
        )
        .unwrap_err();
    assert!(
        matches!(err, ChainError::FloatForbidden(ref p) if p.starts_with("detail")),
        "a float path is named (map order decides which first): {err}"
    );
    // Integers (even zero) stay legal:
    chain
        .append("zero.row", Severity::Info, &json!({"count": 0}), None)
        .expect("zero is an integer");
}
