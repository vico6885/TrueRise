# providers（提供方）

[English](README.md) | [简体中文](README.zh-CN.md)

第三方提供方住在**它们自己的仓库里**。本目录只放端口 trait 与判定实现的
一致性套件：

| 端口 | 定义位置 | 由什么判 |
|---|---|---|
| `SandboxProvider` | `crates/truerise-sandbox/src/provider.rs` | 三区必失探针（逃逸 / 超卖 / 残留）+ `truerise-eval` 判词 |
| `ModelRouter` | `crates/truerise-core/src/router.rs` | 回合钉定：来自非钉定模型的响应使整回合作废（`foreign_model_voids_the_round`） |
| `MemoryBackend` | `crates/truerise-memory/src/lib.rs` | 融合权重、确定性与同分破平测试 |
| `ToolProvider` + `ActionPolicy` | `crates/truerise-core/src/tool.rs`（RFC 0002） | `crates/truerise-cli/tests/tool_gate.rs`：Deny=审计性拒绝零副作用；Ask=批准须落盘；能力等级=执行器返回值；围栏逃逸=审计性拒绝 |
| `ResourcePolicy` | `crates/truerise-governance/src/resource_policy.rs` | 三道闸按序；未知策略一律拒绝 |

> **第四端口已经 RFC 0002 落地**（`docs/rfc/0002-tool-execution-port.md`）。
> `ToolProvider` 定义在 `truerise-core`，由带围栏的 `SandboxToolProvider`
> （`truerise-sandbox`）实现，在边界装配（`crates/truerise-cli/src/assembly.rs`）
> 并受 `ActionPolicy` 三态判定约束。未接钩子的 run 不执行任何东西（缺省拒）。
> 帽的 `tool_subset` 与 `truerise-mcp` 仍按原语义承担工具面的过滤与对外暴露。

要新增提供方，请按 [`../docs/provider_conformance.zh-CN.md`](../docs/provider_conformance.zh-CN.md)：
实现端口、跑 `self_check`、公开报告、开 issue。

提供方永远不能自称达标 —— 自检只产事实，判定权只在 `truerise-eval`。
