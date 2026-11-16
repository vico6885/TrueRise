# Quickstart

[English](quickstart.md) | [简体中文](quickstart.zh-CN.md)

Ten minutes, no API key, no GPU, no network. Every output below was captured
from a real run on this repository.

## 1. Prerequisites

| Requirement | Value | Check |
|---|---|---|
| Rust | `stable` (MSRV floor 1.85, enforced by the `msrv` CI job) | `rustc --version` |
| Python | >= 3.10 (CI runs 3.12) | `python --version` |
| Secrets | none needed | — |

`rust-toolchain.toml` deliberately tracks `stable` rather than a fixed
version, so a fresh clone builds without toolchain negotiation.

## 2. Verify the engine

```bash
git clone <this repository> && cd truerise
cargo test --workspace           # 153 passed, 0 failed, zero warnings
python -m pip install -e python/
python -m pytest python/tests -q # 40 passed
```

## 3. First verdict (the 60-second walkthrough)

```bash
python -m truerise demo
```

Observed output (exit code 1, because the verdict is FAIL):

```
CLAIM     : self-improvement +23% (n=1000 trials, best-of)
FINGERPRINT: model=m* harness=h* env=e* evaluator=v*
VERDICT   : FAIL
            1) prereg diff: thresholds mutated after seal (keys: min_gain_pct, significance)
            2) trials=1000 -> corrected significance 5.0e-05, claim p=0.03 does not clear the corrected bar
            4) evidence: 0 executor-signed records (self-report only) -> effective L0
CERTIFICATE: <repo>/output/truerise/verdict.json
```

Nothing above is hardcoded — every line is derived from inputs. Flip the
inputs and watch individual lines disappear:

| Switch | Effect |
|---|---|
| `--honest-seal` | reason 1 disappears (thresholds now match the seal) |
| `--perturb n_trials=10` | the corrected denominator moves 5.0e-05 -> 5.0e-03 |
| `--sign-records` | reason 4 disappears (records become executor-signed) |
| `--honest` | all three flip -> `VERDICT: PASS`, exit code 0 |
| `--drop-fingerprint` | an integrity line appears (cross-environment comparison refused) |

## 4. Replay the verdict offline

```bash
python -m truerise demo --replay
```

Observed: `REPLAY : recomputed from ledger (N rows, chain verified): identical verdict`

## 5. Tamper drill (do this once — it is the point of the project)

Edit one verdict inside the persisted ledger, then replay:

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

Observed (fail-closed, with the row located):

```
truerise.ledger.LedgerTampered: row 0: content hash mismatch (tampered)
```

The Rust side proves the same property under concurrency and locates both
content-hash and prev-hash tampering:

```bash
cargo test -p truerise-governance --test chain_gate   # 6 tests, incl. 1000-append dual-writer competition
```

## 6. Run the bench

```bash
python -m truerise evals demo
```

Observed tail: `SUMMARY : {'honest': 'PASS', 'cheater': 'FAIL', 'mutator': 'FAIL'}`
with a 120-task pack (`pack: 120 tasks (pack_digest=...)`). The honest run
passes through the **same code path** that fails the two cheats; see
[`anti_cheat_bench.md`](anti_cheat_bench.md).

Individual cheats:

```bash
python -m truerise evals run --cheat --trials 20 --strategy majority  # hidden denominator
python -m truerise evals run --mutate                                  # thresholds moved after seal
```

## 7. Cross-certification (Rust vs Python must agree)

```bash
python tools/check_dual_impl.py
# cross-certification OK: rust and python certificates agree on all contract fields
```

## 8. Run it as a Rust binary

```bash
cargo run -p truerise-cli -- demo
cargo run -p truerise-cli -- demo --honest
cargo run -p truerise-cli -- run --goal "inspect the audit chain"   # tools + budget + verdict
cargo run -p truerise-cli -- repl          # slash commands; same books as the direct path
cargo run -p truerise-cli --example integrity_walkthrough
```

## 9. One RSI generation, recomputed offline

```bash
cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json
# replay: certificate recomputes identically (verdict PASS, failures 0)
```

The certificate was produced by a REAL governed run (propose -> hash-bound
human approval -> apply -> re-evaluate -> ratchet), embeds both skill
contents, and replays without keys or network. Tamper with one byte of it
and the replay refuses. The full protocol:
[`rfc/0003-generation-certificate-and-apply.md`](rfc/0003-generation-certificate-and-apply.md).

## 10. Where to go next

| I want to… | Read |
|---|---|
| understand what each flag does | [`cli.md`](cli.md) |
| contribute a task pack | [`task_pack_guide.md`](task_pack_guide.md) |
| plug in my own sandbox provider | [`provider_conformance.md`](provider_conformance.md) |
| understand why the chain is the authority | [`governance.md`](governance.md) |
| see what is NOT yet claimed | `../README.md` → Limitations |
