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

## 60 seconds

```bash
cargo test --workspace                  # 153 Rust tests, zero warnings
python -m pip install -e python/
python -m pytest python/tests -q        # 40 tests
python -m truerise evals demo           # honest PASS, cheater FAIL, mutator FAIL
python -m truerise demo --replay        # offline recomputation of a verdict
cargo run -p truerise-cli -- demo       # the same walkthrough in Rust
python tools/check_dual_impl.py         # cross-certification: Rust == Python, or red
python -m truerise.demo --replay        # the SAME walkthrough replayed offline
cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json
                                        # one RSI generation, recomputed OFFLINE
```

## The pain points, and the command that proves each catch

| Evaluation failure | What catches it | Verify in one command |
|---|---|---|
| best-of-N reporting with a hidden denominator | runner-counted monotonic trial ledger; the corrected bar = base/N is printed, then enforced | `python -m truerise evals run --cheat --trials 20 --strategy majority` (FAIL: `corrected significance 2.5e-03`) |
| thresholds picked after the results | thresholds sealed (SHA-256) before the run; the claim map is diffed against the seal, offending keys named | `python -m truerise evals run --mutate` (FAIL: `prereg diff ... keys: min_accuracy, claim_p`) |
| model self-report as evidence | evidence ladder L0-L6; records without an executor signature contribute NOTHING | `python -m truerise demo` (reason 4: `0 executor-signed records`) |
| cross-environment score comparison | four-part fingerprint; comparing across registries is REFUSED, self-vs-self included | `python -m truerise demo --drop-fingerprint` (integrity line appears) |
| verdicts that cannot be re-checked | hash-chained ledger; `--replay` recomputes offline, tamper fails closed with row location | `python -m truerise demo --replay` |
| score regression merged silently | ratchet only tightens; regression -> non-zero exit | `cargo test -p truerise-eval` (9 tests) |
| a verdict that cannot be recorded | unrecorded verdicts are ERRORS, not warnings | `cargo test -p truerise-eval` (`judge_and_record`) |
| reward hacking: the evaluated party edits the evaluator | conformance judging authority + sandbox must-fail probes (escape / oversell / residue) | `cargo test -p truerise-sandbox` |
| dual implementations drifting apart | cross-certification gate: same fingerprint, same verdict, field-for-field | `python tools/check_dual_impl.py` |

The full bench protocol (pack format, sealed taxonomy, isolation model,
certificate schema) is specified in [`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md);
the certificate schema is open for review as
[RFC 0001](docs/rfc/0001-claim-certificate.md). Start from
[`docs/README.md`](docs/README.md) (中文：[`docs/README.zh-CN.md`](docs/README.zh-CN.md)).

## What is in the box

| Piece | Scope | License |
|---|---|---|
| `truerise` (Python) | the 60-second demo + `truerise.evals` Anti-Cheat Bench + the zero-dependency SDK (`truerise.sdk`) + long-horizon closed-loop packs + the external-solver protocol | Apache-2.0 / MIT |
| `truerise` (Rust, 13 crates) | contracts and judging authority: hash-chain audit, three-state policy + sandbox port + tool execution (RFC 0002), conformance judge, ratchet, the RSI closed loop (RFC 0003), MCP (zero exposure), observe, the PyO3 binding shell | Apache-2.0 / MIT |
| `truerise-native` (PyO3 wheel) | OPTIONAL accelerator over the same crates; the pure-Python SDK is the complete fallback | Apache-2.0 / MIT |
| task packs (data) | generated fixtures + sealed answers | CC-BY-4.0 (`LICENSE-DATA.md`) |

## Reference readings (committed, recomputable)

| Run | Verdict | Artifact |
|---|---|---|
| honest solver, 1 trial | PASS, accuracy 100% (120/120) | [`readings/v0.1/honest_verdict.json`](readings/v0.1/honest_verdict.json) |
| cheater, 20 trials, majority | FAIL — hidden denominator caught | [`readings/v0.1/cheater_verdict.json`](readings/v0.1/cheater_verdict.json) |
| mutator | FAIL — threshold mutation caught | [`readings/v0.1/mutator_verdict.json`](readings/v0.1/mutator_verdict.json) |

Technical report: [`docs/report/anti_cheat_bench_v0.1.md`](docs/report/anti_cheat_bench_v0.1.md).

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

M0-M5 + P0-P3 complete (spec-mapped gates, 40 rows / 121 identifiers,
machine-checked; 153 Rust + 40 Python tests). The wave-by-wave evidence
lives in `CHANGELOG.md`; the forward plan in `ROADMAP.md`.

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
