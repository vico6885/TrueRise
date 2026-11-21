#!/usr/bin/env python3
"""License-isolation machine assertion (runs in CI right after cargo deny).

Asserts on ``deny.toml``:
1. the legacy Python-edition license BSL-1.1 is NOT in the allow list;
2. no GPL/AGPL identifier is in the allow list (copyleft isolation);
3. the core dual licenses (Apache-2.0, MIT) ARE present;
4. advisory ignores are limited to the explicitly approved id set and each
   carries a written rationale (supply-chain exceptions must not grow silently).

A comment alone is not an assertion - this script is the machine check.
Exit 0 = isolation holds; 1 = violation; 2 = deny.toml unreadable.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DENY = REPO / "deny.toml"

REQUIRED = {"Apache-2.0", "MIT"}
FORBIDDEN_SUBSTRINGS = ("BSL", "GPL")
# Only this one maintenance-status advisory may be silenced. Every id here
# needs a recorded rationale in deny.toml and a review trigger to drop it.
APPROVED_ADVISORY_IGNORES = {"RUSTSEC-2026-0543"}


def main() -> int:
    try:
        text = DENY.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"deny.toml unreadable: {exc}", file=sys.stderr)
        return 2
    allow: list[str] = []
    ignores: list[dict[str, str]] = []
    try:
        import tomllib

        data = tomllib.loads(text)
        allow = list(data["licenses"]["allow"])
        ignores = [
            {"id": str(entry.get("id", "")), "reason": str(entry.get("reason", ""))}
            for entry in data.get("advisories", {}).get("ignore", [])
        ]
    except ModuleNotFoundError:
        block = re.search(r"allow\s*=\s*\[(.*?)\]", text, re.DOTALL)
        if block:
            allow = re.findall(r'"([^"]+)"', block.group(1))
        iblock = re.search(r"^ignore\s*=\s*\[(.*?)\]", text, re.DOTALL | re.MULTILINE)
        if iblock:
            for raw in re.findall(r"\{[^}]*\}", iblock.group(1)):
                ident = re.search(r'id\s*=\s*"([^"]+)"', raw)
                reason = re.search(r'reason\s*=\s*"([^"]+)"', raw)
                ignores.append({
                    "id": ident.group(1) if ident else "",
                    "reason": reason.group(1) if reason else "",
                })
    except (KeyError, ValueError) as exc:
        print(f"deny.toml unparsable: {exc}", file=sys.stderr)
        return 2
    problems: list[str] = []
    for lic in allow:
        if any(f in lic.upper() for f in FORBIDDEN_SUBSTRINGS):
            problems.append(f"forbidden license in allow list: {lic}")
    for lic in sorted(REQUIRED):
        if lic not in allow:
            problems.append(f"required license missing from allow list: {lic}")
    seen = {entry["id"] for entry in ignores}
    unapproved = seen - APPROVED_ADVISORY_IGNORES
    if unapproved:
        problems.append(f"unapproved advisory ignore(s): {sorted(unapproved)}")
    for entry in ignores:
        if not entry["reason"].strip():
            problems.append(f"advisory ignore has no rationale: {entry['id'] or '<missing id>'}")
    if problems:
        print("license isolation violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(
        "license isolation: allow list clean, dual core licenses present, "
        "advisory ignores bounded (exit 0)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
