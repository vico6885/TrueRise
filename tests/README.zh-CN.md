# 测试

[English](README.md) | [简体中文](README.zh-CN.md)

两套测试，都在 CI 里跑，都不允许跳过。

## Rust —— 101 个测试（`cargo test --workspace`）

| 面 | 钉住什么 |
|---|---|
| `truerise-core/tests/leakguard.rs` | 四个泄漏向量，其中两个在 proptest 下；建仓期抓出两处真实缺陷 |
| `truerise-core/tests/terminal_absorb.rs` + `compile_fail/` | 终态转移编译不过 —— 吸收性质是类型级性质 |
| `truerise-governance/tests/chain_gate.rs` | 双写者竞争（2 句柄、1000 次追加、0 丢失）、篡改按行定位（内容哈希与 prev 哈希两类）、幂等重放、持久化重载、legacy 行只读不治 |
| `truerise-cli/tests/golden_repl.rs` | REPL 适配层与直连循环路径产出逐字段相同的三账 |
| `truerise-cli/tests/exit_codes.rs` | 退出码承载判词（FAIL → 1、诚实 → 0、未知命令 → 2）—— 变不了红的闸不叫闸 |
| 各 crate 内的单元测试 | 策略三态、审批永不超时转同意、删除三关、棘轮只收紧、跨环境比较拒绝、预算单例对账、evolve 永不合并、帽内容哈希入链、MCP 零暴露与写类硬拒、观测导出脱敏、记忆融合确定性、压缩单调性、退避可重试类别 |

逐行为映射由机器核对：`spec_map.json` 把 23 行行为规格绑到 67 个具名测试标识符，
某行失去映射或某标识符不再存在时 `python tools/lint_spec_map.py` 即失败。

## Python —— 25 个测试（`python -m pytest python/tests -q`）

| 文件 | 钉住什么 |
|---|---|
| `python/tests/test_demo_perturbation.py` | 判词随输入翻转、无写死内容、离线重算与已存证书一致 |
| `python/tests/test_evals_bench.py` | 题包规模与分类学、生成确定性、公开面不含封盘提示、哨兵泄漏失败关闭、判卷在两个轴上都精确、honest PASS / cheater FAIL / mutator FAIL、重算遇篡改失败关闭、零次试验被拒、取多数无法消掉输掉的试验、识别规则覆盖全部类别 |

标记：`timesafety`（永不跳过）、`unit`、`integration`、`slow`。

## 这里没测的东西

- 任何外部模型跑分 —— 还没有（`README.zh-CN.md` 的「局限」段）。
- 进程内参考实现之外的沙箱提供方（`providers/README.zh-CN.md`）。
- criterion 基线的**比较**：CI 首跑只保存 `main` 基线，20% 回归闸从第二次运行起生效。

## 不属于测试的闸门

`tools/` 下是仓库闸门（违规一律非零退出）：工作区禁词门、历史对象门与 blob 地板、
单文件长度、许可隔离、schema 对齐、规格映射、DCO、时序安全强制声明、SBOM、
双实现互证、文档契约。
