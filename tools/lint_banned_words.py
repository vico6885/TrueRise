#!/usr/bin/env python3
"""Workspace banned-token gate = leak-prevention door 1 + the naming gate.

Scans every repository text file for tokens decoded from
``tools/banned_words.hex`` (the token list exists ONLY in hex-encoded form;
this script holds no token literals). ASCII tokens match on word boundaries,
CJK tokens match as substrings; matching is case-insensitive.

Exit codes: 0 = zero hits (gate green), 1 = leak detected (hits listed),
2 = configuration error (empty/corrupt token list is fail-closed red).

Excluded surfaces (rationale recorded in docs/PROGRESS.md):
- ``.git/``: VCS internals - the history door (lint_git_history.py) owns
  this surface exclusively.
- ``target/``: build artifacts.
- ``Cargo.lock``: generated file; dependency version strings may legitimately
  contain pre-release suffixes.
- exempt path prefixes from the encoded ``X:`` section: reserved for the
  spec-recognised industry plugin crates, which live OUT OF REPO per the
  workspace layout (this repo hosts port traits + conformance only), so the
  section is empty at M0. If a plugin crate ever lands in-repo, its path is
  added to the encoded config - a single-source governance change.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WORDLIST = Path(__file__).resolve().parent / "banned_words.hex"
SKIP_DIRS = {".git", "target", "node_modules"}
SKIP_FILES = {"Cargo.lock"}


def load_config() -> tuple[list[str], list[str]]:
    """Decode the encoded token list.

    Returns:
        (banned tokens, exempt path prefixes). Raises SystemExit(2) when the
        list is empty or corrupt - an empty gate must be red, never green.
    """
    words: list[str] = []
    exempt: list[str] = []
    try:
        raw_lines = WORDLIST.read_text(encoding="ascii").splitlines()
    except OSError as exc:
        print(f"banned-token list unreadable: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
    for raw in raw_lines:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        kind, sep, hexpart = line.partition(":")
        if sep == "" or kind not in ("W", "X"):
            print(f"corrupt wordlist line: {line[:20]!r}", file=sys.stderr)
            raise SystemExit(2)
        try:
            token = bytes.fromhex(hexpart).decode("utf-8")
        except ValueError as exc:
            print(f"corrupt hex in wordlist: {exc}", file=sys.stderr)
            raise SystemExit(2) from exc
        (words if kind == "W" else exempt).append(token)
    if not words:
        print("banned-token list is EMPTY - fail-closed red", file=sys.stderr)
        raise SystemExit(2)
    return words, exempt


def build_pattern(words: list[str]) -> re.Pattern[str]:
    """Compile one case-insensitive alternation over all tokens.

    ASCII tokens get ``\\b`` anchors on both sides (benign superstrings such
    as 'alphabet' or 'quantize' do not false-positive); CJK tokens have no
    word-boundary concept and match as substrings.
    """
    parts = []
    for word in words:
        if all(ord(c) < 128 for c in word):
            parts.append(r"\b" + re.escape(word) + r"\b")
        else:
            parts.append(re.escape(word))
    return re.compile("|".join(parts), re.IGNORECASE)


def main() -> int:
    words, exempt = load_config()
    pattern = build_pattern(words)
    exempt_abs = [str((REPO / p).resolve()) for p in exempt]
    hits: list[str] = []
    scanned = 0
    for path in sorted(REPO.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(REPO)
        if set(rel.parts) & SKIP_DIRS or rel.name in SKIP_FILES:
            continue
        if any(str(path.resolve()).startswith(p) for p in exempt_abs):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, PermissionError, OSError):
            continue  # binary / unreadable: the history door re-scans raw bytes
        scanned += 1
        for lineno, line in enumerate(text.splitlines(), 1):
            if pattern.search(line):
                hits.append(f"{rel.as_posix()}:{lineno}")
    if hits:
        print(f"LEAK: {len(hits)} hit(s):")
        for h in hits:
            print(f"  {h}")
        return 1
    print(f"workspace door: 0 hits across {scanned} files (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
