#!/usr/bin/env python3
"""History-object door = leak-prevention door 2.

Scans ALL git objects (every ref, reachable or not) for the banned tokens,
plus commit texts via ``git log --all -p``. The token list is shared with
the workspace door (single encoded source). Encoded wordlist blobs contain
only hex characters, so the gate cannot self-trigger.

Exit codes: 0 = zero hits, 1 = leak (object id listed), 2 = config error.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lint_banned_words import REPO, build_pattern, load_config  # noqa: E402


def run_git(*args: str) -> str:
    """Run a git command inside the repo and return decoded stdout."""
    out = subprocess.run(["git", *args], cwd=REPO, capture_output=True, check=True)
    return out.stdout.decode("utf-8", errors="replace")


def main() -> int:
    words, _exempt = load_config()
    pattern = build_pattern(words)
    hits: list[str] = []

    # Door 2a: commit messages + patch text of every ref.
    log_text = run_git("log", "--all", "-p")
    for lineno, line in enumerate(log_text.splitlines(), 1):
        if pattern.search(line):
            hits.append(f"git-log:{lineno}")
            if len(hits) > 50:
                break

    # Door 2b: every object in the object database (blobs include unreachable
    # history - the surface a plain worktree grep can never see).
    listing = run_git(
        "cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype)"
    )
    objects = [ln.split() for ln in listing.splitlines() if ln.strip()]
    blobs = [oid for oid, kind in objects if kind == "blob"]
    for oid in blobs:
        blob = subprocess.run(
            ["git", "cat-file", "blob", oid], cwd=REPO, capture_output=True, check=True
        ).stdout
        text = blob.decode("utf-8", errors="replace")
        for lineno, line in enumerate(text.splitlines(), 1):
            if pattern.search(line):
                hits.append(f"object:{oid[:12]}:{lineno}")
                break  # one report per object is enough

    if hits:
        print(f"LEAK(history): {len(hits)} hit(s):")
        for h in hits[:50]:
            print(f"  {h}")
        return 1
    print(f"history door: 0 hits across {len(blobs)} blobs + full ref log (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
