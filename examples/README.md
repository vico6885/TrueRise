# examples

Every entry below is runnable on a fresh clone, no keys, no network.

| What it shows | Run |
|---|---|
| The 60-second integrity walkthrough (Python package): three cheating scenarios, each caught; verdict flips with inputs | `python -m truerise demo` (see `python/README.md` for perturbation switches) |
| The same walkthrough, Rust CLI edition | `cargo run -p truerise-cli -- demo` |
| Embedding the walkthrough as a library call (derive verdicts yourself) | `cargo run -p truerise-cli --example integrity_walkthrough` |
| Dual-implementation cross-certification: both certificates must agree | `python tools/check_dual_impl.py` |
| Interactive REPL (slash commands, audit on chain) | `cargo run -p truerise-cli -- repl` |
| The Anti-Cheat Bench (task packs, sealed answers, claim certificates) | `python -m truerise evals --help` (see `docs/anti_cheat_bench.md`) |
