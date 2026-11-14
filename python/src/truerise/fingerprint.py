"""Four-part claim fingerprint (spec §1.5 / §6 ratchet row).

model x harness x environment x evaluator - ANY part changing yields a new
fingerprint, so cross-environment comparison is impossible by construction.
The digest is SHA-256 over the canonical ``key=value`` joining of the four
parts (fixed order, no whitespace) - stable across runs on one machine.
"""
from __future__ import annotations

import hashlib
from dataclasses import dataclass


@dataclass(frozen=True)
class Fingerprint:
    """Immutable four-part fingerprint of a claim's evaluation context."""

    model: str
    harness: str
    environment: str
    evaluator: str

    def canonical(self) -> str:
        """Canonical serialization: fixed-order ``k=v`` pairs joined by ``|``."""
        return "|".join(
            f"{k}={v}"
            for k, v in (
                ("model", self.model),
                ("harness", self.harness),
                ("environment", self.environment),
                ("evaluator", self.evaluator),
            )
        )

    def digest(self) -> str:
        """SHA-256 hex digest of the canonical form (full 64 chars; truncation
        is a presentation concern and lives in the ledger row, not here)."""
        return hashlib.sha256(self.canonical().encode("utf-8")).hexdigest()

    def short(self) -> str:
        """12-hex presentation truncation (matches the ledger's display width)."""
        return self.digest()[:12]
