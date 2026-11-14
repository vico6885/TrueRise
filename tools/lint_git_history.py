#!/usr/bin/env python3
"""History-object door = leak-prevention door 2.

Scans ALL git objects (every ref, reachable or not) for the banned tokens,
plus commit texts via ``git log --all -p``. The token list is shared with
the workspace door (single encoded source). Encoded wordlist blobs contain
only hex characters, so the gate cannot self-trigger.

Exit codes: 0 = zero hits, 1 = leak (object id listed), 2 = config error.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lint_banned_words import REPO, build_pattern, load_config  # noqa: E402

FLOOR_FILE = Path(__file__).resolve().parent / "history_floor.json"


def run_git(*args: str) -> str:
    """Run a git command inside the repo and return decoded stdout."""
    out = subprocess.run(["git", *args], cwd=REPO, capture_output=True, check=True)
    return out.stdout.decode("utf-8", errors="replace")


def check_history_floor(blobs: int) -> int:
    """History-floor technical gate: blob count must never DECREASE.

    A drop proves a prune/rewrite destroyed history objects (evidence
    destruction). Raising the floor is a governed operation, NOT
    self-authorizable in-repo: the floor document must carry an
    ``authorization`` field pointing at the external governance log entry
    (three-gate record), which reviewers cross-check out-of-repo. A missing
    floor file is a configuration error (exit 2), never a silent pass.
    """
    if not FLOOR_FILE.exists():
        print("HISTORY FLOOR missing: tools/history_floor.json must exist (fail-closed)")
        return 2
    try:
        floor_doc = json.loads(FLOOR_FILE.read_text(encoding="utf-8"))
        floor = int(floor_doc["floor_blobs"])
        authorization = str(floor_doc["authorization"]).strip()
    except (OSError, ValueError, KeyError) as exc:
        print(f"history floor file unreadable: {exc}", file=sys.stderr)
        return 2
    if not authorization:
        print("HISTORY FLOOR unparsable: missing authorization (external-log reference)")
        return 2
    if blobs < floor:
        print(
            f"HISTORY FLOOR VIOLATION: {blobs} blobs < recorded floor {floor} "
            "- history objects were destroyed (prune). "
            "Raising the floor requires a record in the external governance log "
            "(deletion three-gate protocol) referenced by the authorization field."
        )
        return 1
    print(
        f"history floor: {blobs} blobs >= recorded floor {floor} "
        f"(no evidence destruction; authorization: {authorization})"
    )
    return 0


def main() -> int:
    words = load_config()
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
    return check_history_floor(len(blobs))


if __name__ == "__main__":
    raise SystemExit(main())
