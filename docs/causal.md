# The causal-integrity engine

[English](causal.md) | [简体中文](causal.zh-CN.md)

Agent claims increasingly speak causal language: "the tool made the agent
better", "this skill caused the regression", "the improvement holds across
environments". Correlation-grade evidence cannot carry those sentences —
but most eval harnesses have no way to say so. TrueRise ships a
**domain-neutral causal-integrity substrate** (`crates/truerise-eval/src/causal/`)
that grades WHAT KIND of claim the evidence can carry, prices the missing
paperwork, and refuses designs that skip it.

The honest boundary, stated once and enforced everywhere: these primitives
**price evidence and refuse designs; none of them proves causality**.
Identification assumptions remain judgements — the substrate forces them to
be declared, defends their arithmetic, and downgrades claims that skip the
paperwork.

## The six-value claim typology (CA1)

Every causal-flavored claim is typed into one of six values, from weakest
to strongest: correlational → causal-observational → causal-interventional
tiers, each with its own wording law. An association-grade claim never
speaks causal language; the claim gate (`tools/check_claim_terms.py`)
enforces the wording, the typology enforces the tier.

## The assumption ledger (CA2)

`assumption_ledger.json` (repo root) declares the identification
assumptions as machine-checkable rows — one row per assumption, with the
source class and the declared level. **No complete row, no causal tier**:
a causal claim without its paperwork is downgraded to `INCONCLUSIVE`, not
passed. The parser lives in `crates/truerise-eval/src/causal/ledger.rs`;
the ledger file itself is the working example.

## Identification arithmetic (CA3)

- **Manski no-assumption bounds** (`bounds`): the average treatment effect
  interval you get with NO identification assumptions — the honest floor
  any causal claim must clear.
- **E-value** (`evalue`): the confounding fragility price — how strong an
  unmeasured confounder would have to be to explain the observed effect.
- **Invariance screening** (`icp`): which relationships survive across
  environments (the pack-to-pack stability question).
- **Refuter battery** (`refute`): placebo outcomes, subset stability,
  confounder injection — each refuter is a named test.

## Experiment primitives (CA4)

Seeded, byte-reproducible randomization streams (`rng`), blocked / cluster /
crossover assignment designs (`design` — the SUTVA repair), randomization
inference for small studies (`permutation`), and interventional credit
assignment (`shapley`).

## Formal failure attribution (CA5)

Halpern–Pearl actual causation with responsibility scores, over binary
bounded models (`attribution`): when a run fails, the substrate answers
"WHICH component actually caused it" with a formal, recomputable verdict
instead of a prose guess.

## Quasi-experimental extensions

Where a controlled experiment is impossible, the substrate carries the
standard observational designs: difference-in-differences, regression
discontinuity, Wald IV, propensity matching, synthetic control, effect
modification, pairwise rating aggregation, and minimal-pair construction
(`quasi`, `pairwise`, `minimal_pair`, `adjust`, `synthetic`).

## How to consume it

- Every family has named tests: `cargo test -p truerise-eval causal` runs
  the substrate suite (also visible in `spec_map.json`, rows 因果宣称 /
  因果识别 / 因果归因).
- Write your assumptions into `assumption_ledger.json` — the parser refuses
  incomplete rows.
- Causal-tier wording goes through the same claim gate as every other
  promise: see [`limits.md`](limits.md) for the A/B/C promise grades and
  [`CHANGELOG.md`](../CHANGELOG.md) for the CA1–CA5 landing entry.

## Relationship to the rest of the system

- The claim typology feeds the **INCONCLUSIVE** downgrade (three-valued
  verdicts, README "Honesty at both extremes").
- The assumption ledger is consumed by the **write gate** (G29): an
  unverified assumption written into persistent state survives — visibly —
  into later certificates.
- The refuter battery is the evaluation-side counterpart of the sandbox's
  must-fail probes: one prices evidence, the other prices execution.
