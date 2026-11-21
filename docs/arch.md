# Architecture (架构)

EN: TrueRise is a hexagonal (ports-and-adapters) runtime foundation. The core
domain logic lives in `truerise-core` with ZERO framework dependencies; every
external capability enters through one of the **four port traits** (`ModelRouter`
/ `SandboxProvider` / `MemoryBackend` / `ToolProvider`) plus the platform boundary
(`truerise-platform` — the ONLY crate allowed `unsafe`, per-block audited).
`ToolProvider` + `ActionPolicy` landed via RFC 0002: every tool request goes
decide -> execute -> audit (capability from the executor's return value);
denial is an audited outcome, and a run without wired hooks executes nothing.
Governance (audit hash chain, policy, approvals) sits as a boundary
implementation: the core defines the `AuditSink` PORT, governance implements
it — no core→governance dependency (the cycle was deliberately broken).
Judging authority lives in `truerise-eval` and NOTHING else may emit
verdicts; sandbox self-checks produce facts only.

CR: TrueRise 采用六边形（端口-适配器）架构：核心域逻辑零框架依赖；一切外部能力
经**四个端口 trait**（ModelRouter / SandboxProvider / MemoryBackend /
ToolProvider）+ 唯一平台边界（truerise-platform，唯一允许 unsafe 的 crate，
逐块 SAFETY 审计）。`ToolProvider` + `ActionPolicy` 经 RFC 0002 落地：每个工具
请求走 判定 -> 执行 -> 审计（能力等级取执行器返回值）；拒绝是可审计结果；未接
钩子的 run 不执行任何东西。治理（审计哈希链/policy/审批）为边界实现：核心只定义
AuditSink 端口，治理在外圈实现——刻意切断 core→governance 环。判定权威只在
truerise-eval；沙箱自检只产事实。

Crate map (crate 地图):

| crate | 职责 | 依赖 |
|---|---|---|
| truerise-platform | 平台边界：单调时钟/文件锁/进程面 | (none) |
| truerise-wire | 版本化 wire 契约（v1/v2 双版显式） | serde |
| truerise-core | 错误树/四态/typestate 状态机/时间片/loop/compaction | thiserror |
| truerise-governance | 审计哈希链/policy 三态/审批 fenced/删除三关口/持久状态写闸 G29（四路一门） | core, wire, platform, sha2 |
| truerise-sandbox | SandboxProvider port/capability/conformance self-check + RFC 0002 工具执行器 | core, thiserror |
| truerise-memory | BM25+recency+importance 三路融合/Backend port | serde |
| truerise-eval | conformance 判定权威/棘轮/指纹/一致性统计/反作弊判卷/因果完整性基底 CA1-CA5/过程判级 G10 | governance, sandbox, wire |
| truerise-rsi | 预算单例/skill schema/evolve 永不合并 | core |
| truerise-hats | 帽 schema+TOML 加载+content_hash 入链 | governance, wire |
| truerise-mcp | JSON-RPC 2.0 帧/零暴露 server/写拒 client | serde |
| truerise-observe | jsonl span 导出/脱敏/单向禁回灌/G9 轨迹逐步签名 | governance, sha2 |
| truerise-cli | 单二进制：demo/run/repl/onboarding + RFC 0002 装配层 | core, governance, wire, sandbox |
| truerise-py | PyO3 绑定壳：SDK 的可选原生加速器（同一契约、同一批门） | governance, eval, wire |

Three books (三账): every governed run populates the transcript, the audit
chain and the state-transition log simultaneously; the golden test asserts
the repl adapter and the direct path produce identical books.
三账：每次受治理 run 同时产出 transcript/审计链/状态转移流；golden 测试断言
repl 适配层与直连路径产出完全一致。
