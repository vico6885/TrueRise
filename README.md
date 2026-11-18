# TrueRise

[English](README.md) | [简体中文](README.zh-CN.md)

**Harness Governance for Verifiable Recursive Self-Improvement** — every
verifiable noun in that title is held to the evidence by a CI gate
(`tools/check_claim_terms.py`: command + named test + committed artifact,
5/5 currently met).

Concretely: **the integrity-evaluation layer for agentic claims** — a
task-pack bench where cheating is caught mechanically, a claim certificate
a stranger can recompute offline (model steps pinned AND taped), and a
Rust runtime foundation underneath it where one RSI generation runs end to
end: propose -> hash-bound human approval -> apply -> re-evaluate ->
ratchet -> rollback on regression, all recomputable offline.

> One sentence: every "our agent improved X%" claim should carry a
> certificate a third party can recompute. This repository ships the bench,
> the certificate format, and the runtime contracts that make that possible.

TrueRise —— **智能体声明的评测诚信层**：作弊被机器抓住的题包基准 +
可离线复算的声明证书 + 之下作为第二层证据的 Rust 运行时底座。

## Who is it for — five fronts, five pains

| If you are building... | ...you have been burned by | TrueRise gives you | Proof in one command |
|---|---|---|---|
| **an Agent harness / eval framework** | agents that game the grader, "done" claims that are 90% placeholder, PASS/FAIL theater | a bench where cheating is caught mechanically, a per-item completion ledger where silence is refused, and a three-valued verdict (`INCONCLUSIVE` exists) | `python -m truerise evals demo` |
| **an RSI (self-improving) loop** | unverified assertions quietly becoming the next generation's foundations, "impossible" give-ups nobody audits | generation certificates with hash-bound human approval, a give-up audit where surrender needs a search-surface ledger, and a memory-contamination gate | `cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json` |
| **Agentic RL / training pipelines** | proxy metrics gamed after the fact, sample budgets wasted on noise, "improvements" that are one lucky trial | thresholds sealed before the run, a monotonic trial ledger with a corrected denominator, exact paired significance (McNemar), and equal-budget enforcement | `python -m truerise evals run --cheat --trials 20 --strategy majority` (FAIL) |
| **pretraining evaluation** | task packs leaking into training data, checkpoint scores that shift with the environment | four-part fingerprints (model x harness x pack x environment) that refuse cross-environment comparison, byte-reproducible replay, and canaries/decoys in the pack format | `python -m truerise demo --drop-fingerprint` (integrity line appears) |
| **post-training / checkpoint gates** | silent regressions merged between checkpoints, "improvements" nobody can re-derive | a ratchet that only tightens, checkpoint certificates, offline recomputation, and a two-implementation cross-certification gate | `python tools/check_dual_impl.py` |

One honest note up front: TrueRise does not train models. It is the
verification layer a training loop can consume without shooting itself in
the foot — and every mechanism below is pinned by a named test.

## The features, from most important down

**1. The Anti-Cheat Bench — cheating is caught mechanically, not by vibes.**
The classic cheats are wired to fail: report the best of 20 trials with a
hidden denominator (a monotonic trial ledger prints the corrected bar
base/N, then enforces it); pick thresholds after seeing the results
(thresholds are SHA-256-sealed before the run; the claim map is diffed
against the seal, offending keys named); claim credit without an
executor-signed record (evidence ladder L0-L6: self-report contributes
NOTHING); compare scores across environments (a four-part fingerprint
refuses it, self-vs-self included). Task packs carry canaries and decoys,
and the public surface provably carries no sealed answers.

**2. Completion you can verify — the harness does not say "done" on trust.**
`run --intent` binds a run to a sealed contract: acceptance items are
listed up front, each explicitly machine-checkable or human-judged, and the
user confirms with a PER-ITEM token (a blanket confirmation is structurally
impossible). A completion claim must map every item to evidence, `not_done`
(with a reason from a closed taxonomy), or `not_verified` (with the cost of
checking it) — a skipped item is a refusal; silence is never read as
coverage. On your side, `verify --bundle` re-checks the machine items on
YOUR machine with no repository, no keys, no network. A stub scanner and a
reverse-mutation gate keep the tests themselves honest: an unannounced stub
blocks completion, and a test that stays green when its subject is reverted
is flagged as having proved nothing.

**3. One RSI generation, certified end to end.**
`evolve --demo` runs propose -> hash-bound human approval -> apply ->
re-evaluate -> ratchet -> certificate, and rolls back on regression. The
certificate recomputes OFFLINE (`evolve --replay`), is signature-checked
(`verify --cert`; unsigned is "unverified issuer", never silent trust), and
a generation bound to an intent contract must carry its closing completion
ledger or the certificate is refused. Change the contract mid-run and the
verdict is voided, on the chain.

**4. Honesty at both extremes — not just "no lying about success".**
A run that proves nothing gets `INCONCLUSIVE` (its own exit code, 3 - never
readable as PASS or FAIL, never "no difference"). And a give-up is audited
like any claim: `giveup --ledger` requires a search-surface ledger (tried
probes with outcomes, untried ones with their cheapest concrete form and
cost), a sealed budget bound, and a retry plan disjoint from the tried set.
"Impossible" becomes "not found within the declared budget" - or the claim
is refused. The verdict space is exactly three-valued and cannot be
collapsed.

**5. Reproducibility as a format, not a promise.**
Model steps are pinned AND taped; the same tape replays to identical
digests offline. Chain rows ban floats (integers or strings only - the
float rule), canonical JSON is byte-stable across implementations, and
every committed reading (`readings/`) recomputes from the ledger alone -
no keys, no network, no trust.

**6. A runtime foundation that treats "audited" as code.**
The three-state policy (Deny / Ask / Auto - approvals never expire into
consent), a sandboxed tool-execution port (RFC 0002: escape / oversell /
residue must-fail probes), a budget whose global denominator cannot be
sharded, a hash-chain ledger where tampering fails closed WITH row
location, and an MCP server that is zero-exposure by default (empty
allowlist; every call lands on the chain).

**7. A gate set that audits itself.**
15 gate scripts behind 15 CI jobs: the README title is machine-checked
against the evidence; promise wording is graded (A enforceable / B
measurable / C reducible-only) and grade-C outcomes in grade-A wording are
refused; stubs and test-bite strength are scanned; and every behavior row
in `spec_map.json` (54 rows, 209 identifiers) resolves to a real named
test. The README you are reading is inside the enforcement surface.

## 60 seconds

```bash
cargo test --workspace                  # 240 Rust tests, zero warnings
python -m pip install -e python/
python -m pytest python/tests -q        # 40 tests
python -m truerise evals demo           # honest PASS, cheater FAIL, mutator FAIL
python -m truerise demo --replay        # offline recomputation of a verdict
cargo run -p truerise-cli -- demo       # the same walkthrough in Rust
python tools/check_dual_impl.py         # cross-certification: Rust == Python, or red
cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json
                                        # one RSI generation, recomputed OFFLINE
```

## The failure modes, and the command that proves each catch

| Evaluation failure | What catches it | Verify in one command |
|---|---|---|
| best-of-N reporting with a hidden denominator | runner-counted monotonic trial ledger; the corrected bar = base/N is printed, then enforced | `python -m truerise evals run --cheat --trials 20 --strategy majority` (FAIL: `corrected significance 2.5e-03`) |
| thresholds picked after the results | thresholds sealed (SHA-256) before the run; the claim map is diffed against the seal, offending keys named | `python -m truerise evals run --mutate` (FAIL: `prereg diff ... keys: min_accuracy, claim_p`) |
| model self-report as evidence | evidence ladder L0-L6; records without an executor signature contribute NOTHING | `python -m truerise demo` (reason 4: `0 executor-signed records`) |
| cross-environment score comparison | four-part fingerprint; comparing across registries is REFUSED, self-vs-self included | `python -m truerise demo --drop-fingerprint` (integrity line appears) |
| verdicts that cannot be re-checked | hash-chained ledger; `--replay` recomputes offline, tamper fails closed with row location | `python -m truerise demo --replay` |
| score regression merged silently | ratchet only tightens; regression -> non-zero exit | `cargo test -p truerise-eval` (35 tests) |
| a verdict that cannot be recorded | unrecorded verdicts are ERRORS, not warnings | `cargo test -p truerise-eval` (`judge_and_record`) |
| reward hacking: the evaluated party edits the evaluator | conformance judging authority + sandbox must-fail probes (escape / oversell / residue) | `cargo test -p truerise-sandbox` |
| dual implementations drifting apart | cross-certification gate: same fingerprint, same verdict, field-for-field | `python tools/check_dual_impl.py` |

The full bench protocol (pack format, sealed taxonomy, isolation model,
certificate schema) is specified in [`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md);
the certificate schema is open for review as
[RFC 0001](docs/rfc/0001-claim-certificate.md). Start from
[`docs/README.md`](docs/README.md) (中文：[`docs/README.zh-CN.md`](docs/README.zh-CN.md)).

## Built to plug into the agent ecosystem

TrueRise meets the Python agent world through three deliberate surfaces,
plus two transports:

- **MCP (tool supply side).** `truerise mcp --serve` mounts TrueRise-
  governed tools into ANY MCP-capable framework or editor - zero-exposure
  by default (empty allowlist), every call on the chain.
- **Python SDK (harness embedding side).** `truerise.sdk.Session` wraps any
  agent loop - LangGraph graphs, OpenAI Agents SDK runners, or a bare
  function - with budget caps, the hash-chained audit, the three-state
  policy and the ratchet. Two official patterns ship in
  [`examples/langgraph_harness.py`](examples/langgraph_harness.py) and
  [`examples/openai_agents_harness.py`](examples/openai_agents_harness.py),
  both import-guarded: they run without the third-party framework installed.
- **External-solver protocol (evaluation side).** Any solver honouring the
  one-file contract (`TRUERISE_PUBLIC_DIR` in, `submission.jsonl` out) can
  be graded by the bench - the runner executor-signs the records.
- **Model transports.** OpenAI-compatible endpoints (vLLM / Ollama /
  proxies) and the Anthropic Messages API behind the same governed port;
  the run pins to the requested model id and the provider's own echo backs
  the pinning check. Keys live in the environment, never argv or logs.
- **Observability.** One-way OpenTelemetry span export (the chain stays the
  authority); observability never mutates the audit source of truth.
- **Contracts for third-party implementers.** Six JSON Schemas
  (`schemas/`) define the wire shapes - intent contract, completion
  report, verify bundle, give-up claim, tool envelope, audit event - so a
  second implementation can be cross-certified instead of trusted.

## What is in the box

| Piece | Scope | License |
|---|---|---|
| `truerise` (Python) | the 60-second demo + `truerise.evals` Anti-Cheat Bench + the zero-dependency SDK (`truerise.sdk`) + long-horizon closed-loop packs + the external-solver protocol | Apache-2.0 / MIT |
| `truerise` (Rust, 13 crates) | contracts and judging authority: hash-chain audit, three-state policy + sandbox port + tool execution (RFC 0002), conformance judge, ratchet, the RSI closed loop (RFC 0003), MCP (zero exposure), observe, the PyO3 binding shell | Apache-2.0 / MIT |
| `truerise-native` (PyO3 wheel) | OPTIONAL accelerator over the same crates; the pure-Python SDK is the complete fallback | Apache-2.0 / MIT |
| task packs (data) | generated fixtures + sealed answers, with canaries and decoys | CC-BY-4.0 (`LICENSE-DATA.md`) |

## Reference readings (committed, recomputable)

| Run | Verdict | Artifact |
|---|---|---|
| honest solver, 1 trial | PASS, accuracy 100% (120/120) | [`readings/v0.1/honest_verdict.json`](readings/v0.1/honest_verdict.json) |
| cheater, 20 trials, majority | FAIL — hidden denominator caught | [`readings/v0.1/cheater_verdict.json`](readings/v0.1/cheater_verdict.json) |
| mutator | FAIL — threshold mutation caught | [`readings/v0.1/mutator_verdict.json`](readings/v0.1/mutator_verdict.json) |

Technical report: [`docs/report/anti_cheat_bench_v0.1.md`](docs/report/anti_cheat_bench_v0.1.md).
The boundary between what this framework can enforce, only measure, and
never promise is itself a documented artifact:
[`docs/limits.md`](docs/limits.md) (中文：[`docs/limits.zh-CN.md`](docs/limits.zh-CN.md)).

## Limitations (read before citing)

- **No external model run yet.** Every reading above comes from our own
  reference solvers. The external-solver protocol itself has landed
  (`run_external_trials`, executor-signed records); what is missing is the
  operational step — one public model behind a solver honouring the
  contract (`ROADMAP.md` item 1). Until that reading lands, treat the bench
  as a harness demo, not a leaderboard.
- **The internal production deployment is not in this repository.** The same
  contracts run in a real production system under a separate
  source-available license; it stays out of this workspace by design
  (license isolation, `tools/lint_deny_isolation.py`). We say so plainly
  rather than implying an adoption this repository cannot show.
- **Solo project.** One author, DCO-signed; the RFC process
  (`docs/rfc/README.md`) is where external review starts.
- **No RL training results.** The runtime exposes interfaces a training loop
  can consume safely; we claim no training-loop outcome.

## Status & governance

M0-M5 + P0-P3 + W1-W18 + the G-series complete (spec-mapped gates, 54 rows
/ 209 identifiers, machine-checked; 240 Rust + 40 Python tests; 72 DCO-
signed commits; 15 gate scripts behind 15 CI jobs). The wave-by-wave
evidence lives in `CHANGELOG.md`; the forward plan in `ROADMAP.md`.

- Security: see `SECURITY.md`. Every executed action flows through the
  sandbox port and lands on the audit chain with the executor's capability
  level (RFC 0002); secrets never enter the repository, argv, or logs.
- Contributing: DCO sign-off + IP-clean declarations
  (`CONTRIBUTING.md`); task-pack contributions are data-licensed
  CC-BY-4.0 (`LICENSE-DATA.md`).
- Repository topology and dependency direction: `docs/topology.md`.

## License

Code dual-licensed under **Apache-2.0 / MIT** (`LICENSE-APACHE`,
`LICENSE-MIT`); task packs CC-BY-4.0 (`LICENSE-DATA.md`). Contributions
require DCO sign-off plus the IP-clean declarations in `CONTRIBUTING.md`.
