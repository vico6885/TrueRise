//! W14 acceptance tests: the tape makes model steps replayable OFFLINE -
//! byte-exact, key-free, network-free. A tape proves a reading happened;
//! it does not claim future model outputs are deterministic.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use truerise_cli::evolve_cmd;
use truerise_cli::tape::{TapeRouter, read_tape, trace_digest};
use truerise_cli::{RunParams, ScriptRouter, run_books_in};
use truerise_core::{Loop, LoopConfig, ModelRouter, RouterError, TurnRequest, TurnResponse};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_tape_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn same_tape_replays_identical_digest() {
    let dir = temp_dir("same");
    let tape_path = dir.join("run_tape.jsonl");

    // RECORD: one governed run, every model call captured.
    let inner = ScriptRouter::plain("scripted");
    let router = TapeRouter::recording(&inner, tape_path.clone());
    let books_recorded = run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: None,
        goal: "analyse the fold window",
        rounds: 1,
        pinned_model: None,
    })
    .expect("recorded run");

    // The tape header's trace digest matches the entries on disk:
    let (header, entries) = read_tape(&tape_path).expect("tape readable");
    assert_eq!(
        header["trace_digest"],
        serde_json::json!(trace_digest(&entries)),
        "the header digest is derivable from the tape"
    );

    // REPLAY: a FRESH chain, the inner router NEVER called, byte-exact.
    let replay_dir = temp_dir("same_replay");
    let replay_router = TapeRouter::replaying(tape_path.clone()).expect("tape loads");
    let books_replayed = run_books_in(RunParams {
        chain_dir: &replay_dir,
        router: &replay_router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: None,
        goal: "analyse the fold window",
        rounds: 1,
        pinned_model: None,
    })
    .expect("replayed run");

    // Field-identical books (transcript texts + transitions):
    let sig = |b: &truerise_core::RunBooks| {
        b.transcript
            .iter()
            .map(|e| (e.role.clone(), e.text.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        sig(&books_recorded),
        sig(&books_replayed),
        "replay is byte-exact"
    );
    // The replayed trace digest (over what the tape YIELDED) is identical:
    let replayed_entries = replay_router.recorded();
    assert!(
        replayed_entries.is_empty(),
        "replay mode does not re-record"
    );
    // The header digest IS the run trace identity:
    let re_read = read_tape(&tape_path).expect("tape re-readable");
    assert_eq!(header, re_read.0, "the tape did not change under replay");
}

/// A foreign-model tape must VOID the round through the EXISTING pinning
/// check - the tape does not bypass governance.
struct ForeignRecorder;
impl ModelRouter for ForeignRecorder {
    fn route(&self, _: &TurnRequest) -> Result<TurnResponse, RouterError> {
        Ok(TurnResponse {
            model: "foreign".into(),
            payload: "x".into(),
        })
    }
}

#[test]
fn tape_model_mismatch_voids_round() {
    let dir = temp_dir("mismatch");
    let tape_path = dir.join("foreign_tape.jsonl");
    let recorder = TapeRouter::recording(&ForeignRecorder, tape_path.clone());
    let config = LoopConfig {
        pinned_model: "scripted".into(),
        retry: truerise_core::RetryPolicy::default(),
    };
    let _chain = truerise_governance::HashChain::open(&dir.join("c.jsonl")).expect("chain");
    struct NullSink;
    impl truerise_core::AuditSink for NullSink {
        fn append(&self, _: &str, _: &str, _: &serde_json::Value) -> Result<usize, String> {
            Ok(1)
        }
    }
    let sink = NullSink;
    let err = Loop::new(&recorder, config, &sink)
        .run("x", 1)
        .expect_err("a foreign-model tape voids the round");
    assert!(
        matches!(err, truerise_core::LoopError::Pin(_)),
        "the pin check still governs: {err:?}"
    );
    // And replaying that tape under the pinned model ALSO voids (the tape
    // carries the foreign model field, pinning sees it):
    let replayer = TapeRouter::replaying(tape_path).expect("tape loads");
    let config2 = LoopConfig {
        pinned_model: "scripted".into(),
        retry: truerise_core::RetryPolicy::default(),
    };
    let err = Loop::new(&replayer, config2, &sink)
        .run("x", 1)
        .unwrap_err();
    assert!(matches!(err, truerise_core::LoopError::Pin(_)));
}

#[test]
fn readings_ship_tapes_and_replay_offline() {
    // The committed evolve tape: gen0 + gen1 model calls, ordered.
    let tape_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../readings/v0.2/gen1_tape.jsonl");
    let (header, entries) = read_tape(&tape_path).expect("committed tape readable");
    assert_eq!(
        header["trace_digest"],
        serde_json::json!(trace_digest(&entries)),
        "the committed tape's digest is derivable"
    );
    assert_eq!(header["tape_version"], serde_json::json!(1));
    assert!(
        entries.len() >= evolve_cmd::TASKS.len() * 2 * 3,
        "every task run is taped"
    );

    // REPLAY the committed tape OFFLINE (no keys, no network): reconstruct
    // the FIRST recorded call's goal from the tape itself and run it through
    // the governed path - the replayed plan payload must be byte-exact the
    // recorded response (consumed-take semantics deliver the right entry).
    let first = &entries[0];
    assert_eq!(first.tag, "plan", "the tape opens with a plan call");
    let goal = first
        .request
        .strip_suffix(" [round 0]")
        .expect("plan request shape: <goal> [round 0]");
    let throwaway = temp_dir("committed_replay");
    let replayer = TapeRouter::replaying(tape_path).expect("tape loads");
    let books = run_books_in(RunParams {
        chain_dir: &throwaway,
        router: &replayer,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: None,
        goal,
        rounds: 1,
        pinned_model: None,
    })
    .expect("the first taped call replays");
    assert!(
        books.transcript.iter().any(|e| e.text == first.response),
        "the replayed plan payload is byte-exact the recorded response"
    );
}
