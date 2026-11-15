# benches

Criterion baselines for the hot paths, one file per crate. The CI
`benchmark` job saves the `main` baseline on default-branch runs and applies
a **20% regression gate** (critcmp) from the second run onward — a score
regression is a ratchet violation, not a note.

| Baseline | What it covers | Run |
|---|---|---|
| `truerise-governance/benches/chain_baseline.rs` | hash-chain append + full verify (the audit chain is on every verdict's write path) | `cargo bench -p truerise-governance` |
| `truerise-eval/benches/eval_baseline.rs` | fingerprint digest, ratchet tighten/verify, conformance judging | `cargo bench -p truerise-eval` |

Compare two baselines locally:

```bash
cargo bench -- --save-baseline before
# ...change something...
cargo bench -- --save-baseline after
cargo install critcmp --locked
critcmp before after --threshold 20
```
