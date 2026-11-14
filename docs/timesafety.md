# Timesafety / LeakGuard (时序安全)

EN: Four leak vectors, each with a named enforcing test in
`crates/truerise-core/tests/leakguard.rs` and a declaration in
`tests/timesafety/four_vectors.toml` (status `enforced` since M1; the CI
job asserts both files). Half-open intervals everywhere: touching endpoints
do NOT overlap.

CR: 四个泄漏向量，各具名强制测试+four_vectors.toml 声明（M1 起 enforced；CI
断言两文件并存）。全库半开区间：端点相接不算重叠。

| Vector (spec id) | Enforcing test | Mechanism |
|---|---|---|
| future_disturbance_invariance | fit_apply_separation_with_negative_control | train-span statistic invariant under future-only disturbance; negative control MUST move |
| pit_boundary_rejection | pit_boundary_rejection | FoldWindow constructor refuses overlap AND purge shortfall at build time |
| label_uniqueness | sweep::detector_flags_exactly_the_overlapping_pairs | endpoint-ordered sweep (END before START at equal t) |
| fit_apply_separation | fit_apply_separation_with_negative_control | fit reads only `[train.start, train.end)` |

Property tests (proptest) run every CI job; two real defects were caught by
them during M1 (zero-purge overlap hole; sweep ordering) — the suite works.
proptest 每次必跑；M1 期间抓出两个真实缺陷（零 purge 重叠洞/扫描排序）——套件有效。
