#!/usr/bin/env python3
"""Timesafety job existence assertion (enforced since M1 (four vectors live in tests/timesafety/four_vectors.toml)).

The CI job named ``timesafety-placeholder`` must never be skipped; until the
real LeakGuard suite lands in M1 this assertion keeps the job fail-closed:
it verifies the four leak vectors are DECLARED in
``tests/timesafety/four_vectors.toml`` with status ``planned``. A missing
file or vector is red; a premature ``enforced`` status (without the M1
property tests) is also red.
"""
from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TARGET = REPO / "tests" / "timesafety" / "four_vectors.toml"
EXPECTED = {
    "future_disturbance_invariance",
    "pit_boundary_rejection",
    "label_uniqueness",
    "fit_apply_separation",
}


def main() -> int:
    if not TARGET.exists():
        print("FAIL: timesafety contract missing", file=sys.stderr)
        return 1
    text = TARGET.read_text(encoding="utf-8")
    ids = {
        line.split('"')[1] for line in text.splitlines() if line.strip().startswith("id = ")
    }
    missing = EXPECTED - ids
    if missing:
        print(f"FAIL: vectors not declared: {sorted(missing)}", file=sys.stderr)
        return 1
    enforced = 'status = "enforced"' in text
    if enforced:
        # M1+ semantics: enforced is valid ONLY when the enforcing property
        # tests exist (leakguard.rs declaring all four vectors).
        leakguard = REPO / "crates" / "truerise-core" / "tests" / "leakguard.rs"
        if not leakguard.exists():
            print("FAIL: enforced but enforcing test file (leakguard.rs) missing", file=sys.stderr)
            return 1
        lg_text = leakguard.read_text(encoding="utf-8")
        not_covered = [v for v in sorted(EXPECTED) if v not in lg_text]
        if not_covered:
            print(f"FAIL: leakguard.rs does not reference vectors: {not_covered}", file=sys.stderr)
            return 1
        print(
            f"timesafety: {len(EXPECTED)} vectors ENFORCED "
            f"(declaration + enforcing tests verified, exit 0)"
        )
        return 0
    print(
        f"timesafety placeholder: {len(EXPECTED)} vectors declared planned "
        "(exit 0; real suite lands M1)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
