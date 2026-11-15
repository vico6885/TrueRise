# Architecture (架构)

EN: TrueRise is a hexagonal (ports-and-adapters) runtime foundation. The core
domain logic lives in `truerise-core` with ZERO framework dependencies; every
external capability enters through one of **three port traits** (`ModelRouter`
/ `SandboxProvider` / `MemoryBackend`) plus the platform boundary
(`truerise-platform` — the ONLY crate allowed `unsafe`, per-block audited).
Tool exposure is deliberately NOT a port trait today: it is carried by the
hat's `tool_subset` (data, `truerise-hats`), the three-state policy
(`Auto` / `Ask` / `Deny`, approval required on file) and `truerise-mcp`
(zero-exposure server + write-class hard denial). The construction spec names a
fourth port, `ToolProvider`; that divergence is registered in
`providers/README.md`, and this page states the implemented contract rather
than the spec's intent.
Governance (audit hash chain, policy, approvals) sits as a boundary
implementation: the core defines the `AuditSink` PORT, governance implements
it — no core→governance dependency (the cycle was deliberately broken).
Judging authority lives in `truerise-eval` and NOTHING else may emit
verdicts; sandbox self-checks produce facts only.

CR: TrueRise 采用六边形（端口-适配器）架构：核心域逻辑零框架依赖；一切外部能力
经**三个端口 trait**（ModelRouter / SandboxProvider / MemoryBackend）+ 唯一平台
边界（truerise-platform，唯一允许 unsafe 的 crate，逐块 SAFETY 审计）。工具暴露
目前刻意不是端口 trait，而由三样东西承担：帽里的 `tool_subset`（数据，
truerise-hats）、三态策略（`Auto`/`Ask`/`Deny`，批准须落盘）、`truerise-mcp`
（默认零暴露 + 写类硬拒）。施工规格点了第四个端口 `ToolProvider`，该差异已登记在
`providers/README.md`；本页陈述的是**已实现契约**，不是规格意图。治理（审计哈希
链/policy/审批）为边界实现：核心只定义 AuditSink 端口，治理在外圈实现——刻意切断
core→governance 环。判定权威只在 truerise-eval；沙箱自检只产事实。

Crate map (crate 地图):

| crate | 职责 | 依赖 |
|---|---|---|
| truerise-platform | 平台边界：单调时钟/文件锁/进程面 | (none) |
| truerise-wire | 版本化 wire 契约（v1/v2 双版显式） | serde |
| truerise-core | 错误树/四态/typestate 状态机/时间片/loop/compaction | thiserror |
| truerise-governance | 审计哈希链/policy 三态/审批 fenced/删除三关口 | core, wire, platform, sha2 |
| truerise-sandbox | SandboxProvider port/capability/conformance self-check | thiserror |
| truerise-memory | BM25+recency+importance 三路融合/Backend port | serde |
| truerise-eval | conformance 判定权威/棘轮/指纹/一致性统计 | governance, sandbox, wire |
| truerise-rsi | 预算单例/skill schema/evolve 永不合并 | core |
| truerise-hats | 帽 schema+TOML 加载+content_hash 入链 | governance, wire |
| truerise-mcp | JSON-RPC 2.0 帧/零暴露 server/写拒 client | serde |
| truerise-observe | jsonl span 导出/脱敏/单向禁回灌 | governance, sha2 |
| truerise-cli | 单二进制：demo/run/repl/onboarding（IO 壳） | core, governance, wire |

Three books (三账): every governed run populates the transcript, the audit
chain and the state-transition log simultaneously; the golden test asserts
the repl adapter and the direct path produce identical books.
三账：每次受治理 run 同时产出 transcript/审计链/状态转移流；golden 测试断言
repl 适配层与直连路径产出完全一致。
