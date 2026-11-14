#!/usr/bin/env python3
"""Wire-schema alignment gate (fail-closed, spec-level assertions).

Every entry in ``schemas/INDEX.json`` is a spec-level expectation set. This
gate compares the schema FILE CONTENT against the expectations - field sets,
required keys, enum values, additionalProperties - never mere file existence.

Exit codes: 0 = all entries aligned; 1 = any mismatch/missing; 2 = config
error (a missing INDEX.json is red: an empty gate is never silently green).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCHEMAS = REPO / "schemas"


def fail(msg: str) -> int:
    print(f"schema gate FAIL: {msg}", file=sys.stderr)
    return 1


def main() -> int:
    index_path = SCHEMAS / "INDEX.json"
    if not index_path.exists():
        return fail("no INDEX.json - fail-closed red")
    try:
        entries = json.loads(index_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        return fail(f"INDEX.json unparsable: {exc}")
    problems: list[str] = []
    checked = 0
    for name, spec in entries.items():
        if name.startswith("_") or not isinstance(spec, dict):
            continue  # metadata keys are not schema entries
        path = SCHEMAS / f"{name}.json"
        if not path.exists():
            problems.append(f"{name}: schema file missing")
            continue
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except ValueError as exc:
            problems.append(f"{name}: schema unparsable: {exc}")
            continue
        checked += 1
        props = set(doc.get("properties", {}))
        want_fields = set(spec.get("fields", []))
        if props != want_fields:
            problems.append(
                f"{name}: field set mismatch: missing={sorted(want_fields - props)} "
                f"extra={sorted(props - want_fields)}"
            )
        want_required = set(spec.get("required", []))
        got_required = set(doc.get("required", []))
        if got_required != want_required:
            problems.append(
                f"{name}: required mismatch: got={sorted(got_required)} "
                f"want={sorted(want_required)}"
            )
        want_enums = spec.get("enums", {})
        got_enums = {
            k: v.get("enum") for k, v in doc.get("properties", {}).items() if "enum" in v
        }
        for enum_name, values in want_enums.items():
            if got_enums.get(enum_name) != values:
                problems.append(
                    f"{name}: enum mismatch on {enum_name}: "
                    f"got={got_enums.get(enum_name)} want={values}"
                )
        want_ap = spec.get("additionalProperties")
        if want_ap is not None and doc.get("additionalProperties") != want_ap:
            problems.append(
                f"{name}: additionalProperties mismatch: got={doc.get('additionalProperties')} "
                f"want={want_ap}"
            )
    if problems:
        print("schema gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"schema gate: {checked} schema(s) aligned at spec level (fields/required/enums) (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
