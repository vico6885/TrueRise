#!/usr/bin/env python3
"""Single-file length gate: 600 lines max for code files.

Scope: ``crates/**/*.rs``, ``tests/**/*.rs``, ``benches/**/*.rs``,
``examples/**/*.rs``, ``tools/*.py``. Data/doc files (TOML/YAML/MD/JSON/hex)
are out of scope. Rationale: forced decomposition of long implementations.
Exit 0 = all within limit; 1 = violations listed.
"""
from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LIMIT = 600


def main() -> int:
    targets: set[Path] = set()
    for pattern in (
        "crates/**/*.rs",
        "tests/**/*.rs",
        "benches/**/*.rs",
        "examples/**/*.rs",
        "tools/*.py",
    ):
        targets.update(REPO.glob(pattern))
    bad: list[str] = []
    for path in sorted(targets):
        if ".git" in path.parts or "target" in path.parts:
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        if len(lines) > LIMIT:
            bad.append(f"{path.relative_to(REPO).as_posix()}: {len(lines)} lines")
    if bad:
        print("file-length gate violations:")
        for b in bad:
            print(f"  {b}")
        return 1
    print(f"file-length gate: all {len(targets)} code files <= {LIMIT} lines (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
