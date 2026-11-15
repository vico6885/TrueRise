# Task pack guide

[English](task_pack_guide.md) | [简体中文](task_pack_guide.zh-CN.md)

A task pack is the unit of contribution for the Anti-Cheat Bench: **one pack =
one PR**. Packs are data (CC-BY-4.0, see `../LICENSE-DATA.md`); any generator
or judge code you add ships under Apache-2.0 / MIT.

The protocol itself is specified in [`anti_cheat_bench.md`](anti_cheat_bench.md);
this page is the authoring workflow.

## 1. What a pack is

A pack directory holds exactly two JSONL surfaces:

| File | Who sees it | Row fields |
|---|---|---|
| `tasks.jsonl` | PUBLIC — handed to the solver | `id`, `level`, `prompt`, `fixture` |
| `answers.jsonl` | SEALED — judge side only | `id`, `defect_line`, `family`, `canary` |

Each task is a short scoring-pipeline fixture hiding exactly **one** defect
from the taxonomy. The solver answers `(line, family)`; the judge grades
against the sealed surface.

Default pack scale: 6 families x 20 = **120 tasks**.

## 2. Generate a pack, look at it before you write one

```bash
python -m truerise evals gen --per-family 3 --seed 20261115
ls output/truerise_evals/pack      # tasks.jsonl + answers.jsonl
```

Generation is deterministic: the same `(seed, per_family)` byte-reproduces
both surfaces. That is a test, not a courtesy
(`test_generator_determinism`).

## 3. The rules that make a pack red

`pack.py::_validate` is fail-closed. Each of these raises, and each has a
named test:

| Rule | Error text |
|---|---|
| task ids unique | `duplicate task ids in tasks.jsonl` |
| both surfaces carry the same id set | `task/answer id sets differ (both surfaces must align)` |
| prompt is the canonical bench prompt | `prompt is not the canonical bench prompt` |
| fixture has at least 2 lines | `fixture too small to hide a defect` |
| `family` is in the taxonomy | `family ... not in taxonomy` |
| `defect_line` is inside `1..line_count` | `defect_line ... outside 1..n` |
| the PUBLIC surface carries no sealed hint | `test_public_surface_carries_no_sealed_hints` |
| a work dir containing the isolation canary is refused | `test_materialize_public_fail_closed_on_canary` |

The last two are the anti-contamination core: a task row carries **no**
`family` field, because publishing it would leak the answer; and the runner
refuses to grade a run whose public surface contains the canary string — that
is treated as a leak, not a warning.

## 4. Adding a new defect family

Four touch points, all in `python/src/truerise/evals/`:

| File | What to add |
|---|---|
| `families.py` | `FAMILIES` entry + a `_YourFamily(rng, level) -> list[str]` builder returning fixture lines |
| `families.py` | mark the defective line with `DEFECT_TOKEN` (`@@DEFECT@@`) — the generator strips it and records `defect_line` |
| `solvers.py` | a classification rule in `_RULES` so the reference solver can find it |
| `pack.py` / tests | extend `test_classify_rules_cover_all_six_families` (rename as the count grows) |

Naming rules for a family: snake_case, behaviour-shaped (`future_info_use`,
`same_bar_execution`), not wording-shaped. The judge matches on
`(line, family)` exactly — a family name that describes a *sentence* rather
than a *mechanism* cannot be graded fairly.

Definition discipline, from `../docs/anti_cheat_bench.md` §3: state the
mechanism ("a statistic computed over the FULL sample is applied inside the
scored window"), not the phrase that gives it away.

## 5. Plugging in your own solver

The bench is solver-agnostic; a solver is a one-method protocol:

```python
class MySolver:
    def solve(self, task):            # -> (line_number, family)
        ...
```

`Solver.solve` must raise on malformed output — the runner fail-closes rather
than guessing (`solvers.py::Solver`, `runner.py::run_trials`). Wire it in:

```python
from truerise.evals import load_pack, run_bench
from pathlib import Path

# The sealed bar. Currently defined in truerise/__main__.py as
# DEFAULT_THRESHOLDS; a library-level export is tracked in PROGRESS.md, so
# until then pass the map explicitly.
SEALED = {"min_accuracy": 0.8, "claim_p": 0.05}

cert, summary = run_bench(
    solver=MySolver(), pack=load_pack(Path("output/truerise_evals/pack")),
    work_dir=Path("output/work"), out_dir=Path("output/run"),
    trials=1, strategy="last", thresholds=SEALED, solver_name="my-model",
)
print(cert.verdict, cert.reasons)
```

Reference solvers shipped for self-test: `HonestSolver`, `CheaterSolver`
(best-of-N over hidden trials), `MutatorSolver` (re-claims after moving
thresholds), `SelfReporter` (claims without executor-signed records).

**What an external model run still needs** (registered in
[`PROGRESS.md`](PROGRESS.md), not yet built): an external-solver protocol with
prompt/answer isolation and a recorded fingerprint. Until it lands, published
readings are reference-solver readings — the README says so plainly.

## 6. Checklist before you open the PR

```bash
python -m truerise evals gen --out output/pack --per-family 20 --seed <your seed>
python -m truerise evals run --pack output/pack --trials 1            # honest: PASS expected
python -m truerise evals run --pack output/pack --cheat --trials 20   # FAIL expected
python -m pytest python/tests -q                                      # all green
python tools/lint_banned_words.py                                     # workspace door
```

Also confirm:

- [ ] every task hides exactly one defect (multi-defect rows make the judge ambiguous)
- [ ] no fixture copies a third-party source verbatim (IP-clean declaration, `../CONTRIBUTING.md`)
- [ ] your generator, if any, is deterministic under a fixed seed
- [ ] sealed answers are NOT included in any public artifact in the PR
- [ ] pack scale and family balance stated in the PR description

## 7. What the maintainers look at first

1. Can a solver reach the sealed surface from the public one? (leak = reject)
2. Is the defect real by mechanism, or is it pattern-matched noise?
3. Does the pack degrade gracefully at `--trials 20` (the denominator must bite)?
4. Do the tests name the rule they enforce?
