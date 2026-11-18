# Tests

[English](README.md) | [简体中文](README.zh-CN.md)

Two suites, both run in CI, neither skippable.

## Rust — 240 tests (`cargo test --workspace`)

| Surface | What it pins |
|---|---|
| `truerise-core/tests/leakguard.rs` | the four leak vectors, two of them under proptest; caught two real defects during construction |
| `truerise-core/tests/terminal_absorb.rs` + `compile_fail/` | a terminal transition does not compile — the absorption rule is a type-level property |
| `truerise-governance/tests/chain_gate.rs` | dual-writer competition (2 handles, 1000 appends, 0 lost updates), tamper located by row (content hash and prev hash), idempotent replay, persistence reload, legacy rows that never govern |
| `truerise-cli/tests/golden_repl.rs` | the REPL adapter and the direct loop path produce field-identical books |
| `truerise-cli/tests/exit_codes.rs` | the exit code carries the verdict (FAIL → 1, honest → 0, unknown command → 2) — a gate that cannot go red is a note |
| unit tests in each crate | policy three states, approval never times into consent, deletion gates, ratchet only tightens, cross-environment comparison refused, budget singleton reconciliation, evolve never merges, hats content hash, MCP zero exposure and write-class denial, observe redaction, memory fusion determinism, compaction monotonicity, backoff classes |

Per-behaviour mapping is machine-checked: `spec_map.json` binds 54 behaviour
rows to 210 named test identifiers, and `python tools/lint_spec_map.py` fails
when a row loses its mapping or an identifier stops existing.

## Python — 40 tests (`python -m pytest python/tests -q`)

| File | What it pins |
|---|---|
| `python/tests/test_demo_perturbation.py` | verdict flips with inputs; nothing hardcoded; offline replay equals the stored certificate |
| `python/tests/test_evals_bench.py` | pack scale and taxonomy, generator determinism, public surface carries no sealed hint, canary leak fails closed, judge exact on both axes, honest PASS / cheater FAIL / mutator FAIL, tamper fails closed on replay, zero trials refused, majority cannot un-count losing trials, classification covers every family |
| `python/tests/test_sdk.py` | the SDK surface: ledger tamper detection, policy three-state parity with Rust, budget semantics, external-solver isolation |

Markers: none registered in `pyproject.toml` (the marker list that used to
live here described an aspiration, not the code - removed).

## What is NOT tested here

- Any external model run — none exists yet (`README.md` → Limitations).
- Sandbox providers other than the in-process reference (`providers/README.md`).
- The criterion benchmark *comparison*: the first CI run only saves the
  `main` baseline; the 20% gate activates from the second run onward.

## Gates that are not tests

`tools/` holds the repository gates (each exits non-zero on violation):
workspace banned-token door, history-object door and blob floor, file length,
licence isolation, schema alignment, spec mapping, DCO, timesafety enforcement,
SBOM, dual-implementation cross-certification, documentation contract.
