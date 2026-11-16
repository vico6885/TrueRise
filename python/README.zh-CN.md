# truerise（Python 包）

[English](README.md) | [简体中文](README.zh-CN.md)

TrueRise 的 Python 侧：60 秒诚信走查（`truerise.demo`）、Anti-Cheat Bench
（`truerise.evals`）、长程闭环题包、外部解题器协议，以及零依赖 SDK
（`truerise.sdk`）。不需要网络、不需要 API 密钥、不需要 GPU。**验收判据是判词随输入
移动** —— 没有任何写死的东西，且退出码承载判词。

```bash
python -m pip install -e python/          # 在仓库根目录执行
python -m pytest python/tests -q          # 40 个测试
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

## 长程闭环题包

多步确定性流水线，终态可机判；只有**每一步**都与封存值一致才算闭环完成
（跳步拿到正确终值也是 FAIL——考的是闭环，不是终点）：

```python
from truerise.evals.long_horizon import generate_long_horizon_pack, run_long_horizon

pack = generate_long_horizon_pack(out_dir, chains=8, seed=20261116)  # 字节级可复现
summary = run_long_horizon(my_solver, pack, work_dir)                # 只有整数
```

## 外部解题器（单文件契约）

任何读取 `TRUERISE_PUBLIC_DIR`、向 `TRUERISE_OUT_DIR` 写 `submission.jsonl`
的程序都能被基准评测；由 runner 判卷并对每条记录做**执行器签名**
（解题器自己的准确率声明是自报，封顶 L0）：

```python
from truerise.evals.runner import run_external_trials

passes, signed = run_external_trials(["python", "my_solver.py"], pack, work_dir)
```

协议参考实现：`truerise/evals/external_solver.py`。

## SDK（治理面，纯 Python、零依赖）

```python
from truerise.sdk import (                                # 与 Rust 契约互为镜像
    Session, Policy, ApprovalQueue, Budget, Ratchet,
    Fingerprint, seal_thresholds, prereg_diff, evidence_gate,
)
```

`Session` 就是受治理的运行环：三账同时记，工具请求走 RFC 0002 的
判定 -> 执行 -> 审计链，纯文本永不执行。可选的原生加速后端
`truerise-native`（PyO3 轮子）与纯 Python 路径语义一致——`native_available()`
探测，缺了不影响任何功能。

## 库面

```python
from truerise.sdk import (                                # 受治理 harness 的 Python 面
    Session, Policy, ApprovalQueue, Budget, Ratchet,
    Fingerprint, seal_thresholds, prereg_diff, evidence_gate,
)
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

## 已知缺口

封盘阈值表（`DEFAULT_THRESHOLDS`）目前定义在 `src/truerise/__main__.py`（CLI 侧），
而非由 `truerise.evals` 导出。库调用方需显式传入阈值表——这本来就是推荐的契约：
阈值应由调用方在跑之前封印（`truerise.gates.seal_thresholds`）。
