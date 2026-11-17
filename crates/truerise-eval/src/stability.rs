//! Resampling-consistency check (G27): the same input under a fixed budget
//! is run k times; if the conclusions DISAGREE, the conclusion is by
//! definition unreliable - it cannot be promoted, and the instability is
//! reported as an integer (per-mille) into the certificate.
//!
//! Boundary: this does NOT detect truth - it detects UNSTEADINESS. What
//! harms a training decision is precisely "unstable yet treated as settled"
//! (the consensus wording), so an unstable conclusion is forced into the
//! G7 family: non-promotable, never readable as PASS, never as "no
//! difference".

/// The repeated outcomes of ONE task/conclusion under a fixed budget
/// (same input, k runs; a `true` is a pass).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resample {
    outcomes: Vec<bool>,
}

impl Resample {
    /// Build from k outcomes. `None` when fewer than two trials - a single
    /// run cannot speak about stability at all (the consistency question
    /// needs at least two answers).
    #[must_use]
    pub fn new(outcomes: Vec<bool>) -> Option<Self> {
        if outcomes.len() < 2 {
            None
        } else {
            Some(Self { outcomes })
        }
    }

    /// How many trials the check consumed.
    #[must_use]
    pub fn trials(&self) -> usize {
        self.outcomes.len()
    }

    /// Discordant pair count (the RAW disagreement signal). Review fix
    /// (P1): the stability verdict keys off THIS, not the per-mille ratio
    /// (integer division rounded a single dissent among 46 trials down to
    /// zero and read it as Stable).
    #[must_use]
    pub fn discordant_pairs(&self) -> u64 {
        let n = self.outcomes.len();
        let mut discordant = 0u64;
        for i in 0..n {
            for j in (i + 1)..n {
                if self.outcomes[i] != self.outcomes[j] {
                    discordant += 1;
                }
            }
        }
        discordant
    }

    /// Instability as per-mille integers (the float rule): discordant
    /// pairs / total pairs * 1000. REPORTING ONLY - never gates.
    #[must_use]
    pub fn instability_per_mille(&self) -> u64 {
        let n = self.outcomes.len();
        let total_pairs = n * (n - 1) / 2;
        if total_pairs == 0 {
            return 0;
        }
        let scaled = usize::try_from(self.discordant_pairs())
            .unwrap_or(usize::MAX)
            .saturating_mul(1000);
        u64::try_from(scaled)
            .ok()
            .and_then(|v| v.checked_div(u64::try_from(total_pairs).ok()?))
            .unwrap_or(u64::MAX) // saturation reports MAX, never a fake zero
    }
}

/// The stability verdict of one resampled conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stability {
    /// Every repeated outcome agrees - the conclusion is steady (still not
    /// "true": steadiness is not truth, see docs/limits.md).
    Stable,
    /// The outcomes disagree - the conclusion is UNSTEADY and cannot be
    /// promoted (it may not be reported as settled either way).
    Unstable {
        /// Instability as per-mille integers (goes into the certificate).
        instability_per_mille: u64,
    },
}

impl Stability {
    /// The enforcement rule: an unsteady conclusion blocks promotion -
    /// exactly the G7 family (non-promotable, distinct from both PASS and
    /// "no difference").
    #[must_use]
    pub fn blocks_promotion(&self) -> bool {
        matches!(self, Self::Unstable { .. })
    }
}

/// Check one resampled conclusion.
#[must_use]
pub fn check(resample: &Resample) -> Stability {
    // Review fix (P1): ANY discordant pair makes the conclusion unsteady -
    // the per-mille ratio is reporting only; its truncation must never
    // downgrade a real dissent into Stability.
    if resample.discordant_pairs() == 0 {
        Stability::Stable
    } else {
        Stability::Unstable {
            instability_per_mille: resample.instability_per_mille(),
        }
    }
}

/// The certificate row (integers only): verdict + per-mille instability +
/// trial count - nothing else, so the replay recomputes all of it.
#[must_use]
pub fn certificate_row(resample: &Resample) -> serde_json::Value {
    let verdict = check(resample);
    serde_json::json!({
        "stability": match verdict {
            Stability::Stable => "stable",
            Stability::Unstable { .. } => "unstable",
        },
        "instability_per_mille": resample.instability_per_mille(),
        "trials": resample.trials(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn unstable_conclusion_cannot_be_promoted() {
        // One dissent among five runs: the conclusion is unsteady, so it
        // cannot be promoted - the guard is on the verdict type itself.
        let resample = Resample::new(vec![true, true, false, true, true]).expect("five trials");
        let verdict = check(&resample);
        assert!(verdict.blocks_promotion());
        assert!(matches!(verdict, Stability::Unstable { .. }));
    }

    #[test]
    fn instability_reported_in_certificate() {
        // 3 agree / 2 disagree over 5 runs: pairs = 10, discordant pairs =
        // 2*3 = 6 -> instability = 600 per-mille (exact integer math).
        let resample = Resample::new(vec![true, true, true, false, false]).expect("five trials");
        assert_eq!(resample.instability_per_mille(), 600);
        let row = certificate_row(&resample);
        assert_eq!(row["stability"], serde_json::json!("unstable"));
        assert_eq!(row["instability_per_mille"], serde_json::json!(600));
        assert_eq!(row["trials"], serde_json::json!(5));
    }

    #[test]
    fn stable_conclusion_is_steady_but_not_true() {
        let resample = Resample::new(vec![false, false, false]).expect("three trials");
        let verdict = check(&resample);
        assert!(!verdict.blocks_promotion());
        assert_eq!(resample.instability_per_mille(), 0);
        // The boundary is in the TYPE name: Stable, never "True".
        assert!(matches!(verdict, Stability::Stable));
    }

    #[test]
    fn stability_requires_at_least_two_trials() {
        assert!(
            Resample::new(vec![true]).is_none(),
            "one run cannot speak about stability"
        );
        assert!(Resample::new(vec![]).is_none());
    }

    #[test]
    fn one_dissent_among_many_trials_is_never_stable() {
        // Review fix (P1): 1 discordant pair among 1035 (n=46) truncated
        // to 0 per-mille and read as Stable - a real dissent, now caught.
        let mut outcomes = vec![true; 45];
        outcomes.push(false);
        let resample = Resample::new(outcomes).expect("46 trials");
        assert_eq!(resample.discordant_pairs(), 45);
        assert!(check(&resample).blocks_promotion());
    }

    #[test]
    fn instability_math_is_exact_integer() {
        // 4 runs, 1 dissent: pairs = 6, discordant = 3 -> 500 per-mille.
        let resample = Resample::new(vec![true, true, true, false]).expect("four trials");
        assert_eq!(resample.instability_per_mille(), 500);
    }
}
