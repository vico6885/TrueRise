//! Synthetic control (review family B, part 3): the retreat when only ONE
//! production agent exists and no donor-free counterfactual can be
//! randomized.
//!
//! The pre-intervention trajectory of the treated unit is reconstructed as
//! a convex combination of DONOR trajectories (other task suites, other
//! model versions, other agents); the post-intervention GAP between the
//! treated unit and the synthetic reconstruction is the effect estimate.
//! The identifying assumption is explicit: the convex combination that
//! matched the pre-period would have kept matching without the treatment.
//!
//! Optimization: Frank-Wolfe over the simplex with an exact line search
//! (the objective is quadratic and convex) - provably convergent, fully
//! deterministic, no external solver.

/// Refusals for synthetic control.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SynthError {
    /// Donor columns must be row-aligned with the treated pre-series.
    #[error("synth refused: donor {index} has {rows} pre-periods, treated has {expected}")]
    DonorLengthMismatch {
        /// The offending donor index.
        index: usize,
        /// The donor's length.
        rows: usize,
        /// The treated series' length.
        expected: usize,
    },
    /// At least one pre-period and one donor are the minimum to say
    /// anything.
    #[error("synth refused: {0}")]
    TooFewUnits(&'static str),
    /// NaN or infinite values poison the fit.
    #[error("synth refused: non-finite value at pre-period {index}")]
    NotFinite {
        /// Position of the offending value.
        index: usize,
    },
    /// The Frank-Wolfe loop ended above the duality-gap tolerance.
    #[error("synth refused: weight fit did not converge (gap {gap} after {iterations} iterations)")]
    NotConverged {
        /// The final duality gap.
        gap: f64,
        /// Iterations actually used.
        iterations: usize,
    },
    /// The post-value count disagrees with the donor count.
    #[error("synth refused: {posts} post values for {donors} donors")]
    PostCountMismatch {
        /// The donor count.
        donors: usize,
        /// The post-value count.
        posts: usize,
    },
}

/// The synthetic-control outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct SynthResult {
    /// Donor weights on the simplex (non-negative, sum to 1).
    pub weights: Vec<f64>,
    /// Post-intervention gap: treated post value minus the synthetic
    /// reconstruction's post value. This is the effect estimate.
    pub post_gap: f64,
    /// Root-mean-square pre-period fit error - the honesty dial: a large
    /// pre-fit error means the donors never could reconstruct the unit,
    /// and the post gap inherits that failure.
    pub pre_fit_rmse: f64,
    /// Frank-Wolfe iterations actually used.
    pub iterations: usize,
    /// The final Frank-Wolfe duality gap (review P2-6: the convergence
    /// certificate is SURFACED, not just used as a stop test). Near zero =
    /// near-optimal weights; the caller sees exactly how tight the fit is.
    pub convergence_gap: f64,
}

/// Fit the simplex weights and return the post-intervention gap.
///
/// `treated_pre`: the treated unit's pre-period series (length T0).
/// `donor_pre`: one column per donor, each length T0.
/// `treated_post` / `donor_post`: the post-intervention values.
pub fn synthetic_control(
    treated_pre: &[f64],
    donor_pre: &[Vec<f64>],
    treated_post: f64,
    donor_post: &[f64],
) -> Result<SynthResult, SynthError> {
    let t0 = treated_pre.len();
    if t0 == 0 {
        return Err(SynthError::TooFewUnits("pre-periods"));
    }
    if donor_pre.is_empty() {
        return Err(SynthError::TooFewUnits("donors"));
    }
    if donor_post.len() != donor_pre.len() {
        return Err(SynthError::PostCountMismatch {
            donors: donor_pre.len(),
            posts: donor_post.len(),
        });
    }
    for (j, col) in donor_pre.iter().enumerate() {
        if col.len() != t0 {
            return Err(SynthError::DonorLengthMismatch {
                index: j,
                rows: col.len(),
                expected: t0,
            });
        }
    }
    for (i, v) in treated_pre.iter().enumerate() {
        if !v.is_finite() {
            return Err(SynthError::NotFinite { index: i });
        }
    }
    for col in donor_pre {
        for (i, v) in col.iter().enumerate() {
            if !v.is_finite() {
                return Err(SynthError::NotFinite { index: i });
            }
        }
    }
    // Review P2-7: the post channel feeds the gap directly - a NaN post
    // would silently yield a NaN effect estimate.
    if !treated_post.is_finite() || donor_post.iter().any(|v| !v.is_finite()) {
        return Err(SynthError::NotFinite { index: usize::MAX });
    }
    let k = donor_pre.len();
    // Frank-Wolfe over the simplex. Start uniform; per iteration take the
    // simplex vertex minimizing the gradient and line-search exactly (the
    // objective phi(w) = ||y - Xw||^2 is quadratic: g = 2 X^T (Xw - y)).
    // Convergence certificate: the Frank-Wolfe duality gap
    // min_j g.(e_j - w) collapsing to ~0 proves near-optimality (convex
    // problem, no local minima to hide in).
    let mut weights = vec![1.0 / k as f64; k];
    let reconstruct = |w: &[f64], t: usize| -> f64 {
        donor_pre
            .iter()
            .zip(w.iter())
            .map(|(col, wi)| col[t] * wi)
            .sum()
    };
    let objective = |w: &[f64]| -> f64 {
        (0..t0)
            .map(|t| {
                let d = treated_pre[t] - reconstruct(w, t);
                d * d
            })
            .sum()
    };
    const MAX_ITERATIONS: usize = 400;
    let mut used = 0usize;
    let mut last_gap = f64::INFINITY;
    for _ in 0..MAX_ITERATIONS {
        used += 1;
        // Gradient at the current point.
        let mut grad = vec![0.0; k];
        for j in 0..k {
            let mut g = 0.0;
            for t in 0..t0 {
                let residual = treated_pre[t] - reconstruct(&weights, t);
                g += -2.0 * donor_pre[j][t] * residual;
            }
            grad[j] = g;
        }
        // Frank-Wolfe vertex and the duality-gap certificate.
        let s_idx = grad
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let dot: f64 = grad.iter().zip(weights.iter()).map(|(g, w)| g * w).sum();
        let gap = grad[s_idx] - dot;
        last_gap = gap.abs();
        if gap.abs() <= 1e-9 * (dot.abs() + 1.0) {
            break;
        }
        // Exact line search toward e_s: gamma* = -g.d / (d^T H d) with
        // d = e_s - w and H = 2 X^T X (PSD), clamped to [0, 1].
        let mut d = vec![0.0; k];
        for j in 0..k {
            d[j] = if j == s_idx { 1.0 } else { 0.0 } - weights[j];
        }
        let mut gd = 0.0;
        for j in 0..k {
            gd += grad[j] * d[j];
        }
        let mut dd = 0.0;
        for t in 0..t0 {
            let recon_d: f64 = donor_pre
                .iter()
                .zip(d.iter())
                .map(|(col, di)| col[t] * di)
                .sum();
            dd += 2.0 * recon_d * recon_d;
        }
        let gamma = if dd <= 1e-30 {
            0.0
        } else {
            (-gd / dd).clamp(0.0, 1.0)
        };
        for j in 0..k {
            weights[j] += gamma * d[j];
        }
        if gamma * d.iter().map(|di| di.abs()).fold(0.0, f64::max) < 1e-12 {
            break;
        }
    }
    // Review P2-6: at the iteration cap the certificate must be tight, or
    // the refusal is honest (a loose gap means the caller is reading
    // weights that are provably not optimal).
    // Review A2-P2-1: at overflow scale `inf <= inf` would break the loop
    // early and `inf > inf` would pass the acceptance check - a non-finite
    // certificate or objective is refused BEFORE either guard runs.
    if !last_gap.is_finite() || !objective(&weights).is_finite() {
        return Err(SynthError::NotConverged {
            gap: last_gap,
            iterations: used,
        });
    }
    if last_gap > 1e-6 * (objective(&weights).abs() + 1.0) {
        return Err(SynthError::NotConverged {
            gap: last_gap,
            iterations: used,
        });
    }
    let final_obj = objective(&weights);
    let pre_fit_rmse = (final_obj / t0 as f64).sqrt();
    let synthetic_post: f64 = donor_post
        .iter()
        .zip(weights.iter())
        .map(|(v, w)| v * w)
        .sum();
    Ok(SynthResult {
        weights,
        post_gap: treated_post - synthetic_post,
        pre_fit_rmse,
        iterations: used,
        convergence_gap: last_gap,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn exact_replica_donor_concentrates_the_weight() {
        // Donor 0 IS the treated unit's pre-trajectory: the fit must land
        // on that vertex, and the post gap must equal the effect exactly.
        let treated_pre = [3.0, 4.0, 5.0, 6.0];
        let donors = vec![vec![3.0, 4.0, 5.0, 6.0], vec![10.0, 11.0, 12.0, 13.0]];
        let out = synthetic_control(&treated_pre, &donors, 9.0, &[9.0, 17.0]).unwrap();
        assert!(
            (out.weights[0] - 1.0).abs() < 1e-6,
            "weights {:?}",
            out.weights
        );
        assert!((out.post_gap - 0.0).abs() < 1e-6, "gap {}", out.post_gap);
        assert!(out.pre_fit_rmse < 1e-6);
        // Simplex validity holds regardless of convergence speed.
        let sum: f64 = out.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!(out.weights.iter().all(|w| *w >= -1e-12));
    }

    #[test]
    fn mixture_truth_is_recovered_and_the_post_gap_prices_the_effect() {
        // Treated pre = 0.5*A + 0.5*B. With post = 7 the treated unit runs
        // 2 above the synthetic reconstruction (0.5*4 + 0.5*6 = 5) - the
        // gap prices the effect.
        let treated_pre = [2.0, 3.0, 4.0];
        let donors = vec![vec![1.0, 2.0, 3.0], vec![3.0, 4.0, 5.0]];
        let out = synthetic_control(&treated_pre, &donors, 7.0, &[4.0, 6.0]).unwrap();
        assert!((out.post_gap - 2.0).abs() < 1e-6, "gap {}", out.post_gap);
        for (i, w) in out.weights.iter().enumerate() {
            assert!((*w - 0.5).abs() < 1e-3, "weight {i} = {w}");
        }
        // Without the jump the gap is zero: same pre-fit, honest null.
        let out = synthetic_control(&treated_pre, &donors, 5.0, &[4.0, 6.0]).unwrap();
        assert!(out.post_gap.abs() < 1e-6, "phantom gap {}", out.post_gap);
    }

    #[test]
    fn a_poor_donor_pool_admits_it_via_the_pre_fit_error() {
        // No convex combination can track a doubling unit: the pre-fit RMSE
        // stays large, and the caller sees it.
        let treated_pre = [1.0, 2.0, 4.0, 8.0];
        let donors = vec![vec![1.0, 1.0, 1.0, 1.0], vec![2.0, 2.0, 2.0, 2.0]];
        let out = synthetic_control(&treated_pre, &donors, 16.0, &[3.0, 3.0]).unwrap();
        assert!(out.pre_fit_rmse > 1.0, "rmse {}", out.pre_fit_rmse);
    }

    #[test]
    fn malformed_inputs_are_refused() {
        let treated = [1.0, 2.0, 3.0];
        let short = vec![vec![1.0, 2.0]];
        assert!(matches!(
            synthetic_control(&treated, &short, 4.0, &[3.0]),
            Err(SynthError::DonorLengthMismatch { index: 0, .. })
        ));
        assert!(matches!(
            synthetic_control(&treated, &[], 4.0, &[]),
            Err(SynthError::TooFewUnits(_))
        ));
        let dirty = vec![vec![1.0, f64::NAN, 3.0]];
        assert!(matches!(
            synthetic_control(&treated, &dirty, 4.0, &[3.0]),
            Err(SynthError::NotFinite { index: 1 })
        ));
        // Post-value count disagreeing with the donor count.
        let donors = vec![vec![1.0, 2.0, 3.0]];
        assert!(matches!(
            synthetic_control(&treated, &donors, 4.0, &[3.0, 4.0]),
            Err(SynthError::PostCountMismatch {
                donors: 1,
                posts: 2
            })
        ));
        // Review P2-7: a NaN post must refuse, not yield a NaN effect.
        assert!(matches!(
            synthetic_control(&treated, &donors, f64::NAN, &[3.0]),
            Err(SynthError::NotFinite { .. })
        ));
        assert!(matches!(
            synthetic_control(&treated, &donors, 4.0, &[f64::NAN]),
            Err(SynthError::NotFinite { .. })
        ));
    }

    /// Review A2-P2-1 regression: finite inputs at 1e300 scale overflow
    /// the objective; `inf <= inf` used to break the loop early and
    /// `inf > inf` used to pass the acceptance check - the fit must
    /// refuse, never return inf-weighted "optimal" weights.
    #[test]
    fn overflowing_scale_refuses_instead_of_returning_inf_weights() {
        let treated_pre = [1e300, 2e300, 3e300];
        let donors = vec![vec![1e300, 2e300, 3e300], vec![2e300, 4e300, 6e300]];
        assert!(matches!(
            synthetic_control(&treated_pre, &donors, 4e300, &[4e300, 8e300]),
            Err(SynthError::NotConverged { .. })
        ));
    }
}
