#!/usr/bin/env python3
"""SBOM existence + format assertion (CI, after ``cargo cyclonedx --all``).

Format assertions (spec M0 gate): at least one CycloneDX report exists, it
parses, it declares a specVersion, and its component count is at least the
number of workspace member crates. The stricter release-wave assertions
(specVersion == 1.5, component count >= full declared dependency closure)
land with M5.

Exit 0 = SBOM present and format-valid; 1 = missing/invalid.
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
    reports = sorted((REPO / "target").rglob("*.cdx.json"))
    if not reports:
        print(
            "FAIL: no CycloneDX SBOM found under target/ (run: cargo cyclonedx --all)",
            file=sys.stderr,
        )
        return 1
    members = workspace_member_count()
    problems: list[str] = []
    for report in reports:
        try:
            doc = json.loads(report.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"unparsable: {report.name}: {exc}")
            continue
        spec = doc.get("specVersion")
        if not spec:
            problems.append(f"no specVersion: {report.name}")
        components = doc.get("components", [])
        if len(components) < members:
            problems.append(
                f"{report.name}: components {len(components)} < workspace members {members}"
            )
    if problems:
        for p in problems:
            print(f"  {p}")
        return 1
    print(
        f"SBOM gate: {len(reports)} report(s) present, specVersion declared, "
        f"components >= {members} workspace members (exit 0)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
