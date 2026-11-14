# Timesafety (LeakGuard) contract skeleton

Placeholder required by the `timesafety-placeholder` CI job (existence
assertion via `tools/check_leakguard_skeleton.py`). The four leak vectors
are DECLARED in `four_vectors.toml` with status `planned` at M0; the M1
wave flips them to `enforced` with property tests. This job must never
be skipped; the not-doing entry carries the 2-week rollback clock.
