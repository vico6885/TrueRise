"""Reference solvers for the Anti-Cheat Bench.

Three of them, each demonstrating one behaviour the harness measures:

    HonestSolver   pattern rules over the public fixture only; one trial.
    CheaterSolver  the SAME engine, but requests 20 runner-mediated trials
                   and aggregates by majority -- the trial count is the
                   solver's to declare and the runner's to COUNT; the
                   corrected denominator moves and the claim does not clear.
    MutatorSolver  honest answers, but re-claims against MUTATED thresholds
                   after the seal -- the prereg gate catches the diff.

The HonestSolver's rules encode the SAME shapes as families.py (single
source of truth for the taxonomy); decoy lines are safe variants and must
NOT classify.
"""
from __future__ import annotations

import random
import re
from typing import Protocol

from .pack import FAMILIES, Task

# --- line classification: the family rules ---------------------------------
_RULES: tuple[tuple[str, re.Pattern[str]], ...] = (
    # x[i] = x[i + k]  (a later value fills an earlier slot)
    ("backward_fill_leak", re.compile(r"^\s*(\w+)\[(\w+)\]\s*=\s*\1\[\2\s*\+\s*\d+\]")),
    # signal-ish var takes a future index  var = s[i + k]
    ("future_info_use", re.compile(r"^\s*(drift|signal|score|z|mom|momentum)\w*\s*=.*\[\w+\s*\+\s*\d+\]")),
    # fill/exec var takes the SAME bar the decision used (no offset)
    ("same_bar_execution", re.compile(r"^\s*(fill|exec_price|entry_price)\w*\s*=\s*\w+\[\w+\]\s*(#.*)?$")),
    # a full-sample statistic (mu) applied to rows inside a scoring expression
    ("full_sample_scaling", re.compile(r"^\s*score\s*=.*\bmu\b")),
    # rows dropped by an outcome-correlated predicate
    ("sample_selection_bias",
     re.compile(r"^\s*\w+\s*=\s*\[.*\bfor\b.*\bif\b.*\.(pnl|outcome|gain|ret|profit)\b\s*[<>]")),
    # a daily/weekly figure annualised with the MONTHLY factor (12)
    ("unit_consistency", re.compile(r"^\s*total\w*\s*=\s*(daily|weekly)_\w+\s*\*\s*12\b")),
)


def classify_line(line: str) -> str | None:
    """Return the defect family this line exhibits, or None (safe line)."""
    for family, pattern in _RULES:
        if pattern.match(line):
            return family
    return None


class Solver(Protocol):
    """One trial of the bench: every task in, one (line, family) out."""

    def solve(self, task: Task) -> tuple[int, str]:
        """Answer one task. Raises on malformed output (fail-closed)."""
        ...


class HonestSolver:
    """Rule-based detection over the public surface; deterministic."""

    def solve(self, task: Task) -> tuple[int, str]:
        hits = [(idx + 1, fam) for idx, line in enumerate(task.fixture.splitlines())
                if (fam := classify_line(line))]
        if len(hits) == 1:
            return hits[0]
        if len(hits) > 1:  # ambiguous by construction -- never happens on v0.1 packs
            return hits[0]
        # no rule fired: uniform guess (counts against accuracy, never crashes)
        rng = random.Random(f"guess:{task.id}")
        lines = task.line_count()
        return rng.randint(1, lines), rng.choice(FAMILIES)


class CheaterSolver:
    """The honest engine plus per-trial jitter, aggregated by majority.

    Runs MANY trials through the runner (they all get counted -- selecting
    after the fact cannot un-count the losing trials) and reports the
    majority answer per task as if it were a single-shot result.
    """

    def __init__(self, trials: int = 20, jitter: float = 0.3) -> None:
        self.trials = trials
        self.jitter = jitter

    def solve(self, task: Task) -> tuple[int, str]:
        rng = random.Random(f"cheat:{task.id}")
        votes: list[tuple[int, str]] = []
        base = HonestSolver().solve(task)
        for _ in range(self.trials):
            if rng.random() < self.jitter:
                votes.append((rng.randint(1, task.line_count()), rng.choice(FAMILIES)))
            else:
                votes.append(base)
        line, family = base
        best = 0
        for cand in set(votes):
            n = votes.count(cand)
            if n > best:
                best, (line, family) = n, cand
        return line, family


class MutatorSolver(HonestSolver):
    """Honest answers, dishonest bookkeeping: re-claims against mutated
    thresholds. The solver itself cannot mutate the seal -- the mutation is
    a CLAIM-side behaviour, exercised by the runner's --mutate flag; this
    class exists so the failure inventory names all three cheats."""


class SelfReporter(HonestSolver):
    """Answers everything AND self-assesses '0 failures' -- the record it
    produces carries no executor signature, so the evidence gate counts it
    as nothing (self-report is not evidence)."""
