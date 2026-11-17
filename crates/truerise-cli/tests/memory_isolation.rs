//! G50 acceptance: the eval surface and the memory WRITE surface share
//! nothing. A governed run that recalls may READ seeded evidence; it must
//! never WRITE - a put during a run means the trial's independence is
//! broken by the harness itself.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use truerise_cli::{RunParams, ScriptRouter, run_books_in};
use truerise_core::RecallEvidence;
use truerise_memory::{InMemoryBackend, MemoryBackend, Record, Weights};
use truerise_memory::RecallBridge;

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_mem_iso_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A memory backend that COUNTS writes - the tripwire meter.
struct CountingMemory {
    inner: InMemoryBackend,
    puts: AtomicUsize,
}

impl CountingMemory {
    fn new() -> Self {
        Self {
            inner: InMemoryBackend::new(),
            puts: AtomicUsize::new(0),
        }
    }
}

impl MemoryBackend for CountingMemory {
    fn put(&self, record: &Record) -> Result<(), String> {
        self.puts.fetch_add(1, AtomicOrdering::Relaxed);
        self.inner.put(record)
    }
    fn all(&self) -> Result<Vec<Record>, String> {
        self.inner.all()
    }
}

/// A ContextRecall adapter over the counting backend (reads only - the
/// recall path is the ONLY thing the loop can reach: the port has no
/// write method, so the type system already forbids writes; this test
/// pins that guarantee against future refactors).
struct EvalRecall<'a> {
    memory: &'a CountingMemory,
}

impl truerise_core::ContextRecall for EvalRecall<'_> {
    fn recall_for(&self, query: &str) -> Option<RecallEvidence> {
        let recalled =
            truerise_memory::recall(&self.memory.inner, query, &Weights::default()).ok()?;
        let picked: Vec<(Record, f32)> = recalled.into_iter().take(3).collect();
        if picked.is_empty() {
            return None;
        }
        Some(RecallEvidence {
            ids: picked.iter().map(|(r, _)| r.id.clone()).collect(),
            scores_per_mille: picked
                .iter()
                .map(|(_, s)| (*s * 1000.0).round() as i64)
                .collect(),
            context: picked
                .iter()
                .map(|(r, _)| r.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        })
    }
}

#[test]
fn eval_run_performs_no_memory_writes() {
    let memory = CountingMemory::new();
    // SETUP write (before the run): one seeded record, counted.
    memory
        .put(&Record {
            id: "fold-window-note".into(),
            text: "the fold window is half-open: touching endpoints do not overlap".into(),
            importance: 0.9,
            recency: 1.0,
        })
        .expect("seed");
    assert_eq!(memory.puts.load(AtomicOrdering::Relaxed), 1);
    let bridge = RecallBridge::new(InMemoryBackend::new(), Weights::default(), 3);
    let _ = bridge; // the CLI wiring keeps its own bridge; the eval adapter below is the subject
    let adapter = EvalRecall { memory: &memory };
    let dir = temp_dir("no_writes");
    let router = ScriptRouter::plain("scripted");
    let books = run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: Some(&adapter),
        goal: "recall may read; the run must not write",
        rounds: 1,
        pinned_model: None,
    })
    .expect("governed run");
    assert_eq!(
        memory.puts.load(AtomicOrdering::Relaxed),
        1,
        "the run consumed seeded evidence but performed ZERO memory writes (transcript rows: {})",
        books.transcript.len()
    );
}

#[test]
fn eval_and_memory_share_no_writable_surface() {
    // The port-level fact: ContextRecall exposes NO write method - the loop
    // cannot write memory even if it wanted to. This test documents the
    // surface contract in executable form.
    let memory = CountingMemory::new();
    let adapter = EvalRecall { memory: &memory };
    let port: &dyn truerise_core::ContextRecall = &adapter;
    // The only reachable operation is recall; the put counter is unreachable
    // through the port (compile-time separation, runtime-asserted here).
    let _ = port.recall_for("anything");
    assert_eq!(memory.puts.load(AtomicOrdering::Relaxed), 0);
}
