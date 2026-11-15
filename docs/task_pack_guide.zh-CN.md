# 题包贡献指南

[English](task_pack_guide.md) | [简体中文](task_pack_guide.zh-CN.md)

题包是 Anti-Cheat Bench 的贡献单元：**一个题包 = 一个 PR**。题包属数据，按 CC-BY-4.0 授权（见 `../LICENSE-DATA.md`）；随附的生成器或判分代码按 Apache-2.0 / MIT 授权。

协议本体在 [`anti_cheat_bench.zh-CN.md`](anti_cheat_bench.zh-CN.md)，本页只讲怎么写。

## 1. 题包是什么

一个题包目录恰好放两张 JSONL 面：

| 文件 | 谁能看到 | 行字段 |
|---|---|---|
| `tasks.jsonl` | 公开面 —— 交给解题方 | `id`、`level`、`prompt`、`fixture` |
| `answers.jsonl` | 封盘面 —— 只在判卷侧 | `id`、`defect_line`、`family`、`canary` |

每题是一段评分管线夹具，里面藏**恰好一个**分类学缺陷。解题方回答 `(行号, 类别)`，判卷侧对着封盘面打分。

缺省题包规模：6 类 × 20 = **120 题**。

## 2. 先生成一个看看，再动手写

```bash
python -m truerise evals gen --per-family 3 --seed 20261115
ls output/truerise_evals/pack      # tasks.jsonl + answers.jsonl
```

生成是确定性的：同样的 `(seed, per_family)` 能逐字节复现两张面。这是测试项而不是礼貌
（`test_generator_determinism`）。

## 3. 会让题包变红的规则

`pack.py::_validate` 失败关闭。下列每条都会抛异常，且各有具名测试：

| 规则 | 错误文本 |
|---|---|
| 题目 id 唯一 | `duplicate task ids in tasks.jsonl` |
| 两张面的 id 集合一致 | `task/answer id sets differ (both surfaces must align)` |
| prompt 必须是本题包基准的规范题面 | `prompt is not the canonical bench prompt` |
| 夹具至少 2 行 | `fixture too small to hide a defect` |
| `family` 必须在分类学内 | `family ... not in taxonomy` |
| `defect_line` 必须落在 `1..line_count` | `defect_line ... outside 1..n` |
| 公开面不得带封盘提示 | `test_public_surface_carries_no_sealed_hints` |
| 含隔离哨兵的工作目录拒绝判卷 | `test_materialize_public_fail_closed_on_canary` |

最后两条是防污染核心：题目行**不带** `family` 字段，因为公开它就是泄答案；运行器若发现公开面里含隔离哨兵串就拒绝判卷 —— 这按泄漏处理，不是警告。

## 4. 新增一个缺陷类别

四个落点，全在 `python/src/truerise/evals/`：

| 文件 | 要加什么 |
|---|---|
| `families.py` | `FAMILIES` 条目 + 一个 `_YourFamily(rng, level) -> list[str]` 构造器，返回夹具行 |
| `families.py` | 用 `DEFECT_TOKEN`（`@@DEFECT@@`）标出缺陷行 —— 生成器会剥掉标记并记录 `defect_line` |
| `solvers.py` | 在 `_RULES` 里加一条识别规则，让参考解题器能找到它 |
| `pack.py` / 测试 | 扩展 `test_classify_rules_cover_all_six_families`（类别数变了就同步改名） |

类别命名纪律：snake_case、按**机制**而非措辞命名（`future_info_use`、`same_bar_execution`）。判卷按 `(行号, 类别)` 精确匹配 —— 描述"句子长相"而不是"发生机制"的类别名无法公平判分。

定义写法沿用 `../docs/anti_cheat_bench.md` §3 的纪律：陈述机制（"在全样本上算出的统计量被用进被打分的窗口内"），而不是写出会泄底的措辞。

## 5. 挂上你自己的解题器

基准与解题器无关，解题器只有一个方法的协议：

```python
class MySolver:
    def solve(self, task):            # -> (行号, 类别)
        ...
```

`Solver.solve` 遇到畸形输出必须抛异常 —— 运行器选择失败关闭而不是猜（`solvers.py::Solver`、`runner.py::run_trials`）。接入方式：

```python
from truerise.evals import load_pack, run_bench
from pathlib import Path

# 封盘阈值。目前定义在 truerise/__main__.py 的 DEFAULT_THRESHOLDS，
# 提升到库级导出已登记在 PROGRESS.md；在那之前请显式传入这张表。
SEALED = {"min_accuracy": 0.8, "claim_p": 0.05}

cert, summary = run_bench(
    solver=MySolver(), pack=load_pack(Path("output/truerise_evals/pack")),
    work_dir=Path("output/work"), out_dir=Path("output/run"),
    trials=1, strategy="last", thresholds=SEALED, solver_name="my-model",
)
print(cert.verdict, cert.reasons)
```

仓内自带的参考解题器：`HonestSolver`、`CheaterSolver`（藏分母的 best-of-N）、
`MutatorSolver`（改完阈值再重新声明）、`SelfReporter`（没有执行器签名就宣称结论）。

**外部模型跑分还缺什么**（已登记在 [`PROGRESS.md`](PROGRESS.md)，尚未实现）：一套带题面/答案隔离、并记录指纹的外部解题器协议。在它落地前，公开读数都是参考解题器读数 —— README 已直说这点。

## 6. 开 PR 前的自检

```bash
python -m truerise evals gen --out output/pack --per-family 20 --seed <你的种子>
python -m truerise evals run --pack output/pack --trials 1            # 预期诚实运行 PASS
python -m truerise evals run --pack output/pack --cheat --trials 20   # 预期 FAIL
python -m pytest python/tests -q                                      # 全绿
python tools/lint_banned_words.py                                     # 工作区禁词门
```

同时确认：

- [ ] 每题恰好藏一个缺陷（多缺陷会让判卷含糊）
- [ ] 夹具没有逐字复制第三方源码（来源清洁声明，`../CONTRIBUTING.md`）
- [ ] 你的生成器（若有）在固定种子下确定性可复现
- [ ] PR 里任何公开产物都不含封盘答案
- [ ] PR 描述写明题包规模与类别配比

## 7. 维护者第一眼看的四件事

1. 解题方能否从公开面够到封盘面？（能 = 直接拒）
2. 这个缺陷是真机制，还是模式匹配的噪声？
3. 在 `--trials 20` 下题包是否会退化（分母必须咬得住）？
4. 测试有没有用它所执法那条规则的名字命名？
