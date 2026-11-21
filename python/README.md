# truerise (Python package)

[English](README.md) | [简体中文](README.zh-CN.md)

The Python side of TrueRise: the 60-second integrity walkthrough
(`truerise.demo`), the Anti-Cheat Bench (`truerise.evals`), long-horizon
closed-loop packs, the external-solver protocol, and the zero-dependency
SDK (`truerise.sdk`). No network, no API key, no GPU. **The acceptance
test is that verdicts move with inputs** — nothing is hardcoded, and the
exit code carries the verdict.

```bash
python -m pip install -e python/          # from the repository root
python -m pytest python/tests -q          # 42 tests
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

## Long-horizon closed-loop packs

Multi-step deterministic pipelines whose FINAL state is machine-checkable;
a task counts as complete only when EVERY sealed step matches (a right
endpoint through a skipped step is a FAIL — the point is the loop):

```python
from truerise.evals.long_horizon import generate_long_horizon_pack, run_long_horizon

pack = generate_long_horizon_pack(out_dir, chains=8, seed=20261116)  # byte-reproducible
summary = run_long_horizon(my_solver, pack, work_dir)                # integers only
```

## External solvers (the one-file contract)

Any program that reads `TRUERISE_PUBLIC_DIR` and writes
`submission.jsonl` into `TRUERISE_OUT_DIR` can be evaluated by the bench;
the runner grades and EXECUTOR-SIGNS every record (a solver's own accuracy
claim is self-report and caps at L0):

```python
from truerise.evals.runner import run_external_trials

passes, signed = run_external_trials(["python", "my_solver.py"], pack, work_dir)
```

Protocol reference implementation: `truerise/evals/external_solver.py`.

## Library surface

> G29 deferral (honest disclosure): the SDK's `ratchet_propose` still calls
> the un-gated `propose` — the Python contract is frozen pre-release; the
> Rust core is the authoritative implementation. The py surface moves to
> `propose_gated` with the SDK batch (see CHANGELOG "G29 write gate").

```python
from truerise.sdk import (                                # the governed harness, pure Python
    Session, Policy, ApprovalQueue, Budget, Ratchet,      # mirrors the Rust contracts
    Fingerprint, seal_thresholds, prereg_diff, evidence_gate,
)
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

## Known gap

The sealed threshold map (`DEFAULT_THRESHOLDS`) is defined in
`src/truerise/__main__.py` (the CLI) rather than exported from
`truerise.evals`. Library callers pass their threshold map explicitly —
which is the recommended contract anyway: thresholds should be SEALED by
the caller before the run (`truerise.gates.seal_thresholds`).
