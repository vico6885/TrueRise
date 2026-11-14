#!/usr/bin/env python3
"""Wire-schema alignment gate (fail-closed).

M0 state: ``schemas/`` is intentionally empty until the wire crate lands in
M1.
- If ``schemas/EMPTY_UNTIL_M1`` exists: emit an explicit SKIP line (which
  MUST be recorded in the docs/PROGRESS.md not-doing list) and exit 0. A
  marker coexisting with any ``*.json`` schema file is an error.
- No marker AND no schemas: exit 1 (an empty gate is never silently green).

M1+: ``INDEX.json`` lists every schema; each entry must exist and every
check compares fields / required / enum values at spec level - never file
references.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCHEMAS = REPO / "schemas"
MARKER = SCHEMAS / "EMPTY_UNTIL_M1"


def main() -> int:
    schema_files = sorted(p for p in SCHEMAS.glob("*.json"))
    if MARKER.exists():
        if schema_files:
            print("FAIL: EMPTY_UNTIL_M1 marker coexists with schema files", file=sys.stderr)
            return 1
        print(
            "SKIP: schemas/ intentionally empty until M1 (marker present) - "
            "record in PROGRESS not-doing list"
        )
        return 0
    index = SCHEMAS / "INDEX.json"
    if not index.exists():
        print("FAIL: no INDEX.json and no EMPTY_UNTIL_M1 marker - fail-closed red", file=sys.stderr)
        return 1
    entries = json.loads(index.read_text(encoding="utf-8"))
    problems: list[str] = []
    for name, spec in entries.items():
        path = SCHEMAS / f"{name}.json"
        if not path.exists():
            problems.append(f"missing schema file: {name}.json")
            continue
        doc = json.loads(path.read_text(encoding="utf-8"))
        for field in spec.get("required", []):
            if field not in doc.get("required", []):
                problems.append(f"{name}: required field not asserted: {field}")
        for enum_name, values in spec.get("enums", {}).items():
            if doc.get("enums", {}).get(enum_name) != values:
                problems.append(f"{name}: enum mismatch: {enum_name}")
    if problems:
        print("schema gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"schema gate: {len(entries)} schema(s) aligned at spec level (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
