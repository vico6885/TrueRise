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
