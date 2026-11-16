# Anti-Cheat Bench：面向智能体声明的评测诚信层

[English](anti_cheat_bench_v0.1.md) | [简体中文](anti_cheat_bench_v0.1.zh-CN.md)

*TrueRise 技术报告 v0.1 —— 2026-11-15。本报告里每一个数字都指向 `readings/v0.1/` 下已入库的
产物，或一条你可以自己跑的测试；没有任何一段是只有文字没有证据的。*

---

## 1. 问题：声明不可证伪

智能体评测的声明以四种可复现的方式失效：

1. **多重检验未记账** —— 跑 N 次只报最好那次，且从不说明 N。
2. **阈值事后改** —— 看完结果再定标准。
3. **自述当证据** —— 模型自己声称自己的分数；没有执行、没有签名。
4. **跨环境比大小** —— 不同技术栈产出的分数被当作可比。

每一种失效都有一台机械抓法。Anti-Cheat Bench 就是把这四台抓法装进一次运行的最小基准。

## 2. 设计

**题包。** 一行 JSONL = 一段评分管线夹具，恰好藏六类缺陷中的一类（未来信息使用、全样本统计量
被用进窗口内、后向回填、同根成交、与结局相关的删行、单位/日历不一致）。题包是**生成的**而不是手写的：
每类一个带种子的随机源，把六个模板展开成 120 题（`readings/v0.1/pack_manifest.json`）；确定性由测试钉死
（同种子 → 两面逐字节一致）。

**两张面。** `tasks.jsonl` 公开，`answers.jsonl` 封盘并内嵌隔离哨兵。运行器把公开面拷进一个全新目录，
**只**把那个路径交给解题方；判卷前先扫哨兵，泄漏即失败关闭
（`test_materialize_public_fail_closed_on_canary`）。题目行不带类别字段 —— 公开它就是泄答案。

**试验中介。** 解题方无法偷跑隐藏试验：每次试验都过运行器，证书里的试验数是运行器记的数。事后选择
（取多数、取最优）聚合的仍是同一批已计入的试验，不能把输的那些消掉
（`test_aggregate_majority_cannot_un_count`）。

**证书。** 每次运行以一份声明证书收尾（RFC 0001）：判词、逐条编号的理由、运行器计的试验分母、
校正后的显著性阈值（base/N）、只由执行器签名记录推导的证据等级、阈值封盘值、题包摘要、答案封盘值，
以及一条 `--replay`：从哈希链账本离线重算判词（被篡改即失败关闭，并按行定位）。

**双实现。** 同一套诚信走查在 Rust（`cargo run -p truerise-cli -- demo`）与 Python
（`python -m truerise demo`）各有一份。CI 要求两份证书在判词、声明、指纹、封盘摘要、校正阈值、
失败种类集合上全部一致（`tools/check_dual_impl.py`，CI job `cross-cert`）—— 同一指纹、同一判词，
否则变红。

## 3. 读数

| 运行 | 判词 | 关键行 | 产物 |
|---|---|---|---|
| honest，1 次试验 | PASS | 准确率 100%（120/120），零理由 | `honest_verdict.json` |
| cheater，20 次取多数 | FAIL | `trials=20 -> corrected significance 2.5e-03, claim p=0.05 does not clear the corrected bar` | `cheater_verdict.json` |
| mutator，封盘 0.8/0.05 后改用 0.5/0.03 | FAIL | `prereg diff: thresholds mutated after seal (keys: min_accuracy, claim_p)` | `mutator_verdict.json` |

Rust/Python 互证闸门在本机两种模式下均通过（`tools/check_dual_impl.py`，退出码 0；CI 以
`cross-cert` job 复跑）。

**失败清单才是重点。** 建仓期参考解题器的首跑读数是 100/120，20 处失分**全部集中在一类** ——
真缺陷在解题器的规则里（一条正则要求在统计量名之后出现索引记号），不在题包里。失分明细正是把它
暴露出来的东西；修复与重跑记录在 `CHANGELOG.md`。我们公开这个过程，因为一个从不
展示自身失分的基准，卖的就是它本该消灭的那种不可证伪性。

## 4. 本报告不宣称什么（局限）

- **判别难度。** v0.1 参考解题器能到 100%，是因为它的规则编码了与生成器同一套分类学形状
  （同一事实源）。v0.1 演示的是**抓法机制**，不是检测器强度；更难的题包（同类安全孪生、方向判别）
  在 v0.2。
- **尚无外部模型跑分。** 上述读数全部来自我们自己的参考解题器。待办的信誉关键一步 —— 一个公开模型
  走外部解题器协议、读数入库 —— 协议已落地；该读数由 `ROADMAP.md` 第 1 项持有，在任何第三方声称采用之前必须先完成。
- **进程内证据有上限。** 参考解题器在进程内运行；其记录由运行器加盖执行器签名，但 v0.1 的证据阶梯
  把更深的执行形态（沙箱化子进程）划给 v0.2 路径，经 `truerise-sandbox` 一致性达标。
- **无强化学习训练环结果。** 运行时暴露了可被训练环安全消费的接口（防伪闸门），我们不宣称任何训练成效。
- **单人项目。** 一位作者；评审与外部贡献由 `CONTRIBUTING.md`（DCO + 来源清洁声明）与 RFC 流程治理。

## 5. 自己验一遍

```bash
git clone <本仓库> && cd truerise
cargo test --workspace                       # 101 个 Rust 测试
python -m pip install -e python/ && python -m pytest python/tests -q   # 25 个测试
python -m truerise evals demo                # honest PASS, cheater FAIL, mutator FAIL
python -m truerise demo --replay             # 离线重算
cargo run -p truerise-cli -- demo            # Rust 走查，同一判词
python tools/check_dual_impl.py              # 互证闸门
```

---

*联系：开 GitHub issue 或 PR（须 DCO 签名）。声明证书的模式正以 RFC 0001 开放评审 ——
"两名维护者批准"这条规则从本仓第一次外部评审开始生效。*
