#!/usr/bin/env python3
"""Wiring gate: the binary must drive every crate in the roster.

``wiring_manifest.json`` declares the roster of crates the ``truerise-cli``
binary is EXPECTED to reach through production dependency edges. This gate
computes the real production transitive closure from ``cargo metadata`` and
fails while any roster entry is an unreachable island.

The gate proves reachability, never usefulness: a crate can only be wired by
a work package whose enforcement tests exercise it (see docs/PROGRESS.md).
A wiring change without an enforcement test is a fake wire and fails review.

``--selftest`` runs the gate logic against two in-memory roster mutations and
fails if the gate does not bite in BOTH directions:
  1. removing a crate that IS in the closure must keep the verdict green
     (the gate checks roster <= closure, not exact equality);
  2. adding a member crate that is NOT in the closure must turn it red.

Exit codes: 0 = roster reachable (or selftest bit correctly);
1 = island(s) found (or selftest caught a toothless gate);
2 = config/tooling error (missing manifest, unparsable manifest, unknown
roster entry, cargo metadata failure - fail-closed).
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MANIFEST = REPO / "wiring_manifest.json"
ROOT = "truerise-cli"


def fail(msg: str) -> int:
    print(f"wiring gate FAIL: {msg}", file=sys.stderr)
    return 1


def config_error(msg: str) -> int:
    print(f"wiring gate config error: {msg}", file=sys.stderr)
    return 2


def load_roster() -> list[str]:
    if not MANIFEST.exists():
        raise FileNotFoundError("wiring_manifest.json missing - fail-closed red")
    doc = json.loads(MANIFEST.read_text(encoding="utf-8"))
    roster = doc.get("roster")
    if not isinstance(roster, list) or not roster:
        raise ValueError("roster empty or malformed")
    return roster


def member_graph() -> dict[str, set[str]]:
    """Workspace member name -> set of workspace-member names it normally depends on."""
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(REPO / "Cargo.toml")],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {proc.stderr.strip()[:400]}")
    meta = json.loads(proc.stdout)
    members = set(meta.get("workspace_members", []))
    graph: dict[str, set[str]] = {}
    for pkg in meta.get("packages", []):
        if pkg.get("id") not in members:
            continue
        name = pkg["name"]
        deps = {
            d["name"]
            for d in pkg.get("dependencies", [])
            if d.get("kind") is None and d["name"] in {p["name"] for p in meta["packages"] if p.get("id") in members}
        }
        graph[name] = deps
    if ROOT not in graph:
        raise RuntimeError(f"workspace member {ROOT} not found")
    return graph


def closure_from(graph: dict[str, set[str]], root: str) -> set[str]:
    seen: set[str] = set()
    stack = [root]
    while stack:
        node = stack.pop()
        if node in seen:
            continue
        seen.add(node)
        stack.extend(graph.get(node, ()))
    return seen


def verdict(roster: list[str], closure: set[str]) -> list[str]:
    """Missing roster entries (empty list = green)."""
    return sorted(set(roster) - closure)


def check_selftest(closure: set[str], members: list[str]) -> int:
    """wiring_manifest_roster_reachable_from_cli_binary - the gate must bite
    in both directions or it is decoration (check_wiring_selftest_bites_both_directions)."""
    in_closure = sorted(closure & set(members) - {ROOT})
    if not in_closure:
        return config_error("selftest needs at least one reachable member")
    # Direction 1: shrink the roster to the closure - still green.
    if verdict(in_closure, closure):
        print("wiring selftest FAIL: shrinking the roster to the closure turned the gate red")
        return 1
    # Direction 2: add a member outside the closure - must go red naming it.
    outside = sorted(set(members) - closure)
    if not outside:
        print("wiring selftest: no unreachable member exists; bite-direction-2 is vacuously satisfied")
        return 0
    canary = outside[0]
    got = verdict(in_closure + [canary], closure)
    if got != [canary]:
        print(f"wiring selftest FAIL: adding unreachable {canary} did not bite (got {got})")
        return 1
    print("wiring selftest: gate bites both directions "
          f"(roster=closure green; roster+{canary} red) (exit 0)")
    return 0


def main() -> int:
    selftest = "--selftest" in sys.argv[1:]
    try:
        roster = load_roster()
        graph = member_graph()
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        return config_error(str(exc))
    closure = closure_from(graph, ROOT)
    if selftest:
        return check_selftest(closure, sorted(graph))
    unknown = sorted(set(roster) - set(graph))
    if unknown:
        return config_error(f"roster entries are not workspace members: {unknown}")
    if ROOT in roster:
        return config_error("the root binary (truerise-cli) is not a roster entry")
    missing = verdict(roster, closure)
    if missing:
        print("wiring gate violations - roster entries unreachable from the binary:")
        for name in missing:
            print(f"  island: {name}")
        return 1
    print(f"wiring gate: {len(roster)}/{len(roster)} roster crates reachable from {ROOT} (exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
