#!/usr/bin/env python3
"""License-isolation machine assertion (runs in CI right after cargo deny).

Asserts on ``deny.toml``:
1. the legacy Python-edition license BSL-1.1 is NOT in the allow list;
2. no GPL/AGPL identifier is in the allow list (copyleft isolation);
3. the core dual licenses (Apache-2.0, MIT) ARE present.

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


def main() -> int:
    try:
        text = DENY.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"deny.toml unreadable: {exc}", file=sys.stderr)
        return 2
    allow: list[str] = []
    try:
        import tomllib

        data = tomllib.loads(text)
        allow = list(data["licenses"]["allow"])
    except ModuleNotFoundError:
        block = re.search(r"allow\s*=\s*\[(.*?)\]", text, re.DOTALL)
        if block:
            allow = re.findall(r'"([^"]+)"', block.group(1))
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
    if problems:
        print("license isolation violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"license isolation: allow list clean, dual core licenses present (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
