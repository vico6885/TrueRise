# Repository topology and dependency direction

> Bilingual: English first, 中文在后。This document is the machine-checked
> DAG statement the open-source review requires; the enforcement points are
> listed per rule.

## Dependency direction (one-way, leaf = authority)

```
        truerise-core / truerise-wire / truerise-platform      (leaf: contracts)
                    ▲
        truerise-governance / truerise-sandbox                  (authority engines)
                    ▲
        truerise-eval (judging authority)  truerise-memory  truerise-rsi
                    ▲                          truerise-hats  truerise-mcp
        truerise-cli (single binary)           truerise-observe
                    ▲
        truerise (Python package: 0.1-series demo + truerise.evals)
```

Rules (each with its enforcement point):

1. **Leaf crates depend on nothing in-repo.** `truerise-core`, `truerise-wire`,
   `truerise-platform` have no path dependencies —
   `cargo tree -p truerise-core --depth 1` shows no workspace crates.
2. **The Python package depends on the same contracts BY BEHAVIOUR, not by
   import**: the dual implementation shares the spec, and CI re-proves field
   equivalence every push (`tools/check_dual_impl.py`, `cross-cert` job).
3. **No cycles**: `cargo metadata` dependency graph is acyclic (Rust
   enforces this at build time).
4. **License isolation stays intact**: `tools/lint_deny_isolation.py` +
   `deny.toml` keep the non-Apache/MIT license families out of the open
   workspace; the internal production deployment of the same contracts
   lives OUT of this repository and never merges in (its BSL grant is a
   separate asset — see README Limitations).
5. **Pin policy for external consumers**: crates are versioned
   `0.2.0-dev.0` until the first public tag; after that, external consumers
   pin EXACT versions (the Python package pins the same way via pip); the
   wire contracts (`truerise-wire`) are the only cross-language surface and
   change only via RFC.

## 单仓形态说明（中文）

本仓 = 全部开源内容（Rust 权威实现 + Python 0.1 系列 + Anti-Cheat Bench 门面）。
生产消费方（BSL 授权的内部部署）不进入本仓，只以 README Limitations 的方式
诚实声明其存在。三条铁律：许可隔离门不改；Python 与 Rust 走行为契约互证不共码；
对外依赖一律精确版本钉死。
