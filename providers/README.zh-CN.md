# providers（提供方）

[English](README.md) | [简体中文](README.zh-CN.md)

第三方提供方住在**它们自己的仓库里**。本目录只放端口 trait 与判定实现的
一致性套件：

| 端口 | 定义位置 | 由什么判 |
|---|---|---|
| `SandboxProvider` | `crates/truerise-sandbox/src/provider.rs` | 三区必失探针（逃逸 / 超卖 / 残留）+ `truerise-eval` 判词 |
| `ModelRouter` | `crates/truerise-core/src/router.rs` | 回合钉定：来自非钉定模型的响应使整回合作废（`foreign_model_voids_the_round`） |
| `MemoryBackend` | `crates/truerise-memory/src/lib.rs` | 融合权重、确定性与同分破平测试 |
| `ResourcePolicy` | `crates/truerise-governance/src/resource_policy.rs` | 三道闸按序；未知策略一律拒绝 |

> **关于第四端口的定稿口径。** 施工规格点名了第四个端口 trait
> `ToolProvider`。本工作区实现的是**三个端口 trait + 一个策略端口**，工具暴露改由
> 三样已有机制承担：帽里的 `tool_subset`（数据，`crates/truerise-hats`）、
> 三态策略（`Auto`/`Ask`/`Deny`，批准须落盘）、`truerise-mcp`（默认零暴露 +
> 写类硬拒）。文档因此陈述**已实现契约**，不复述规格意图。
> 日后真要引入 `ToolProvider` trait，那是走 RFC 的**新增能力**
> （`docs/rfc/README.md`），不是改文档能解决的事。

要新增提供方，请按 [`../docs/provider_conformance.zh-CN.md`](../docs/provider_conformance.zh-CN.md)：
实现端口、跑 `self_check`、公开报告、开 issue。

提供方永远不能自称达标 —— 自检只产事实，判定权只在 `truerise-eval`。
