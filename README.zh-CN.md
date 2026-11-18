# TrueRise —— 用机器抓住 Agent 基准作弊

[English](README.md) | [简体中文](README.zh-CN.md)

[![License: Apache-2.0 | MIT](https://img.shields.io/badge/license-Apache--2.0%20%7C%20MIT-blue)](#状态)
[![DCO](https://img.shields.io/badge/DCO-signed-all%20commits-green)](CONTRIBUTING.md)
[![status](https://img.shields.io/badge/status-pre--release-orange)](#状态)
<!-- CI / PyPI 徽章已备好模板，仓库公开后即加（RELEASING.md A 与 F 项） -->

"我们的 agent 把准确率从 60% 提到了 90%。"

好的。哪个模型？哪套题包？及格线是在看到结果**之前**定的，还是之后？是不是跑了 20 次、
只把最好的那次放进了汇报？

TrueRise 把这类声明变成一张陌生人可离线重算的证书——五种经典作弊由机器抓住，
不靠评审员的眼力：

```text
$ python -m truerise evals run --cheat --trials 20 --strategy majority
CLAIM     : defect detection accuracy 100% on 120 tasks (trials=20)
VERDICT   : FAIL
            2) trials=20 -> corrected significance 2.5e-03, claim p=0.05 does not clear the corrected bar
```

*节选（省略 FINGERPRINT、ACCURACY、CERTIFICATE 行）。及格线是机械的：
0.05 / 20 次 = 2.5e-03——跑 20 次挑最好那次的汇报过不了这道门，哪怕准确率 100%。*

预发布阶段：从本仓库克隆安装。

```bash
pip install -e python/
truerise evals demo        # 诚实运行 PASS，作弊运行 FAIL，改阈值运行 FAIL
```

## 为什么需要它

Agent 基准分数正在变成决策货币——模型卡、排行榜、采购汇报。但产出分数的
harness 本身是"被信任"而不是"被审计"的：阈值可以事后挪动、重试可以藏进幻灯片、
"我们的 harness 判的"就是全部证据。TrueRise 是缺的那层审计：它包住评测，
在运行前封存判定规则，把结果变成一张可离线重算的证书——就像下载文件有校验和，
这是给"关于 agent 的声明"的校验和。

## 谁需要它

| 如果你在做…… | ……你大概率被这样坑过 | 一条命令验证 |
|---|---|---|
| **Agent harness / 评测框架** | 判分被刷、"完成了"的声明是占位符、PASS/FAIL 表演 | `python -m truerise evals demo` |
| **RSI（自我改进）循环** | 未验证断言变成下一代的地基、"做不到"的放弃没人审计 | `cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json` |
| **Agentic RL / 训练流水线** | 代理指标被事后刷分、单次侥幸被报成提升 | `python -m truerise evals run --cheat --trials 20 --strategy majority`（FAIL） |
| **预训练 / 后训练 checkpoint 评测** | 分数随环境漂移、checkpoint 之间静默回退 | `python -m truerise demo --drop-fingerprint` |

## 五种经典作弊，与当场抓住它的那道闸

| 作弊手法 | 抓住它的机制 |
|---|---|
| 藏起分母（跑 N 次只报最好那次） | 单调试验账本先打印校正及格线 base/N，再执法 |
| 看完结果再定阈值 | 阈值运行前以 SHA-256 封盘；事后改动逐键点名 |
| 把模型自述当证据 | 证据阶梯 L0–L6：未签名的自述贡献量为零 |
| 跨环境比大小 | 四元组指纹（模型 × harness × 题包 × 环境）直接拒绝 |
| 被评方改判分器 | 判定权威唯一 + 沙箱必失探针；账本把篡改定位到行 |

以上每一行都是一个具名测试，不是一段形容词。完整协议见
[`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md)（[中文版](docs/anti_cheat_bench.zh-CN.md)）。

## 与你手头工具的关系

TrueRise 不是你评测框架的替代品——它是包在外面的审计层：

| 你已经在用 | 它给你什么 | TrueRise 补上什么 |
|---|---|---|
| `inspect_ai` / `lm-eval-harness` / OpenAI Evals | 评测运行与分数 | 封存的判定规则、校正分母、精确配对统计、可离线重算的证书 |
| `promptfoo` / LangSmith | 提示词迭代与调用轨迹 | 哈希链审计轨迹：篡改失败关闭并定位到行 |
| LangGraph / OpenAI Agents SDK / 你自己的循环 | agent 本体 | 受治理的工具执行、预算执法、逐次运行的完成证书（`truerise.sdk.Session`） |

接入是一份契约，不是逐框架的垫片：任何 harness 通过单文件解题器协议
（输入 `TRUERISE_PUBLIC_DIR`，输出 `submission.jsonl`）进入基准——
见 [`docs/integrations.md`](docs/integrations.md)。我们刻意不提供逐框架适配层。

## 陌生人能验证什么——以及 harness 会拒绝什么

- **可自查的"完成"。** `run --intent` 把运行绑定到封盘合同：验收项事前列清、
  逐项确认（整体一键确认在结构上不可能），完成声明必须把每一项映射到证据、
  `not_done`（带理由）、或 `not_verified`（带核实的成本）。在你自己的机器上，
  `verify --bundle` 无需本仓、无需密钥、无需网络即可复验机检项。
- **两个极端都诚实。** 什么都没证明的运行拿 `INCONCLUSIVE`（独立退出码 3——
  永远不会读成 PASS 或 FAIL）。放弃同声明一样受审计：需要搜索面账单与封盘
  预算上界，否则拒绝。"不可能"只能说成"在声明预算内未找到路"。
- **可复现是一种格式。** 模型步钉定并录制；同一条 tape 离线重放得到逐字节
  相同的摘要；[`readings/`](readings/v0.2/gen1_certificate.json) 的入库读数
  只凭账本即可重算。
- **一代 RSI，持证上岗。** 提案 → 哈希绑定批准 → 应用 → 重评 → 棘轮 → 证书，
  回归即回滚——mid-run 改合同？判词作废，落链。
- **你正在读的这份 README 就在它自己的执法面之内**：长名中每个可验证名词都由
  `tools/check_claim_terms.py` 对照证据机检（每个 met 行 = 命令 + 具名测试 +
  落盘产物）；承诺措辞分级（机器只能降低概率的结果，不允许用"永不发生"的措辞
  书写）；桩与测试咬合被扫描。

## 安装与运行（预发布）

```bash
git clone <本仓库> && cd TrueRise   # 仓库 URL 待组织名决定
pip install -e python/              # 纯 Python，零运行时依赖
python -m truerise evals demo       # 三判词走查
python -m truerise demo             # 60 秒走查：三种作弊，一份判词
python -m truerise demo --replay    # 离线重算一份判词
cargo test --workspace              # 完整 Rust 套件
```

PyPI / crates.io 名称待定（`RELEASING.md` F 项）；在此之前是源码安装。

## 实现细节（默认折叠）

<details>
<summary><b>七组特性，详细版</b></summary>

1. **反作弊基准。** 题包带金丝雀与诱饵；公开面可证明不含封盘答案；阈值运行前
   封盘；单调试验账本带校正分母；精确配对显著性（McNemar）+ 等预算硬门；
   证据阶梯 L0–L6，未签名自述贡献为零。
2. **可验证完成。** 意图合同（`schemas/intent_contract.v1.schema.json`）带逐项
   裁定；三栏完成台账（done / not_done / not_verified）；桩扫描门；反向变异
   测试门；自助验收包。
3. **持证的 RSI 世代。** RFC 0003 闭环 + 签名且可离线重算的证书 + 记忆污染闸
   （召回开启的运行被排除出基线）。
4. **两个极端都诚实。** `INCONCLUSIVE` 判词 + 有界放弃复核（三值判词；搜索面
   账单；与已试集不相交的复核计划）。
5. **可复现作为格式。** tape 录制/回放；链行浮点禁令；canonical JSON 在 Rust 与
   Python 双实现间逐字节稳定（有测试钉死）；四元组指纹。
6. **被审计的运行时底座。** 三态策略、沙箱化工具执行（RFC 0002）、不可切碎的
   预算分母、定位到行的哈希链、默认零暴露的 MCP server、单向 OpenTelemetry 导出。
7. **自我审计的门禁集。** 15 个门脚本配 15 个 CI job；承诺分级登记册（A 可强制 /
   B 可测量 / C 只能降低），C 级结果写成 A 级措辞即拒；桩门；测试咬合门；
   54 行规格绑定 209 个具名标识符。

</details>

<details>
<summary><b>局限（引用前请先读）</b></summary>

- **尚无外部模型跑分。** 现有读数全部来自我们自己的参考解题器；外部解题器协议
  已落地，缺的是运营步骤（`ROADMAP.md` 第 1 项）。在此之前请把本基准当作
  harness 演示，不要当作排行榜。
- **生产消费方不在本仓**（许可隔离，刻意设计）。
- **单人项目**，一位 DCO 签名作者。**无强化学习训练结果**——运行时暴露了可被
  训练环安全消费的接口，我们不宣称任何训练环成效。

</details>

## 下一步

- 跑一遍上面的 `python -m truerise demo`——不需要 API 密钥，不需要网络。
- 读基准协议：[`docs/anti_cheat_bench.md`](docs/anti_cheat_bench.md)（[中文版](docs/anti_cheat_bench.zh-CN.md)) ·
  教程：[`docs/quickstart.md`](docs/quickstart.md)（[中文版](docs/quickstart.zh-CN.md)） ·
  命令权威页：[`docs/cli.md`](docs/cli.md)（[中文版](docs/cli.zh-CN.md)） ·
  我们拒绝承诺什么：[`docs/limits.md`](docs/limits.md)（[中文版](docs/limits.zh-CN.md)）。
- 贡献一个题包（数据，CC-BY-4.0）：指南在
  [`docs/task_pack_guide.md`](docs/task_pack_guide.md)——题包是最容易的第一贡献，
  走同样的门禁。
- 评审一份 RFC：[`docs/rfc/README.md`](docs/rfc/README.md) 是外部评审的入口。

## 状态

预发布（`0.2.0-dev.0`）。13 个 Rust crate + 零依赖 Python SDK（+ 可选 PyO3
加速器），15 个门脚本配 15 个 CI job，每个提交带 DCO 签名。证据门持续全绿时，
本仓库当得起的长名是：**Harness Governance for Verifiable Recursive
Self-Improvement**——该名字中每个可验证名词都由 CI 门钉住
（`tools/check_claim_terms.py`）——（**智能体声明的评测诚信层**）。分波证据在 `CHANGELOG.md`；
前进计划在 `ROADMAP.md`；安全政策在 `SECURITY.md`；许可：代码 Apache-2.0 /
MIT，题包数据 CC-BY-4.0（`LICENSE-DATA.md`）。
