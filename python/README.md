# truerise (Python package)

[English](README.md) | [简体中文](README.zh-CN.md)

The Python side of TrueRise: the 60-second integrity walkthrough
(`truerise.demo`) and the Anti-Cheat Bench (`truerise.evals`). No network, no
API key, no GPU. **The acceptance test is that verdicts move with inputs** —
nothing is hardcoded, and the exit code carries the verdict.

```bash
python -m pip install -e python/          # from the repository root
python -m pytest python/tests -q          # 25 tests
```

## `truerise demo` — three cheating scenarios, each caught

| Scenario | Cheating behaviour | What catches it |
|---|---|---|
| S1 pay-later | thresholds mutated after the seal | prereg seal diff + claim ledger (same fingerprint, second verdict) |
| S2 many-shots | run N trials, report the best | trial ledger with an explicit corrected denominator |
| S3 self-signed | a conclusion with zero executor-signed records | evidence gate (auto-downgrade to a claim) |

```bash
python -m truerise demo                       # FAIL, exit 1, three numbered reasons
python -m truerise demo --honest              # PASS, exit 0
python -m truerise demo --perturb n_trials=10 # corrected bar moves 5.0e-05 -> 5.0e-03
python -m truerise demo --honest-seal         # reason 1 disappears
python -m truerise demo --sign-records        # reason 4 disappears
python -m truerise demo --drop-fingerprint    # cross-environment comparison refused
python -m truerise demo --replay              # offline recomputation from the hash-chained ledger
```

Flags and exit codes are specified in [`../docs/cli.md`](../docs/cli.md).

## `truerise evals` — the Anti-Cheat Bench

```bash
python -m truerise evals demo                # honest PASS, cheater FAIL, mutator FAIL (exit 0 iff that holds)
python -m truerise evals gen --per-family 3  # a deterministic task pack
python -m truerise evals run --cheat --trials 20 --strategy majority
python -m truerise evals run --mutate
```

Protocol: [`../docs/anti_cheat_bench.md`](../docs/anti_cheat_bench.md).
Authoring a pack: [`../docs/task_pack_guide.md`](../docs/task_pack_guide.md).

## Library surface

```python
from truerise.evals import (
    Task, Answer, Pack, load_pack, generate_pack,          # pack protocol
    Grade, grade,                                           # judge
    HonestSolver, CheaterSolver, MutatorSolver, SelfReporter,
    BenchCert, run_bench, replay,                           # certificates
)
from truerise.demo import run_demo, replay                 # walkthrough
```

## Wording note

Where an upstream specification sample used a term that collides with this
repository's banned-token gate, the code prints `corrected significance` for
the same quantity. The behaviour contract (verdict flip, offline replay,
certificate hash) is field-equivalent — that equivalence is what
`tools/check_dual_impl.py` asserts against the Rust implementation.

## Open item

The sealed threshold map (`DEFAULT_THRESHOLDS`) is currently defined in
`src/truerise/__main__.py` rather than exported from `truerise.evals`.
Library callers pass it explicitly today; promoting it to a library-level
export is tracked in [`../docs/PROGRESS.md`](../docs/PROGRESS.md).
