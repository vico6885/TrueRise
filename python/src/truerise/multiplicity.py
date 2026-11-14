"""Multiplicity accountant (spec §1.5 S2 catch - simplified ledger, v0.1).

Every trial run is counted into ONE unique ledger (the budget ledger keyed by
fingerprint). The corrected denominator is printed explicitly: running N
trials and reporting the best one multiplies the false-positive chance, so
the significance bar tightens to ``base / N`` (Bonferroni-style correction).
The accountant never deletes counts - selecting after the fact cannot
un-count the losing trials.
"""
from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class MultiplicityAccountant:
    """Single unique trial counter (keyed by fingerprint)."""

    base_significance: float = 0.05
    _counts: dict[str, int] = field(default_factory=dict)

    def count(self, fingerprint_key: str, trials: int = 1) -> int:
        """Add ``trials`` to the fingerprint's count (monotonic; never reset)."""
        if trials < 0:
            raise ValueError("trial counts are non-negative")
        self._counts[fingerprint_key] = self._counts.get(fingerprint_key, 0) + trials
        return self._counts[fingerprint_key]

    def trials(self, fingerprint_key: str) -> int:
        return self._counts.get(fingerprint_key, 0)

    def corrected_significance(self, fingerprint_key: str) -> float:
        """Explicit corrected bar: base / N (N >= 1). This is the number the
        claim must beat AFTER accounting for how many tries it had."""
        n = max(1, self.trials(fingerprint_key))
        return self.base_significance / n

    def passes(self, fingerprint_key: str, claim_significance: float) -> bool:
        """A claim passes only if it beats the CORRECTED bar."""
        return claim_significance <= self.corrected_significance(fingerprint_key)
