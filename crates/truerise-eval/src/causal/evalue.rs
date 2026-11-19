//! Sensitivity arithmetic (CA3): the E-value, a defensive number.
//!
//! The E-value (VanderWeele & Ding, 2017) answers the only question that
//! matters after an observational effect is estimated: HOW MUCH unmeasured
//! confounding - on the risk-ratio scale, simultaneously with the treatment
//! and with the outcome - would be required to explain the observed
//! association away entirely? An E-value of 3.4 says: a single unmeasured
//! factor associated with both treatment and outcome at least at RR 3.4
//! could account for the whole association; smaller confounding could not.
//!
//! Boundary: the E-value does NOT defend a claim - it PRICES the defense.
//! A claim whose assumption row cites an E-value must cite the number as a
//! fragility price, never as proof of causality.

/// Refusal reasons for the E-value.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EvalueError {
    /// A risk ratio must be positive and finite.
    #[error("evalue refused: risk ratio {0} is not positive and finite")]
    BadRiskRatio(f64),
}

/// The E-value of a risk ratio.
///
/// For `RR >= 1`: `RR + sqrt(RR * (RR - 1))`. For a protective estimate
/// (`RR < 1`) the ratio is inverted first and the result inverted back, so
/// the E-value always quantifies confounding strength on the harmful scale.
/// `RR = 1` (no association) prices at exactly 1: it takes no confounding
/// at all to explain away nothing.
pub fn evalue_rr(rr: f64) -> Result<f64, EvalueError> {
    if !rr.is_finite() || rr <= 0.0 {
        return Err(EvalueError::BadRiskRatio(rr));
    }
    if (rr - 1.0).abs() < f64::EPSILON {
        return Ok(1.0);
    }
    let harmful = if rr >= 1.0 { rr } else { 1.0 / rr };
    // Algebraic rearrangement that never forms harmful^2: at harmful =
    // 1e200 the naive `sqrt(harmful * (harmful - 1))` overflows to inf even
    // though the true E-value 2e200 is representable (review P2-6).
    let value = harmful + harmful * (1.0 - 1.0 / harmful).sqrt();
    if rr >= 1.0 {
        Ok(value)
    } else {
        Ok(1.0 / value)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn evalue_of_two_is_two_plus_root_two() {
        let v = evalue_rr(2.0).unwrap();
        assert!((v - (2.0 + 2.0_f64.sqrt())).abs() < 1e-12, "was {v}");
    }

    #[test]
    fn evalue_of_four_is_four_plus_two_root_three() {
        let v = evalue_rr(4.0).unwrap();
        assert!((v - (4.0 + 12.0_f64.sqrt())).abs() < 1e-12, "was {v}");
    }

    #[test]
    fn protective_estimates_invert_instead_of_flipping_sign() {
        // RR = 0.5 is the mirror of RR = 2: same fragility price, expressed
        // below 1. A negative or swapped result would be an arithmetic bug.
        let v = evalue_rr(0.5).unwrap();
        assert!((v - 1.0 / (2.0 + 2.0_f64.sqrt())).abs() < 1e-12, "was {v}");
        assert!(v < 1.0);
    }

    #[test]
    fn no_association_prices_at_exactly_one() {
        assert!((evalue_rr(1.0).unwrap() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn nonsense_ratios_are_refused() {
        assert!(matches!(evalue_rr(0.0), Err(EvalueError::BadRiskRatio(_))));
        assert!(matches!(evalue_rr(-2.0), Err(EvalueError::BadRiskRatio(_))));
        assert!(matches!(
            evalue_rr(f64::NAN),
            Err(EvalueError::BadRiskRatio(_))
        ));
    }

    /// Review P2-6 regression: extreme but finite ratios must not overflow
    /// through the intermediate harmful-squared term when the true E-value
    /// is representable.
    #[test]
    fn extreme_ratios_stay_representable() {
        let v = evalue_rr(1e200).unwrap();
        assert!(v.is_finite(), "1e200 overflowed to {v}");
        assert!((v - 2e200).abs() < 1e186, "was {v}");
        let v = evalue_rr(1e-300).unwrap();
        assert!(v.is_finite() && v > 0.0, "1e-300 collapsed to {v}");
        assert!(v < 1e-299, "protective inverse out of range: {v}");
    }
}
