# 快速上手

[English](quickstart.md) | [简体中文](quickstart.zh-CN.md)

十分钟，不需要 API 密钥、不需要 GPU、不需要网络。下面每一段输出都来自本仓的真实运行截取。

## 1. 前置条件

| 要求 | 取值 | 自检 |
|---|---|---|
| Rust | `stable`（MSRV 下限 1.85，由 CI 的 `msrv` job 执法） | `rustc --version` |
| Python | >= 3.10（CI 跑 3.12） | `python --version` |
| 密钥 | 不需要 | — |

`rust-toolchain.toml` 刻意跟随 `stable` 而不钉死版本，这样新克隆的机器不需要协商工具链就能构建。

## 2. 验证引擎

```bash
git clone <本仓库> && cd truerise
cargo test --workspace           # 153 passed, 0 failed，零告警
python -m pip install -e python/
python -m pytest python/tests -q # 25 passed
```

## 3. 第一份判词（60 秒走查）

```bash
python -m truerise demo
```

实测输出（退出码 1，因为判词是 FAIL）：

```
CLAIM     : self-improvement +23% (n=1000 trials, best-of)
FINGERPRINT: model=m* harness=h* env=e* evaluator=v*
VERDICT   : FAIL
            1) prereg diff: thresholds mutated after seal (keys: min_gain_pct, significance)
            2) trials=1000 -> corrected significance 5.0e-05, claim p=0.03 does not clear the corrected bar
            4) evidence: 0 executor-signed records (self-report only) -> effective L0
CERTIFICATE: <仓库路径>/output/truerise/verdict.json
```

上面没有任何一行是写死的，全部由输入推导。改输入，对应那行就消失：

| 开关 | 效果 |
|---|---|
| `--honest-seal` | 判词 1 消失（阈值与封盘一致） |
| `--perturb n_trials=10` | 校正分母从 5.0e-05 移到 5.0e-03 |
| `--sign-records` | 判词 4 消失（记录带上执行器签名） |
| `--honest` | 三条一起翻 → `VERDICT: PASS`，退出码 0 |
| `--drop-fingerprint` | 出现完整性判词行（跨环境比较被拒绝） |

## 4. 离线重算判词

```bash
python -m truerise demo --replay
```

实测：`REPLAY : recomputed from ledger (N rows, chain verified): identical verdict`

## 5. 篡改演练（请务必做一次，这才是本项目的重点）

改动持久化账本里的一条判词，然后重算：

```bash
mkdir -p output/tamper_test
cp output/truerise/claims.jsonl output/truerise/verdict.json output/tamper_test/
python - <<'PY'
import json, pathlib
p = pathlib.Path('output/tamper_test/claims.jsonl')
rows = p.read_text(encoding='utf-8').splitlines()
last = json.loads(rows[-1]); last['detail']['verdict'] = 'PASS'
rows[-1] = json.dumps(last, ensure_ascii=False)
p.write_text('\n'.join(rows) + '\n', encoding='utf-8')
PY
python -m truerise demo --replay --out output/tamper_test
```

实测（失败关闭，并定位到行）：

```
truerise.ledger.LedgerTampered: row 0: content hash mismatch (tampered)
```

Rust 侧在同一性质上再加并发与两种篡改的定位：

```bash
cargo test -p truerise-governance --test chain_gate   # 6 个测试，含 1000 次追加的双写者竞争
```

## 6. 跑基准

```bash
python -m truerise evals demo
```

实测末行：`SUMMARY : {'honest': 'PASS', 'cheater': 'FAIL', 'mutator': 'FAIL'}`，题包 120 题（`pack: 120 tasks (pack_digest=...)`）。诚实运行走的是**同一条代码路径**，却在两种作弊上失败；协议见 [`anti_cheat_bench.zh-CN.md`](anti_cheat_bench.zh-CN.md)。

单独触发某一种作弊：

```bash
python -m truerise evals run --cheat --trials 20 --strategy majority  # 藏分母
python -m truerise evals run --mutate                                  # 封盘后改阈值
```

## 7. 双实现互证（Rust 与 Python 必须一致）

```bash
python tools/check_dual_impl.py
# cross-certification OK: rust and python certificates agree on all contract fields
```

## 8. 用 Rust 单二进制跑

```bash
cargo run -p truerise-cli -- demo
cargo run -p truerise-cli -- demo --honest
cargo run -p truerise-cli -- run --goal "inspect the audit chain"   # 工具 + 预算 + 判词
cargo run -p truerise-cli -- repl          # 斜杠命令；产出与直连路径逐字段相同
cargo run -p truerise-cli --example integrity_walkthrough
```

## 9. 一代 RSI，离线复算

```bash
cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json
# replay: certificate recomputes identically (verdict PASS, failures 0)
```

这份证书由一次真实受治理 run 产出（提案 -> 哈希绑定的人工批准 -> 应用 -> 重评 ->
棘轮），内嵌两代技能全文，重放无需密钥、无需网络。改动证书任何一个字节，重放即拒。
完整协议见 [`rfc/0003-generation-certificate-and-apply.md`](rfc/0003-generation-certificate-and-apply.md)。

## 10. 接下来读什么

| 我想…… | 读这里 |
|---|---|
| 弄清每个开关做什么 | [`cli.zh-CN.md`](cli.zh-CN.md) |
| 贡献一个题包 | [`task_pack_guide.zh-CN.md`](task_pack_guide.zh-CN.md) |
| 接我自己的沙箱提供方 | [`provider_conformance.zh-CN.md`](provider_conformance.zh-CN.md) |
| 理解为什么哈希链是唯一权威 | [`governance.md`](governance.md) |
| 看哪些东西我们还没宣称 | `../README.zh-CN.md` 的「局限」段 |
