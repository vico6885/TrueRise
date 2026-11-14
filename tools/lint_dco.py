#!/usr/bin/env python3
"""DCO gate: every commit must carry a ``Signed-off-by:`` trailer.

DCO proves license granting, NOT source provenance - the IP-clean three
declarations in CONTRIBUTING.md cover provenance and are mandatory too.
Exit 0 = all commits signed; 1 = unsigned commits listed; 2 = git failure.
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def main() -> int:
    proc = subprocess.run(
        ["git", "log", "--format=%H%x1f%an%x1f%B%x1e", "--all"],
        cwd=REPO,
        capture_output=True,
    )
    if proc.returncode != 0:
        print(f"git log failed: {proc.stderr.decode(errors='replace')}", file=sys.stderr)
        return 2
    unsigned: list[str] = []
    count = 0
    for record in proc.stdout.decode("utf-8", errors="replace").split("\x1e"):
        record = record.strip("\n\x1f ")
        if not record:
            continue
        parts = record.split("\x1f")
        if len(parts) < 3:
            continue
        sha, author, body = parts[0], parts[1], parts[2]
        count += 1
        if "Signed-off-by:" not in body:
            unsigned.append(f"{sha[:12]} ({author})")
    if unsigned:
        print("DCO violations (missing Signed-off-by):")
        for u in unsigned:
            print(f"  {u}")
        return 1
    print(f"DCO gate: all {count} commit(s) signed (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
