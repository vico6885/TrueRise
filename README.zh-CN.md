# TrueRise

[English](README.md) | [简体中文](README.zh-CN.md)

**智能体声明的评测诚信层** —— 一套作弊会被机器抓住的题包基准、一份陌生人可离线复算的声明证书，以及托住它们的 Rust 运行时底座。

> 一句话：任何"我们的智能体提升了 X%"的声明，都该附带一张第三方可离线重算的证书。本仓交付题包基准、证书格式，以及让这件事成立的行为契约。

英文全句口号 **Harness Governance for Verifiable Recursive Self-Improvement** 中的每个可验证名词，都由 CI 门禁（`tools/check_claim_terms.py`：命令 + 具名测试 + 落盘产物，当前 5/5 met）按证据执法。

## 为谁而造——五条战线，五种痛

| 如果你在做…… | ……你大概率被这样坑过 | TrueRise 给你什么 | 一条命令验证 |
|---|---|---|---|
| **Agent Harness / 评测框架** | 被评方刷分作弊、"完成了"的声明九成是占位符、判词只有 PASS/FAIL 的表演 | 作弊被机器抓住的题包基准 + 逐项完成台账（沉默即拒绝）+ 三值判词（存在 `INCONCLUSIVE`） | `python -m truerise evals demo` |
| **RSI（自我改进）循环** | 未验证的断言悄悄变成下一代的地基、"做不到"的放弃没人审计 | 哈希绑定人类批准的世代证书 + 放弃必须有搜索面账单的复核机制 + 记忆污染闸 | `cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json` |
| **Agentic RL / 训练流水线** | 代理指标被事后刷分、样本预算浪费在噪声上、"提升"只是单次侥幸 | 阈值先封盘、单调试验账本带校正分母、精确配对显著性（McNemar）、等预算强制 | `python -m truerise evals run --cheat --trials 20 --strategy majority`（FAIL） |
| **预训练评测** | 题包泄漏进训练数据、checkpoint 分数随环境漂移 | 四元组指纹（模型×harness×题包×环境）拒绝跨环境比较、字节级可复算重放、题包内金丝雀与诱饵 | `python -m truerise demo --drop-fingerprint`（出现完整性判词行） |
| **后训练 / checkpoint 门禁** | 回退被静默合并、无法复核的"提升" | 只收紧的棘轮、checkpoint 证书、离线重算、双实现互证门 | `python tools/check_dual_impl.py` |

先说一句实话：TrueRise 不做训练本身。它是训练环可以安全消费的验证层——下文的每个机制都有一个具名测试钉住。

## 特性清单，按重要性从高到低

**1. 反作弊基准——作弊被机器抓住，不靠感觉。**
经典作弊手法全部接线到失败：跑 20 次只报最好那次（单调试验账本先打印校正后的及格线 base/N，再执法）；看完结果再定阈值（阈值在运行前以 SHA-256 封盘，声明与封盘逐键比对，违规键直接点名）；没有执行器签名的记录想领功劳（证据阶梯 L0–L6：自述贡献量为零）；跨环境比大小（四元组指纹拒绝，连自己跟自己都不许）。题包自带金丝雀与诱饵，公开面可证明不含封盘答案。

**2. 可验证的"完成"——harness 不靠信任说"做完了"。**
`run --intent` 把一次运行绑定到封盘合同：验收项事前列清（每项显式标注机检或人判），用户以**逐项令牌**确认（整体一键确认在结构上不可能）。完成声明必须把每一项映射到证据、`not_done`（带封闭分类的理由）、或 `not_verified`（带核实的成本）——跳过任何一项即拒绝，沉默永远不会被读成覆盖。在你自己的机器上，`verify --bundle` 无需本仓、无需密钥、无需网络即可复验机检项。桩扫描门与反向变异门管住测试本身：未申报桩阻断完成声明，被反打后仍然绿的测试会被判定"什么都没证明"。

**3. 一代 RSI，端到端出证书。**
`evolve --demo` 跑完 提案 -> 哈希绑定的人类批准 -> 应用 -> 重评 -> 棘轮 -> 证书，回归即回滚。证书可离线重算（`evolve --replay`），带签名校验（`verify --cert`；未签名即 "unverified issuer"——不会被静默当作已验证）；绑定意图合同的世代必须携带闭合的完成台账，否则拒绝发证。mid-run 改合同？判词作废，落链。

**4. 两个极端都诚实——不只是"不谎报成功"。**
什么都没证明的运行拿 `INCONCLUSIVE`（独立退出码 3——永远不可读作 PASS 或 FAIL，也不是"无差异"）。放弃同声明一样受审计：`giveup --ledger` 要求搜索面账单（已试探针带结果、未试探针带最便宜的具体形式与成本）、封盘预算上界、与已试集不相交的复核计划。"不可能"只能说成"在声明预算内未找到路"——否则拒绝。判词空间严格三值，不可压缩。

**5. 可复现是一种格式，不是一句承诺。**
模型步先钉定再录制；同一条 tape 离线重放得到逐字节相同的摘要。链行禁浮点（只收整数或字符串——浮点规则）、canonical JSON 跨实现字节稳定、每一份入库读数（`readings/`）只凭账本即可重算——不要密钥、不要网络、不需要信任。

**6. 把"被审计"写进代码的运行时底座。**
三态策略（Deny / Ask / Auto——批准永不过期成默许）、沙箱化的工具执行端口（RFC 0002：逃逸 / 超卖 / 残留三区必失探针）、全局分母不可切碎的预算账本、篡改即失败关闭且定位到行的哈希链账本、默认零暴露的 MCP server（空白 allowlist；每次调用落链）。

**7. 一套会自我审计的门禁。**
15 个门脚本背后的 15 个 CI job：README 标题被机器对照证据执法；承诺措辞分级（A 可强制 / B 可测量 / C 只能降低），把 C 级结果写成 A 级措辞的句子会被拒绝；桩与测试咬合被扫描；`spec_map.json` 的每一行行为（54 行、209 个标识符）都解析到真实的具名测试。你正在读的这份 README 就在执法面之内。

## 60 秒

```bash
cargo test --workspace                  # 240 个 Rust 测试，零告警
python -m pip install -e python/
python -m pytest python/tests -q        # 40 个测试
python -m truerise evals demo           # 诚实运行 PASS，作弊运行 FAIL，改标准运行 FAIL
python -m truerise demo --replay        # 判词的离线重算
cargo run -p truerise-cli -- demo       # 同一套走查的 Rust 版
python tools/check_dual_impl.py         # 双实现互证：Rust 与 Python 判词不一致即红
cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json
                                        # 一代 RSI，离线重算
```

## 失效形态，以及当场证明它被抓的那条命令

| 评测失效形态 | 抓住它的机制 | 一条命令验证 |
|---|---|---|
| 报最优结果却藏起分母（跑 N 次只报最好那次） | 由运行器记账的单调试验账本；校正后的阈值 = base/N，先打印再执法 | `python -m truerise evals run --cheat --trials 20 --strategy majority`（FAIL：`corrected significance 2.5e-03`） |
| 看完结果再定阈值 | 阈值在运行前以 SHA-256 封盘；声明与封盘逐键比对，违规键直接点名 | `python -m truerise evals run --mutate`（FAIL：`prereg diff ... keys: claim_p, min_accuracy`） |
| 把模型自述当证据 | 证据阶梯 L0–L6；没有执行器签名的记录贡献量为零 | `python -m truerise demo`（判词 4：`0 executor-signed records`） |
| 跨环境比大小 | 四元组指纹；跨注册表比较一律拒绝，连自己跟自己也不许 | `python -m truerise demo --drop-fingerprint`（出现完整性判词行） |
| 判词事后无法复核 | 哈希链账本；`--replay` 离线重算，被篡改则失败关闭并定位到行 | `python -m truerise demo --replay` |
| 分数回退被静默合并 | 棘轮只收紧；回退映射为非零退出码 | `cargo test -p truerise-eval`（35 个测试） |
| 判词无法落账 | 无法落账的判词是错误，不是警告 | `cargo test -p truerise-eval`（`judge_and_record`） |
| 奖励劫持：被评方改动评 | 判定权威唯一 + 沙箱三区必失探针（逃逸 / 超卖 / 残留） | `cargo test -p truerise-sandbox` |
| 双实现各自漂移 | 互证闸门：同一指纹、逐字段同一判词 | `python tools/check_dual_impl.py` |

基准全协议（题包格式、封盘分类学、隔离模型、证书模式）见
[`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md)（[中文版](docs/anti_cheat_bench.zh-CN.md)）；
证书格式以 [RFC 0001](docs/rfc/0001-claim-certificate.md) 开放评审。
文档从哪里读起见 [`docs/README.zh-CN.md`](docs/README.zh-CN.md)。

## 为接入 Agent 开源生态而设计

TrueRise 以三个刻意排序的面接入 Python Agent 世界，外加两条传输通道：

- **MCP（工具供给侧）。** `truerise mcp --serve` 把受 TrueRise 治理的工具挂进任何支持 MCP 的框架或编辑器——默认零暴露（空白 allowlist），每次调用落链。
- **Python SDK（harness 嵌入侧）。** `truerise.sdk.Session` 包住任何 agent 循环——LangGraph 图、OpenAI Agents SDK runner、或者一个裸函数——加上预算上限、哈希链审计、三态策略与棘轮。两个官方范式随仓交付：[`examples/langgraph_harness.py`](examples/langgraph_harness.py) 与 [`examples/openai_agents_harness.py`](examples/openai_agents_harness.py)，均带 import 守卫：不装第三方框架也能跑。
- **外部解题器协议（评测侧）。** 任何遵守单文件契约（输入 `TRUERISE_PUBLIC_DIR`，输出 `submission.jsonl`）的解题器都能被基准评分——运行器为记录加执行器签名。
- **模型传输。** OpenAI 兼容端点（vLLM / Ollama / 代理）与 Anthropic Messages API 接在同一个治理端口之后；运行钉定到请求的模型 id，provider 自报的回显参与钉定校验。密钥只走环境变量，不进 argv 与日志。
- **可观测。** 单向 OpenTelemetry span 导出（链始终是权威）；可观测层不改动审计事实源。
- **给第二实现者的契约。** 六份 JSON Schema（`schemas/`）定义线上形状——意图合同、完成报告、验收包、放弃声明、工具信封、审计事件——第二实现可以被互证，而不是被信任。

## 箱子里有什么

| 部件 | 范围 | 许可 |
|---|---|---|
| `truerise`（Python） | 60 秒走查 + `truerise.evals` 反作弊基准 + 零依赖 SDK（`truerise.sdk`）+ 长程闭环题包 + 外部解题器协议 | Apache-2.0 / MIT |
| `truerise`（Rust，13 个 crate） | 契约与判定权威：哈希链审计、三态策略 + 沙箱端口 + 工具执行（RFC 0002）、一致性判定、棘轮、RSI 闭环（RFC 0003）、MCP（默认零暴露）、可观测导出、PyO3 绑定壳 | Apache-2.0 / MIT |
| `truerise-native`（PyO3 wheel） | 同一批 crate 之上的可选加速器；纯 Python SDK 是完整回退 | Apache-2.0 / MIT |
| 题包（数据） | 生成的夹具 + 封盘答案，带金丝雀与诱饵 | CC-BY-4.0（`LICENSE-DATA.md`） |

## 参考读数（已入库，可复算）

| 运行 | 判词 | 产物 |
|---|---|---|
| 诚实解题器，1 次试验 | PASS，准确率 100%（120/120） | [`readings/v0.1/honest_verdict.json`](readings/v0.1/honest_verdict.json) |
| 作弊器，20 次试验取多数 | FAIL —— 藏分母被抓 | [`readings/v0.1/cheater_verdict.json`](readings/v0.1/cheater_verdict.json) |
| 改标准器 | FAIL —— 封盘后被改的阈值被抓 | [`readings/v0.1/mutator_verdict.json`](readings/v0.1/mutator_verdict.json) |

技术报告：[`docs/report/anti_cheat_bench_v0.1.md`](docs/report/anti_cheat_bench_v0.1.md)（[中文版](docs/report/anti_cheat_bench_v0.1.zh-CN.md)）。
本框架能强制什么、只能测量什么、永远不承诺什么——这条边界本身也是落盘产物：[`docs/limits.md`](docs/limits.md)（[中文版](docs/limits.zh-CN.md)）。

## 局限（引用前请先读这一段）

- **尚无外部模型跑分。** 上表全部读数来自我们自己的参考解题器。外部解题器协议本身已经落地（`run_external_trials`，执行器签名记录）；缺的是运营步骤——一个遵守契约的解题器接一个公开模型（`ROADMAP.md` 第 1 项）。在该读数落地之前，请把本基准当作 harness 演示，不要当作排行榜。
- **生产消费方不在本仓。** 同一套契约在一个真实的生产系统里运行，但那份部署是源码可见、使用受限的另一套许可，按许可隔离门不得进入本工作区（`tools/lint_deny_isolation.py`）。我们直说这一点，而不是暗示本仓拿不出的采用证据。
- **单人项目。** 一位作者，全部提交带 DCO 签名；外部评审从 RFC 流程开始（`docs/rfc/README.md`）。
- **无强化学习训练结果。** 运行时暴露了可被训练环安全消费的接口，我们不宣称任何训练环成效。

## 状态与治理

M0–M5 与 P0–P3 及 W1–W18、G 系列完成（规格映射闸门机器核对，54 行行为规格 / 209 个具名标识符；240 个 Rust 测试 + 40 个 Python 测试；72 个 DCO 签名提交；15 个门脚本配 15 个 CI job）。交付记录在 `CHANGELOG.md`，前进计划在 `ROADMAP.md`。

- 安全：见 `SECURITY.md`。一切被执行的动作都经沙箱端口、以执行器返回的能力等级落审计链（RFC 0002）；密钥永不进入仓库、argv 与日志。
- 贡献：DCO 签名 + 来源清洁声明（`CONTRIBUTING.md`）；题包贡献按数据许可走 CC-BY-4.0（`LICENSE-DATA.md`）。
- 仓库形态与依赖方向：[`docs/topology.md`](docs/topology.md)。
- 命令与开关的单一权威页：[`docs/cli.md`](docs/cli.md)（[中文版](docs/cli.zh-CN.md)）。
- 边界声明（能强制/只能测量/永不承诺）：[`docs/limits.md`](docs/limits.md)（[中文版](docs/limits.zh-CN.md)）。

## 许可

代码双许可 **Apache-2.0 / MIT**（`LICENSE-APACHE`、`LICENSE-MIT`）；题包数据 CC-BY-4.0（`LICENSE-DATA.md`）。贡献须带 DCO 签名并满足 `CONTRIBUTING.md` 的来源清洁声明。
