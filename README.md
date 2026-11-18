# TrueRise — catch agent benchmark cheating, mechanically.

[English](README.md) | [简体中文](README.zh-CN.md)

[![License: Apache-2.0 | MIT](https://img.shields.io/badge/license-Apache--2.0%20%7C%20MIT-blue)](#status)
[![DCO](https://img.shields.io/badge/DCO-signed-all%20commits-green)](CONTRIBUTING.md)
[![status](https://img.shields.io/badge/status-pre--release-orange)](#status)
<!-- CI / crates.io badges are ready to add on publish (RELEASING.md items A & F) -->

"Our agent improved accuracy from 60% → 90%."

Cool. Which model? Which task pack? Was the pass threshold picked *before*
seeing the results — or after? Did the agent run the eval 20 times while
only the best attempt made it into the slide?

TrueRise turns an eval claim into a certificate a stranger can recompute
offline — and the five classic cheats are caught by machinery, not by
reviewers:

```text
$ python -m truerise evals run --cheat --trials 20 --strategy majority
CLAIM     : defect detection accuracy 100% on 120 tasks (trials=20)
VERDICT   : FAIL
            2) trials=20 -> corrected significance 2.5e-03, claim p=0.05 does not clear the corrected bar
```

*Excerpt (FINGERPRINT, ACCURACY, CERTIFICATE lines omitted). The bar is
mechanical: 0.05 / 20 trials = 2.5e-03 — a best-of-20 report cannot clear
it, even at 100% accuracy.*

Pre-release: install from a clone of this repository.

```bash
pip install -e python/
truerise evals demo        # honest PASS, cheater FAIL, threshold-mutator FAIL
```

## Why this exists

Agent benchmark numbers have become decision currency — model cards,
leaderboards, procurement decks. But the harness that produces them is
trusted, not audited: thresholds can move after the fact, retries can hide
in a slide deck, and "our harness said so" is the whole proof. TrueRise is
the missing audit layer: it wraps the evaluation, seals the judgment rules
before the run, and turns the result into a certificate that recomputes
offline — the same way a checksum works for a download, but for a claim
about an agent.

## Who needs it

| If you are... | ...you have been burned by | The catch in one command |
|---|---|---|
| building an **agent harness / eval framework** | graders being gamed, "done" claims that are placeholders, PASS/FAIL theater | `python -m truerise evals demo` |
| running an **RSI (self-improving) loop** | unverified assertions becoming the next generation's foundations, unaudited "impossible" give-ups | `evolve --replay` recomputes a generation offline ([quickstart §9](docs/quickstart.md)) |
| doing **Agentic RL / training** | proxy metrics gamed after the fact, one lucky trial reported as an improvement | `python -m truerise evals run --cheat --trials 20 --strategy majority` (FAIL) |
| evaluating **pretraining / post-training checkpoints** | scores that shift with the environment, silent regressions between checkpoints | `python -m truerise demo --drop-fingerprint` |

## The five classic cheats, and the catch

| Cheat | What catches it |
|---|---|
| hide the denominator (best-of-N reporting) | a monotonic trial ledger prints the corrected bar base/N, then enforces it |
| pick thresholds after the results | thresholds sealed (SHA-256) before the run; post-hoc edits are named, key by key |
| claim credit from model self-report | evidence ladder L0–L6: an unsigned self-report contributes NOTHING |
| compare across environments | four-part fingerprints (model × harness × pack × environment) refuse it |
| edit the evaluator itself | conformance judging authority + sandbox must-fail probes; the ledger locates tampering to the row |

Every row above is a named test, not a paragraph of adjectives. The full protocol lives in
[`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md).

## How it fits the tools you already use

TrueRise is not a replacement for your eval framework — it is the audit
layer around one:

| You already use | It gives you | TrueRise adds |
|---|---|---|
| `inspect_ai` / `lm-eval-harness` / OpenAI Evals | eval runs and scores | sealed judgment rules, corrected denominators, exact paired stats, and a recomputable certificate for the result |
| `promptfoo` / LangSmith | prompt iteration and traces | a hash-chained audit trail where tampering fails closed with row location |
| LangGraph / OpenAI Agents SDK / your own loop | the agent | governed tool execution, budget enforcement, and per-run completion certificates (`truerise.sdk.Session`) |

Integration is a contract, not a shim: any harness feeds the bench through
a one-file solver protocol (`TRUERISE_PUBLIC_DIR` in, `submission.jsonl`
out) — [docs/integrations.md](docs/integrations.md). We deliberately ship
no per-framework adapters.

## What a stranger can verify — and what the harness refuses

- **Completion you can check.** `run --intent` binds a run to a sealed
  contract: acceptance items listed up front, confirmed per item (a blanket
  "I read it" token is structurally impossible), and the completion claim
  must map every item to evidence, `not_done` (with a reason), or
  `not_verified` (with the cost of checking it). On your side,
  `verify --bundle` re-checks the machine items on your machine — no
  repository, keys, or network.
- **Honesty at both extremes.** A run that proves nothing gets
  `INCONCLUSIVE` (exit code 3 — never readable as PASS or FAIL). A give-up
  is audited like a claim: it needs a search-surface ledger and a sealed
  budget bound, or it is refused. "Impossible" may only mean "not found
  within the declared budget".
- **Reproducibility as a format.** Model steps are pinned and taped; the
  same tape replays to identical digests offline; committed readings in
  [`readings/`](readings/v0.2/gen1_certificate.json) recompute from the
  ledger alone.
- **An RSI generation, certified.** propose → hash-bound approval → apply →
  re-evaluate → ratchet → certificate, with rollback on regression — and a
  contract change mid-run voids the verdict, on the chain.
- **The README you are reading is inside its own enforcement surface**: the
  long-form name's verifiable nouns are machine-checked against evidence
  (`tools/check_claim_terms.py`: every "met" row = command + named test +
  committed artifact), promise wording is graded (an outcome the machinery
  can only reduce is refused in wording that claims it never happens), and
  stubs as well as test strength are scanned.

## Install and run (pre-release)

```bash
git clone <this repository> && cd TrueRise   # repository URL pending the org decision
pip install -e python/                       # pure Python, zero runtime deps
python -m truerise evals demo                # the three-verdict walkthrough
python -m truerise demo                      # the 60-second walkthrough: three cheats, one verdict
python -m truerise demo --replay             # recompute a verdict offline
cargo test --workspace                       # the full Rust suite
```

PyPI / crates.io names are pending (`RELEASING.md` item F); until then this
is a source install.

## Under the hood

<details>
<summary><b>The seven feature groups, in detail</b></summary>

1. **Anti-cheat bench.** Task packs with canaries and decoys; the public
   surface provably carries no sealed answers; thresholds sealed before the
   run; a monotonic trial ledger with a corrected denominator; exact paired
   significance (McNemar) with an equal-budget hard gate; an evidence ladder
   L0–L6 where unsigned self-reports contribute nothing.
2. **Verifiable completion.** The intent contract (`schemas/intent_contract.v1.schema.json`)
   with per-item adjudication, the three-way completion ledger
   (done / not_done / not_verified), a stub scanner, a reverse-mutation test
   gate, and a self-service verification bundle.
3. **Certified RSI generation.** The closed loop of RFC 0003 with a signed,
   offline-recomputable certificate and a memory-contamination gate
   (recall-enabled runs are excluded from baselines).
4. **Honesty at both extremes.** The `INCONCLUSIVE` verdict and the bounded
   give-up audit (three-valued outcome; a search-surface ledger; a retry
   plan disjoint from the tried set).
5. **Reproducibility as a format.** Tape record/replay, the float rule on
   chain rows, canonical JSON byte-stable across the Rust and Python
   implementations (tested byte-for-byte), four-part fingerprints.
6. **Audited runtime foundation.** Three-state policy, sandboxed tool
   execution (RFC 0002), an unshardable budget denominator, a tamper-
   locating hash chain, a zero-exposure MCP server, one-way OpenTelemetry
   export.
7. **The self-auditing gate set.** 15 gate scripts behind 15 CI jobs; a
   promise-grade registry (A enforceable / B measurable / C reducible-only)
   with the gate refusing grade-C outcomes written as grade-A promises; a
   stub gate; a test-strength gate; 54 spec rows binding 209 named
   identifiers.

</details>

<details>
<summary><b>Limitations (read before citing)</b></summary>

- **No external model run yet.** Every reading comes from our own reference
  solvers; the external-solver protocol has landed, the operational step is
  pending (`ROADMAP.md` item 1). Treat the bench as a harness demo, not a
  leaderboard.
- **The internal production deployment is not in this repository** (license
  isolation, by design).
- **Solo project**, one DCO-signed author. **No RL training results** — the
  runtime exposes interfaces a training loop can consume; we claim no
  training-loop outcome.

</details>

## Next steps

- Run `python -m truerise demo` above — it needs no API key and no network.
- Read the bench protocol: [`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md) ·
  the tutorial: [`docs/quickstart.md`](docs/quickstart.md) ·
  the command reference: [`docs/cli.md`](docs/cli.md) ·
  what we refuse to promise: [`docs/limits.md`](docs/limits.md).
- Contribute a task pack (data, CC-BY-4.0): the guide is
  [`docs/task_pack_guide.md`](docs/task_pack_guide.md) — packs are the
  easiest first contribution and go through the same gates.
- Review an RFC: [`docs/rfc/README.md`](docs/rfc/README.md) is where
  external review starts.

## Status

Pre-release (`0.2.0-dev.0`). 13 Rust crates + a zero-dependency Python SDK
(+ an optional PyO3 accelerator), 15 gate scripts behind 15 CI jobs, every
commit DCO-signed. The long-form name this repository earns once the
evidence gates stay green: **Harness Governance for Verifiable Recursive
Self-Improvement** — every verifiable noun in that name is CI-checked
(`tools/check_claim_terms.py`) — (中文：**智能体声明的评测诚信层**). Wave-by-wave evidence in
`CHANGELOG.md`; forward plan in `ROADMAP.md`; security policy in
`SECURITY.md`; license: Apache-2.0 / MIT for code, CC-BY-4.0 for task-pack
data (`LICENSE-DATA.md`).
