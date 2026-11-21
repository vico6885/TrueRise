//! The ratchet: a baseline that only moves DOWN (spec §6: 棘轮只减不增).
//!
//! A baseline maps metric names to f64 thresholds. A NEW baseline may only
//! ever TIGHTEN (lower) the thresholds; any loosening (a metric allowed to
//! get worse) is a [`RatchetViolation`]. Injecting a score regression into a
//! verified run must fail with exit-grade severity.

use std::collections::BTreeMap;

/// A metric baseline: metric -> max allowed (regression = worse than max).
pub type Baseline = BTreeMap<String, f64>;

/// Ratchet violations (all are exit-grade).
#[derive(Debug, thiserror::Error)]
pub enum RatchetViolation {
    /// A new baseline loosened a threshold (ratchet only tightens).
    #[error("ratchet violation: metric {metric} loosened ({old} -> {new})")]
    Loosened {
        /// Metric name.
        metric: String,
        /// Old (tighter) threshold.
        old: f64,
        /// New (looser) threshold.
        new: f64,
    },
    /// A verified score regressed past the baseline.
    #[error("ratchet violation: metric {metric} regressed ({value} > allowed {allowed})")]
    Regression {
        /// Metric name.
        metric: String,
        /// Actual value.
        value: f64,
        /// Allowed maximum.
        allowed: f64,
    },
}

/// The gated propose's two failure faces: the write gate refused the
/// citation, or the ratchet refused the direction.
#[derive(Debug, thiserror::Error)]
pub enum GatedProposeError {
    /// No support reference (or the chain sealed) - the write never
    /// happened.
    #[error("baseline update refused by the write gate: {0}")]
    Gate(#[from] truerise_governance::WriteGateError),
    /// The candidate loosened a threshold.
    #[error("baseline update refused by the ratchet: {0}")]
    Ratchet(#[from] RatchetViolation),
}

/// Validate that ``candidate`` only tightens relative to ``baseline``.
///
/// # Errors
/// [`RatchetViolation::Loosened`] per loosened metric.
pub fn tighten_only(baseline: &Baseline, candidate: &Baseline) -> Result<(), RatchetViolation> {
    for (metric, new_val) in candidate {
        if let Some(old_val) = baseline.get(metric) {
            if new_val > old_val {
                return Err(RatchetViolation::Loosened {
                    metric: metric.clone(),
                    old: *old_val,
                    new: *new_val,
                });
            }
        }
    }
    Ok(())
}

/// Verify a run's scores against the active baseline.
///
/// # Errors
/// [`RatchetViolation::Regression`] per regressed metric.
pub fn verify_scores(baseline: &Baseline, scores: &Baseline) -> Result<(), RatchetViolation> {
    for (metric, allowed) in baseline {
        if let Some(value) = scores.get(metric) {
            if value > allowed {
                return Err(RatchetViolation::Regression {
                    metric: metric.clone(),
                    value: *value,
                    allowed: *allowed,
                });
            }
        }
    }
    Ok(())
}

/// The active ratchet (holds the current baseline; new baselines merge by
/// tightening only).
#[derive(Debug, Default)]
pub struct Ratchet {
    baseline: Baseline,
}

impl Ratchet {
    /// Start from an initial baseline (e.g. loaded from a checked-in file).
    pub fn new(initial: Baseline) -> Self {
        Self { baseline: initial }
    }

    /// The active baseline.
    pub fn baseline(&self) -> &Baseline {
        &self.baseline
    }

    /// Propose a new baseline: accepted only when it tightens every metric.
    ///
    /// # Errors
    /// [`RatchetViolation::Loosened`]; the ratchet is NOT mutated on error.
    pub fn propose(&mut self, candidate: &Baseline) -> Result<(), RatchetViolation> {
        tighten_only(&self.baseline, candidate)?;
        for (metric, value) in candidate {
            self.baseline.insert(metric.clone(), *value);
        }
        Ok(())
    }

    /// G29: a baseline update THROUGH the persistent-state write gate.
    ///
    /// The GATED baseline path's policy is REFUSE: a persisted ratchet
    /// move without a support reference is refused - the gated path never
    /// admits an unverified number (acceptance:
    /// `baseline_update_requires_recomputation_support`). The un-gated
    /// `propose` remains for the Python SDK surface and the bench, whose
    /// run-path gating is an explicitly declared deferral (review B3-P2-8).
    ///
    /// # Errors
    /// [`GatedProposeError::Gate`] without a support reference; [`GatedProposeError::Ratchet`]
    /// when the candidate loosens a threshold.
    pub fn propose_gated(
        &mut self,
        candidate: &Baseline,
        support: Option<&truerise_governance::SupportRef>,
        ledger: &mut truerise_governance::UnverifiedLedger,
        chain: Option<&truerise_governance::HashChain>,
        generation: u64,
    ) -> Result<(), GatedProposeError> {
        let assertion = serde_json::to_string(candidate)
            .unwrap_or_else(|_| "baseline:<unserializable>".to_string());
        let gate = truerise_governance::WriteGate::new(generation);
        gate.decide(
            truerise_governance::WritePath::BaselineUpdate,
            &assertion,
            support,
            ledger,
            chain,
        )
        .map_err(GatedProposeError::Gate)?;
        self.propose(candidate).map_err(GatedProposeError::Ratchet)
    }

    /// Verify a run (exit-grade: callers map the error to a non-zero exit).
    ///
    /// # Errors
    /// [`RatchetViolation::Regression`].
    pub fn verify(&self, scores: &Baseline) -> Result<(), RatchetViolation> {
        verify_scores(&self.baseline, scores)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn base() -> Baseline {
        Baseline::from([("regression_rate".to_string(), 0.02_f64)])
    }

    /// G29 acceptance: a baseline update WITHOUT a g25 recomputation
    /// reference is refused outright (the baseline never carries an
    /// unverified number), and a cited update moves the ratchet.
    #[test]
    fn baseline_update_requires_recomputation_support() {
        let mut ratchet = Ratchet::new(base());
        let mut ledger = truerise_governance::UnverifiedLedger::new();
        let tighter = Baseline::from([("regression_rate".to_string(), 0.01_f64)]);
        // No support: refused, and the baseline did not move.
        assert!(matches!(
            ratchet.propose_gated(&tighter, None, &mut ledger, None, 0),
            Err(GatedProposeError::Gate(
                truerise_governance::WriteGateError::UnsupportedWrite {
                    path: "baseline_update",
                    ..
                }
            ))
        ));
        assert!((ratchet.baseline()["regression_rate"] - 0.02).abs() < 1e-12);
        // With the g25 recomputation reference the update moves the
        // ratchet.
        ratchet
            .propose_gated(
                &tighter,
                Some(&truerise_governance::SupportRef::Recomputation {
                    claim: "scores recomputed offline".to_string(),
                    digest: "digest".to_string(),
                }),
                &mut ledger,
                None,
                0,
            )
            .unwrap();
        assert!((ratchet.baseline()["regression_rate"] - 0.01).abs() < 1e-12);
        assert!(ledger.entries().is_empty(), "baseline path never marks");
    }

    #[test]
    fn ratchet_only_tightens() {
        let mut r = Ratchet::new(base());
        let tighter = Baseline::from([("regression_rate".to_string(), 0.01)]);
        assert!(r.propose(&tighter).is_ok());
        assert_eq!(r.baseline()["regression_rate"], 0.01);
        // Loosening refused, ratchet unchanged:
        let looser = Baseline::from([("regression_rate".to_string(), 0.05)]);
        assert!(matches!(
            r.propose(&looser),
            Err(RatchetViolation::Loosened { metric, old, new })
                if metric == "regression_rate" && old == 0.01 && new == 0.05
        ));
        assert_eq!(
            r.baseline()["regression_rate"],
            0.01,
            "unchanged on refusal"
        );
    }

    #[test]
    fn score_regression_is_exit_grade() {
        let r = Ratchet::new(base());
        let mut scores = base();
        scores.insert("regression_rate".to_string(), 0.03);
        assert!(matches!(
            r.verify(&scores),
            Err(RatchetViolation::Regression { metric, value, allowed })
                if metric == "regression_rate" && value == 0.03 && allowed == 0.02
        ));
        // Within baseline: ok.
        scores.insert("regression_rate".to_string(), 0.02);
        assert!(r.verify(&scores).is_ok());
    }

    #[test]
    fn new_metrics_can_be_added_by_proposal() {
        let mut r = Ratchet::new(base());
        let with_new = Baseline::from([("new_metric".to_string(), 0.5)]);
        assert!(
            r.propose(&with_new).is_ok(),
            "new metric (no prior) is addable"
        );
        assert_eq!(r.baseline()["new_metric"], 0.5);
    }
}
