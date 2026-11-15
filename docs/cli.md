# CLI reference

[English](cli.md) | [简体中文](cli.zh-CN.md)

Two entry points share one behaviour contract. This page is the authoritative
list; if a flag is not here, it does not exist.

## Python entry point — `truerise`

Installed by `python -m pip install -e python/` (also runnable from a clone
with `PYTHONPATH=python/src`).

### `truerise demo` — the integrity walkthrough

Three cheating scenarios, each independently catchable.

| Flag | Default | Effect |
|---|---|---|
| `--perturb KEY=VALUE` | — | flip an input; `n_trials=10` moves the corrected denominator, `claim_p=0.001` clears the corrected bar |
| `--honest-seal` | off | thresholds match the seal -> prereg reason disappears |
| `--sign-records` | off | records carry an executor signature -> evidence reason disappears |
| `--honest` | off | all three scenarios honest -> `PASS` |
| `--drop-fingerprint` | off | ledger key marked dropped -> cross-environment comparison refused |
| `--out DIR` | `output/truerise` | certificate + ledger directory |
| `--replay` | off | offline recomputation from the hash-chained ledger |

**Exit code**: `0` when the verdict is `PASS`, `1` when `FAIL` (so CI can gate
on a verdict), `2` on a usage error.

### `truerise evals` — the Anti-Cheat Bench

| Subcommand | Flags | Effect |
|---|---|---|
| `evals gen` | `--out` (default `output/truerise_evals/pack`), `--per-family` (20), `--seed` (20261115) | generate a deterministic task pack; 6 families x `--per-family` = 120 rows by default |
| `evals run` | `--pack`, `--trials` (1), `--strategy` (`last`\|`majority`), `--cheat`, `--mutate`, `--work`, `--out` | run a reference solver over a pack and emit a claim certificate |
| `evals demo` | `--out`, `--per-family`, `--seed` | generate a pack, then run honest / cheater / mutator |

**Exit code for `evals demo`**: `0` **only if** the acceptance contract holds —
`{honest: PASS, cheater: FAIL, mutator: FAIL}`. Any other outcome is `1`. The
acceptance test is therefore the exit code, not the printed text.

**Exit code for `evals run`**: `0` on `PASS`, `1` on `FAIL`.

## Rust entry point — `truerise-cli`

```bash
cargo run -p truerise-cli -- <command>
```

| Command | Effect |
|---|---|
| `demo [--honest]` | the same three-scenario walkthrough, Rust edition |
| `run --goal <text>` | a governed minimal run: transcript + state-transition log + audit rows populated and mutually consistent |
| `repl` | interactive shell; slash commands `/demo`, `/audit`, `/quit`, `/exit` |
| `onboarding` | first-run wizard; writes `config.toml` and **never stores a key** (env or OS keyring only) |
| `help`, `--help`, `-h` | subcommand list |

No argument at all starts the `repl`. Unknown command prints help and exits `2`.

### Exit codes are part of the certified contract

| Command | Rust | Python |
|---|---|---|
| `demo` (cheating inputs) | `1` | `1` |
| `demo --honest` | `0` | `0` |
| unknown command | `2` | `2` (argparse usage error) |

Both sides map verdict FAIL to `1`, and the cross-certification gate now
compares the exit code alongside the certificate fields — including a check
that each side's code agrees with **its own** verdict, so both sides agreeing
on a useless constant would still fail the gate.

History worth keeping: the Rust CLI used to return `0` even on a FAIL verdict,
and `tools/check_dual_impl.py` compared certificate fields only. A CI lane
gating on the Rust binary's exit code therefore would not have gone red. That
divergence is now closed by `crates/truerise-cli/tests/exit_codes.rs` plus the
exit-code comparison in the gate.

## Guarantees that hold across both entry points

| Guarantee | Enforcement |
|---|---|
| Same fingerprint -> same verdict, field for field | `tools/check_dual_impl.py` (CI job `cross-cert`) |
| A verdict that cannot be recorded is an error | `judge_and_record`, `cargo test -p truerise-eval` |
| Score regression is a non-zero condition | ratchet, `cargo test -p truerise-eval` |
| The REPL is an IO adapter, not a second engine | `cargo test -p truerise-cli --test golden_repl` |
| Tampering fails closed and names the row | `cargo test -p truerise-governance --test chain_gate`; Python `LedgerTampered` |
| No key ever lands in config, argv or logs | `SECURITY.md`, onboarding skeleton, observe redaction test |
