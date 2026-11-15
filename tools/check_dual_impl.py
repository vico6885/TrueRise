#!/usr/bin/env python3
"""Dual-implementation cross-certification gate.

Runs the SAME integrity walkthrough on both implementations -- the Rust CLI
(`cargo run -p truerise-cli -- demo`) and the Python package
(`python -m truerise demo`) -- and asserts the certificates agree on every
contract field:

    verdict, claim text, fingerprint line, prereg seal digest (12-hex),
    the corrected significance denominator, the SET of failure kinds caught
    (prereg-diff / trials / fingerprint / evidence), and the EXIT CODE
    (PASS -> 0, FAIL -> 1 on both sides, and each side checked against its own
    verdict so a shared-but-useless exit code cannot pass silently).

Reason WORDING is deliberately NOT compared verbatim: the implementations
share the behaviour contract (verdict flip / offline replay / certificate
hash), not the sentence text (implementation-freedom clause, python/README).
Seal digest equality IS required: both sides seal the same canonical
threshold JSON, so any digest divergence is a real contract break.

Exit 0 = cross-certified; 1 = field mismatch (diff printed); 2 = either side
failed to run.
"""
from __future__ import annotations

import math
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REASON_KINDS = (
    ("prereg", "prereg diff"),
    ("trials", "trials="),
    ("fingerprint", "fingerprint:"),
    ("evidence", "evidence:"),
)


def parse_demo(stdout: str) -> dict:
    """Parse the demo walkthrough stdout into contract fields."""
    fields: dict = {"reasons": [], "seal": ""}
    for line in stdout.splitlines():
        if line.startswith("CLAIM"):
            fields["claim"] = line.split(":", 1)[1].strip()
        elif line.startswith("FINGERPRINT"):
            fields["fingerprint"] = line.split(":", 1)[1].strip()
        elif line.startswith("VERDICT"):
            fields["verdict"] = line.split(":", 1)[1].strip()
        elif line.startswith("CERTIFICATE"):
            m = re.search(r"sealed at ([0-9a-f]{12})", line)
            fields["seal"] = m.group(1) if m else ""
        elif re.match(r"^\s+\d\)", line):
            fields["reasons"].append(line.strip())
    return fields


def reason_kinds(reasons: list[str]) -> set[str]:
    """Map reason lines to their failure KIND (wording is implementation-free)."""
    kinds: set[str] = set()
    for text in reasons:
        for kind, marker in REASON_KINDS:
            if marker in text:
                kinds.add(kind)
                break
    return kinds


def corrected_bar(reasons: list[str]) -> float | None:
    """Extract the corrected significance NUMBER from the trials reason.

    Compared numerically: exponent digit padding differs between the
    implementations' formatters (5.0e-5 vs 5.0e-05) for the same value."""
    for text in reasons:
        m = re.search(r"corrected significance ([0-9.eE+-]+)", text)
        if m:
            return float(m.group(1))
    return None


def run_implementation(cmd: list[str], side: str, env: dict | None = None) -> tuple[str, int]:
    """Run one implementation's demo; return (stdout, exit code).

    Anything beyond 0/1 is a crash and exits 2 here. The exit code IS part of
    the contract now: a verdict that cannot turn a CI lane red is a note, not
    a gate, so both sides must map PASS -> 0 and FAIL -> 1 identically.
    """
    proc = subprocess.run(
        cmd, cwd=REPO, capture_output=True, text=True, encoding="utf-8", env=env,
    )
    if proc.returncode not in (0, 1):
        print(f"{side} demo failed (rc={proc.returncode}):\n{proc.stderr}", file=sys.stderr)
        sys.exit(2)
    return proc.stdout, proc.returncode


def compare(label: str, rust: dict, py: dict) -> list[str]:
    """Return the list of contract-field mismatches (empty = certified)."""
    problems: list[str] = []
    for field in ("verdict", "claim", "fingerprint", "seal", "exit_code"):
        if rust.get(field) != py.get(field):
            problems.append(f"{field}: rust={rust.get(field)!r} python={py.get(field)!r}")
    # Equality is not enough: the shared value must also be the right one, or
    # both sides could agree on a gate that never goes red.
    for side, fields in (("rust", rust), ("python", py)):
        expected = 0 if fields.get("verdict") == "PASS" else 1
        if fields.get("verdict") in ("PASS", "FAIL") and fields.get("exit_code") != expected:
            problems.append(f"{side} exit_code={fields.get('exit_code')!r} for verdict "
                            f"{fields.get('verdict')!r}, expected {expected}")
    if reason_kinds(rust["reasons"]) != reason_kinds(py["reasons"]):
        problems.append(f"reason kinds: rust={sorted(reason_kinds(rust['reasons']))} "
                        f"python={sorted(reason_kinds(py['reasons']))}")
    rust_bar, py_bar = corrected_bar(rust["reasons"]), corrected_bar(py["reasons"])
    # None on BOTH sides = no trials reason in this mode (honest) = agreement.
    one_missing = (rust_bar is None) != (py_bar is None)
    bars_differ = rust_bar is not None and py_bar is not None \
        and not math.isclose(rust_bar, py_bar, rel_tol=1e-9)
    if one_missing or bars_differ:
        problems.append(f"corrected bar: rust={rust_bar!r} python={py_bar!r}")
    if problems:
        print(f"[{label}] MISMATCH")
    return problems


def main() -> int:
    all_problems: list[str] = []
    # PYTHONPATH so the in-tree package runs without a pip install (CI installs
    # it anyway; this keeps the gate usable on a bare clone).
    py_env = {**os.environ, "PYTHONPATH": str(REPO / "python" / "src")}
    for mode, label in (([], "default"), (["--honest"], "honest")):
        rust_out, rust_rc = run_implementation(
            ["cargo", "run", "-q", "-p", "truerise-cli", "--", "demo", *mode], "rust")
        with tempfile.TemporaryDirectory() as tmp:
            py_out, py_rc = run_implementation(
                [sys.executable, "-m", "truerise", "demo", "--out", tmp, *mode],
                "python", py_env)
        rust = parse_demo(rust_out)
        rust["exit_code"] = rust_rc
        py = parse_demo(py_out)
        py["exit_code"] = py_rc
        all_problems += compare(label, rust, py)
    if all_problems:
        for p in all_problems:
            print(f"  {p}")
        print("cross-certification FAILED")
        return 1
    print("cross-certification OK: rust and python certificates agree on all contract fields")
    return 0


if __name__ == "__main__":
    sys.exit(main())
