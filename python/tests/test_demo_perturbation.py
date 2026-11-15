"""Perturbation-flipping acceptance: verdict lines derive from inputs, nothing hardcoded.

The spec's real acceptance (§1.5): same code path, different inputs -> the
corresponding verdict line flips; --honest flips all -> PASS; --replay
recomputes offline; tampering with the ledger fails closed.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from truerise.demo import DEFAULT_TRIALS, replay, run_demo
from truerise.gates import EvidenceRecord, evidence_gate
from truerise.ledger import Ledger, LedgerTampered
from truerise.multiplicity import MultiplicityAccountant


def test_default_all_three_caught(tmp_path: Path) -> None:
    v = run_demo(tmp_path)
    assert v.verdict == "FAIL"
    assert any(r.startswith("1) prereg diff") for r in v.reasons)
    assert any(r.startswith("2) trials=") for r in v.reasons)
    assert any(r.startswith("4) evidence: 0 executor-signed") for r in v.reasons)
    assert v.trials == DEFAULT_TRIALS
    assert v.corrected_significance < 0.001  # 0.05/1000 = 5e-5


def test_honest_seal_flips_s1_only(tmp_path: Path) -> None:
    v = run_demo(tmp_path, honest_seal=True)
    assert v.verdict == "FAIL"  # S2/S3 still fail
    assert not any(r.startswith("1) prereg diff") for r in v.reasons)
    assert any(r.startswith("2) trials=") for r in v.reasons)


def test_claim_p_flips_s2(tmp_path: Path) -> None:
    v = run_demo(tmp_path, claim_p=0.001, n_trials=10, honest_seal=True, sign_records=True)
    assert v.verdict == "PASS"  # all three honest -> PASS
    assert not any(r.startswith("2)") for r in v.reasons)


def test_honest_run_end_to_end(tmp_path: Path) -> None:
    """An honest claim: run once (no many-shots), p under the base bar, sealed
    thresholds untouched, signed record -> PASS through the same code path."""
    v = run_demo(tmp_path, n_trials=1, claim_p=0.04, honest_seal=True, sign_records=True)
    assert v.verdict == "PASS"
    assert v.reasons == []


def test_n_trials_moves_the_denominator_only(tmp_path: Path) -> None:
    v1000 = run_demo(tmp_path / "a")
    v10 = run_demo(tmp_path / "b", n_trials=10)
    assert v10.trials == 10
    assert v10.corrected_significance == pytest.approx(0.005)  # 0.05/10
    assert v1000.corrected_significance == pytest.approx(0.00005)  # 0.05/1000
    # S2 still fails at 10 trials (claim p=0.03 > 0.005) but the printed bar moved:
    assert any("0.005" in r or "5.0e-03" in r for r in v10.reasons)


def test_sign_records_flips_s3(tmp_path: Path) -> None:
    v = run_demo(tmp_path, sign_records=True, honest_seal=True)
    assert not any(r.startswith("4) evidence: 0 executor-signed") for r in v.reasons)
    assert v.evidence_signed_records == 1  # the self-report record stays unsigned
    assert v.evidence_effective_level == 4  # signed+declared proves L4 (cap only hits undeclared)


def test_undeclared_class_caps_even_when_signed() -> None:
    ev = evidence_gate([EvidenceRecord(source="s", declared_level=6, executor_signed=True,
                                       declared_source_class="undeclared")], claimed_level=6)
    assert ev.effective_level == 3  # UNDECLARED_CEILING (only down, per spec)
    assert any("caps at L3" in d["reason"] for d in ev.downgrades)


def test_drop_fingerprint_adds_integrity_line(tmp_path: Path) -> None:
    v = run_demo(tmp_path, drop_fingerprint=True)
    assert any(r.startswith("3) fingerprint:") for r in v.reasons)
    assert v.fingerprint["digest"] == "dropped"


def test_replay_recomputes_identical_verdict(tmp_path: Path) -> None:
    run_demo(tmp_path)
    v = replay(tmp_path)
    assert v.verdict == "FAIL"
    assert "identical verdict" in v.replay
    assert "chain verified" in v.replay


def test_ledger_tampering_fails_closed_with_row_location(tmp_path: Path) -> None:
    run_demo(tmp_path)
    ledger_path = tmp_path / "claims.jsonl"
    rows = [json.loads(x) for x in ledger_path.read_text(encoding="utf-8").splitlines() if x.strip()]
    rows[-1]["detail"]["verdict"] = "PASS"  # tamper the stored verdict
    ledger_path.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
    with pytest.raises(LedgerTampered) as exc:
        Ledger.load(ledger_path)
    assert any("content hash mismatch" in p for p in exc.value.problems)
    # and the replay refuses to bless it
    with pytest.raises(LedgerTampered):
        replay(tmp_path)


def test_idempotent_replay_skips_duplicate_fingerprint(tmp_path: Path) -> None:
    run_demo(tmp_path)
    run_demo(tmp_path)  # same fingerprint again -> ledger row skipped
    lines = [x for x in (tmp_path / "claims.jsonl").read_text(encoding="utf-8").splitlines() if x.strip()]
    governing = [json.loads(x) for x in lines if json.loads(x)["kind"] == "claim.verdict.v1"]
    assert len(governing) == 1


def test_multiplicity_never_deletes_counts() -> None:
    acc = MultiplicityAccountant(base_significance=0.05)
    acc.count("k", 500)
    acc.count("k", 500)
    assert acc.trials("k") == 1000
    assert acc.corrected_significance("k") == pytest.approx(0.00005)
    assert not acc.passes("k", 0.03)


def test_unsigned_records_contribute_nothing() -> None:
    ev = evidence_gate([EvidenceRecord(source="s", declared_level=6, executor_signed=False)],
                       claimed_level=6)
    assert ev.records_counted == 0
    assert ev.effective_level == 0
    assert any("self-report" in d["reason"] for d in ev.downgrades)


def test_canonical_json_float_rule_refuses_floats_in_rows():
    """W17: the float rule on the Python side too - integers or strings only."""
    import pytest

    from truerise.ledger import Ledger

    ledger = Ledger()
    ledger.append("clean.row", {"count": 3, "name": "ok"})
    with pytest.raises(ValueError, match="float forbidden"):
        ledger.append("floaty.row", {"ratio": 0.5})
    # Integers (even zero) stay legal:
    ledger.append("zero.row", {"count": 0})
