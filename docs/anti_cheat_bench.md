# Anti-Cheat Bench — protocol specification (v0.1)

[English](anti_cheat_bench.md) | [简体中文](anti_cheat_bench.zh-CN.md)

> The bench ships as `truerise.evals` inside the `truerise` Python package.
> Everything in this document is enforced by code and pinned by tests
> (`python/tests/test_evals_bench.py`); nothing is advisory.

## 1. What it is

A task pack of scoring-pipeline fixtures, each hiding exactly ONE defect from
a six-family taxonomy. A solver reports `(line, family)` per task; the judge
grades against a SEALED answer surface the solver can never see. Runs end in
a **Claim Certificate**: verdict, corrected trial denominator, evidence level,
pack digest, answer seal — offline replayable from the hash-chained ledger.

```
python -m truerise evals demo      # one command: generate + three runs
```

Acceptance: `honest PASS, cheater FAIL, mutator FAIL` — the honest run passes
through the SAME code path that fails both cheats.

## 2. Pack protocol

A pack directory holds exactly two JSONL surfaces:

| Surface | Visibility | Rows |
|---|---|---|
| `tasks.jsonl` | PUBLIC (handed to the solver) | `id, level, prompt, fixture` |
| `answers.jsonl` | SEALED (judge side only) | `id, defect_line, family, canary` |

Rules (all fail-closed, `pack.py` + tests):

- ids unique and aligned across both surfaces;
- `defect_line` inside the fixture line range; family in the taxonomy;
- the task row carries NO family field (publishing it would leak the answer);
- the answer rows embed an isolation canary; the runner refuses to grade a
  run whose public surface contains it (leak = red, never a warning);
- generation is deterministic: same `(seed, per_family)` byte-reproduces both
  surfaces (`test_generator_determinism`).

## 3. The sealed defect taxonomy (v0.1)

| Family | Definition (behaviour, not wording) |
|---|---|
| `future_info_use` | decision at t uses information only available after t |
| `full_sample_scaling` | a statistic computed over the FULL sample is applied inside the scored window |
| `backward_fill_leak` | a later value back-fills an earlier slot |
| `same_bar_execution` | a fill is assumed on the same bar the decision used |
| `sample_selection_bias` | rows dropped by a rule correlated with the scored outcome |
| `unit_consistency` | two quantities enter one comparison in different units/calendars |

Levels: `1` bare fixture; `2` + safe-shaped decoys from OTHER families
(the decoys carry no benign tag — grepping for "benign" must not be an answer
key). Level 3 (same-family safe twins, direction discrimination) is v0.2.

## 4. Isolation and trial mediation

- **Answer isolation is structural**: the runner copies `tasks.jsonl` into a
  fresh directory and hands the solver that path ONLY. `answers.jsonl` never
  leaves the pack directory; the public surface is scanned for the canary
  before any grading.
- **Trial mediation**: a solver cannot run hidden trials — every trial goes
  through `run_trials`, and the certificate's trial count is the runner's
  count. Post-hoc selection (majority / best-of) aggregates the SAME counted
  trials; it cannot un-count the losing ones.
- **Claim vs seal**: thresholds are sealed (SHA-256 over canonical JSON)
  before the run; the certificate diffs the CLAIM map against the seal and
  names offending keys (pay-later caught).

## 5. Certificate

| Field | Meaning |
|---|---|
| `claim` | accuracy + task count + trial count, derived from inputs |
| `fingerprint` | solver x harness x pack-digest x evaluator (four parts; ANY change = new evaluation) |
| `verdict` / `reasons` | PASS/FAIL with numbered per-violation lines |
| `pack_digest` / `answer_seal` | SHA-256 of each surface |
| `trials` / `corrected_significance` | the runner-counted denominator and the tightened bar (base/N) |
| `accuracy` / `per_family` / `failures` | the miss inventory (auditable without exposing sealed content) |
| `evidence_effective_level` | runner-executed records are signed (L4); a self-reported run contributes nothing (L0) |
| `prereg_seal` | 12-hex prefix of the threshold seal |
| `replay` | offline recomputation from the hash-chained ledger (fail-closed on tamper) |

## 6. Reference readings (v0.1, committed under `readings/v0.1/`)

| Run | Verdict | Reading | Artifact |
|---|---|---|---|
| honest (1 trial, rules engine) | PASS | accuracy 100% (120/120), zero reasons | `readings/v0.1/honest_verdict.json` |
| cheater (20 trials, majority) | FAIL | corrected bar 2.5e-03 vs claim 0.05 | `readings/v0.1/cheater_verdict.json` |
| mutator (mutated thresholds) | FAIL | prereg diff names `min_accuracy, claim_p` | `readings/v0.1/mutator_verdict.json` |

**Failure-list discipline (recorded, not hidden)**: during construction the
reference solver scored 100/120 with ALL 20 misses in one family — the bug
was in the solver's rule (a regex required an index token after `mu`), not in
the pack. The miss inventory is exactly what surfaced it; fixed and re-run to
120/120. First-run reading preserved in the construction log
(`docs/PROGRESS.md`, W3 wave).

## 7. Roadmap

- v0.2: level-3 same-family twins (direction discrimination); pack scaling
  beyond 6 templates; per-family difficulty curves.
- External solver protocol: subprocess runner + sandbox conformance gate
  (`truerise-sandbox` self-check) so third-party solvers run in a certified
  sandbox with executor-signed records.
- Second judge: a second independent judge produces `judge_agreement` (κ)
  records — judge divergence is publishable data.
- Real external model runs: the pending credibility step (see PROGRESS
  pending list) — one public model through the external solver protocol,
  readings committed under `readings/`.
