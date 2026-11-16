//! W6 acceptance tests: memory participates in plan-context construction
//! (with audited evidence), and observe exports the REAL run chain one-way.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;
use truerise_cli::{RunParams, ScriptRouter, run_books_in};
use truerise_governance::HashChain;
use truerise_memory::{InMemoryBackend, MemoryBackend, RecallBridge, Record, Weights};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_run_memobs_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn seeded_bridge() -> RecallBridge {
    let backend = InMemoryBackend::new();
    backend
        .put(&Record {
            id: "fold-window-note".into(),
            text: "the fold window is half-open: touching endpoints do not overlap".into(),
            importance: 0.9,
            recency: 0.9,
        })
        .expect("seed");
    backend
        .put(&Record {
            id: "unrelated-note".into(),
            text: "the weather today is unrelated to the goal".into(),
            importance: 0.4,
            recency: 0.5,
        })
        .expect("seed");
    RecallBridge::new(backend, Weights::default(), 3)
}

fn chain_rows(chain: &HashChain) -> Vec<(String, serde_json::Value)> {
    chain
        .governing_rows()
        .iter()
        .map(|r| (r.event.kind.clone(), r.event.detail.clone()))
        .collect()
}

#[test]
fn recall_row_records_scores_and_injection() {
    let dir = temp_dir("recall");
    let bridge = seeded_bridge();
    let router = ScriptRouter::plain("scripted");
    let books = run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: Some(&bridge),
        goal: "analyse the fold window",
        rounds: 1,
        pinned_model: None,
    })
    .expect("run completes");

    let chain = HashChain::open(&dir.join("run.jsonl")).expect("chain");
    let rows = chain_rows(&chain);
    let recalls: Vec<_> = rows.iter().filter(|(k, _)| k == "memory.recall").collect();
    assert_eq!(recalls.len(), 1, "one recall row per plan turn: {rows:?}");
    let detail = &recalls[0].1;
    // Fusion gives EVERY record a score (recency/importance channels); the
    // BM25-matching record must rank FIRST and unrelated content must not
    // outrank it. top_k=3 > corpus -> all records inject, ranked.
    assert_eq!(detail.get("injected"), Some(&json!(2)));
    assert_eq!(
        detail.get("ids"),
        Some(&json!(["fold-window-note", "unrelated-note"])),
        "the matching record ranks first (BM25 dominance)"
    );
    // Scores are per-mille INTEGERS (float rule): every value integral.
    let scores = detail.get("scores_per_mille").expect("scores present");
    for value in scores.as_array().expect("array") {
        assert!(value.is_i64(), "score must be an integer: {value}");
    }
    // The injected context really reached the plan turn (book 1):
    assert!(
        books
            .transcript
            .iter()
            .any(|e| e.role == "model" && e.text.contains("recall context")),
        "the plan payload carried the recalled context"
    );
}

#[test]
fn export_produces_one_span_per_row_and_redacts() {
    let dir = temp_dir("export");
    let bridge = seeded_bridge();
    let router = ScriptRouter::plain("scripted");
    run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: Some(&bridge),
        goal: "goal",
        rounds: 1,
        pinned_model: None,
    })
    .expect("run completes");
    // A configuration row carrying a secret-looking key (the export must
    // redact it; the chain keeps the truth - the chain is the authority).
    {
        let chain_path = dir.join("run.jsonl");
        let chain = HashChain::open(&chain_path).expect("chain");
        chain
            .append(
                "run.config",
                truerise_wire::Severity::Info,
                &json!({ "model": "scripted", "api_key": "sk-should-be-redacted" }),
                None,
            )
            .expect("config row");
    }
    let chain_path = dir.join("run.jsonl");
    let out_path = dir.join("observe").join("spans.jsonl");
    let spans = truerise_observe::export_jsonl(&chain_path, &out_path).expect("export");
    let chain = HashChain::open(&chain_path).expect("chain");
    assert_eq!(spans, chain.len(), "one span per chain row");
    let exported = std::fs::read_to_string(&out_path).expect("spans file");
    assert!(
        !exported.contains("sk-should-be-redacted"),
        "secrets never leave through the span export"
    );
    assert!(
        exported.contains("<redacted>"),
        "the sensitive key is visibly redacted"
    );
    // Spans carry the row hashes, so the projection stays verifiable.
    assert!(exported.contains("\"hash\""));
}

#[test]
fn export_never_mutates_chain_digest() {
    let dir = temp_dir("digest");
    let router = ScriptRouter::plain("scripted");
    run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: None,
        goal: "goal",
        rounds: 1,
        pinned_model: None,
    })
    .expect("run completes");
    let chain_path = dir.join("run.jsonl");
    let before = truerise_observe::chain_digest(&chain_path);
    let out_path = dir.join("observe").join("spans.jsonl");
    truerise_observe::export_jsonl(&chain_path, &out_path).expect("export");
    let after = truerise_observe::chain_digest(&chain_path);
    assert_eq!(before, after, "the export is one-way: digest unchanged");
}
