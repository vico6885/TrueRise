"""Fixture builders for the six defect families.

Each builder returns fixture lines whose defect line carries the temporary
``@@DEFECT@@`` marker and a NEUTER comment -- the marker locates the line
during assembly and is stripped before publication; the comment says what the
code does, never what it sins. Safe-shaped decoys are plain code lines with
no benign tag (a grep for "benign" must not be an answer key).

Levels:

    level 1  no decoys (the defect is the only suspicious line)
    level 2  2-3 decoys shaped like OTHER families' SAFE variants

Level 3 (same-family safe twins, direction discrimination) is planned for
v0.2 -- see docs/anti_cheat_bench.md roadmap.

Every line pattern a solver can rely on lives here ONCE and is reused by the
generator and the reference solver -- the taxonomy has a single source of
truth.
"""
from __future__ import annotations

import random
from collections.abc import Callable

from .pack import FAMILIES

DEFECT_TOKEN = "@@DEFECT@@"

# safe shapes (decoys): one per family -- the benign twin of each defect
SAFE_PATTERNS: tuple[str, ...] = (
    "past = series[i - 1]  # past window",
    "next_fill = close[i + 1]  # next-bar fill",
    "mu_w = sum(window) / len(window)  # window statistic",
    "kept = [r for r in rows if r.ts >= cutoff]  # time filter",
    "if series[i] is None:\n    series[i] = series[i - 1]  # gap repair",
    "annual = monthly_rate * 12  # monthly figure, monthly factor",
)

NAME_POOL = ("series", "quotes", "values", "bars")


def _names(rng: random.Random) -> dict[str, str]:
    pool = list(NAME_POOL)
    rng.shuffle(pool)
    return {"s": pool[0], "c": pool[1], "r": pool[2], "w": pool[3]}


def _future_info_use(rng: random.Random, level: int) -> list[str]:
    n = _names(rng)
    look = rng.choice([1, 2, 3])
    return [
        f"def score_at(i, {n['s']}):",
        f"    past = {n['s']}[i - 1]",
        f"    drift = {n['s']}[i + {look}] {DEFECT_TOKEN}  # momentum term",
        "    signal = drift - past",
        "    return signal if signal > 0 else 0",
    ]


def _full_sample_scaling(rng: random.Random, level: int) -> list[str]:
    n = _names(rng)
    return [
        f"def calibrate({n['s']}):",
        f"    mu = sum({n['s']}) / len({n['s']})  # mean of the given sample",
        "    return mu",
        "",
        f"def score_rows({n['s']}):",
        f"    mu = calibrate({n['s']})",
        "    out = []",
        f"    for i in range(len({n['s']})):",
        f"        score = ({n['s']}[i] - mu) * 100 {DEFECT_TOKEN}  # standardise each row",
        "        out.append(score)",
        "    return out",
    ]


def _backward_fill_leak(rng: random.Random, level: int) -> list[str]:
    n = _names(rng)
    return [
        f"def repair_gaps({n['s']}):",
        f"    for i in reversed(range(len({n['s']}) - 1)):",
        f"        if {n['s']}[i] is None:",
        f"            {n['s']}[i] = {n['s']}[i + 1] {DEFECT_TOKEN}  # fill gap",
        f"    return {n['s']}",
    ]


def _same_bar_execution(rng: random.Random, level: int) -> list[str]:
    n = _names(rng)
    return [
        "def trade(i, close, signal):",
        f"    want_entry = signal({n['c']}, i)",
        f"    fill = {n['c']}[i] {DEFECT_TOKEN}  # entry price",
        "    return fill if want_entry else None",
    ]


def _sample_selection_bias(rng: random.Random, level: int) -> list[str]:
    n = _names(rng)
    return [
        "def curate(rows):",
        f"    kept = [x for x in {n['r']} if x.pnl > 0] {DEFECT_TOKEN}  # winners only",
        "    return kept",
    ]


def _unit_consistency(rng: random.Random, level: int) -> list[str]:
    return [
        "def aggregate(daily_rate, threshold_frac):",
        f"    total = daily_rate * 12 {DEFECT_TOKEN}  # annualise the rate",
        "    if total < threshold_frac:",
        "        return 0",
        "    return total",
    ]


BUILDERS: dict[str, Callable[[random.Random, int], list[str]]] = {
    "future_info_use": _future_info_use,
    "full_sample_scaling": _full_sample_scaling,
    "backward_fill_leak": _backward_fill_leak,
    "same_bar_execution": _same_bar_execution,
    "sample_selection_bias": _sample_selection_bias,
    "unit_consistency": _unit_consistency,
}
assert set(BUILDERS) == set(FAMILIES), "taxonomy and builders must stay aligned"


def build_fixture(rng: random.Random, family: str, level: int) -> tuple[str, int]:
    """Build one fixture: the family defect + level-dependent decoys.

    Deterministic under ``rng``. Returns ``(fixture_text, defect_line)`` with
    defect_line 1-based as displayed and the marker STRIPPED from the text.
    """
    lines = BUILDERS[family](rng, level)
    token_idx = next(i for i, ln in enumerate(lines) if DEFECT_TOKEN in ln)
    defect_text = lines[token_idx].replace(f" {DEFECT_TOKEN}", "").replace(DEFECT_TOKEN, "")
    lines[token_idx] = defect_text
    if level >= 2:
        for decoy in rng.sample(SAFE_PATTERNS, k=3):
            slot = rng.randint(1, len(lines) + 1)
            for piece in reversed(decoy.splitlines()):
                lines.insert(slot - 1, f"    {piece}")
    fixture = "\n".join(lines) + "\n"
    # the decoys are distinct shapes, never byte-equal to the defect line
    defect_line = next(
        i for i, ln in enumerate(fixture.splitlines()) if ln.strip() == defect_text.strip()
    ) + 1
    return fixture, defect_line
