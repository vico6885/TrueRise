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

    /// Instability as per-mille integers (the float rule): discordant
    /// pairs / total pairs * 1000. Exact integer arithmetic throughout.
    #[must_use]
    pub fn instability_per_mille(&self) -> u64 {
        let n = self.outcomes.len();
        let total_pairs = n * (n - 1) / 2;
        let mut discordant = 0usize;
        for i in 0..n {
            for j in (i + 1)..n {
                if self.outcomes[i] != self.outcomes[j] {
                    discordant += 1;
                }
            }
        }
        // checked division: no manual zero guard, no panic path
        u64::try_from(discordant * 1000)
            .ok()
            .and_then(|n| n.checked_div(u64::try_from(total_pairs).ok()?))
            .unwrap_or(0)
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
    if resample.instability_per_mille() == 0 {
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
    fn instability_math_is_exact_integer() {
        // 4 runs, 1 dissent: pairs = 6, discordant = 3 -> 500 per-mille.
        let resample = Resample::new(vec![true, true, true, false]).expect("four trials");
        assert_eq!(resample.instability_per_mille(), 500);
    }
}
