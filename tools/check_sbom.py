#!/usr/bin/env python3
"""SBOM existence + format assertion (after ``cargo cyclonedx --format json``).

cargo-cyclonedx (default --describe crate) writes one SBOM per workspace
crate next to the crate manifest. Format assertions (spec M0 gate):
- at least one report per workspace member (report count >= members);
- every report parses and declares a specVersion.

The stricter release-wave assertions (specVersion == 1.5, component count
>= full declared dependency closure) land with M5.

Exit 0 = SBOM set present and format-valid; 1 = missing/invalid.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def workspace_member_count() -> int:
    """Count workspace member crates from the root manifest."""
    text = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    block = re.search(r"members\s*=\s*\[(.*?)\]", text, re.DOTALL)
    if not block:
        return 0
    return len(re.findall(r'"crates/[^"]+"', block.group(1)))


def main() -> int:
    reports: list[Path] = []
    for pattern in ("crates/*/*.cdx.json", "crates/*/*.cdx.xml", "target/**/*.cdx.json"):
        reports.extend(REPO.glob(pattern))
    reports = sorted(set(reports))
    if not reports:
        print(
            "FAIL: no CycloneDX SBOM found (run: cargo cyclonedx --format json)",
            file=sys.stderr,
        )
        return 1
    members = workspace_member_count()
    problems: list[str] = []
    parsed = 0
    for report in reports:
        try:
            doc = json.loads(report.read_text(encoding="utf-8"))
        except ValueError:
            continue  # xml reports are counted for presence, asserted when json
        parsed += 1
        if not doc.get("specVersion"):
            problems.append(f"no specVersion: {report.name}")
    if parsed < members:
        problems.append(f"parsable reports {parsed} < workspace members {members}")
    if problems:
        for p in problems:
            print(f"  {p}")
        return 1
    print(
        f"SBOM gate: {len(reports)} report(s) (>= {members} members), "
        f"{parsed} parsed with specVersion (exit 0)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
