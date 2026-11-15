# TrueRise

[English](README.md) | [简体中文](README.zh-CN.md)

**智能体声明的评测诚信层** —— 一套作弊会被机器抓住的题包基准、一份陌生人可离线复算的声明证书，以及托住它们的 Rust 运行时底座。

> 一句话：任何"我们的智能体提升了 X%"的声明，都该附带一张第三方可离线重算的证书。本仓交付题包基准、证书格式，以及让这件事成立的行为契约。

## 60 秒

```bash
cargo test --workspace                  # 101 个 Rust 测试，零告警
python -m pip install -e python/
python -m pytest python/tests -q        # 25 个测试
python -m truerise evals demo           # 诚实运行 PASS，作弊运行 FAIL，改标准运行 FAIL
python -m truerise demo --replay        # 判词的离线重算
cargo run -p truerise-cli -- demo       # 同一套走查的 Rust 版
python tools/check_dual_impl.py         # 双实现互证：Rust 与 Python 判词不一致即红
```

以上每一条都在本机实测通过；测试数、题数、判词文案与 `readings/v0.1/` 落盘产物一致。

## 行业痛点，以及当场证明它被抓的那条命令

| 评测失效形态 | 抓住它的机制 | 一条命令验证 |
|---|---|---|
| 报最优结果却藏起分母（跑 N 次只报最好那次） | 由运行器记账的单调试验账本；校正后的阈值 = base/N，先打印再执法 | `python -m truerise evals run --cheat --trials 20 --strategy majority`（FAIL：`corrected significance 2.5e-03`） |
| 看完结果再定阈值 | 阈值在运行前以 SHA-256 封盘；声明与封盘逐键比对，违规键直接点名 | `python -m truerise evals run --mutate`（FAIL：`prereg diff ... keys: claim_p, min_accuracy`） |
| 把模型自述当证据 | 证据阶梯 L0–L6；没有执行器签名的记录贡献量为零 | `python -m truerise demo`（判词 4：`0 executor-signed records`） |
| 跨环境比大小 | 四元组指纹；跨注册表比较一律拒绝，连自己跟自己也不许 | `python -m truerise demo --drop-fingerprint`（出现完整性判词行） |
| 判词事后无法复核 | 哈希链账本；`--replay` 离线重算，被篡改则失败关闭并定位到行 | `python -m truerise demo --replay` |
| 分数回退被静默合并 | 棘轮只收紧；回退映射为非零退出码 | `cargo test -p truerise-eval`（9 个测试） |
| 判词无法落账 | 无法落账的判词是错误，不是警告 | `cargo test -p truerise-eval`（`judge_and_record`） |
| 奖励劫持：被评方改动评 | 判定权威唯一 + 沙箱三区必失探针（逃逸 / 超卖 / 残留） | `cargo test -p truerise-sandbox` |
| 双实现各自漂移 | 互证闸门：同一指纹、逐字段同一判词 | `python tools/check_dual_impl.py` |

基准全协议（题包格式、封盘分类学、隔离模型、证书模式）见
[`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md)（[中文版](docs/anti_cheat_bench.zh-CN.md)）；
证书格式以 [RFC 0001](docs/rfc/0001-claim-certificate.md) 开放评审。
文档从哪里读起见 [`docs/README.zh-CN.md`](docs/README.zh-CN.md)。

## 箱子里有什么

| 部件 | 范围 | 许可 |
|---|---|---|
| `truerise`（Python） | 60 秒走查 + `truerise.evals` Anti-Cheat Bench（题包、封盘答案、声明证书） | Apache-2.0 / MIT |
| `truerise`（Rust，12 个 crate） | 契约与判定权威：哈希链审计、三态策略、沙箱端口、一致性判定、棘轮、指纹、MCP（默认零暴露）、可观测导出 | Apache-2.0 / MIT |
| 题包（数据） | 生成的夹具 + 封盘答案 | CC-BY-4.0（`LICENSE-DATA.md`） |

## 参考读数（已入库，可复算）

| 运行 | 判词 | 产物 |
|---|---|---|
| 诚实解题器，1 次试验 | PASS，准确率 100%（120/120） | [`readings/v0.1/honest_verdict.json`](readings/v0.1/honest_verdict.json) |
| 作弊器，20 次试验取多数 | FAIL —— 藏分母被抓 | [`readings/v0.1/cheater_verdict.json`](readings/v0.1/cheater_verdict.json) |
| 改标准器 | FAIL —— 封盘后被改的阈值被抓 | [`readings/v0.1/mutator_verdict.json`](readings/v0.1/mutator_verdict.json) |

技术报告：[`docs/report/anti_cheat_bench_v0.1.md`](docs/report/anti_cheat_bench_v0.1.md)（[中文版](docs/report/anti_cheat_bench_v0.1.zh-CN.md)）。

## 局限（引用前请先读这一段）

- **尚无外部模型跑分。** 上表全部读数来自我们自己的参考解题器。把一个公开模型经"外部解题器协议"跑一遍是已登记的下一步（`docs/PROGRESS.md`）——在它落地之前，请把本基准当作 harness 演示，不要当作排行榜。
- **生产消费方不在本仓。** 同一套契约在一个真实的生产系统里运行，但那份部署是源码可见、使用受限的另一套许可，按许可隔离门不得进入本工作区（`tools/lint_deny_isolation.py`）。我们直说这一点，而不是暗示本仓拿不出的采用证据。
- **单人项目。** 一位作者，全部提交带 DCO 签名；外部评审从 RFC 流程开始（`docs/rfc/README.md`）。
- **无强化学习训练结果。** 运行时暴露了可被训练环安全消费的接口，我们不宣称任何训练环成效。

## 状态与治理

M0–M5 五波完成（规格映射闸门机器核对，23 行行为规格 / 67 个具名标识符）；W3（基准门面）记录在 `docs/PROGRESS.md`。

- 安全：见 `SECURITY.md`。一切代码执行必须经一致性达标的沙箱提供方；密钥永不进入仓库、argv 与日志。
- 贡献：DCO 签名 + 来源清洁声明（`CONTRIBUTING.md`）；题包贡献按数据许可走 CC-BY-4.0（`LICENSE-DATA.md`）。
- 仓库形态与依赖方向：[`docs/topology.md`](docs/topology.md)。
- 命令与开关的单一权威页：[`docs/cli.md`](docs/cli.md)（[中文版](docs/cli.zh-CN.md)）。

## 许可

代码双许可 **Apache-2.0 / MIT**（`LICENSE-APACHE`、`LICENSE-MIT`）；题包数据 CC-BY-4.0（`LICENSE-DATA.md`）。贡献须带 DCO 签名并满足 `CONTRIBUTING.md` 的来源清洁声明。
