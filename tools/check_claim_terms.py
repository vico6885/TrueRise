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
   title (hard rule, CHANGELOG.md).
3. PROMISE GRADES (docs/limits.md): ``promise_grades.packages`` must cover
   the whole backlog (every G-id from G1 up to the max appears at least
   once), rows are unique per (package, aspect), grades are only A/B/C, and
   the grade-C-as-A phrase list never appears in any guarded surface - an
   allowlist line counts only when the line itself carries an explicit
   negation, so a boundary sentence must say "does not promise".
4. CLAIM TYPOLOGY + ASSUMPTION LEDGER (CA1/CA2): a term may declare a
   ``claim_type`` from the six-value vocabulary. Causal-typed MET terms
   require a complete, defended row in ``assumption_ledger.json``
   (all four identification assumptions: sutva, consistency,
   exchangeability, positivity) - no row, no causal claim: the claim is a
   hypothesis and must be worded and tiered as one. Association-grade
   types (temporal_precedence, correlational) may never use causal
   language in their meaning text. Every defended ledger row cites a
   named Rust test and a committed artifact, exactly like a met term.

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

Named behaviours (spec_map "因果宣称" row):
- causal_typed_claim_requires_defended_assumption_row: a met term typed
  causal_observational or causal_interventional without a complete,
  defended ledger row is a violation - defend it or downgrade the wording.
- association_grade_wording_never_claims_causality: temporal_precedence
  and correlational meanings may not contain causal markers (precedence
  is not causation).
- unknown_claim_type_breaks_the_registry_fail_closed: a claim_type outside
  the six-value vocabulary is a registry error, not a row warning.
- assumption_ledger_rows_cite_existing_evidence: every defended ledger row
  must cite a named Rust test that exists and an artifact file that
  exists - the ledger inherits the met-term evidence rule.
- assumption_ledger_is_present_and_parsable: a missing or unparsable
  assumption_ledger.json is fail-closed red, even with zero causal claims.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "claim_terms.json"
ASSUMPTION_LEDGER = REPO / "assumption_ledger.json"
TITLE_ZONE = 7  # H1 line + the following 6 lines
TEST_TARGET = re.compile(r"--test ([a-z0-9_]+)")

# CA1/CA2 vocabularies. The tokens mirror the Rust semantics in
# crates/truerise-eval/src/causal/{typology,ledger}.rs - the two sides MUST
# stay word-for-word identical or the gate and the runtime disagree.
CLAIM_TYPES = {
    "correlational",
    "causal_observational",
    "causal_interventional",
    "predictive",
    "measurement_validity",
    "temporal_precedence",
}
LEDGER_REQUIRING_TYPES = {"causal_observational", "causal_interventional"}
ASSOCIATION_GRADE_TYPES = {"temporal_precedence", "correlational"}
CAUSAL_WORDING = re.compile(r"cause|causal|导致|因果|归因", re.IGNORECASE)
ASSUMPTION_TOKENS = {"sutva", "consistency", "exchangeability", "positivity"}
LEDGER_STATUSES = {"defended", "undefended"}


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
    registry_broken: list[str] = []
    rust_tests = collect_rust_test_names()
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
            # Review P2-6: cite a real #[test] fn, not any fn-shaped
            # superstring in the tree.
            if entry.get("test") not in rust_tests:
                problems.append(
                    f"met term '{term}': named test is not a #[test] fn in the Rust tree: "
                    f"{entry.get('test')!r}"
                )
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
    ledger_rows = check_claim_typology(doc, problems, registry_broken, rust_tests)
    if registry_broken:
        print("claim gate registry broken (fail-closed):")
        for b in registry_broken:
            print(f"  {b}")
        return 2
    if problems:
        print("claim gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    met = sum(1 for t in terms if t.get("status") == "met")
    pg = doc.get("promise_grades", {})
    pkgs = pg.get("packages", [])
    typed = sum(1 for t in terms if t.get("claim_type"))
    print(
        f"claim gate: title zone agrees with the evidence ledger "
        f"({met} met / {len(terms) - met} unmet, {len(pkgs)} grade rows, "
        f"{typed} typed terms, {ledger_rows} ledger rows, exit 0)"
    )
    return 0


# Negation markers that make an allowlist line a boundary sentence instead of
# a promise. The line must contain one of these for the allowlist entry to be
# accepted (machine-checked: an allowlisted promise is not a boundary).
# Review fix (P2): the old list missed the most natural Chinese negation
# (没/并非/从不) and ACCEPTED the double-negative 无不 (= everywhere/always,
# an affirmative smuggling itself through the "无" hit).
NEGATION = re.compile(r"不|未|没|无(?!不)|并非|从不|别|non-|not |never |no |don'?t |does not |cannot ")


def collect_rust_test_names() -> set[str]:
    """Every `#[test]` fn name in the Rust tree (the strict-audit approach).

    Review P2-6: a bare substring needle ("fn tier_for") matched any
    superstring and any non-test function; citation checks now key on real
    test names only.
    """
    names: set[str] = set()
    pattern = re.compile(r"#\[test\]\s*(?:\n\s*#\[.*\]\s*\n\s*)*fn\s+([a-zA-Z0-9_]+)")
    for p in (REPO / "crates").rglob("*.rs"):
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for match in pattern.finditer(text):
            names.add(match.group(1))
    return names


def check_claim_typology(
    doc: dict, problems: list[str], registry_broken: list[str], rust_tests: set[str]
) -> int:
    """CA1/CA2 enforcement over claim types and the assumption ledger.

    Returns the number of ledger rows loaded (for the gate's summary line).
    Vocabulary errors are REGISTRY BROKEN (exit 2, fail-closed); missing or
    incomplete defenses for met causal terms are VIOLATIONS (exit 1) - the
    fix is to defend the claim or to downgrade its wording, never to
    silently pass.
    """
    ledger_rows: list[dict] = []
    ledger_by_term: dict[str, dict] = {}
    try:
        ledger_doc = json.loads(ASSUMPTION_LEDGER.read_text(encoding="utf-8"))
        raw_rows = ledger_doc.get("rows")
        if not isinstance(raw_rows, list):
            registry_broken.append("assumption_ledger.json: missing 'rows' array")
        else:
            ledger_rows = raw_rows
    except (OSError, ValueError) as exc:
        registry_broken.append(f"assumption_ledger.json missing or unparsable: {exc} - "
                               "an absent ledger is fail-closed red even with zero causal claims")
        return 0
    # Registry-level invariant FIRST: structural errors fail closed no
    # matter what the terms reference.
    for index, row in enumerate(ledger_rows):
        if not isinstance(row, dict):
            registry_broken.append(f"assumption_ledger.json row {index}: not an object")
            continue
        # Review P2-2: a claim_term that is not a STRING (null, list,
        # number) must be registry-broken, not coerced through str(None).
        term_value = row.get("claim_term")
        term = term_value if isinstance(term_value, str) else ""
        label = term if term else f"<row {index}>"
        if not term.strip():
            registry_broken.append(
                f"assumption_ledger.json row {index}: claim_term empty or not a string"
            )
        else:
            if term in ledger_by_term:
                registry_broken.append(f"assumption_ledger.json: duplicate rows for term {term!r}")
            ledger_by_term[term] = row
        tokens = row.get("assumptions")
        if not isinstance(tokens, list) or not tokens:
            registry_broken.append(f"assumption_ledger.json row {index} ({label!r}): no assumptions declared")
        else:
            seen: set[str] = set()
            for token in tokens:
                # Review P2-1: unhashable JSON values (lists, dicts) crash
                # set membership - isinstance-guard first, fail closed.
                if not isinstance(token, str) or token not in ASSUMPTION_TOKENS:
                    registry_broken.append(
                        f"assumption_ledger.json row {index} ({label!r}): unknown assumption {token!r}"
                    )
                    continue
                if token in seen:
                    registry_broken.append(
                        f"assumption_ledger.json row {index} ({label!r}): assumption {token!r} declared twice"
                    )
                seen.add(token)
        status = row.get("status")
        if status not in LEDGER_STATUSES:
            registry_broken.append(
                f"assumption_ledger.json row {index} ({label!r}): status must be 'defended' or 'undefended'"
            )
        # Review P2-5: empty or non-string evidence fields are a structural
        # refusal (the Rust parser refuses the whole ledger for the same
        # defect) - the two taxonomies must agree in the fail-closed
        # direction.
        for field in ("defense", "test", "artifact"):
            value = row.get(field)
            if not isinstance(value, str) or not value.strip():
                registry_broken.append(
                    f"assumption_ledger.json row {index} ({label!r}): field '{field}' empty or "
                    "not a string - the promise must be stated even while undefended"
                )
        if status == "defended":
            cited = row.get("test")
            if isinstance(cited, str) and cited not in rust_tests:
                problems.append(
                    f"assumption_ledger.json row {index} ({label!r}): defended row cites a test that "
                    f"is not a #[test] fn in the Rust tree: {cited!r} (review P2-6)"
                )
            artifact_value = row.get("artifact")
            artifact = REPO / str(artifact_value) if isinstance(artifact_value, str) else REPO / ""
            if not artifact.exists():
                problems.append(
                    f"assumption_ledger.json row {index} ({label!r}): defended row cites missing artifact "
                    f"{artifact_value!r}"
                )
    # Term-level rules.
    for entry in doc.get("terms", []):
        term = entry.get("term", "")
        claim_type = entry.get("claim_type")
        if claim_type is None:
            continue
        if claim_type not in CLAIM_TYPES:
            registry_broken.append(
                f"{term}: unknown claim_type {claim_type!r} - the six-value vocabulary is "
                "correlational / causal_observational / causal_interventional / predictive / "
                "measurement_validity / temporal_precedence"
            )
            continue
        if claim_type in ASSOCIATION_GRADE_TYPES and CAUSAL_WORDING.search(str(entry.get("meaning", ""))):
            problems.append(
                f"{term}: {claim_type} meaning uses causal language - precedence and "
                "correlation are association-grade evidence; reword or change the claim type"
            )
        if claim_type in LEDGER_REQUIRING_TYPES and entry.get("status") == "met":
            row = ledger_by_term.get(term)
            if row is None:
                problems.append(
                    f"{term}: met {claim_type} claim has NO assumption row - "
                    "no assumption row, no causal claim: defend it or downgrade the wording"
                )
                continue
            raw_tokens = row.get("assumptions") or []
            tokens = {t for t in raw_tokens if isinstance(t, str)}
            complete = (
                row.get("status") == "defended"
                and tokens == ASSUMPTION_TOKENS
                and str(row.get("defense", "")).strip()
            )
            if not complete:
                missing = sorted(ASSUMPTION_TOKENS - tokens)
                problems.append(
                    f"{term}: assumption row incomplete (status={row.get('status')!r}, "
                    f"missing assumptions: {missing or 'none'}) - a met causal claim needs all "
                    "four identification assumptions defended"
                )
    return len(ledger_rows)


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
