# truerise（Python 包）

[English](README.md) | [简体中文](README.zh-CN.md)

TrueRise 的 Python 侧：60 秒诚信走查（`truerise.demo`）与 Anti-Cheat Bench
（`truerise.evals`）。不需要网络、不需要 API 密钥、不需要 GPU。**验收判据是判词随输入
移动** —— 没有任何写死的东西，且退出码承载判词。

```bash
python -m pip install -e python/          # 在仓库根目录执行
python -m pytest python/tests -q          # 25 个测试
```

## `truerise demo` —— 三个作弊场景，各自被抓

| 场景 | 作弊行为 | 抓住它的机制 |
|---|---|---|
| S1 先跑后改 | 封盘之后再改阈值 | 预注册封盘比对 + 声明账本（同指纹的第二次判词） |
| S2 大量试跑 | 跑 N 次只报最好那次 | 带显式校正分母的试验账本 |
| S3 自签自证 | 结论没有任何执行器签名记录 | 证据闸（自动降级为声明） |

```bash
python -m truerise demo                       # FAIL，退出码 1，三条编号理由
python -m truerise demo --honest              # PASS，退出码 0
python -m truerise demo --perturb n_trials=10 # 校正阈值 5.0e-05 -> 5.0e-03
python -m truerise demo --honest-seal         # 理由 1 消失
python -m truerise demo --sign-records        # 理由 4 消失
python -m truerise demo --drop-fingerprint    # 跨环境比较被拒绝
python -m truerise demo --replay              # 从哈希链账本离线重算
```

开关与退出码的权威定义在 [`../docs/cli.zh-CN.md`](../docs/cli.zh-CN.md)。

## `truerise evals` —— Anti-Cheat Bench

```bash
python -m truerise evals demo                # honest PASS, cheater FAIL, mutator FAIL（仅此组合退出码为 0）
python -m truerise evals gen --per-family 3  # 确定性题包
python -m truerise evals run --cheat --trials 20 --strategy majority
python -m truerise evals run --mutate
```

协议：[`../docs/anti_cheat_bench.zh-CN.md`](../docs/anti_cheat_bench.zh-CN.md)。
怎么写题包：[`../docs/task_pack_guide.zh-CN.md`](../docs/task_pack_guide.zh-CN.md)。

## 库面

```python
from truerise.evals import (
    Task, Answer, Pack, load_pack, generate_pack,          # 题包协议
    Grade, grade,                                           # 判卷
    HonestSolver, CheaterSolver, MutatorSolver, SelfReporter,
    BenchCert, run_bench, replay,                           # 证书
)
from truerise.demo import run_demo, replay                 # 走查
```

## 措辞说明

上游规格样例里的某个术语与本仓的禁词闸门冲突，因此代码对同一个量打印
`corrected significance`。行为契约（判词翻转、离线重算、证书哈希）逐字段等价 ——
这个等价正是 `tools/check_dual_impl.py` 拿去与 Rust 实现对比的东西。

## 待办一项

封盘阈值表（`DEFAULT_THRESHOLDS`）目前定义在 `src/truerise/__main__.py`，而不是由
`truerise.evals` 导出。库调用方现在需显式传入；提升到库级导出已登记在
[`../docs/PROGRESS.md`](../docs/PROGRESS.md)。
