#!/usr/bin/env python3
"""SBOM existence + format assertion (CI, after ``cargo cyclonedx --all``).

M0 scope (declared super-spec addition, recorded in docs/PROGRESS.md):
assert at least one CycloneDX report exists and parses with a specVersion.
The full M5 assertions (specVersion == 1.5, component count >= declared
dependency count) deliberately land with the release wave.
Exit 0 = SBOM present; 1 = missing/invalid.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def main() -> int:
    reports = sorted((REPO / "target").rglob("*.cdx.json"))
    if not reports:
        print(
            "FAIL: no CycloneDX SBOM found under target/ (run: cargo cyclonedx --all)",
            file=sys.stderr,
        )
        return 1
    problems: list[str] = []
    for report in reports:
        try:
            doc = json.loads(report.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"unparsable: {report.name}: {exc}")
            continue
        if "specVersion" not in doc:
            problems.append(f"no specVersion: {report.name}")
    if problems:
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"SBOM gate: {len(reports)} report(s) present with specVersion (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
