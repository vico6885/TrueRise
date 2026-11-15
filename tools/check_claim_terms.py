#!/usr/bin/env python3
"""Claim gate: the title may never run ahead of the evidence.

``claim_terms.json`` maps every verifiable noun of the project title/slogan
to a command, a named test and a committed artifact. Two assertions:

1. UNMET terms must not appear in any guarded surface. The guarded surface
   is the TITLE ZONE of each registered file: the H1 heading plus the
   following 6 lines (tagline zone). Elsewhere the word is prose, not a
   claim, and is out of scope.
2. MET terms must have ALL their evidence present in this commit: the named
   test identifier exists in the Rust tree, the artifact file exists, and
   the command names an existing integration-test target when it has the
   ``--test <name>`` shape. Broken evidence = red = the word leaves the
   title (hard rule, docs/PROGRESS.md).

The gate is honest-by-construction: flipping a row to ``met`` without its
evidence in the same commit is a CI red, not a README edit.

Exit codes: 0 = title zone matches the evidence ledger; 1 = violations;
2 = registry broken (fail-closed).

Named behaviours (spec_map "口号" row):
- claim_gate_red_means_the_word_leaves_the_title: an unmet term found in a
  guarded title zone is a violation whose fix is removing the word.
- claim_terms_map_term_to_command_test_artifact: every term row carries all
  three evidence pointers, even while unmet.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "claim_terms.json"
TITLE_ZONE = 7  # H1 line + the following 6 lines
TEST_TARGET = re.compile(r"--test ([a-z0-9_]+)")


def fail(msg: str) -> int:
    print(f"claim gate FAIL: {msg}", file=sys.stderr)
    return 1


def title_zone(path: Path) -> str:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith("# ")), 0)
    return "\n".join(lines[start : start + TITLE_ZONE])


def main() -> int:
    try:
        doc = json.loads(REGISTRY.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return fail(f"claim_terms.json unreadable: {exc}")
    surfaces = doc.get("guarded_surfaces")
    terms = doc.get("terms")
    if not surfaces or not isinstance(terms, list) or not terms:
        return fail("registry must carry guarded_surfaces and a non-empty terms list")
    problems: list[str] = []
    for entry in terms:
        term = entry.get("term", "")
        status = entry.get("status")
        for field in ("command", "test", "artifact"):
            if not entry.get(field):
                problems.append(f"{term}: field '{field}' empty - the promise must be stated even while unmet")
        if status == "unmet":
            for surface in surfaces:
                path = REPO / surface
                if not path.exists():
                    problems.append(f"guarded surface missing: {surface}")
                    continue
                if re.search(re.escape(term), title_zone(path), re.IGNORECASE):
                    problems.append(
                        f"unmet term '{term}' appears in the title zone of {surface} - "
                        "remove the word or land the evidence (hard rule: red gate = word leaves the title)"
                    )
        elif status == "met":
            needle = f"fn {entry.get('test', '')}"
            tree_hits = any(
                needle in p.read_text(encoding="utf-8", errors="replace")
                for p in (REPO / "crates").rglob("*.rs")
            )
            if not tree_hits:
                problems.append(f"met term '{term}': named test not found in the Rust tree: {needle}")
            artifact = REPO / str(entry.get("artifact", ""))
            if not artifact.exists():
                problems.append(f"met term '{term}': artifact missing: {entry.get('artifact')}")
            target = TEST_TARGET.search(str(entry.get("command", "")))
            if target and not (REPO / "crates" / "truerise-cli" / "tests" / f"{target.group(1)}.rs").exists():
                problems.append(
                    f"met term '{term}': command cites missing integration target crates/truerise-cli/tests/{target.group(1)}.rs"
                )
        else:
            problems.append(f"{term}: status must be 'met' or 'unmet' (got {status!r})")
    if problems:
        print("claim gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    met = sum(1 for t in terms if t.get("status") == "met")
    print(f"claim gate: title zone agrees with the evidence ledger ({met} met / {len(terms) - met} unmet, exit 0)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
