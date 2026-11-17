#!/usr/bin/env python3
"""Test-bite gate (G34): does the test prove ANYTHING?

Reverse-mutation guard: revert the change a test claims to cover, re-run the
test. If it is still green, the test never proved anything - the completion
claim it backs is downgraded to "unverified" (verification theatre).

Two modes:

1. ``--selftest`` (the CI mode; default with no args): builds a THROWAWAY
   git repo in a temp dir and proves the mechanism mechanically -
   a checker that asserts the feature BITES (red after the feature is
   reverted), and a deliberately "green but useless" checker is flagged as
   never proving anything. The real worktree is never touched.

2. ``--floor <since>``: for every changed test file between ``since`` and
   HEAD, the ``#[test]`` count and ``assert``-family count must not DROP
   (removing tests to sneak past a gate is the same theatre), and a DELETED
   test file is a violation that needs an explicit reason in
   ``test_bites_registry.json``.

Honest scope (docs/limits.md discipline): bite-checking proves a test is
CORRELATED with its subject; it cannot prove the test's assertions measure
what the user wanted - that is G32's per-item evidence job.

Exit codes: 0 = mechanism proven / floor holds; 1 = violation; 2 = config
error.

Named behaviours (spec_map "反向变异" row):
- revert_kept_tests_green_is_a_failure
- assertion_count_floor_holds
- test_file_removal_requires_approval
- selftest_proves_bite_and_flags_theatre
- stale_test_bites_allowlist_row_is_refused
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "test_bites_registry.json"
# Review fix (P2): subdirectory test files (tests/common/mod.rs and any
# deeper tree) were outside the floor - deleting them needed no approval.
TEST_FILE = re.compile(r"^crates/[^/]+/tests/.+\.rs$|^tests/[^/]+\.rs$")


def fail(msg: str) -> int:
    print(f"test-bite gate FAIL: {msg}", file=sys.stderr)
    return 1


def run(cmd: list[str], cwd: Path | None = None) -> tuple[int, str]:
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    return proc.returncode, proc.stdout + proc.stderr


def selftest() -> int:
    """Prove the mechanism inside a throwaway repo (worktree untouched)."""
    with tempfile.TemporaryDirectory(prefix="truerise_bites_") as tmp:
        repo = Path(tmp)
        run(["git", "init", "-q"], repo)
        run(["git", "config", "user.email", "bites@drill.local"], repo)
        run(["git", "config", "user.name", "bite-drill"], repo)
        # Commit A: the FEATURE (the subject a test claims to cover).
        (repo / "feature.py").write_text("VALUE = 7\n", encoding="utf-8")
        run(["git", "add", "feature.py"], repo)
        run(["git", "commit", "-q", "-m", "feature"], repo)
        # Commit B: two checkers - one BITES, one is green-but-useless.
        (repo / "check_bites.py").write_text(
            "import sys; sys.exit(0 if __import__('feature').VALUE == 7 else 1)\n",
            encoding="utf-8",
        )
        (repo / "check_theatre.py").write_text(
            "import sys; sys.exit(0)\n",
            encoding="utf-8",
        )
        run(["git", "add", "check_bites.py", "check_theatre.py"], repo)
        run(["git", "commit", "-q", "-m", "checkers"], repo)
        # Before the revert both checkers are green (that is the trap).
        for checker in ("check_bites.py", "check_theatre.py"):
            code, _ = run([sys.executable, checker], repo)
            if code != 0:
                return fail(f"selftest precondition broken: {checker} red before revert")
        # Revert the FEATURE commit and re-run both checkers.
        head = run(["git", "rev-parse", "HEAD"], repo)[1].strip()
        feature_sha = run(["git", "rev-parse", "HEAD~1"], repo)[1].strip()
        rev = run(["git", "revert", "--no-edit", feature_sha], repo)
        if rev[0] != 0:
            return fail(f"selftest revert failed: {rev[1]}")
        biting, _ = run([sys.executable, "check_bites.py"], repo)
        theatre, _ = run([sys.executable, "check_theatre.py"], repo)
        _ = head
        verdicts = {
            "check_bites.py": "bites" if biting != 0 else "never_proved_anything",
            "check_theatre.py": "bites" if theatre != 0 else "never_proved_anything",
        }
        expected = {"check_bites.py": "bites", "check_theatre.py": "never_proved_anything"}
        if verdicts != expected:
            return fail(f"selftest verdict mismatch: {verdicts}")
        print(
            "test-bite gate: mechanism proven in a throwaway repo "
            "(biting checker went red after revert; the green-but-useless checker "
            "was flagged never_proved_anything; worktree untouched, exit 0)"
        )
        return 0


def count_tokens(text: str) -> tuple[int, int]:
    tests = len(re.findall(r"#\[test\]", text))
    asserts = len(re.findall(r"\bassert[!_]|assert_eq!|assert_ne!", text))
    return tests, asserts


def git_show(path: str, rev: str) -> str:
    code, out = run(["git", "show", f"{rev}:{path}"])
    return out if code == 0 else ""


def floor(since: str) -> int:
    """Changed test files must not lose tests or assertions."""
    code, diff = run(["git", "diff", "--name-status", f"{since}..HEAD"])
    if code != 0:
        return fail(f"git diff failed: {diff}")
    try:
        registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
        rows = registry.get("removed_tests", [])
    except (OSError, ValueError) as exc:
        return fail(f"test_bites_registry.json unreadable: {exc}")
    allowed = {row.get("path", "") for row in rows if str(row.get("reason", "")).strip()}
    stale = {row.get("path", "") for row in rows if not str(row.get("reason", "")).strip()}
    if stale:
        print(
            "test-bite gate violations:\n"
            + "\n".join(f"  allowlist row without a reason: {p}" for p in sorted(stale))
        )
        return 1
    problems: list[str] = []
    for line in diff.splitlines():
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        status, path = parts[0][0], parts[-1]
        if not TEST_FILE.match(path):
            continue
        if status == "D":
            if path not in allowed:
                problems.append(
                    f"test file deleted without an approved reason: {path} "
                    "(test_file_removal_requires_approval)"
                )
            continue
        before_t, before_a = count_tokens(git_show(path, since))
        after_t, after_a = count_tokens(git_show(path, "HEAD"))
        if after_t < before_t:
            problems.append(
                f"{path}: #[test] count dropped {before_t} -> {after_t} "
                "(removing tests to sneak past a gate is verification theatre)"
            )
        if after_a < before_a:
            problems.append(
                f"{path}: assert count dropped {before_a} -> {after_a} "
                "(assertion_count_floor_holds)"
            )
    if problems:
        print("test-bite gate violations:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"test-bite gate: floor holds since {since} (no dropped tests/assertions, exit 0)")
    return 0


def main() -> int:
    args = sys.argv[1:]
    if not args or args == ["--selftest"]:
        return selftest()
    if args[0] == "--floor":
        if len(args) != 2:
            return fail("usage: check_test_bites.py --floor <since>")
        return floor(args[1])
    return fail(f"unknown argument: {args[0]}")


if __name__ == "__main__":
    raise SystemExit(main())
