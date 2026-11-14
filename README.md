# TrueRise

A general-purpose runtime foundation for **Agentic RL** and **next-generation
RSI (Recursive Self-Improvement) harness architectures** — integrity-evaluation
layer first · built-in RSI capability · single-core multi-hat · type-level
time-safety discipline · no vertical industry coupling.

> A Runtime Governance Layer for Long-Horizon RSI.

TrueRise —— 面向 **Agentic RL 与下一代 RSI（递归自我改进）Harness 架构**的通用底座
（评测诚信层首发 · RSI 能力内建 · 单核多帽 · 时序安全类型级纪律），无垂直行业色彩。

> 面向长程 RSI 的运行时治理层。

## Status

**Development in progress — not yet released.** The repository is built wave
by wave (M0-M5); a wave counts as done only when its spec-mapped gate passes
(see `docs/PROGRESS.md`).

| Wave | Scope | Status |
|---|---|---|
| M0 | repository scaffold, CI gates, leak-prevention double doors | **current** |
| M1 | wire + platform + core (modes / errors / state machine / timecore) | planned |
| M2 | governance (hash chain / policy / approvals) + sandbox | planned |
| M3 | cognitive loop + compaction + memory fusion | planned |
| M4 | eval (judging authority / ratchet / fingerprints) + rsi + hats | planned |
| M5 | mcp + observe + multi-platform distribution | planned |

## Crates

| Crate | Scope |
|---|---|
| `truerise-core` | Cognitive core: loop, mode switch, time slices, state machine, errors, compaction |
| `truerise-wire` | Versioned wire contracts (v1 legacy and v2 tightened, explicit at the type level) |
| `truerise-platform` | The only platform boundary: file locks, monotonic clocks, process trees |
| `truerise-governance` | Audit hash chain (singleton), three-state policy, approval queue, resource-policy port |
| `truerise-sandbox` | Sandbox provider port, lifecycle core, capability bits, conformance self-check |
| `truerise-eval` | Conformance judging authority, bench harness, ratchet, fingerprints |
| `truerise-memory` | Three-way fusion recall (BM25 + recency + importance, no vectors), memory backend port |
| `truerise-rsi` | evolve (never auto-merge), skill schema + loader, single budget ledger |
| `truerise-hats` | Hat schema + TOML loader (hats are data, not traits), preset hat packs |
| `truerise-mcp` | MCP server (zero exposure by default) / client (write actions hard-denied) |
| `truerise-observe` | Audit events to spans: jsonl (default) / OTLP behind the `otel` feature |
| `truerise-cli` | Single-binary entry: REPL / run / demo / onboarding wizard |

## Release forms

- **0.1-series** — integrity-evaluation CLI (Python single package first,
  60-second demo). Completed on the Python side.
- **0.2-series** — Rust core + Python bindings; dual implementations producing
  the same verdict = live conformance cross-certification.
- **0.3+** — full harness foundation (M0-M5 waves).

## Security & governance invariants

- All code execution goes through conformance-certified sandbox providers.
- The audit hash chain is the single authority; no component is exempt.
- `unsafe` is forbidden workspace-wide; CI denies it.
- Secrets never enter the repository, argv, or logs; keys go to the OS
  keyring or environment, never to config files in plain text.

## License

Dual-licensed under **Apache-2.0 / MIT** (see `LICENSE-APACHE`,
`LICENSE-MIT`). Contributions require DCO sign-off plus the IP-clean
declarations in `CONTRIBUTING.md`.
