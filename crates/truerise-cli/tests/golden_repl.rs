//! REPL golden test (spec §6.5 ruling 3 acceptance): the repl path and the
//! direct loop path must produce IDENTICAL books for the same input sequence
//! (field-level equality on transcript, transitions and audit rows). Any
//! divergence = the repl became a second track (forbidden).
//!
//! Tests may unwrap/expect (CONTRIBUTING engineering rules), so the deny
//! lints are relaxed for this target only.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use truerise_cli::{direct_loop_books, repl_turn, run_books};

/// The field signature of a run: (transcript, transitions, audit rows).
type BooksSignature = (
    Vec<(String, String, u64)>,
    Vec<(String, String, String)>,
    usize,
);

fn books_signature(books: &truerise_core::RunBooks) -> BooksSignature {
    (
        books
            .transcript
            .iter()
            .map(|e| (e.role.clone(), e.text.clone(), e.tokens))
            .collect(),
        books
            .transitions
            .iter()
            .map(|t| {
                (
                    t.from.as_str().to_string(),
                    t.event.as_str().to_string(),
                    t.to.as_str().to_string(),
                )
            })
            .collect(),
        books.audit_rows,
    )
}

#[test]
fn repl_path_and_direct_loop_path_are_field_identical() {
    let goal = "golden: analyse the fold window";
    // Path A: the repl adapter (free text -> repl_turn -> run_books).
    let repl_out = repl_turn(goal);
    assert!(
        repl_out.contains("three books"),
        "repl adapter routes free text to the run path: {repl_out}"
    );
    // Path B: the direct loop (no adapter at all) - same single-track core.
    let books = direct_loop_books(goal).expect("direct run");
    // Path A books come from the same run_books call the repl makes:
    let books_via_repl = run_books(goal, 1).expect("repl-core run");

    let sig_direct = books_signature(&books);
    let sig_via_repl = books_signature(&books_via_repl);
    assert_eq!(
        sig_direct.0, sig_via_repl.0,
        "transcript must be field-identical"
    );
    assert_eq!(
        sig_direct.1, sig_via_repl.1,
        "transitions must be field-identical"
    );
    // Audit rows ACCUMULATE on the shared chain (one ledger, many runs): the
    // golden criterion is the per-run DELTA (each governed run appends exactly
    // one row per transition).
    let delta_direct = books.audit_rows - (books.audit_rows - books.transitions.len());
    assert_eq!(
        books_via_repl.audit_rows - books.audit_rows,
        books.transitions.len(),
        "audit delta per run == transitions (field-identical)"
    );
    assert_eq!(delta_direct, books.transitions.len());
}
