"""Append-only claim ledger with hash chain (spec §6 audit-chain row).

Row: {seq, ts, kind, severity, detail, prev_hash, hash} where
``hash = sha256(canonical json of all fields except hash)``. Append-only:
an append verifies the whole chain first (seq continuity, prev-hash linkage,
content hash) and fails closed on any tampering, locating the row number.
Legacy (v1) rows are legal to READ but never participate in governance
decisions (``governs()`` returns False for them).

Idempotent replay: a row with the same canonical key (kind + fingerprint +
claim digest) is written once; replays skip.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path

GENESIS = "0" * 64
LEGACY_PREFIX = "legacy:"


@dataclass
class Row:
    """One ledger record."""

    seq: int
    ts: str
    kind: str
    severity: str
    detail: dict
    prev_hash: str
    hash: str = ""
    legacy: bool = False

    def canonical(self) -> str:
        """Canonical json - byte-compatible with the Rust HashChain.

        Review fix (P1, cross-impl): the old canonical carried ``legacy``
        and OMITTED ``hash``; the Rust chain hashes
        ``{"detail","hash":null,"kind","prev_hash","seq","severity","ts"}``
        (sorted, no legacy). Two implementations hashing different bytes
        for the same spec row is the exact cross-implementation bug class
        docs/determinism.md forbids. Aligned byte-for-byte now.
        """
        d = {
            "detail": self.detail,
            "hash": None,
            "kind": self.kind,
            "prev_hash": self.prev_hash,
            "seq": self.seq,
            "severity": self.severity,
            "ts": self.ts,
        }
        return json.dumps(d, sort_keys=True, separators=(",", ":"), ensure_ascii=False)

    def content_hash(self) -> str:
        return hashlib.sha256(self.canonical().encode("utf-8")).hexdigest()

    def governs(self) -> bool:
        """Legacy rows are readable but excluded from governance decisions."""
        return not self.kind.startswith(LEGACY_PREFIX)


@dataclass
class Ledger:
    """In-memory append-only ledger, optionally persisted as JSON lines."""

    path: Path | None = None
    rows: list[Row] = field(default_factory=list)

    # -- chain integrity ---------------------------------------------------
    def verify(self) -> list[str]:
        """Full recompute of the chain. Returns a list of problems; empty = intact.

        Three checks per row: seq continuity (0,1,2...), prev-hash linkage,
        content hash. On failure the offending row number is reported
        (fail-closed, located).
        """
        problems: list[str] = []
        prev = GENESIS
        for i, row in enumerate(self.rows):
            if row.seq != i:
                problems.append(f"row {i}: seq discontinuity (got {row.seq}, want {i})")
            if row.prev_hash != prev:
                problems.append(f"row {i}: prev_hash mismatch (chain broken here)")
            if row.hash != row.content_hash():
                problems.append(f"row {i}: content hash mismatch (tampered)")
            prev = row.hash
        return problems

    # -- append ------------------------------------------------------------
    def append(self, kind: str, detail: dict, severity: str = "info",
               fingerprint_key: str | None = None, ts: str = "") -> Row:
        # Review fix (P2): constructing a Ledger over an EXISTING file and
        # appending used to write a seq-0 genesis on top of the old rows -
        # every later load() then went red. Load-if-exists closes the
        # footgun (the same behavior demo/run_bench already had).
        if not self.rows and self.path is not None and self.path.exists() and self.path.stat().st_size > 0:
            existing = Ledger.load(self.path)
            self.rows = existing.rows
        """Append one row after full-chain verification.

        Idempotent replay: when ``fingerprint_key`` is given and a prior
        GOVERNING row carries the same key in ``detail["key"]``, the call is a
        no-op returning the existing row (first write wins, replays skip).
        """
        problems = self.verify()
        if problems:
            raise LedgerTampered(problems)
        # Float rule (docs/determinism.md): chain rows carry integers or
        # strings ONLY - `10.0` vs `10` divergence is a real cross-
        # implementation bug class; the write gate refuses floats.
        def _find_float(value, path="detail"):
            if isinstance(value, float):
                return path
            if isinstance(value, dict):
                for key, item in value.items():
                    found = _find_float(item, f"{path}.{key}")
                    if found:
                        return found
            if isinstance(value, list):
                for i, item in enumerate(value):
                    found = _find_float(item, f"{path}[{i}]")
                    if found:
                        return found
            return None

        float_path = _find_float(detail)
        if float_path is not None:
            raise ValueError(f"float forbidden in row {float_path} (integer or string only)")
        if fingerprint_key is not None:
            for row in self.rows:
                if row.governs() and row.detail.get("key") == fingerprint_key:
                    return row
        row = Row(
            seq=len(self.rows),
            ts=ts,
            kind=kind,
            severity=severity,
            detail=detail,
            prev_hash=self.rows[-1].hash if self.rows else GENESIS,
        )
        row.hash = row.content_hash()
        self.rows.append(row)
        self._flush()
        return row

    def _flush(self) -> None:
        if self.path is None:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with self.path.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(asdict(self.rows[-1]), ensure_ascii=False) + "\n")

    # -- load --------------------------------------------------------------
    @classmethod
    def load(cls, path: Path) -> "Ledger":
        """Load a persisted ledger (JSON lines) and verify the chain."""
        ledger = cls(path=path)
        if path.exists():
            for line in path.read_text(encoding="utf-8").splitlines():
                if not line.strip():
                    continue
                d = json.loads(line)
                ledger.rows.append(Row(**d))
        problems = ledger.verify()
        if problems:
            raise LedgerTampered(problems)
        return ledger


class LedgerTampered(RuntimeError):
    """Chain verification failed; carries the per-row problem list."""

    def __init__(self, problems: list[str]) -> None:
        super().__init__("; ".join(problems))
        self.problems = problems
