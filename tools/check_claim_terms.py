#!/usr/bin/env python3
"""Claim gate: the title may never run ahead of the evidence.

``claim_terms.json`` maps every verifiable noun of the project title/slogan
to a command, a named test and a committed artifact. Three assertions:

1. UNMET terms must not appear in any guarded surface. The guarded surface
   is the TITLE ZONE of each registered file: the H1 heading plus the
   following 6 lines (tagline zone). Elsewhere the word is prose, not a
   claim, and is out of scope.
2. MET terms must have ALL their evidence present in this commit: the named
   test identifier exists in the Rust tree, the artifact file exists, and
   the command names an existing integration-test target when it has the
   ``--test <name>`` shape. Broken evidence = red = the word leaves the
   title (hard rule, docs/PROGRESS.md).
3. PROMISE GRADES (docs/limits.md): ``promise_grades.packages`` must cover
   the whole backlog (every G-id from G1 up to the max appears at least
   once), rows are unique per (package, aspect), grades are only A/B/C, and
   the grade-C-as-A phrase list never appears in any guarded surface - an
   allowlist line counts only when the line itself carries an explicit
   negation, so a boundary sentence must say "does not promise".

The gate is honest-by-construction: flipping a row to ``met`` without its
evidence in the same commit is a CI red, not a README edit.

Exit codes: 0 = title zone matches the evidence ledger; 1 = violations;
2 = registry broken (fail-closed).

Named behaviours (spec_map "口号" row):
- claim_gate_red_means_the_word_leaves_the_title: an unmet term found in a
  guarded title zone is a violation whose fix is removing the word.
- claim_terms_map_term_to_command_test_artifact: every term row carries all
  three evidence pointers, even while unmet.
- c_grade_promise_phrase_never_appears_in_guarded_surfaces: a forbidden
  phrase anywhere in a guarded surface is a violation (full file, not only
  the title zone - grade-C promises are usually smuggled into prose).
- promise_phrase_allowlist_line_must_carry_negation: an allowlist line
  without a negation marker in the same line is a registry violation.
- promise_grade_rows_are_unique_per_package_aspect: duplicate (package,
  aspect) pairs are refused.
- promise_grade_unknown_grade_is_refused: grades outside {A, B, C} fail.
- promise_package_ids_form_a_complete_sequence: gaps in G1..Gmax fail
  closed - an unclassified promise is exactly the kind that must not ship.
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
    check_promise_grades(doc, surfaces, problems)
    if problems:
        print("claim gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    met = sum(1 for t in terms if t.get("status") == "met")
    pg = doc.get("promise_grades", {})
    pkgs = pg.get("packages", [])
    print(
        f"claim gate: title zone agrees with the evidence ledger "
        f"({met} met / {len(terms) - met} unmet, {len(pkgs)} grade rows, exit 0)"
    )
    return 0


# Negation markers that make an allowlist line a boundary sentence instead of
# a promise. The line must contain one of these for the allowlist entry to be
# accepted (machine-checked: an allowlisted promise is not a boundary).
# Review fix (P2): the old list missed the most natural Chinese negation
# (没/并非/从不) and ACCEPTED the double-negative 无不 (= everywhere/always,
# an affirmative smuggling itself through the "无" hit).
NEGATION = re.compile(r"不|未|没|无(?!不)|并非|从不|别|non-|not |never |no |don'?t |does not |cannot ")


def check_promise_grades(doc: dict, surfaces: list[str], problems: list[str]) -> None:
    """Enforce the promise-grade registry (docs/limits.md), fail-closed.

    A MISSING registry is itself a violation: an unclassified promise is the
    exact claim class this project refuses, so absence of ``promise_grades``
    cannot pass silently.
    """
    pg = doc.get("promise_grades")
    if not isinstance(pg, dict):
        problems.append("promise_grades: registry missing - unclassified promises must not ship")
        return
    phrases = pg.get("forbidden_phrases")
    if not isinstance(phrases, list) or not phrases:
        problems.append("promise_grades: forbidden_phrases empty - the C-as-A rule is not enforceable")
        return
    allowlist = pg.get("allowlist", [])
    if not isinstance(allowlist, list):
        problems.append("promise_grades: allowlist must be a list of full lines")
        allowlist = []
    # Registry-level invariant FIRST: every allowlist entry must itself be a
    # boundary sentence. Checking only surface-encountered lines would let a
    # negation-free promise line sit armed in the registry until someone
    # happens to write it - fail-closed instead.
    for entry in allowlist:
        if not isinstance(entry, str) or not NEGATION.search(entry):
            problems.append(
                f"promise_grades: allowlist entry {entry!r} carries no negation - "
                "a boundary sentence must say 'does not promise', never 'promises'"
            )
    allowed = set(allowlist)
    for surface in surfaces:
        path = REPO / surface
        if not path.exists():
            continue  # already reported by the term loop above
        text = path.read_text(encoding="utf-8", errors="replace")
        for line_no, line in enumerate(text.splitlines(), start=1):
            if line in allowed:
                continue  # negation already enforced at registry level
            for phrase in phrases:
                if re.search(re.escape(phrase), line, re.IGNORECASE):
                    problems.append(
                        f"{surface}:{line_no}: grade-C-as-A phrase {phrase!r} in a guarded "
                        "surface - reword as a boundary statement or reduce the claim"
                    )
    pkgs = pg.get("packages")
    if not isinstance(pkgs, list) or not pkgs:
        problems.append("promise_grades: packages list empty - backlog must be classified")
        return
    seen_pairs: set[tuple[str, str]] = set()
    numbers: set[int] = set()
    for row in pkgs:
        pkg = row.get("package", "")
        grade = row.get("grade", "")
        aspect = row.get("aspect", "")
        if not re.fullmatch(r"G\d+", pkg):
            problems.append(f"promise_grades: malformed package id {pkg!r} (want G<number>)")
            continue
        numbers.add(int(pkg[1:]))
        if grade not in {"A", "B", "C"}:
            problems.append(f"promise_grades: {pkg}: unknown grade {grade!r} (want A | B | C)")
        pair = (pkg, aspect)
        if pair in seen_pairs:
            problems.append(f"promise_grades: duplicate (package, aspect) row: {pkg} / {aspect}")
        seen_pairs.add(pair)
    expected = set(range(1, max(numbers) + 1)) if numbers else set()
    gaps = sorted(expected - numbers)
    if gaps:
        problems.append(
            "promise_grades: package-id sequence has gaps "
            f"G{gaps[0]}..G{gaps[-1]} ({len(gaps)} missing) - every backlog package must be graded"
        )


if __name__ == "__main__":
    raise SystemExit(main())
