"""The 60-second demo: three cheating scenarios, each independently catchable.

Spec §1.5 contract:
    CLAIM     : "self-improvement +23% (n=1000 trials, best-of)"
    FINGERPRINT: model=m* harness=h* env=e* evaluator=v*
    VERDICT   : FAIL with per-violation reasons
    CERTIFICATE: output/truerise/verdict.json, offline re-computable (--replay)

Acceptance = perturbation-flipping (nothing hardcoded): every line of the
verdict is derived from inputs, so each scenario has an honest twin that
flips ITS line off:
    --honest-seal     S1 reason disappears (thresholds match the seal)
    --perturb claim_p=0.001   S2 reason disappears (claim clears the corrected bar)
    --sign-records    S3 reason disappears (records become executor-signed)
    --perturb n_trials=10     the corrected denominator visibly moves (5e-5 -> 5e-3)
    --drop-fingerprint        fingerprint integrity line appears (ledger refuses)
    --honest                  all three flipped -> PASS
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path

from .fingerprint import Fingerprint
from .gates import EvidenceRecord, evidence_gate, prereg_diff, seal_thresholds
from .ledger import Ledger
from .multiplicity import MultiplicityAccountant

CLAIM_TEXT = "self-improvement +23% (n={trials} trials, best-of)"
DEFAULT_TRIALS = 1000
DEFAULT_CLAIM_P = 0.03
BASE_SIGNIFICANCE = 0.05
SEALED_THRESHOLDS = {"min_gain_pct": 10.0, "significance": BASE_SIGNIFICANCE}


@dataclass
class Verdict:
    """Certificate payload: everything needed to recompute the verdict offline."""

    claim: str
    fingerprint: dict
    verdict: str  # PASS | FAIL
    reasons: list[str]
    certificate_path: str = ""
    prereg_seal: str = ""
    trials: int = 0
    corrected_significance: float = 0.0
    evidence_effective_level: int = 0
    evidence_signed_records: int = 0
    replay: str = ""

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, ensure_ascii=False)


def run_demo(out_dir: Path, *, n_trials: int = DEFAULT_TRIALS,
             claim_p: float = DEFAULT_CLAIM_P, honest_seal: bool = False,
             sign_records: bool = False, drop_fingerprint: bool = False) -> Verdict:
    """Run the three scenarios; every verdict line is derived from the inputs."""
    out_dir.mkdir(parents=True, exist_ok=True)
    ledger_path = out_dir / "claims.jsonl"
    # Persisted ledger first: idempotent replay + seq continuity across runs.
    ledger = Ledger.load(ledger_path) if ledger_path.exists() else Ledger(path=ledger_path)
    accountant = MultiplicityAccountant(base_significance=BASE_SIGNIFICANCE)
    fp = Fingerprint(model="m*", harness="h*", environment="e*", evaluator="v*")
    # The ledger key binds the fingerprint AND the evaluation parameters:
    # any parameter change is a different evaluation (spec: ANY change -> new key).
    params = json.dumps({"n_trials": n_trials, "claim_p": claim_p,
                         "honest_seal": honest_seal, "sign_records": sign_records},
                        sort_keys=True, separators=(",", ":"))
    fp_key = hashlib.sha256((fp.digest() + params).encode("utf-8")).hexdigest()
    if drop_fingerprint:
        fp_key = "dropped:" + fp_key  # cross-environment comparison refused

    # --- S1 pay-later: seal first, then check the thresholds against the seal ---
    sealed = seal_thresholds(SEALED_THRESHOLDS)
    current = dict(SEALED_THRESHOLDS) if honest_seal else {"min_gain_pct": 10.0, "significance": claim_p}
    diff = prereg_diff(sealed, current)

    # --- S2 many-shots: count ALL trials, correct the bar, print it ---
    counted = accountant.count(fp_key, n_trials)
    corrected = accountant.corrected_significance(fp_key)
    s2_pass = accountant.passes(fp_key, claim_p)

    # --- S3 self-signed: unsigned records -> auto-downgrade ---
    records = [
        EvidenceRecord(source="harness-stdout", declared_level=4,
                       executor_signed=sign_records, declared_source_class="declared" if sign_records else "undeclared"),
        EvidenceRecord(source="model-self-report", declared_level=4,
                       executor_signed=False, declared_source_class="undeclared"),
    ]
    ev = evidence_gate(records, claimed_level=4)

    reasons: list[str] = []
    if diff["offending"]:
        reasons.append(f"1) prereg diff: thresholds mutated after seal (keys: {', '.join(diff['offending'])})")
    if not s2_pass:
        reasons.append(
            f"2) trials={counted} -> corrected significance {corrected:.1e}, "
            f"claim p={claim_p} does not clear the corrected bar"
        )
    if drop_fingerprint:
        reasons.append("3) fingerprint: ledger key marked dropped - cross-environment comparison refused")
    if ev.records_counted == 0:
        reasons.append(f"4) evidence: 0 executor-signed records (self-report only) -> effective L{ev.effective_level}")
    verdict = "FAIL" if reasons else "PASS"

    claim_text = CLAIM_TEXT.format(trials=counted)
    ledger.append(
        kind="claim.verdict.v1",
        detail={"key": fp_key, "claim": claim_text, "verdict": verdict,
                "reasons": reasons, "trials": counted, "claim_p": claim_p},
        severity="warning" if verdict == "FAIL" else "info",
        fingerprint_key=fp_key,
    )
    cert = Verdict(
        claim=claim_text,
        fingerprint={"model": fp.model, "harness": fp.harness,
                     "environment": fp.environment, "evaluator": fp.evaluator,
                     "digest": "dropped" if drop_fingerprint else fp.short()},
        verdict=verdict,
        reasons=reasons,
        prereg_seal=sealed[:12],
        trials=counted,
        corrected_significance=corrected,
        evidence_effective_level=ev.effective_level,
        evidence_signed_records=ev.records_counted,
        certificate_path=str((out_dir / "verdict.json").resolve()),
    )
    (out_dir / "verdict.json").write_text(cert.to_json(), encoding="utf-8")
    return cert


def replay(out_dir: Path) -> Verdict:
    """Offline recomputation: re-derive the verdict from the ledger (chain
    re-verified, fail-closed); it must equal the stored certificate verdict."""
    cert_path = out_dir / "verdict.json"
    if not cert_path.exists():
        raise FileNotFoundError(f"no certificate at {cert_path}; run the demo first")
    stored = json.loads(cert_path.read_text(encoding="utf-8"))
    ledger = Ledger.load(out_dir / "claims.jsonl")
    governing = [r for r in ledger.rows if r.governs() and r.kind == "claim.verdict.v1"]
    if not governing:
        raise ValueError("certificate has no governing ledger row; cannot replay")
    recomputed = governing[-1].detail["verdict"]
    if recomputed != stored["verdict"]:
        stored["verdict"] = f"MISMATCH (stored={stored['verdict']}, recomputed={recomputed})"
    else:
        stored["replay"] = f"recomputed from ledger ({len(ledger.rows)} rows, chain verified): identical verdict"
    return Verdict(**stored)
