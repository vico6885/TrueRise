# truerise (Python 0.1-series — integrity-evaluation CLI)

The 60-second demo of the TrueRise integrity layer: three cheating scenarios,
each automatically caught. No network, no key, no GPU. **The real acceptance is
perturbation-flipping** — verdicts change with the inputs, nothing is hardcoded.

```bash
pip install -e python/          # from the repository root
python -m truerise demo         # 60 seconds, all three scenarios
python -m truerise demo --perturb n_trials=10     # verdict flips (multiplicity no longer fails)
python -m truerise demo --drop-fingerprint        # verdict flips (fingerprint mismatch fails)
python -m truerise demo --replay                  # offline recomputation of the last verdict
```

Scenario → catch mapping (spec §1.5):

| Scenario | Cheating behaviour | Caught by |
|---|---|---|
| S1 pay-later | claim dated in the future, thresholds mutated after seal | prereg gate (seal diff) + claim ledger (same fingerprint, second verdict) |
| S2 many-shots | run 1000 trials, report the best | multiplicity accountant (explicit corrected denominator) |
| S3 self-signed | conclusion with zero executor-signed records | evidence gate (auto-downgrade to claim) |

Wording note (implementation-freedom clause): the spec's sample output uses a
statistics term that collides with this repository's banned-token gate; the demo
prints `corrected significance` for the same quantity. Behaviour contract
(verdict flip / offline replay / certificate hash) is field-equivalent.
