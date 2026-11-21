# 命令参考

[English](cli.md) | [简体中文](cli.zh-CN.md)

两个入口共享同一套行为契约。本页是权威清单：这里没有的开关就不存在。

## Python 入口 —— `truerise`

安装：`python -m pip install -e python/`（克隆态也可用 `PYTHONPATH=python/src` 直接跑）。

### `truerise demo` —— 诚信走查

三个作弊场景，各自独立可抓。

| 开关 | 缺省 | 效果 |
|---|---|---|
| `--perturb KEY=VALUE` | — | 翻转输入：`n_trials=10` 让校正分母移动，`claim_p=0.001` 让声明越过校正阈值 |
| `--honest-seal` | 关 | 阈值与封盘一致 → 预注册判词消失 |
| `--sign-records` | 关 | 记录带上执行器签名 → 证据判词消失 |
| `--honest` | 关 | 三个场景一起诚实 → `PASS` |
| `--drop-fingerprint` | 关 | 账本键标记为 dropped → 跨环境比较被拒绝 |
| `--out DIR` | `output/truerise` | 证书与账本目录 |
| `--replay` | 关 | 从哈希链账本离线重算 |

**退出码**：判词 `PASS` 为 `0`，`FAIL` 为 `1`（因此 CI 可以拿判词设闸），用法错误为 `2`。

### `truerise evals` —— Anti-Cheat Bench

| 子命令 | 开关 | 效果 |
|---|---|---|
| `evals gen` | `--out`（默认 `output/truerise_evals/pack`）、`--per-family`（20）、`--seed`（20261115） | 生成确定性题包；默认 6 类 × `--per-family` = 120 题 |
| `evals run` | `--pack`、`--trials`（1）、`--strategy`（`last`\|`majority`）、`--cheat`、`--mutate`、`--work`、`--out` | 用参考解题器跑题包并产出声明证书 |
| `evals demo` | `--out`、`--per-family`、`--seed` | 生成题包后依次跑 honest / cheater / mutator |

**`evals demo` 的退出码**：**仅当**验收契约成立才为 `0` ——
`{honest: PASS, cheater: FAIL, mutator: FAIL}`。任何其他结果都是 `1`。
也就是说验收判据是退出码，不是打印出来的文字。

**`evals run` 的退出码**：`PASS` 为 `0`，`FAIL` 为 `1`。

## Rust 入口 —— `truerise-cli`

```bash
cargo run -p truerise-cli -- <命令>
```

| 命令 | 效果 |
|---|---|
| `demo [--honest]` | 同一套三场景走查的 Rust 版 |
| `run --goal <文本>` | 一次完全受治理的运行：脚本化模型 + 经沙箱端口的工具执行（RFC 0002）+ 预算（W3）+ 由判定权威出具的判词（W4）。打印三账；退出码携带判词 |
| `run --intent <contract.toml>` | 以意图合同运行（G32）：合同先验证、再**封盘**（SHA-256 覆盖其 canonical JSON），封盘哈希以 `intent.seal` 行落链。合同目标取代 `--goal`。无验收项、未知条目类别、机检项少于 3 的合同以 `2` 退出拒绝（可运行，但永远不能作为完成依据） |
| `run --intent-token <token.json>` | 提交用户的**逐项裁定**令牌（每个验收项一条 accept/reject/edit，绑定封盘哈希）。跳过任一项或哈希不符的令牌以 `2` 退出拒绝。无令牌时横幅与链行如实标注 `adjudicated: no`——该合同不能作为完成依据 |
| `run --approve <tool>` | 在 run 之前把批准落盘（进入 ask-allowed 的唯一路径；可重复） |
| `run --budget <n>` | 每帽配额（轮次 + 工具单位）；超预算停止运行并以 `1` 退出 |
| `run --baseline <path>` | 判词用的棘轮基线（默认 `baselines/run_scores.json`，自 cwd 向上查找） |
| `run --hat <toml>` | 以帽运行：内容哈希入链，`tool_subset` 成为外层策略门；子集里的未知工具在加载期即拒绝运行 |
| `run --provider openai\|anthropic --model <id>` | 同一治理端口后接**真实模型**（预算/策略/判词不变）。`--provider-url <u>` 指向本地 vLLM/Ollama/代理。密钥只走环境变量（`TRUERISE_API_KEY` / `ANTHROPIC_API_KEY`）；缺钥即拒绝。运行钉定到模型 id，且 provider 自报的回显参与钉定校验 |
| `evolve --demo --approve <hash>` | 一代 RSI（RFC 0003）：提案 -> 哈希绑定批准 -> 应用 -> 重评 -> 棘轮 -> 证书；`--inject-regression` 演示回滚；`--tape <path>` 录制每次模型调用 |
| `evolve --demo --intent <toml> --completion <json>` | 把世代绑定到意图合同（G35/G36）：证书嵌入对封盘合同闭合的完成台账。合同没有台账即**拒绝发证**（退出 1）；replay 离线复算台账 |
| `evolve --replay --cert <path>` | 离线复算世代证书（退出 0 = 逐字节一致；被篡改的证书会被拒绝） |
| `verify --bundle <path> --intent <contract.toml>` | 自助验收包（G39）：每个机检验收项一条预期观测，覆盖面从合同封盘生成（漏一条即拒绝）。三桶报告——PASS / FAIL / NOT-VERIFIED（显式转送，绝不沉默）；全部 PASS 才退出 `0`。在你自己的机器上即可运行，无需本仓 |
| `verify --cert <path> [--issuer-key <hex>]` | 校验证书签名（ed25519）；未签名的证书报告 `unverified issuer` 并以 `1` 退出——绝不静默当真 |
| `mcp --serve [--chain <dir>]` | 经 stdio 起默认零暴露的 MCP server；每次调用都落链 |
| `giveup --ledger <claim.json> [--intent <toml>]` | 记录一个**有界**放弃声明（G44）：搜索面账单、封盘成本上界、与已试集**不相交**的复核计划。无界或偷懒式放弃以 `2` 退出拒绝；判词空间严格三值（completed / not_completed / infeasible_within_budget），不可压缩。有界复核本体待预算联动（G2/G21）后落地 |
| `repl` | 交互壳；斜杠命令 `/demo`、`/audit`、`/quit`、`/exit` |
| `onboarding` | 首次运行向导；写 `config.toml`，**永不落任何密钥**（只走环境变量或系统钥匙串） |
| `help`、`--help`、`-h` | 子命令清单 |

不给任何参数即进入 `repl`。未知命令打印帮助并以 `2` 退出。
签名/校验的密钥只走环境变量（`TRUERISE_SIGNING_KEY` / `TRUERISE_TRUSTED_KEY`）；
provider 密钥同理（`TRUERISE_API_KEY`）。

### 退出码是被互证锁住的契约的一部分

| 命令 | Rust | Python |
|---|---|---|
| `demo`（作弊输入） | `1` | `1` |
| `demo --honest` | `0` | `0` |
| 未知命令 | `2` | `2`（argparse 用法错误） |
| run 判词 INCONCLUSIVE（未完成；构造上不可晋升） | `3` | Rust 侧已落地（exit 3）；Python 侧待补 |

两侧都把判词 FAIL 映射为 `1`，且互证闸门现在把退出码与证书字段一起比 ——
还额外校验每一侧的退出码是否与**它自己的**判词一致，所以"两边一致地返回一个
没用的常数"这种情形同样过不了闸。

这段历史保留下来：Rust CLI 曾经无论判词是否 FAIL 都返回 `0`，而
`tools/check_dual_impl.py` 只比证书字段。后果是任何拿 Rust 二进制退出码设闸的
CI 泳道都不会变红。该差异现由 `crates/truerise-cli/tests/exit_codes.rs`
与闸门里的退出码比对一并关闭。

## 两个入口都成立的保证

| 保证 | 执法点 |
|---|---|
| 同一指纹 → 逐字段同一判词 | `tools/check_dual_impl.py`（CI job `cross-cert`） |
| 无法落账的判词即错误 | `judge_and_record`，`cargo test -p truerise-eval` |
| 分数回退是非零条件 | 棘轮，`cargo test -p truerise-eval` |
| REPL 只是 IO 适配层，不是第二套引擎 | `cargo test -p truerise-cli --test golden_repl` |
| 篡改失败关闭并点名行号 | `cargo test -p truerise-governance --test chain_gate`；Python 侧 `LedgerTampered` |
| 密钥永不进入配置、argv 与日志 | `SECURITY.md`、onboarding 骨架、观测导出脱敏测试 |
