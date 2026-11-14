#!/usr/bin/env python3
"""SBOM existence + format assertion (after ``cargo cyclonedx --format json``).

cargo-cyclonedx (default ``--describe crate``) writes exactly one SBOM next
to each crate manifest. Strict assertions (spec M0 gate):
- EVERY workspace member directory contains exactly one ``*.cdx.json``;
- every report MUST parse (a parse failure is a listed problem, never
  silently skipped);
- every report declares a ``specVersion``.

Only ``crates/<member>/`` reports count - build-tree residue cannot pad the
count. The stricter release-wave assertions (specVersion == 1.5, component
count >= full declared dependency closure) land with M5.

Exit 0 = all member reports present, parsed, specVersion-declared;
1 = any problem; 2 = manifest unparsable.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def workspace_members() -> list[str]:
    """Return workspace member crate directory names from the root manifest."""
    text = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    block = re.search(r"members\s*=\s*\[(.*?)\]", text, re.DOTALL)
    if not block:
        return []
    return re.findall(r'"crates/([^"]+)"', block.group(1))


def main() -> int:
    members = workspace_members()
    if not members:
        print("FAIL: root manifest has no workspace members", file=sys.stderr)
        return 2
    problems: list[str] = []
    parsed = 0
    for member in members:
        crate_dir = REPO / "crates" / member
        reports = sorted(crate_dir.glob("*.cdx.json"))
        if len(reports) != 1:
            problems.append(f"{member}: expected exactly 1 SBOM report, found {len(reports)}")
            continue
        report = reports[0]
        try:
            doc = json.loads(report.read_text(encoding="utf-8"))
        except ValueError as exc:
            problems.append(f"{member}: SBOM unparsable: {exc}")
            continue
        if not doc.get("specVersion"):
            problems.append(f"{member}: SBOM has no specVersion")
            continue
        parsed += 1
    if problems:
        print("SBOM gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"SBOM gate: {parsed}/{len(members)} member reports parsed with specVersion (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
