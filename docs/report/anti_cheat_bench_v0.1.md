# The Anti-Cheat Bench: an integrity-evaluation layer for agentic claims

[English](anti_cheat_bench_v0.1.md) | [简体中文](anti_cheat_bench_v0.1.zh-CN.md)

*TrueRise technical report, v0.1 — 2026-11-15. Every number in this report
points at a committed artifact under `readings/v0.1/` or a test you can run;
nothing is prose-only.*

---

## 1. The problem: claims are unfalsifiable

Agentic-evaluation claims fail in four repeatable ways:

1. **p-hacking** — run N trials, report the best one, never state N.
2. **Threshold mutation** — pick the bar after seeing the results.
3. **Self-report as evidence** — the model asserts its own score; nothing
   executed, nothing signed.
4. **Cross-environment comparison** — scores from different stacks compared
   as if they were commensurable.

Each failure has a mechanical catch. The Anti-Cheat Bench is the smallest
artifact that demonstrates all four catches end-to-end, on a public task
pack, with an offline-recomputable certificate — and it refuses to publish
any claim that cannot survive them.

## 2. Design

**Task packs.** One JSONL row = one scoring-pipeline fixture hiding exactly
one defect from a six-family taxonomy (future information use, full-sample
statistic applied inside the window, backward fill, same-bar execution,
outcome-correlated row dropping, unit/calendar mismatch). The pack is
GENERATED, not hand-written: one seeded RNG per family expands six templates
into 120 tasks (`readings/v0.1/pack_manifest.json`); determinism is
test-pinned (same seed → byte-identical surfaces).

**Two surfaces.** `tasks.jsonl` is public; `answers.jsonl` is sealed and
embeds an isolation canary. The runner copies the public surface into a
fresh directory and hands the solver that path ONLY; it scans for the canary
before grading and fails closed on a leak
(`test_materialize_public_fail_closed_on_canary`). The task rows carry no
family field — publishing it would leak the answer.

**Trial mediation.** A solver cannot run hidden trials: every trial goes
through the runner, and the certificate's trial count is the runner's count.
Selecting after the fact (majority, best-of) aggregates the same counted
trials; it cannot un-count the losing ones (`test_aggregate_majority_cannot_un_count`).

**Certificates.** Each run ends in a Claim Certificate (RFC 0001): verdict,
numbered reasons, runner-counted trial denominator, corrected significance
bar (base/N), evidence level derived only from executor-signed records,
threshold seal, pack digest, answer seal, and a `--replay` that recomputes
the verdict from the hash-chained ledger (fail-closed on tamper, per-row
location).

**Dual implementation.** The same integrity walkthrough exists in Rust
(`cargo run -p truerise-cli -- demo`) and Python
(`python -m truerise demo`). CI requires the two certificates to agree on
verdict, claim, fingerprint, seal digest, corrected bar, and the set of
failure kinds (`tools/check_dual_impl.py`, the `cross-cert` job) — same
fingerprint, same verdict, or red.

## 3. Readings

All artifacts are committed under `readings/v0.1/`; regenerate any of them
with `python -m truerise evals demo` (deterministic pack, seed 20261115).

| Run | Verdict | Key line | Artifact |
|---|---|---|---|
| honest, 1 trial | PASS | accuracy 100% (120/120), zero reasons | `honest_verdict.json` |
| cheater, 20 trials, majority | FAIL | `trials=20 -> corrected significance 2.5e-03, claim p=0.05 does not clear the corrected bar` | `cheater_verdict.json` |
| mutator, thresholds 0.5/0.03 after sealing 0.8/0.05 | FAIL | `prereg diff: thresholds mutated after seal (keys: min_accuracy, claim_p)` | `mutator_verdict.json` |

The Rust/Python cross-certification gate passes both modes on this machine
(`tools/check_dual_impl.py`, exit 0; CI re-runs it as the `cross-cert` job).

**The failure list is the point.** During construction the reference solver
scored 100/120 with ALL 20 misses concentrated in one family — a real bug in
the solver's rule (a regex required an index token after the statistic name),
not in the pack. The miss inventory is exactly what surfaced it; the fix and
the re-run are recorded in `CHANGELOG.md`. We publish the
process because a bench that never shows its own misses is selling the same
unfalsifiability it exists to remove.

## 4. What this does NOT claim (Limitations)

- **Detection difficulty.** The v0.1 reference solver reaches 100% because
  the rules encode the same taxonomy shapes the generator uses (one source
  of truth). v0.1 demonstrates the CATCH machinery, not detector strength;
  harder packs (same-family safe twins, direction discrimination) are v0.2.
- **No external model run yet.** Every reading above is from our own
  reference solvers. The external-solver protocol has landed; the pending
  credibility step — one public model behind a contract-honouring solver,
  readings committed — is owned by `ROADMAP.md` item 1 and must land before
  any third-party claim of adoption.
- **In-process evidence is capped.** Reference solvers run in-process;
  their records are executor-signed by the runner but the v0.1 evidence
  ladder treats deeper execution (sandboxed subprocesses) as the v0.2 path
  via `truerise-sandbox` conformance.
- **No RL training-loop results.** The runtime exposes interfaces a
  training loop can consume safely (anti-hack gate); we claim no training
  results.
- **Solo project.** One author; review and external contribution are
  governed by `CONTRIBUTING.md` (DCO + IP-clean declarations) and the RFC
  process.

## 5. Verify it yourself

```bash
git clone <this repository> && cd truerise
cargo test --workspace                       # 101 Rust tests
python -m pip install -e python/ && python -m pytest python/tests -q   # 25 tests
python -m truerise evals demo                # honest PASS, cheater FAIL, mutator FAIL
python -m truerise demo --replay             # offline recomputation
cargo run -p truerise-cli -- demo            # Rust walkthrough, same verdict
python tools/check_dual_impl.py              # cross-certification gate
```

---

*Contact: open a GitHub issue or a PR (DCO required). The Claim Certificate
schema is open for review as RFC 0001 — the two-maintainer approval rule
starts with this repository's first external review.*
