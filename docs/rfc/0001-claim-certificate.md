# RFC 0001: Claim Certificate schema

- **Status**: Draft (needs two maintainer approvals per `docs/rfc/README.md`)
- **Started**: 2026-11-15
- **Implementation**: `python/src/truerise/evals/certify.py` (BenchCert),
  demo-grade precedent in `python/src/truerise/demo.py` (Verdict)

## 1. Motivation

Any public claim of the form "our agent improved X%" is unfalsifiable today:
the reader cannot know how many trials were run, whether thresholds moved
after the fact, whether the environment matched, or whether any evidence
exists beyond the model's self-report. The Claim Certificate makes such a
claim SHIPPABLE: a small JSON document that a third party can recompute
offline from a hash-chained ledger, without trusting the claimant.

One sentence: **every "we improved X%" claim should carry a certificate a
stranger can recompute.**

## 2. Schema (v1)

```jsonc
{
  "claim": "defect detection accuracy 100% on 120 tasks (trials=1)",
  "fingerprint": {
    "model": "solver:honest",       // what produced the scored behaviour
    "harness": "evals-runner",      // what mediated the trials
    "environment": "pack:<digest12>", // which sealed surface
    "evaluator": "truerise-evals/0.1",
    "digest": "<sha256-12>"         // over the canonical four-part join
  },
  "verdict": "PASS",                 // PASS | FAIL | MISMATCH (replay only)
  "reasons": [],                     // numbered per-violation lines
  "pack_digest": "<sha256>",         // public surface
  "answer_seal": "<sha256>",         // sealed surface (answers)
  "n_tasks": 120,
  "trials": 1,                       // runner-counted, monotonic per fingerprint
  "corrected_significance": 0.05,    // base / trials
  "accuracy": 1.0,
  "evidence_effective_level": 4,     // L0-L6; unsigned records contribute 0
  "per_family": { "...": {"correct": 0, "total": 0} },
  "failures": [],                    // miss inventory (submitted-vs-sealed)
  "prereg_seal": "<sha256-12>",      // thresholds sealed BEFORE the run
  "certificate_path": "",            // local; stripped in published readings
  "replay": ""                       // filled by --replay; fail-closed on tamper
}
```

Invariants (machine-checked):

1. `trials` is counted by the RUNNER, never declared by the solver; it is
   monotonic per fingerprint (a trial cannot be un-counted).
2. `corrected_significance = base / max(1, trials)`; the claim must beat it.
3. `evidence_effective_level` derives ONLY from executor-signed records;
   self-report is not evidence.
4. A verdict that cannot be appended to the hash chain is an ERROR, not a
   warning (unrecorded verdicts break auditability).
5. `--replay` recomputes the verdict from the governing ledger row; any
   divergence surfaces as `MISMATCH` (and the ledger itself fails closed on
   tamper with per-row location).

## 3. Versioning and stability

- Schema version rides on the `evaluator` field (`truerise-evals/0.1`).
- Field ADDITIONS are compatible; renames/removals require a new minor
  version and a migration note in this RFC.
- The demo-grade certificate (0.1-series `Verdict`) is the compatibility
  ancestor: field names it uses (claim/fingerprint/verdict/reasons/trials/
  corrected_significance/prereg_seal/replay) are FROZEN by the
  dual-implementation cross-certification gate (`tools/check_dual_impl.py`).

## 4. Security considerations

- The four-part fingerprint makes cross-environment comparison impossible by
  construction (comparing across registries is REFUSED, self-vs-self too).
- Thresholds are sealed before the run; the certificate names the keys of any
  post-seal mutation.
- The certificate contains no secrets; published readings strip
  machine-local paths (`certificate_path`).

## 5. Open questions

1. Should `accuracy` be accompanied by an interval (Wilson) for small n?
2. Should the schema gain an optional `second_judge` block for κ records
   once a second judge exists?
3. Canonical JSON profile for hashing (currently: `sort_keys`, no
   whitespace, UTF-8) — worth promoting to a named spec so other
   implementations cannot diverge (the seal-digest divergence caught by the
   cross-cert gate on 2026-11-15 argues yes).
