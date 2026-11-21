//! G29 write-gate behavioural tests (policy units + wiring semantics),
//! moved out of the implementation file for the file-length gate.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use truerise_governance::HashChain;
use truerise_governance::write_gate::*;

fn temp_chain(tag: &str) -> (tempfile::TempDir, HashChain) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{tag}_chain.jsonl"));
    let chain = HashChain::open(&path).unwrap();
    (dir, chain)
}

#[test]
fn memory_write_requires_support_reference() {
    let gate = WriteGate::new(0);
    let mut ledger = UnverifiedLedger::new();
    // Memory path policy = REFUSE: no support, no write - not even a
    // mark (the acceptance contract for this path is refusal).
    let out = gate.decide(
        WritePath::MemoryWrite,
        "metric drift causes the failure rise",
        None,
        &mut ledger,
        None,
    );
    assert!(matches!(
        out,
        Err(WriteGateError::UnsupportedWrite {
            path: "memory_write",
            ..
        })
    ));
    assert!(ledger.entries().is_empty(), "refused writes must not mark");
    // With a g25 recomputation reference the same write admits.
    let out = gate.decide(
        WritePath::MemoryWrite,
        "metric drift precedes the failure rise",
        Some(&SupportRef::Recomputation {
            claim: "drift stat".to_string(),
            digest: "abc".to_string(),
        }),
        &mut ledger,
        None,
    );
    assert!(matches!(out, Ok(WriteDecision::Admitted)));
}

#[test]
fn skill_load_rejects_unverified_precondition() {
    let gate = WriteGate::new(0);
    let mut ledger = UnverifiedLedger::new();
    // Skill-load policy = REFUSE (its own acceptance contract), and the
    // refusal carries the assertion digest so the caller can match a
    // marked assumption against it.
    let out = gate.decide(
        WritePath::SkillLoad,
        "the patched harness passes the full bench",
        None,
        &mut ledger,
        None,
    );
    assert!(out.is_err());
}

/// THE key acceptance: an assumption marked at gen 0 still appears in
/// every later generation's certificate rows - it can never quietly
/// become "looks verified". Washing keeps it visible, flagged.
#[test]
fn unverified_assumption_survives_all_later_generations() {
    let (_dir, chain) = temp_chain("survive");
    let gate0 = WriteGate::new(0);
    let mut ledger = UnverifiedLedger::new();
    let decision = gate0
        .decide(
            WritePath::ProposalApply,
            "the patched harness passes the full bench",
            None,
            &mut ledger,
            Some(&chain),
        )
        .unwrap();
    let WriteDecision::AdmittedAsUnverifiedAssumption { assumption_id } = decision else {
        panic!("proposal-apply must mark, got {decision:?}");
    };
    // Generations 1..3 each render certificate rows: the gen-0 mark is
    // still there, unchanged, in every one of them. (The loop variable
    // must NOT be named `gen` - a reserved keyword in edition 2024.)
    for later in 1..=3u64 {
        let rows = ledger.certificate_rows();
        assert_eq!(rows.len(), 1, "gen {later}: row count drifted");
        assert!(
            rows[0].contains(&assumption_id) && rows[0].contains("gen0"),
            "gen {later} lost the gen-0 assumption: {}",
            rows[0]
        );
    }
    // Washing pays the mark off with a legitimate support kind - and
    // the washed entry stays VISIBLE with its wash record.
    ledger
        .wash(
            &assumption_id,
            &SupportRef::Recomputation {
                claim: "gen1 replay".to_string(),
                digest: "def".to_string(),
            },
            3,
            Some(&chain),
        )
        .unwrap();
    let rows = ledger.certificate_rows();
    assert!(
        rows[0].contains("washed:g25_recomputation@gen3"),
        "{}",
        rows[0]
    );
    assert!(rows[0].contains("gen0"), "wash must not erase the origin");
    // Washing again is refused: a mark is paid once.
    assert!(matches!(
        ledger.wash(
            &assumption_id,
            &SupportRef::ExternalTruth {
                source: "bench".to_string(),
                digest: "x".to_string(),
            },
            4,
            None
        ),
        Err(WriteGateError::UnknownOrWashed(_))
    ));
}

/// The honest boundary: washing without one of the three support kinds
/// is impossible, and every wash lands on the chain.
#[test]
fn washing_lands_on_chain_and_requires_a_support_kind() {
    let (_dir, chain) = temp_chain("wash");
    let gate = WriteGate::new(0);
    let mut ledger = UnverifiedLedger::new();
    let decision = gate
        .decide(
            WritePath::ProposalApply,
            "assertion text",
            None,
            &mut ledger,
            Some(&chain),
        )
        .unwrap();
    let WriteDecision::AdmittedAsUnverifiedAssumption { assumption_id } = decision else {
        panic!("mark expected");
    };
    ledger
        .wash(
            &assumption_id,
            &SupportRef::ActionEvidence {
                step_digest: "step1".to_string(),
            },
            1,
            Some(&chain),
        )
        .unwrap();
    // The wash action itself is on the chain.
    let rows = chain.governing_rows();
    assert!(
        rows.iter().any(|r| r.event.kind == "write_gate.washed"),
        "wash event missing from the chain"
    );
}

#[test]
fn unknown_marks_cannot_be_washed() {
    let mut ledger = UnverifiedLedger::new();
    assert!(matches!(
        ledger.wash(
            "nonexistent",
            &SupportRef::ExternalTruth {
                source: "s".to_string(),
                digest: "d".to_string(),
            },
            1,
            None
        ),
        Err(WriteGateError::UnknownOrWashed(_))
    ));
}

/// A gate decision that cannot land on the chain does not happen - the
/// chain is the single authority (zero-exemption audit).
#[test]
fn chain_sealed_refuses_the_gate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chain.jsonl");
    let chain = HashChain::open(&path).unwrap();
    // Corrupt the chain file so the next append fails closed.
    std::fs::write(&path, "tampered\n").unwrap();
    let gate = WriteGate::new(0);
    let mut ledger = UnverifiedLedger::new();
    let out = gate.decide(
        WritePath::MemoryWrite,
        "text",
        Some(&SupportRef::Recomputation {
            claim: "c".to_string(),
            digest: "d".to_string(),
        }),
        &mut ledger,
        Some(&chain),
    );
    assert!(matches!(out, Err(WriteGateError::ChainSealed(_))));
}
