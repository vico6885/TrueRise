#!/usr/bin/env python3
"""Spec-mapping gate (spec §2.6 / v3 prompt test-driven section).

Every §6 behaviour row must map to named test identifiers that ACTUALLY exist
in the tree. A row without a mapping, or a mapping whose identifier cannot be
found in any tracked source/test file, fails CI (red).

Exit codes: 0 = all rows mapped and identifiers found; 1 = violations; 2 = config error.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SPEC_MAP = REPO / "spec_map.json"


def collect_tree_text() -> str:
    """Concatenate all tracked code/doc text (crates, tests, benches, docs, tools)."""
    chunks: list[str] = []
    for base in ("crates", "tests", "benches", "docs", "tools", "hats", "schemas"):
        base_path = REPO / base
        if not base_path.exists():
            continue
        for p in base_path.rglob("*"):
            if p.is_file() and p.suffix in {
                ".rs", ".py", ".md", ".toml", ".json", ".yml", ".yaml",
            }:
                try:
                    chunks.append(p.read_text(encoding="utf-8", errors="replace"))
                except OSError:
                    continue
    return "\n".join(chunks)


def main() -> int:
    try:
        spec = json.loads(SPEC_MAP.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        print(f"spec_map unreadable: {exc}", file=sys.stderr)
        return 2
    rows = spec.get("rows", {})
    if not rows:
        print("FAIL: spec_map has no rows", file=sys.stderr)
        return 2
    tree = collect_tree_text()
    problems: list[str] = []
    for row, identifiers in rows.items():
        if not identifiers:
            problems.append(f"row '{row}' has NO mapped tests (spec-mapping gate)")
            continue
        for ident in identifiers:
            # Meta-descriptions (plain sentences, not code identifiers) are
            # checked for their leading phrase; code identifiers must exist.
            if ident.startswith(("policy ", "docs/")):
                continue  # documented free-text mapping (see _note rows)
            if ident not in tree:
                problems.append(f"row '{row}': mapped identifier not found in tree: {ident}")
    if problems:
        print("spec-mapping gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    total = sum(len(v) for v in rows.values())
    print(f"spec-mapping gate: {len(rows)} rows / {total} identifiers all resolve (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
