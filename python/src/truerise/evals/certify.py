"""Claim Certificate for a bench run (the evals-side verdict + its bookkeeping).

Contract (mirrors the 0.1-series demo, extended to pack scale):

    CLAIM      : detection accuracy + task count + trial count, from inputs
    FINGERPRINT: solver x harness x pack-digest x evaluator (four parts)
    VERDICT    : PASS | FAIL with per-violation reasons
    CERTIFICATE: verdict.json, offline replayable from the ledger

Verdict inputs (each is one catch):

    prereg     thresholds must match the seal computed BEFORE the run
    multiplicity  the corrected significance bar = base / counted trials
    quality    accuracy must clear the sealed min_accuracy
    evidence   runner-executed records are signed; a run that produced no
               execution record self-reports and caps at L0

Thresholds are sealed by the caller BEFORE the run (seal_thresholds), and the
certificate records BOTH the seal and the current map, so a mismatch names
its offending keys (pay-later caught).
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path

from truerise.fingerprint import Fingerprint
from truerise.gates import EvidenceRecord, evidence_gate, prereg_diff, seal_thresholds
from truerise.ledger import Ledger
from truerise.multiplicity import MultiplicityAccountant

VERDICT_KIND = "evals.verdict.v1"


@dataclass
class BenchCert:
    """Certificate payload: everything needed to recompute the verdict offline."""

    claim: str
    fingerprint: dict
    verdict: str  # PASS | FAIL
    reasons: list[str]
    pack_digest: str
    answer_seal: str
    n_tasks: int
    trials: int
    accuracy: float = 0.0
    corrected_significance: float = 0.0
    evidence_effective_level: int = 0
    per_family: dict = field(default_factory=dict)
    failures: list = field(default_factory=list)
    prereg_seal: str = ""
    certificate_path: str = ""
    replay: str = ""

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, ensure_ascii=False)


def run_bench(*, solver, pack, work_dir: Path, out_dir: Path,
              trials: int, strategy: str, thresholds: dict,
              solver_name: str, executed_by_runner: bool = True,
              claim_thresholds: dict | None = None) -> tuple[BenchCert, dict]:
    """One full bench run: trials -> grading -> scored -> certificate.

    Returns (certificate, score_summary). ``thresholds`` is the SEALED map
    (fixed before the run); ``claim_thresholds`` is the map the claim NOW
    uses -- honest runs pass None (claim == seal), a pay-later mutator passes
    a mutated map and the prereg diff names its offending keys.
    """
    from .runner import aggregate, run_trials, score  # local import: keeps runner import-light

    out_dir.mkdir(parents=True, exist_ok=True)
    ledger_path = out_dir / "claims.jsonl"
    ledger = Ledger.load(ledger_path) if ledger_path.exists() else Ledger(path=ledger_path)

    fp = Fingerprint(model=f"solver:{solver_name}", harness="evals-runner",
                     environment=f"pack:{pack.pack_digest[:12]}",
                     evaluator="truerise-evals/0.1")
    current = thresholds if claim_thresholds is None else claim_thresholds
    params = json.dumps({"trials": trials, "strategy": strategy,
                         "thresholds": thresholds, "claim": current},
                        sort_keys=True, separators=(",", ":"))
    fp_key = hashlib.sha256((fp.digest() + params).encode("utf-8")).hexdigest()

    accountant = MultiplicityAccountant(base_significance=0.05)
    counted = accountant.count(fp_key, trials)
    corrected = accountant.corrected_significance(fp_key)

    sealed = seal_thresholds(thresholds)
    diff = prereg_diff(sealed, current)  # the CLAIM map is diffed against the seal

    passes = run_trials(solver, pack, work_dir, trials)
    graded = aggregate(passes, strategy)
    summary = score(graded, pack)

    # evidence: the runner executed the solver (signed); a self-reported run
    # would carry no executor signature and contribute nothing (L0).
    records = [EvidenceRecord(source="evals-runner", declared_level=4,
                              executor_signed=executed_by_runner,
                              declared_source_class="declared")]
    ev = evidence_gate(records, claimed_level=4)

    claim_significance = current.get("claim_p", 0.05)
    reasons: list[str] = []
    if diff["offending"]:
        reasons.append(f"1) prereg diff: thresholds mutated after seal (keys: {', '.join(diff['offending'])})")
    if claim_significance > corrected:
        reasons.append(
            f"2) trials={counted} -> corrected significance {corrected:.1e}, "
            f"claim p={claim_significance} does not clear the corrected bar"
        )
    min_accuracy = thresholds.get("min_accuracy", 0.0)  # quality gate uses the SEALED bar
    if summary["accuracy"] < min_accuracy:
        reasons.append(
            f"3) accuracy {summary['accuracy']:.0%} below sealed min_accuracy {min_accuracy:.0%}"
        )
    if ev.records_counted == 0:
        reasons.append("4) evidence: 0 executor-signed records (self-report only)")
    verdict = "FAIL" if reasons else "PASS"

    claim = (f"defect detection accuracy {summary['accuracy']:.0%} on {summary['total']} tasks "
             f"(trials={counted})")
    cert = BenchCert(
        claim=claim,
        fingerprint={"model": fp.model, "harness": fp.harness,
                     "environment": fp.environment, "evaluator": fp.evaluator,
                     "digest": fp.short()},
        verdict=verdict,
        reasons=reasons,
        pack_digest=pack.pack_digest,
        answer_seal=pack.answer_seal,
        n_tasks=summary["total"],
        trials=counted,
        accuracy=summary["accuracy"],
        corrected_significance=corrected,
        evidence_effective_level=ev.effective_level,
        per_family=summary["per_family"],
        failures=summary["failures"],
        prereg_seal=sealed[:12],
    )
    ledger.append(
        kind=VERDICT_KIND,
        detail={"key": fp_key, "claim": claim, "verdict": verdict, "reasons": reasons,
                "trials": counted,
                "accuracy_per_mille": round(summary["accuracy"] * 1000),  # int (float rule)
                "pack_digest": pack.pack_digest, "answer_seal": pack.answer_seal},
        severity="warning" if verdict == "FAIL" else "info",
        fingerprint_key=fp_key,
    )
    cert.certificate_path = str((out_dir / "verdict.json").resolve())
    (out_dir / "verdict.json").write_text(cert.to_json(), encoding="utf-8")
    return cert, summary


def replay(out_dir: Path) -> BenchCert:
    """Offline recomputation: re-derive the verdict from the ledger (chain
    re-verified, fail-closed); it must equal the stored certificate verdict."""
    cert_path = out_dir / "verdict.json"
    if not cert_path.exists():
        raise FileNotFoundError(f"no certificate at {cert_path}; run the bench first")
    stored = json.loads(cert_path.read_text(encoding="utf-8"))
    ledger = Ledger.load(out_dir / "claims.jsonl")
    governing = [r for r in ledger.rows if r.governs() and r.kind == VERDICT_KIND]
    if not governing:
        raise ValueError("certificate has no governing ledger row; cannot replay")
    recomputed = governing[-1].detail["verdict"]
    if recomputed != stored["verdict"]:
        stored["verdict"] = f"MISMATCH (stored={stored['verdict']}, recomputed={recomputed})"
    else:
        stored["replay"] = f"recomputed from ledger ({len(ledger.rows)} rows, chain verified): identical verdict"
    return BenchCert(**stored)
