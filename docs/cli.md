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
| `run --goal <text>` | a FULLY governed run: scripted model + tool execution through the sandbox port (RFC 0002) + budget (W3) + a judged verdict (W4). Three books printed; exit code carries the verdict |
| `run --approve <tool>` | record an approval ON FILE before the run (the only transition into ask-allowed; repeatable) |
| `run --budget <n>` | per-hat quota (rounds + tool units); over budget stops the run with exit `1` |
| `run --baseline <path>` | ratchet baseline for the verdict (default: `baselines/run_scores.json`, walked up from cwd) |
| `run --hat <toml>` | run under a hat: content hash registered on the chain, `tool_subset` becomes the OUTER policy gate; an unknown tool in the subset refuses the run at load |
| `run --provider openai\|anthropic --model <id>` | a REAL model behind the same governed port (budget/policy/verdict unchanged). `--provider-url <u>` targets local vLLM/Ollama/proxies. Keys env-only (`TRUERISE_API_KEY` / `ANTHROPIC_API_KEY`); a missing key fails closed. The run PINS to the model id, and the provider's own response echo backs the pinning check |
| `evolve --demo --approve <hash>` | one RSI generation (RFC 0003): propose -> hash-bound approval -> apply -> re-evaluate -> ratchet -> certificate; `--inject-regression` demonstrates the rollback; `--tape <path>` records every model call |
| `evolve --replay --cert <path>` | recompute a generation certificate OFFLINE (exit 0 = identical; a tampered cert is refused) |
| `verify --cert <path> [--issuer-key <hex>]` | verify a certificate signature (ed25519); an unsigned cert reports `unverified issuer` and exits `1` — never silent trust |
| `mcp --serve [--chain <dir>]` | the zero-exposure MCP server over stdio; every call lands on the chain |
| `repl` | interactive shell; slash commands `/demo`, `/audit`, `/quit`, `/exit` |
| `onboarding` | first-run wizard; writes `config.toml` and **never stores a key** (env or OS keyring only) |
| `help`, `--help`, `-h` | subcommand list |

No argument at all starts the `repl`. Unknown command prints help and exits `2`.
Keys for the signing/verify pair live in the environment ONLY
(`TRUERISE_SIGNING_KEY` / `TRUERISE_TRUSTED_KEY`); provider keys likewise
(`TRUERISE_API_KEY`).

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
