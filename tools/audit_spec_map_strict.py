#!/usr/bin/env python3
"""Strict spec_map audit (NOT a CI gate - a manual audit utility).

lint_spec_map.py accepts an identifier if the text appears anywhere in the
tracked tree; this audit is stricter: an identifier must be

1. a real Rust ``#[test]`` fn name, or
2. a real Python ``def test_*`` name, or
3. an anchor inside a gate script (behaviour name / enforcement fn) - the
   gate scripts' own self-tests cover those.

Exit 0 = every identifier anchored; exit 1 = weak anchors listed.
Run: python -I tools/audit_spec_map_strict.py
"""
import json
import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

def main() -> int:
    spec = json.load(open(REPO / "spec_map.json", encoding="utf-8"))
    rows = spec["rows"]
    rs_files = list((REPO / "crates").rglob("*.rs"))
    py_files = list((REPO / "python").rglob("*.py")) + list((REPO / "python-native").rglob("*.py"))
    tool_files = list((REPO / "tools").glob("*.py"))
    rust_test_fns = set()
    for f in rs_files:
        text = f.read_text(encoding="utf-8", errors="replace")
        pattern = re.compile(
            r"#\[test\]\s*(?:\n\s*#\[.*\]\s*\n\s*)*fn\s+([a-zA-Z0-9_]+)"
        )
        for m in pattern.finditer(text):
            rust_test_fns.add(m.group(1))
    py_test_fns = set()
    for f in py_files:
        text = f.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r"def\s+(test_[a-zA-Z0-9_]+)", text):
            py_test_fns.add(m.group(1))
    gate_anchors = set()
    for f in tool_files:
        t = f.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r"[a-z0-9_]{12,}", t):
            gate_anchors.add(m.group(0))
    total = weak = 0
    weak_list = []
    for row, idents in rows.items():
        for ident in idents:
            total += 1
            if ident in rust_test_fns or ident in py_test_fns or ident in gate_anchors:
                continue
            weak += 1
            weak_list.append((row, ident))
    print(f"strict spec_map audit: {total} identifiers -> "
          f"{total - weak} anchored (rust test / python test / gate anchor), {weak} weak")
    for row, ident in weak_list:
        print(f"  WEAK: [{row}] {ident}")
    return 1 if weak else 0

if __name__ == "__main__":
    raise SystemExit(main())
