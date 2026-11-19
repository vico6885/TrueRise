//! Partial identification (CA3): honest intervals when identification
//! fails.
//!
//! The Manski no-assumption bounds are the floor of causal honesty: with
//! NO behavioral assumption at all, the average treatment effect is
//! identified only up to an interval built from the outcome support and
//! the observed conditional means. Every observational claim in this
//! repository that cannot defend its identification assumptions owes the
//! reader THIS interval - an INCONCLUSIVE verdict with a shape, per the
//! three-valued verdict family (G7) and the bounded-surrender contract
//! (G44).
//!
//! Reference: Manski (1990), "Nonparametric bounds on treatment effects".

/// Refusal reasons for partial identification.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BoundsError {
    /// The treated fraction is not a probability.
    #[error("bounds refused: treated fraction {0} is not in [0, 1]")]
    BadTreatedFraction(f64),
    /// The outcome support is degenerate.
    #[error("bounds refused: support [{lo}, {hi}] is empty or inverted")]
    BadSupport {
        /// Declared lower support bound.
        lo: f64,
        /// Declared upper support bound.
        hi: f64,
    },
    /// An observed conditional mean outside the declared support is a data
    /// contradiction - refuse rather than emit a meaningless interval.
    #[error("bounds refused: observed mean {mean} lies outside the declared support [{lo}, {hi}]")]
    MeanOutsideSupport {
        /// The offending conditional mean.
        mean: f64,
        /// Declared lower support bound.
        lo: f64,
        /// Declared upper support bound.
        hi: f64,
    },
    /// NaN inputs poison the interval endpoints.
    #[error("bounds refused: non-finite input")]
    NotFinite,
}

/// Inputs to the no-assumption ATE bound, all validated before use.
#[derive(Debug, Clone, PartialEq)]
pub struct ManskiInput {
    /// Fraction of units in the treated arm, `P(T=1)`.
    pub p_treated: f64,
    /// Observed mean outcome among treated units, `E[Y | T=1]`.
    pub mean_treated: f64,
    /// Observed mean outcome among control units, `E[Y | T=0]`.
    pub mean_control: f64,
    /// Lower bound of the outcome support.
    pub y_lo: f64,
    /// Upper bound of the outcome support.
    pub y_hi: f64,
}

/// The identified interval for the average treatment effect under no
/// behavioral assumptions.
#[derive(Debug, Clone, PartialEq)]
pub struct ManskiBounds {
    /// Worst-case lower endpoint: the effect could be as harmful as this.
    pub lower: f64,
    /// Worst-case upper endpoint: the effect could be as beneficial as this.
    pub upper: f64,
}

impl ManskiBounds {
    /// Interval width: the price of the assumptions you declined to make.
    /// Zero width would mean full identification (impossible here unless
    /// the support is degenerate).
    #[must_use]
    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }
}

/// The Manski no-assumption bound on the ATE.
///
/// Mechanism: for the unobserved counterfactual mean of each arm, the only
/// defensible statement is that it lies in the outcome support; the
/// observed conditional mean pins down the observed fraction of each
/// potential outcome. The endpoint formula for `E[Y(1)]`:
/// `[p1*m1 + (1-p1)*y_lo, p1*m1 + (1-p1)*y_hi]`, symmetrically for
/// `E[Y(0)]`; the ATE interval is the widest difference of endpoints.
pub fn manski_ate_bounds(input: &ManskiInput) -> Result<ManskiBounds, BoundsError> {
    let ManskiInput {
        p_treated,
        mean_treated,
        mean_control,
        y_lo,
        y_hi,
    } = *input;
    if !p_treated.is_finite()
        || !mean_treated.is_finite()
        || !mean_control.is_finite()
        || !y_lo.is_finite()
        || !y_hi.is_finite()
    {
        return Err(BoundsError::NotFinite);
    }
    if !(0.0..=1.0).contains(&p_treated) {
        return Err(BoundsError::BadTreatedFraction(p_treated));
    }
    if y_lo > y_hi {
        return Err(BoundsError::BadSupport { lo: y_lo, hi: y_hi });
    }
    for mean in [mean_treated, mean_control] {
        if mean < y_lo || mean > y_hi {
            return Err(BoundsError::MeanOutsideSupport {
                mean,
                lo: y_lo,
                hi: y_hi,
            });
        }
    }
    let p0 = 1.0 - p_treated;
    let e1_lo = p_treated * mean_treated + p0 * y_lo;
    let e1_hi = p_treated * mean_treated + p0 * y_hi;
    let e0_lo = p0 * mean_control + p_treated * y_lo;
    let e0_hi = p0 * mean_control + p_treated * y_hi;
    Ok(ManskiBounds {
        lower: e1_lo - e0_hi,
        upper: e1_hi - e0_lo,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn manski_no_assumption_bounds_span_the_classic_case() {
        // Binary outcome, p1 = 0.5, m1 = 0.8, m0 = 0.4:
        // E[Y(1)] in [0.4, 0.9], E[Y(0)] in [0.2, 0.7] -> ATE in [-0.3, 0.7].
        let input = ManskiInput {
            p_treated: 0.5,
            mean_treated: 0.8,
            mean_control: 0.4,
            y_lo: 0.0,
            y_hi: 1.0,
        };
        let b = manski_ate_bounds(&input).unwrap();
        assert!((b.lower - (-0.3)).abs() < 1e-12, "lower was {}", b.lower);
        assert!((b.upper - 0.7).abs() < 1e-12, "upper was {}", b.upper);
    }

    #[test]
    fn one_sided_data_still_spans_the_full_support_width() {
        // No treated units at all: the effect is identified only up to the
        // whole support - the honest answer to "no data" is width, not a
        // point.
        let input = ManskiInput {
            p_treated: 0.0,
            mean_treated: 0.0, // unused but must sit inside the support
            mean_control: 0.4,
            y_lo: 0.0,
            y_hi: 1.0,
        };
        let b = manski_ate_bounds(&input).unwrap();
        assert!((b.width() - 1.0).abs() < 1e-12);
        assert!((b.lower - (-0.4)).abs() < 1e-12);
        assert!((b.upper - 0.6).abs() < 1e-12);
    }

    #[test]
    fn degenerate_support_collapses_to_a_point_interval() {
        // y fixed at 7 for every unit: both potential outcomes are 7, so
        // the ATE is 0 regardless of assignment - full identification by
        // trivial support.
        let input = ManskiInput {
            p_treated: 0.3,
            mean_treated: 7.0,
            mean_control: 7.0,
            y_lo: 7.0,
            y_hi: 7.0,
        };
        let b = manski_ate_bounds(&input).unwrap();
        assert!((b.lower - 0.0).abs() < 1e-12);
        assert!((b.upper - 0.0).abs() < 1e-12);
    }

    #[test]
    fn contradictions_and_nonsense_are_refused() {
        let good = ManskiInput {
            p_treated: 0.5,
            mean_treated: 0.8,
            mean_control: 0.4,
            y_lo: 0.0,
            y_hi: 1.0,
        };
        // Observed mean outside the declared support: a data contradiction.
        let mut bad = good.clone();
        bad.mean_treated = 1.5;
        assert!(matches!(
            manski_ate_bounds(&bad),
            Err(BoundsError::MeanOutsideSupport { .. })
        ));
        // Inverted support.
        let mut bad = good.clone();
        bad.y_lo = 2.0;
        bad.y_hi = 1.0;
        assert!(matches!(
            manski_ate_bounds(&bad),
            Err(BoundsError::BadSupport { .. })
        ));
        // Fraction above 1.
        let mut bad = good.clone();
        bad.p_treated = 1.5;
        assert!(matches!(
            manski_ate_bounds(&bad),
            Err(BoundsError::BadTreatedFraction(_))
        ));
        // NaN anywhere.
        let mut bad = good.clone();
        bad.mean_control = f64::NAN;
        assert!(matches!(
            manski_ate_bounds(&bad),
            Err(BoundsError::NotFinite)
        ));
    }
}
