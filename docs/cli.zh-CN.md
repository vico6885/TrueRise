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
| `run --approve <tool>` | 在 run 之前把批准落盘（进入 ask-allowed 的唯一路径；可重复） |
| `run --budget <n>` | 每帽配额（轮次 + 工具单位）；超预算停止运行并以 `1` 退出 |
| `run --baseline <path>` | 判词用的棘轮基线（默认 `baselines/run_scores.json`，自 cwd 向上查找） |
| `run --hat <toml>` | 以帽运行：内容哈希入链，`tool_subset` 成为外层策略门；子集里的未知工具在加载期即拒绝运行 |
| `run --provider openai\|anthropic --model <id>` | 同一治理端口后接**真实模型**（预算/策略/判词不变）。`--provider-url <u>` 指向本地 vLLM/Ollama/代理。密钥只走环境变量（`TRUERISE_API_KEY` / `ANTHROPIC_API_KEY`）；缺钥即拒绝。运行钉定到模型 id，且 provider 自报的回显参与钉定校验 |
| `evolve --demo --approve <hash>` | 一代 RSI（RFC 0003）：提案 -> 哈希绑定批准 -> 应用 -> 重评 -> 棘轮 -> 证书；`--inject-regression` 演示回滚；`--tape <path>` 录制每次模型调用 |
| `evolve --replay --cert <path>` | 离线复算世代证书（退出 0 = 逐字节一致；被篡改的证书会被拒绝） |
| `verify --cert <path> [--issuer-key <hex>]` | 校验证书签名（ed25519）；未签名的证书报告 `unverified issuer` 并以 `1` 退出——绝不静默当真 |
| `mcp --serve [--chain <dir>]` | 经 stdio 起默认零暴露的 MCP server；每次调用都落链 |
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
