# Anti-Cheat Bench —— 协议规格（v0.1）

[English](anti_cheat_bench.md) | [简体中文](anti_cheat_bench.zh-CN.md)

> 基准以 `truerise.evals` 的形态随 Python 包发布。本文每一句都由代码执法、被测试钉住
> （`python/tests/test_evals_bench.py`），没有一条是建议。

## 1. 它是什么

一组评分管线夹具题包，每题恰好藏一个来自六类分类学的缺陷。解题方对每题提交
`(行号, 类别)`；判卷方对着解题方永远看不到的**封盘答案面**打分。每次运行以一份
**声明证书**收尾：判词、校正后的试验分母、证据等级、题包摘要、答案封盘值 —— 全部可由
哈希链账本离线重算。

```bash
python -m truerise evals demo      # 一条命令：生成题包 + 三次运行
```

验收判据：`honest PASS, cheater FAIL, mutator FAIL` —— 诚实运行走的是把两种作弊判 FAIL 的
**同一条代码路径**。

## 2. 题包协议

一个题包目录恰好放两张 JSONL 面：

| 面 | 可见性 | 行字段 |
|---|---|---|
| `tasks.jsonl` | 公开（交给解题方） | `id, level, prompt, fixture` |
| `answers.jsonl` | 封盘（只在判卷侧） | `id, defect_line, family, canary` |

规则（全部失败关闭，执法点在 `pack.py` 与测试里）：

- 两面的 id 唯一且彼此对齐；
- `defect_line` 必须落在夹具行号范围内，`family` 必须在分类学内；
- 题目行**不带** `family` 字段（公开它就是泄答案）；
- 答案行内嵌隔离哨兵；运行器若发现公开面含哨兵就拒绝判卷（泄漏按红处理，绝不降级成警告）；
- 生成确定性：同一 `(seed, per_family)` 逐字节复现两面（`test_generator_determinism`）。

## 3. 封盘缺陷分类学（v0.1）

| 类别 | 定义（按行为，不按措辞） |
|---|---|
| `future_info_use` | t 时刻的决策用到了只有在 t 之后才可得的信息 |
| `full_sample_scaling` | 在全样本上算出的统计量被用进被打分的窗口内 |
| `backward_fill_leak` | 后面的值回填了前面的槽位 |
| `same_bar_execution` | 假设在同一根 K 线上用当根的信息成交 |
| `sample_selection_bias` | 被某条规则删掉的行与被打分的结局相关 |
| `unit_consistency` | 两个量以不同单位或不同日历进入同一个比较 |

难度档：`1` 裸夹具；`2` 加入来自**其他**类别的安全诱饵（诱饵不带任何良性标记 —— 搜
"benign" 这个词不能成为答案钥匙）。第 3 档（同类安全孪生、考方向判别）留到 v0.2。

## 4. 隔离与试验中介

- **答案隔离是结构性的**：运行器把 `tasks.jsonl` 拷进一个全新目录，只把那个路径交给解题方。
  `answers.jsonl` 从不出题包目录；判卷前先扫公开面找哨兵。
- **试验由运行器中介**：解题方无法偷跑隐藏试验 —— 每一次试验都过 `run_trials`，证书里的试验数
  是运行器记的数。事后选择（取多数 / 取最优）只能聚合**已被计入的同样次数**，不能把输的那些消掉。
- **声明 vs 封盘**：阈值在运行前以 canonical JSON 的 SHA-256 封盘；证书把**声明用的表**与封盘
  逐键比对并点名违规键（先跑后改就此被抓住）。

## 5. 证书字段

| 字段 | 含义 |
|---|---|
| `claim` | 准确率 + 题数 + 试验数，由输入推导 |
| `fingerprint` | 解题器 × harness × 题包摘要 × 判卷器（四元组；任一项变化即为新评测） |
| `verdict` / `reasons` | PASS/FAIL，逐条编号给出违规理由 |
| `pack_digest` / `answer_seal` | 两张面各自的 SHA-256 |
| `trials` / `corrected_significance` | 运行器计的分母，以及收紧后的阈值（base/N） |
| `accuracy` / `per_family` / `failures` | 失分明细（可审计，且不暴露封盘内容） |
| `evidence_effective_level` | 由运行器执行的记录带签名（L4）；自述运行贡献为零（L0） |
| `prereg_seal` | 阈值封盘值的前 12 位十六进制 |
| `replay` | 从哈希链账本离线重算（被篡改则失败关闭） |

## 6. 参考读数（v0.1，已入库在 `readings/v0.1/`）

| 运行 | 判词 | 读数 | 产物 |
|---|---|---|---|
| honest（1 次试验，规则引擎） | PASS | 准确率 100%（120/120），零理由 | `readings/v0.1/honest_verdict.json` |
| cheater（20 次取多数） | FAIL | 校正阈值 2.5e-03，声明 0.05 不过 | `readings/v0.1/cheater_verdict.json` |
| mutator（改阈值后重声明） | FAIL | 预注册比对点名 `min_accuracy, claim_p` | `readings/v0.1/mutator_verdict.json` |

**失败清单纪律（记下来，不藏）**：建仓期参考解题器的首跑读数是 100/120，20 处失分**全部落在
同一个类别** —— 缺陷在解题器的规则里（一条正则要求在 `mu` 之后出现索引记号），不在题包里。
正是失分明细把它暴露出来的；修完重跑为 120/120。首跑读数保留在交付记录里
（`CHANGELOG.md`）。

## 7. 路线

- v0.2：第 3 档同类孪生题（考方向判别）；题包模板扩展到 6 套以上；分类别难度曲线。
- 外部解题器协议：已落地 —— `runner.run_external_trials` 以子进程运行第三方解题器，
  只暴露公开面，并由 runner 对每条判卷记录做执行器签名（解题器自己的声明封顶 L0）。
- 第二判卷器：第二个独立判卷器产出 `judge_agreement`（κ）记录 —— 判卷器分歧本身就是可发表的数据。
- 真实外部模型跑分：尚未完成的信誉关键一步（见 `ROADMAP.md` 第 1 项）—— 一个公开模型接一个
  守契约的解题器，读数入库到 `readings/`。
