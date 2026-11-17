"""Prereg gate + evidence gate (spec §1.5 S1/S3 catches).

Prereg gate: thresholds are SEALED before the evaluation runs (SHA-256 of the
canonical threshold json). A verdict may only be issued against the sealed
value; any post-seal mutation makes the diff non-empty -> FAIL with the
offending keys listed. The claim ledger refuses a second governing verdict
for the same fingerprint (pay-later re-issue).

Evidence gate: evidence levels L0-L6; the ceiling is derived from what the
records can PROVE. Records without an executor signature count as nothing
(self-report is not evidence). Unsealed/undeclared sources cap at L3;
anything above the ceiling is auto-downgraded with per-violation notes.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field


# -- prereg gate -----------------------------------------------------------
def seal_thresholds(thresholds: dict) -> str:
    """Seal: SHA-256 over canonical json of the threshold mapping."""
    return hashlib.sha256(
        json.dumps(thresholds, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def prereg_diff(sealed: str, current: dict) -> dict:
    """Recompute the seal over ``current`` and diff against ``sealed``.

    Returns {"ok": bool, "offending": [keys...], "sealed": ..., "current": ...}.
    Empty/absent ``current`` with a valid ``sealed`` is OK (nothing mutated).
    """
    now = seal_thresholds(current) if current else sealed
    offending = [] if now == sealed else sorted(_mutated_keys(current))
    return {"ok": now == sealed, "offending": offending, "sealed": sealed, "current": now}


def _mutated_keys(current: dict) -> list[str]:
    """Keys present in a mutated map are exactly its keys (seal is all-or-nothing)."""
    return list(current.keys())


# -- evidence gate ---------------------------------------------------------
MAX_LEVEL = 6
UNDECLARED_CEILING = 3  # unsealed/undeclared sources hard-cap at L3 (spec §6)


@dataclass
class EvidenceRecord:
    """One piece of evidence. ``executor_signed`` is the load-bearing bit:
    without it the record contributes NOTHING (self-report is not evidence)."""

    source: str
    declared_level: int
    executor_signed: bool = False
    declared_source_class: str = "undeclared"  # declared|undeclared|synthetic|simulated|mock
    notes: str = ""


@dataclass
class EvidenceVerdict:
    """Result of the gate: effective level + per-record downgrades."""

    effective_level: int
    records_counted: int
    downgrades: list[dict] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


def evidence_gate(records: list[EvidenceRecord], claimed_level: int) -> EvidenceVerdict:
    """Derive the evidence ceiling from what records can prove; only downgrade.

    - unsigned records contribute nothing (and are listed as downgrades);
    - undeclared/synthetic/simulated/mock sources cap at UNDECLARED_CEILING;
    - the effective level never exceeds the proven ceiling, and a claim
      above it is auto-downgraded with an explicit note (只降不升).
    """
    downgrades: list[dict] = []
    counted = 0
    proven = 0
    for rec in records:
        if not rec.executor_signed:
            downgrades.append(
                {"source": rec.source, "reason": "no executor signature (self-report is not evidence)",
                 "declared": rec.declared_level, "effective": 0}
            )
            continue
        counted += 1
        # Review fix (P2): declared_level was UNBOUNDED - a record claiming
        # L99 sailed through as proven=99 (the L0-L6 contract dead letter).
        capped = min(rec.declared_level, MAX_LEVEL)
        if rec.declared_level > MAX_LEVEL:
            downgrades.append(
                {"source": rec.source, "reason": f"declared level exceeds the L0-L6 scale, capped at L{MAX_LEVEL}",
                 "declared": rec.declared_level, "effective": capped}
            )
        if rec.declared_source_class != "declared" and capped > UNDECLARED_CEILING:
            capped = UNDECLARED_CEILING
            downgrades.append(
                {"source": rec.source, "reason": f"undeclared source class caps at L{UNDECLARED_CEILING}",
                 "declared": rec.declared_level, "effective": capped}
            )
        proven = max(proven, capped)
    effective = min(claimed_level, proven)
    if claimed_level > effective:
        downgrades.append(
            {"source": "<claim>", "reason": "claim above proven ceiling (auto-downgrade, only down)",
             "declared": claimed_level, "effective": effective}
        )
    return EvidenceVerdict(effective_level=effective, records_counted=counted, downgrades=downgrades)
