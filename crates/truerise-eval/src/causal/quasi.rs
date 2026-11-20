//! Quasi-experimental identification (review family B, part 1): the three
//! designs that turn OBSERVATIONAL run logs into defensible contrasts
//! without a randomized experiment.
//!
//! - [`did`]: difference-in-differences over a treated and a control arm
//!   with pre-periods; the parallel-trends statistic is EVIDENCE for the
//!   assumption ledger's defense (declared under `exchangeability`), not a
//!   gate - a violated pre-trend must be defended or the claim downgraded.
//! - [`sharp_rdd`]: regression discontinuity at a routing/score cutoff -
//!   locally the assignment is as good as random, which makes this the
//!   strongest observational design available to an agent runtime.
//! - [`wald_iv`]: instrumented effect via a binary assignment quirk
//!   (hash bucket, time-slice shard): the Wald estimator targets the
//!   complier average effect (CACE), and weak instruments are REFUSED, not
//!   reported.
//!
//! All three produce causal-OBSERVATIONAL grade evidence: each result
//! carries its design's identifying assumption by name so the tier
//! semantics ([`super::typology`]) can demand the matching ledger row.

/// Refusals shared by the quasi-experimental designs.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QuasiError {
    /// A pre-period series is too short to speak about trends.
    #[error("quasi-design refused: {0}")]
    TooFewPeriods(&'static str),
    /// One instrument arm has no units at all - there is no contrast.
    #[error("quasi-design refused: {field} is empty - both arms need units")]
    EmptyArm {
        /// The offending field name.
        field: &'static str,
    },
    /// Row-aligned inputs disagree in length.
    #[error("quasi-design refused: {field} has {rows} rows, expected {expected}")]
    LengthMismatch {
        /// The offending field name.
        field: &'static str,
        /// Its row count.
        rows: usize,
        /// The expected row count.
        expected: usize,
    },
    /// NaN or infinite values poison every estimate; `sharp_rdd` also
    /// raises this for a non-finite or non-positive bandwidth argument.
    #[error("quasi-design refused: non-finite value in {field} at index {index}")]
    NotFinite {
        /// The offending field name.
        field: &'static str,
        /// Position of the offending value.
        index: usize,
    },
    /// A bandwidth that is not a positive finite number.
    #[error("quasi-design refused: bandwidth {0} is not positive and finite")]
    BadBandwidth(f64),
    /// A local fit starved below its residual-df minimum.
    #[error(
        "quasi-design refused: {side} side of the cutoff has {rows} usable rows, needs at least {min}"
    )]
    StarvedSide {
        /// Which side ("left" / "right").
        side: &'static str,
        /// Usable rows on that side.
        rows: usize,
        /// The minimum contract.
        min: usize,
    },
    /// The first stage is too weak for a Wald estimate - the numerator
    /// would be noise divided by noise.
    #[error("quasi-design refused: weak instrument (first-stage difference {first_stage})")]
    WeakInstrument {
        /// The observed first-stage difference.
        first_stage: f64,
    },
    /// Binary-encoded inputs carrying non-binary values.
    #[error("quasi-design refused: {field}[{index}] = {value} is not binary (0 or 1)")]
    NotBinary {
        /// The offending field name.
        field: &'static str,
        /// Position of the offending value.
        index: usize,
        /// The offending value.
        value: f64,
    },
}

fn finite(values: &[f64], field: &'static str) -> Result<(), QuasiError> {
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(QuasiError::NotFinite { field, index: i });
        }
    }
    Ok(())
}

fn binary(values: &[f64], field: &'static str) -> Result<(), QuasiError> {
    for (i, v) in values.iter().enumerate() {
        if *v != 0.0 && *v != 1.0 {
            return Err(QuasiError::NotBinary {
                field,
                index: i,
                value: *v,
            });
        }
    }
    Ok(())
}

/// The difference-in-differences outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct DidResult {
    /// ATT: (treated post - treated baseline) - (control post - control
    /// baseline).
    pub att: f64,
    /// Treated arm's pre-period mean.
    pub treated_baseline: f64,
    /// Control arm's pre-period mean.
    pub control_baseline: f64,
    /// OLS slope of the treated arm's pre-period series.
    pub pre_slope_treated: f64,
    /// OLS slope of the control arm's pre-period series.
    pub pre_slope_control: f64,
    /// pre_slope_treated - pre_slope_control: the parallel-trends
    /// statistic. Near zero DEFENDS the ledger's exchangeability row; far
    /// from zero means the defense must explain the divergence or the
    /// claim downgrades.
    pub pre_trend_diff: f64,
}

/// Difference-in-differences over group-level period means.
///
/// `treated_pre` / `control_pre` are the per-period group means before the
/// intervention (at least two periods each); the post values are the
/// group means after. The design's identifying assumption (parallel
/// trends) is reported as a statistic, never as a pass.
pub fn did(
    treated_pre: &[f64],
    control_pre: &[f64],
    treated_post: f64,
    control_post: f64,
) -> Result<DidResult, QuasiError> {
    for (series, name) in [(treated_pre, "treated_pre"), (control_pre, "control_pre")] {
        if series.len() < 2 {
            return Err(QuasiError::TooFewPeriods(name));
        }
        finite(series, name)?;
    }
    if !treated_post.is_finite() || !control_post.is_finite() {
        return Err(QuasiError::NotFinite {
            field: "post",
            index: 0,
        });
    }
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let treated_baseline = mean(treated_pre);
    let control_baseline = mean(control_pre);
    let slope = |s: &[f64]| -> f64 {
        let xs: Vec<f64> = (0..s.len()).map(|i| i as f64).collect();
        let x_mean = mean(&xs);
        let y_mean = mean(s);
        let cov: f64 = xs
            .iter()
            .zip(s.iter())
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .sum();
        let var: f64 = xs.iter().map(|x| (x - x_mean) * (x - x_mean)).sum();
        cov / var
    };
    let pre_slope_treated = slope(treated_pre);
    let pre_slope_control = slope(control_pre);
    Ok(DidResult {
        att: (treated_post - treated_baseline) - (control_post - control_baseline),
        treated_baseline,
        control_baseline,
        pre_slope_treated,
        pre_slope_control,
        pre_trend_diff: pre_slope_treated - pre_slope_control,
    })
}

/// The sharp-RDD outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct RddResult {
    /// The discontinuity: difference of the two local-linear intercepts at
    /// the cutoff.
    pub effect: f64,
    /// Usable rows left of the cutoff.
    pub n_left: usize,
    /// Usable rows right of the cutoff.
    pub n_right: usize,
    /// The bandwidth actually applied (echoed for the ledger row).
    pub bandwidth: f64,
}

/// Sharp regression discontinuity with a FIXED bandwidth and local linear
/// fits on both sides.
///
/// Treatment is `running >= cutoff`. The effect is the intercept
/// difference at the cutoff - a causal-observational estimate whose
/// defense cites the continuity of the counterfactual outcome function at
/// the cutoff. The bandwidth is a researcher choice and MUST be declared
/// in the assumption row; optimal-bandwidth selection is deliberately not
/// implemented (a data-driven bandwidth would be an undeclared assumption).
pub fn sharp_rdd(
    running: &[f64],
    outcome: &[f64],
    cutoff: f64,
    bandwidth: f64,
) -> Result<RddResult, QuasiError> {
    if running.len() != outcome.len() {
        return Err(QuasiError::LengthMismatch {
            field: "running",
            rows: running.len(),
            expected: outcome.len(),
        });
    }
    finite(running, "running")?;
    finite(outcome, "outcome")?;
    if !cutoff.is_finite() {
        return Err(QuasiError::NotFinite {
            field: "cutoff",
            index: 0,
        });
    }
    if !bandwidth.is_finite() || bandwidth <= 0.0 {
        // Review A2-P2-6b: a finite-but-nonpositive bandwidth is not a
        // "non-finite value" - it is a bad argument.
        return Err(QuasiError::BadBandwidth(bandwidth));
    }
    let mut left: Vec<(f64, f64)> = Vec::new();
    let mut right: Vec<(f64, f64)> = Vec::new();
    for (&r, &y) in running.iter().zip(outcome.iter()) {
        if r >= cutoff && r <= cutoff + bandwidth {
            right.push((r - cutoff, y));
        } else if r < cutoff && r >= cutoff - bandwidth {
            left.push((r - cutoff, y));
        }
    }
    if left.len() < 3 {
        return Err(QuasiError::StarvedSide {
            side: "left",
            rows: left.len(),
            min: 3,
        });
    }
    if right.len() < 3 {
        return Err(QuasiError::StarvedSide {
            side: "right",
            rows: right.len(),
            min: 3,
        });
    }
    // Local linear fit y = a + b*(r - cutoff) on each side; the effect is
    // a_right - a_left, evaluated AT the cutoff by construction.
    let fit_intercept = |points: &[(f64, f64)]| -> f64 {
        let n = points.len() as f64;
        let x_mean = points.iter().map(|p| p.0).sum::<f64>() / n;
        let y_mean = points.iter().map(|p| p.1).sum::<f64>() / n;
        let cov: f64 = points.iter().map(|p| (p.0 - x_mean) * (p.1 - y_mean)).sum();
        let var: f64 = points.iter().map(|p| (p.0 - x_mean) * (p.0 - x_mean)).sum();
        y_mean - (cov / var) * x_mean
    };
    let effect = fit_intercept(&right) - fit_intercept(&left);
    Ok(RddResult {
        effect,
        n_left: left.len(),
        n_right: right.len(),
        bandwidth,
    })
}

/// The Wald-IV outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct WaldResult {
    /// ITT: reduced-form effect of the instrument on the outcome.
    pub itt: f64,
    /// First-stage difference in treatment take-up across instrument arms.
    pub first_stage: f64,
    /// The Wald estimate: ITT / first-stage - the complier average causal
    /// effect (CACE).
    pub cace: f64,
    /// Take-up rate among instrument-assigned units.
    pub take_up_assigned: f64,
    /// Take-up rate among unassigned units.
    pub take_up_unassigned: f64,
}

/// Weak-instrument contract: a first-stage difference below this is noise
/// wearing a ratio; the design is refused rather than estimated.
pub const MIN_FIRST_STAGE: f64 = 1e-3;

/// Wald instrumental-variables estimate with a BINARY instrument and a
/// BINARY treatment.
///
/// The instrument must be an arbitrary assignment quirk (hash bucket,
/// time-slice shard) that affects the outcome ONLY through the treatment.
/// The result is the complier average causal effect: runs that obey the
/// assignment. Runs that abandon the study early are non-compliance, which
/// is exactly the family the bounded-surrender contract (G44) prices.
pub fn wald_iv(
    instrument: &[f64],
    treatment: &[f64],
    outcome: &[f64],
) -> Result<WaldResult, QuasiError> {
    let n = outcome.len();
    if instrument.len() != n || treatment.len() != n {
        return Err(QuasiError::LengthMismatch {
            field: "instrument/treatment",
            rows: instrument.len(),
            expected: n,
        });
    }
    finite(instrument, "instrument")?;
    finite(treatment, "treatment")?;
    finite(outcome, "outcome")?;
    binary(instrument, "instrument")?;
    binary(treatment, "treatment")?;
    let mut n1 = 0usize;
    let mut n0 = 0usize;
    let mut x1 = 0.0;
    let mut x0 = 0.0;
    let mut y1 = 0.0;
    let mut y0 = 0.0;
    for i in 0..n {
        if instrument[i] == 1.0 {
            n1 += 1;
            x1 += treatment[i];
            y1 += outcome[i];
        } else {
            n0 += 1;
            x0 += treatment[i];
            y0 += outcome[i];
        }
    }
    if n1 == 0 || n0 == 0 {
        return Err(QuasiError::EmptyArm {
            field: "instrument arm",
        });
    }
    let take_up_assigned = x1 / n1 as f64;
    let take_up_unassigned = x0 / n0 as f64;
    let first_stage = take_up_assigned - take_up_unassigned;
    let itt = y1 / n1 as f64 - y0 / n0 as f64;
    if first_stage.abs() < MIN_FIRST_STAGE {
        return Err(QuasiError::WeakInstrument { first_stage });
    }
    Ok(WaldResult {
        itt,
        first_stage,
        cace: itt / first_stage,
        take_up_assigned,
        take_up_unassigned,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn did_recovers_the_classic_two_by_two() {
        // treated baseline 11, control baseline 21; posts 15 / 23:
        // ATT = (15-11) - (23-21) = 2. Pre-trends parallel (both +2 per
        // period: 10->12 and 20->22).
        let out = did(&[10.0, 12.0], &[20.0, 22.0], 15.0, 23.0).unwrap();
        assert!((out.att - 2.0).abs() < 1e-12, "ATT was {}", out.att);
        assert!(out.pre_trend_diff.abs() < 1e-12);
        assert!((out.pre_slope_treated - 2.0).abs() < 1e-12);
    }

    #[test]
    fn did_reports_divergent_pre_trends_as_a_statistic() {
        // Treated pre-periods rising twice as fast: the parallel-trends
        // defense is weakened and the module must SAY so numerically.
        // Treated rises +4/period, control +1/period: diff = 3.
        let out = did(&[10.0, 14.0], &[20.0, 21.0], 16.0, 22.0).unwrap();
        assert!(
            (out.pre_trend_diff - 3.0).abs() < 1e-12,
            "was {}",
            out.pre_trend_diff
        );
    }

    #[test]
    fn did_needs_two_pre_periods() {
        assert!(matches!(
            did(&[10.0], &[20.0, 22.0], 15.0, 23.0),
            Err(QuasiError::TooFewPeriods(_))
        ));
        let mut bad = vec![10.0, f64::NAN];
        assert!(matches!(
            did(&bad, &[20.0, 22.0], 15.0, 23.0),
            Err(QuasiError::NotFinite { .. })
        ));
        bad = vec![10.0, 12.0];
        let mut bad_c = vec![20.0, 22.0];
        bad_c[0] = f64::INFINITY;
        assert!(matches!(
            did(&bad, &bad_c, 15.0, 23.0),
            Err(QuasiError::NotFinite { .. })
        ));
    }

    #[test]
    fn sharp_rdd_recovers_an_exact_discontinuity() {
        // y = 2*1[r >= 0] + 3r on a symmetric grid: both local fits are
        // exact, the intercept difference is exactly the jump.
        let mut running = Vec::new();
        let mut outcome = Vec::new();
        let mut r = -5.0;
        while r <= 5.0 {
            running.push(r);
            outcome.push(if r >= 0.0 { 2.0 + 3.0 * r } else { 3.0 * r });
            r += 0.5;
        }
        let out = sharp_rdd(&running, &outcome, 0.0, 2.0).unwrap();
        assert!((out.effect - 2.0).abs() < 1e-9, "effect was {}", out.effect);
        assert!(out.n_left >= 3 && out.n_right >= 3);
        assert!((out.bandwidth - 2.0).abs() < 1e-12);
    }

    #[test]
    fn sharp_rdd_reports_no_jump_when_continuous() {
        let mut running = Vec::new();
        let mut outcome = Vec::new();
        let mut r = -5.0;
        while r <= 5.0 {
            running.push(r);
            outcome.push(3.0 * r);
            r += 0.5;
        }
        let out = sharp_rdd(&running, &outcome, 0.0, 2.0).unwrap();
        assert!(out.effect.abs() < 1e-9, "phantom jump of {}", out.effect);
    }

    #[test]
    fn starved_sides_are_refused() {
        // Window [cutoff-2, cutoff): only -1.5 and -1.0 qualify (2 rows).
        let running = [-3.0, -1.5, -1.0, 0.1, 0.2, 0.3];
        let outcome = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        assert!(matches!(
            sharp_rdd(&running, &outcome, 0.0, 2.0),
            Err(QuasiError::StarvedSide {
                side: "left",
                rows: 2,
                min: 3
            })
        ));
        // A non-positive bandwidth is a bad argument, not a bad value.
        assert!(matches!(
            sharp_rdd(&running, &outcome, 0.0, 0.0),
            Err(QuasiError::BadBandwidth(0.0))
        ));
    }

    #[test]
    fn wald_iv_recovers_the_complier_effect_under_partial_compliance() {
        // Four runs: z = [1,0,1,0]; units 0,1 comply, units 2,3 never take
        // up; y = 2x exactly. ITT = 1, first stage = 0.5, CACE = 2.
        let z = [1.0, 0.0, 1.0, 0.0];
        let x = [1.0, 0.0, 0.0, 0.0];
        let y = [2.0, 0.0, 0.0, 0.0];
        let out = wald_iv(&z, &x, &y).unwrap();
        assert!((out.itt - 1.0).abs() < 1e-12, "ITT was {}", out.itt);
        assert!((out.first_stage - 0.5).abs() < 1e-12);
        assert!((out.cace - 2.0).abs() < 1e-12, "CACE was {}", out.cace);
    }

    #[test]
    fn weak_instruments_and_non_binary_inputs_are_refused() {
        // No take-up difference at all: the ratio would be noise/noise.
        let z = [1.0, 0.0, 1.0, 0.0];
        let x = [1.0, 1.0, 1.0, 1.0];
        let y = [1.0, 2.0, 3.0, 4.0];
        assert!(matches!(
            wald_iv(&z, &x, &y),
            Err(QuasiError::WeakInstrument { first_stage })
            if first_stage == 0.0
        ));
        // Non-binary treatment.
        let x_bad = [1.0, 0.5, 0.0, 0.0];
        let y = [2.0, 0.0, 0.0, 0.0];
        assert!(matches!(
            wald_iv(&z, &x_bad, &y),
            Err(QuasiError::NotBinary {
                field: "treatment",
                index: 1,
                ..
            })
        ));
        // One instrument arm empty.
        let z_one = [1.0, 1.0, 1.0, 1.0];
        let x_ok = [1.0, 0.0, 0.0, 0.0];
        assert!(matches!(
            wald_iv(&z_one, &x_ok, &y),
            Err(QuasiError::EmptyArm {
                field: "instrument arm"
            })
        ));
    }
}
