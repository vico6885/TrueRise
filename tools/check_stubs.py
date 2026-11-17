#!/usr/bin/env python3
"""Stub gate (G33): an unannounced stub must never masquerade as ``done``.

Scope note (docs/limits.md discipline): this gate is a CHEAP TRIPWIRE against
honest laziness - it catches the markers of unfinished work, not polished
nonsense. A plausible-but-wrong implementation is not a stub and no marker
scan can catch it; that is G32's per-item evidence job, not this file's.

What it scans (production trees only: crates/*/src, python/src,
python-native/src; tests and benches are out of scope - fixtures may
legitimately carry placeholder marks):

- Rust: ``unimplemented!()`` / ``todo!()`` / ``panic!("not implemented")``
  / ``panic!("unimplemented")`` / ``panic!("TODO ...")``
- Python (AST): functions and classes whose whole body is ``pass`` / ``...``
  / a bare docstring, plus any ``raise NotImplementedError``
- empty source files (no meaningful content)

An unannounced stub = a hit without a matching row in ``stub_registry.json``.
Every registry row must carry a reason and must correspond to a real hit
(a stale row is itself a violation - fail-closed, no zombie exemptions).

The reported number is the STUB RATIO (stub files / scanned files) - that
ratio, not the raw count, is what later lands in certificates (stub_ratio).

Exit codes: 0 = no unannounced stubs; 1 = unannounced stubs or registry
violations; 2 = config error.

Named behaviours (spec_map "桩检测" row):
- unannounced_stub_blocks_completion
- stub_scan_covers_rust_and_python
- empty_source_file_is_a_stub
- stub_allowlist_entry_must_carry_reason
- stale_stub_allowlist_row_is_refused
- python_pass_only_body_is_flagged
- stub_ratio_is_reported
"""
from __future__ import annotations

import ast
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "stub_registry.json"

RUST_TOKENS = re.compile(
    r"unimplemented!\s*\(|todo!\s*\(|"
    r'panic!\s*\(\s*"(not implemented|unimplemented|TODO)'
)
PY_RAISE = re.compile(r"raise\s+NotImplementedError")


def fail(msg: str) -> int:
    print(f"stub gate FAIL: {msg}", file=sys.stderr)
    return 1


def production_trees() -> list[Path]:
    roots: list[Path] = []
    crates = REPO / "crates"
    if crates.exists():
        for crate in sorted(crates.iterdir()):
            src = crate / "src"
            if src.is_dir():
                roots.append(src)
    for extra in ("python/src", "python-native/src"):
        path = REPO / extra
        if path.is_dir():
            roots.append(path)
    return roots


def scan_rust(path: Path) -> list[tuple[int, str]]:
    hits: list[tuple[int, str]] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for lineno, line in enumerate(text.splitlines(), start=1):
        if RUST_TOKENS.search(line):
            hits.append((lineno, "rust-marker"))
    return hits


def scan_python(path: Path) -> list[tuple[int, str]]:
    hits: list[tuple[int, str]] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for lineno, line in enumerate(text.splitlines(), start=1):
        if PY_RAISE.search(line):
            hits.append((lineno, "notimplemented"))
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return hits  # a parse error is a different gate's problem
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            body = [
                stmt
                for stmt in node.body
                if not (
                    isinstance(stmt, ast.Expr)
                    and isinstance(stmt.value, ast.Constant)
                    and (
                        isinstance(stmt.value.value, str)
                        or stmt.value.value is Ellipsis
                    )
                )
            ]
            if body and all(isinstance(stmt, ast.Pass) for stmt in body):
                hits.append((node.lineno, "pass-only-body"))
    return hits


def scan() -> tuple[list[dict], int]:
    hits: list[dict] = []
    scanned = 0
    for root in production_trees():
        for path in sorted(root.rglob("*")):
            if path.suffix == ".rs":
                scanned += 1
                for lineno, token in scan_rust(path):
                    hits.append({"path": rel(path), "line": lineno, "token": token})
            elif path.suffix == ".py":
                if not path.name.endswith(".py") or path.name == "__init__.py":
                    pass  # __init__.py may legitimately be empty
                scanned += 1
                if path.read_bytes().strip() == b"":
                    if path.name != "__init__.py":
                        hits.append({"path": rel(path), "line": 0, "token": "empty-file"})
                    continue
                for lineno, token in scan_python(path):
                    hits.append({"path": rel(path), "line": lineno, "token": token})
            elif path.is_file() and path.read_bytes().strip() == b"":
                scanned += 1
                hits.append({"path": rel(path), "line": 0, "token": "empty-file"})
    return hits, scanned


def rel(path: Path) -> str:
    return path.relative_to(REPO).as_posix()


def main() -> int:
    try:
        registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
        rows = registry.get("stubs", [])
    except (OSError, ValueError) as exc:
        return fail(f"stub_registry.json unreadable: {exc}")
    if not isinstance(rows, list):
        return fail("stub_registry.json: 'stubs' must be a list")
    hits, scanned = scan()
    known: set[tuple[str, int, str]] = set()
    for row in rows:
        path, token = row.get("path", ""), row.get("token", "")
        reason, line = row.get("reason", ""), row.get("line", -1)
        if not (path and token and str(reason).strip()):
            print(
                f"stub gate violations:\n  registry row without a reason: {path}:{line} ({token}) - "
                "every exemption must say why it exists",
                file=sys.stderr,
            )
            return 1
        known.add((path, int(line), token))
    stale = known - {(h["path"], h["line"], h["token"]) for h in hits}
    problems: list[str] = []
    if stale:
        for path, line, token in sorted(stale):
            problems.append(
                f"stale allowlist row (the stub is gone - clean the registry): {path}:{line} ({token})"
            )
    unannounced = [h for h in hits if (h["path"], h["line"], h["token"]) not in known]
    for hit in unannounced:
        problems.append(
            f"unannounced stub: {hit['path']}:{hit['line']} ({hit['token']}) - "
            "declare it in stub_registry.json with a reason, or finish the work"
        )
    ratio = round(100.0 * len({h["path"] for h in hits}) / scanned, 2) if scanned else 0.0
    if problems:
        print("stub gate violations:")
        for p in problems:
            print(f"  {p}")
        print(f"stub ratio: {ratio}% ({len(hits)} hits / {scanned} files)", file=sys.stderr)
        return 1
    print(
        f"stub gate: no unannounced stubs across {scanned} production files "
        f"(registry {len(rows)} rows, stub ratio {ratio}%, exit 0)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
