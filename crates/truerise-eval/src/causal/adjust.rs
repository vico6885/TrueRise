//! Covariate adjustment (review family B, part 2): propensity-score
//! weighting for self-selected treatment, and effect-modification testing
//! for the "works for WHICH runs" question.
//!
//! Boundary stated once: an agent runtime can usually run a TRUE
//! intervention (rerun without the component), so adjustment is the
//! FALLBACK for logs that were merely observed - the review verdict holds
//! that observational designs are the retreat, never the default. The
//! positivity contract is fail-closed: a propensity score outside the
//! declared floor refuses the design instead of clipping silently.

/// Refusals for covariate adjustment.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AdjustError {
    /// Row-aligned inputs disagree in length.
    #[error("adjustment refused: {field} has {rows} rows, outcome has {outcome_rows}")]
    LengthMismatch {
        /// The offending field name.
        field: &'static str,
        /// Its row count.
        rows: usize,
        /// The outcome's row count.
        outcome_rows: usize,
    },
    /// NaN or infinite values poison every estimate.
    #[error("adjustment refused: non-finite value in {field} at index {index}")]
    NotFinite {
        /// The offending field name.
        field: &'static str,
        /// Position of the offending value.
        index: usize,
    },
    /// Treatment must be binary.
    #[error("adjustment refused: treatment[{index}] = {value} is not binary")]
    NotBinary {
        /// Position of the offending value.
        index: usize,
        /// The offending value.
        value: f64,
    },
    /// The logistic fit did not converge within the iteration contract.
    #[error(
        "adjustment refused: propensity fit did not converge (separation or degenerate covariates)"
    )]
    PropensityNotConverged,
    /// A propensity score crossed the positivity floor: the design cannot
    /// defend exchangeability on this stratum, so the whole design is
    /// refused (review verdict: fail-closed, never silently clipped).
    #[error(
        "adjustment refused: positivity violated - propensity {score} crosses the floor {floor}"
    )]
    PositivityViolated {
        /// The offending propensity score.
        score: f64,
        /// The declared floor.
        floor: f64,
    },
    /// The floor argument itself is nonsense (not a probability below 0.5).
    #[error("adjustment refused: positivity floor {0} is not a probability below 0.5")]
    BadFloor(f64),
    /// Too few rows for the fitted parameters.
    #[error(
        "adjustment refused: {rows} rows cannot support {params} parameters (intercept + covariates)"
    )]
    TooFewRows {
        /// Available rows.
        rows: usize,
        /// Fitted parameter count.
        params: usize,
    },
    /// The interaction design is rank-deficient (duplicated columns or too
    /// few rows) - no propensity is involved, so the refusal names the
    /// actual defect (review A2-P2-6c).
    #[error("adjustment refused: the interaction design is rank-deficient")]
    Underdetermined,
}

/// Design for propensity adjustment: binary treatment, covariate columns,
/// outcome - all row-aligned.
#[derive(Debug, Clone, PartialEq)]
pub struct AdjustDesign {
    /// Binary treatment per row (0/1).
    pub treatment: Vec<f64>,
    /// Covariate columns (confounders), row-aligned.
    pub covariates: Vec<Vec<f64>>,
    /// Outcome per row.
    pub outcome: Vec<f64>,
}

impl AdjustDesign {
    fn validate(&self) -> Result<usize, AdjustError> {
        let rows = self.outcome.len();
        if self.treatment.len() != rows {
            return Err(AdjustError::LengthMismatch {
                field: "treatment",
                rows: self.treatment.len(),
                outcome_rows: rows,
            });
        }
        for col in &self.covariates {
            if col.len() != rows {
                return Err(AdjustError::LengthMismatch {
                    field: "covariate",
                    rows: col.len(),
                    outcome_rows: rows,
                });
            }
        }
        let params = 1 + self.covariates.len();
        if rows < params + 1 {
            return Err(AdjustError::TooFewRows { rows, params });
        }
        for (i, v) in self.outcome.iter().enumerate() {
            if !v.is_finite() {
                return Err(AdjustError::NotFinite {
                    field: "outcome",
                    index: i,
                });
            }
        }
        for (i, t) in self.treatment.iter().enumerate() {
            if !t.is_finite() {
                return Err(AdjustError::NotFinite {
                    field: "treatment",
                    index: i,
                });
            }
            if *t != 0.0 && *t != 1.0 {
                return Err(AdjustError::NotBinary {
                    index: i,
                    value: *t,
                });
            }
        }
        for col in &self.covariates {
            for (i, v) in col.iter().enumerate() {
                if !v.is_finite() {
                    return Err(AdjustError::NotFinite {
                        field: "covariate",
                        index: i,
                    });
                }
            }
        }
        Ok(rows)
    }

    /// Design matrix with the intercept column first.
    fn matrix(&self) -> Vec<Vec<f64>> {
        let rows = self.outcome.len();
        let mut cols = vec![vec![1.0; rows]];
        cols.extend(self.covariates.iter().cloned());
        cols
    }
}

fn sigmoid(x: f64) -> f64 {
    // Guarded: extreme logits saturate instead of overflowing to inf.
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// The propensity-weighted effect report.
#[derive(Debug, Clone, PartialEq)]
pub struct IpwReport {
    /// Hájek (weight-normalized) ATE estimate.
    pub ate: f64,
    /// Horvitz-Thompson (unnormalized) ATE estimate - sensitive to extreme
    /// weights, reported so the divergence between the two is visible.
    pub ate_unnormalized: f64,
    /// Smallest fitted propensity (positivity diagnostic).
    pub min_propensity: f64,
    /// Largest fitted propensity (positivity diagnostic).
    pub max_propensity: f64,
    /// Largest RAW inverse-propensity weight actually applied (1/p or
    /// 1/(1-p); the estimates themselves are weight-normalized).
    pub max_weight: f64,
}

/// Logistic propensity fit via IRLS on top of the crate's own OLS solver.
///
/// Returns per-row propensity scores. Separation (a covariate perfectly
/// predicting treatment) refuses rather than returning silly 0/1 scores.
fn fit_propensity(design: &AdjustDesign) -> Result<Vec<f64>, AdjustError> {
    let rows = design.outcome.len();
    let cols = design.matrix();
    let params = cols.len();
    let mut beta = vec![0.0; params];
    for _ in 0..50 {
        // p = sigmoid(x beta); weights w = p(1-p); working response
        // z = x beta + (t - p) / w. The IRLS step is a weighted OLS on
        // sqrt(w)-scaled columns, which the crate's ols_fit already solves.
        let mut scaled_cols: Vec<Vec<f64>> = cols.iter().map(|_| vec![0.0; rows]).collect();
        let mut scaled_response = vec![0.0; rows];
        for r in 0..rows {
            let mut eta = 0.0;
            for (c, col) in cols.iter().enumerate() {
                eta += beta[c] * col[r];
            }
            let p = sigmoid(eta);
            let w = (p * (1.0 - p)).max(1e-10);
            let z = eta + (design.treatment[r] - p) / w;
            for (c, col) in cols.iter().enumerate() {
                scaled_cols[c][r] = col[r] * w.sqrt();
            }
            scaled_response[r] = z * w.sqrt();
        }
        let col_refs: Vec<&[f64]> = scaled_cols.iter().map(Vec::as_slice).collect();
        let new_beta = super::refute::ols_fit(&col_refs, &scaled_response)
            .ok_or(AdjustError::PropensityNotConverged)?;
        let delta: f64 = beta
            .iter()
            .zip(new_beta.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        // Review A2-P2-2: f64::max swallows NaN - an overflowed IRLS step
        // must refuse, not masquerade as convergence.
        if !delta.is_finite() || new_beta.iter().any(|b| !b.is_finite()) {
            return Err(AdjustError::PropensityNotConverged);
        }
        beta = new_beta;
        if delta < 1e-10 {
            let scores: Vec<f64> = (0..rows)
                .map(|r| {
                    let mut eta = 0.0;
                    for (c, col) in cols.iter().enumerate() {
                        eta += beta[c] * col[r];
                    }
                    sigmoid(eta)
                })
                .collect();
            if scores.iter().any(|p| !p.is_finite()) {
                return Err(AdjustError::PropensityNotConverged);
            }
            return Ok(scores);
        }
    }
    Err(AdjustError::PropensityNotConverged)
}

/// Propensity-score IPW effect estimate with a FAIL-CLOSED positivity
/// contract.
///
/// `positivity_floor`: the [floor, 1-floor] window every fitted propensity
/// must sit inside; `None` defaults to 0.01. A violation REFUSES the
/// design - an uncovered stratum is exactly the positivity assumption
/// failing, and silently clipping it would manufacture an estimate from
/// an unidentified region. Passing `Some(0.0)` disables the check
/// entirely: the caller then owns the positivity defense in the ledger
/// (scores of 1e-16 pass unrefused - a deliberate, documented escape).
pub fn ipw_effect(
    design: &AdjustDesign,
    positivity_floor: Option<f64>,
) -> Result<IpwReport, AdjustError> {
    let rows = design.validate()?;
    let floor = positivity_floor.unwrap_or(0.01);
    if !(0.0..0.5).contains(&floor) {
        return Err(AdjustError::BadFloor(floor));
    }
    let scores = fit_propensity(design)?;
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    for &p in &scores {
        if p < floor || p > 1.0 - floor {
            return Err(AdjustError::PositivityViolated { score: p, floor });
        }
    }
    let mut num_t = 0.0;
    let mut den_t = 0.0;
    let mut num_c = 0.0;
    let mut den_c = 0.0;
    let mut ht_t = 0.0;
    let mut ht_c = 0.0;
    let mut max_weight = 0.0f64;
    for r in 0..rows {
        let p = scores[r];
        if design.treatment[r] == 1.0 {
            let w = 1.0 / p;
            max_weight = max_weight.max(w);
            num_t += design.outcome[r] * w;
            den_t += w;
            ht_t += design.outcome[r] * w;
        } else {
            let w = 1.0 / (1.0 - p);
            max_weight = max_weight.max(w);
            num_c += design.outcome[r] * w;
            den_c += w;
            ht_c += design.outcome[r] * w;
        }
    }
    // Review A2-P1-1: the Horvitz-Thompson estimator divides by the TOTAL
    // row count, not the arm counts - dividing per-arm smuggles an n/n_arm
    // scale factor in that drowns the weight-instability signal the field
    // exists to surface.
    Ok(IpwReport {
        ate: num_t / den_t - num_c / den_c,
        ate_unnormalized: ht_t / rows as f64 - ht_c / rows as f64,
        min_propensity: min,
        max_propensity: max,
        max_weight,
    })
}

/// The effect-modification report.
#[derive(Debug, Clone, PartialEq)]
pub struct ModificationReport {
    /// Effect in the baseline subgroup (subgroup = 0).
    pub effect_baseline: f64,
    /// Effect DIFFERENCE between subgroups (the interaction coefficient).
    pub effect_delta: f64,
    /// Effect in the marked subgroup (baseline + delta).
    pub effect_subgroup: f64,
}

/// Effect modification via the interaction design y ~ 1 + t + g + t*g.
///
/// This answers "works for WHICH runs" without a full CATE learner: the
/// interaction coefficient IS the CATE difference between subgroups, with
/// the subgroup declared up front rather than mined (mining subgroups
/// until one shows a difference is the garden of forking paths, and the
/// intent contract (G32) already voids that).
pub fn effect_modification(
    treatment: &[f64],
    subgroup: &[f64],
    outcome: &[f64],
) -> Result<ModificationReport, AdjustError> {
    if treatment.len() != outcome.len() || subgroup.len() != outcome.len() {
        return Err(AdjustError::LengthMismatch {
            field: "treatment/subgroup",
            rows: treatment.len(),
            outcome_rows: outcome.len(),
        });
    }
    // Four fitted parameters (intercept, t, g, t*g) need one spare row.
    if outcome.len() < 5 {
        return Err(AdjustError::TooFewRows {
            rows: outcome.len(),
            params: 4,
        });
    }
    for (i, t) in treatment.iter().enumerate() {
        if !t.is_finite() {
            return Err(AdjustError::NotFinite {
                field: "treatment",
                index: i,
            });
        }
        if *t != 0.0 && *t != 1.0 {
            return Err(AdjustError::NotBinary {
                index: i,
                value: *t,
            });
        }
    }
    for (i, g) in subgroup.iter().enumerate() {
        if !g.is_finite() {
            return Err(AdjustError::NotFinite {
                field: "subgroup",
                index: i,
            });
        }
        if *g != 0.0 && *g != 1.0 {
            return Err(AdjustError::NotBinary {
                index: i,
                value: *g,
            });
        }
    }
    let rows = outcome.len();
    let ones = vec![1.0; rows];
    let t_vec = treatment.to_vec();
    let g_vec = subgroup.to_vec();
    let interaction: Vec<f64> = treatment
        .iter()
        .zip(subgroup.iter())
        .map(|(t, g)| t * g)
        .collect();
    let cols: Vec<&[f64]> = vec![&ones, &t_vec, &g_vec, &interaction];
    let beta = super::refute::ols_fit(&cols, outcome).ok_or(AdjustError::Underdetermined)?;
    let effect_baseline = beta[1];
    let effect_delta = beta[3];
    Ok(ModificationReport {
        effect_baseline,
        effect_delta,
        effect_subgroup: effect_baseline + effect_delta,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Seeded study: confounder w balanced; treatment leans on w; outcome
    /// y = 2t + 4w + small noise. The naive difference is biased UP by the
    /// confounder; IPW must pull the estimate back toward 2.
    fn confounded_study(n: usize, seed: u64) -> (AdjustDesign, f64) {
        let mut rng = super::super::rng::SplitMix64::new(seed);
        let mut treatment = Vec::with_capacity(n);
        let mut covariate = Vec::with_capacity(n);
        let mut outcome = Vec::with_capacity(n);
        let mut naive_sum_t = 0.0;
        let mut naive_n_t = 0usize;
        let mut naive_sum_c = 0.0;
        for i in 0..n {
            let w = (i % 2) as f64;
            let lean = if w == 1.0 { 0.7 } else { 0.3 };
            let t = if rng.next_f64() < lean { 1.0 } else { 0.0 };
            let y = 2.0 * t + 4.0 * w + (rng.next_f64() - 0.5) * 0.2;
            treatment.push(t);
            covariate.push(w);
            outcome.push(y);
            if t == 1.0 {
                naive_sum_t += y;
                naive_n_t += 1;
            } else {
                naive_sum_c += y;
            }
        }
        let naive = naive_sum_t / naive_n_t as f64 - naive_sum_c / (n - naive_n_t) as f64;
        let design = AdjustDesign {
            treatment,
            covariates: vec![covariate],
            outcome,
        };
        (design, naive)
    }

    #[test]
    fn ipw_removes_the_self_selection_bias_the_naive_difference_carries() {
        let (design, naive) = confounded_study(600, 11);
        let report = ipw_effect(&design, None).unwrap();
        assert!(
            (naive - 2.0).abs() > 0.5,
            "fixture degenerated: naive estimate {} looks unbiased",
            naive
        );
        assert!(
            (report.ate - 2.0).abs() < 0.35,
            "IPW {} did not recover the true effect 2",
            report.ate
        );
        assert!(
            (report.ate - 2.0).abs() < (naive - 2.0).abs(),
            "IPW must be CLOSER to the truth than the naive difference"
        );
        // Positivity diagnostics are real numbers inside the window.
        assert!(report.min_propensity > 0.01 && report.max_propensity < 0.99);
        // Review A2-P1-1: the unnormalized field is the CANONICAL
        // Horvitz-Thompson (divide by TOTAL rows) - the arm-count scaling
        // of the old formula sat ~3.77 on this fixture, ~2x off truth.
        assert!(
            (report.ate_unnormalized - 2.0).abs() < 0.35,
            "HT estimate {} is not the canonical estimator",
            report.ate_unnormalized
        );
    }

    #[test]
    fn positivity_violation_is_refused_not_clipped() {
        // A covariate that ALMOST predicts treatment (10% flips): fitted
        // propensities reach ~0.9/0.1, and the strict floor 0.2 forces a
        // deterministic positivity refusal - the fit itself converges.
        let n = 60;
        let treatment: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
        // Flip on i%7==3: covers BOTH parities (i=3 is treated, i=10 is
        // control), so the fit converges with extreme-but-finite scores.
        let covariate: Vec<f64> = (0..n)
            .map(|i| {
                if i % 7 == 3 {
                    1.0 - treatment[i]
                } else {
                    treatment[i]
                }
            })
            .collect();
        let design = AdjustDesign {
            treatment: treatment.clone(),
            covariates: vec![covariate],
            outcome: vec![1.0; n],
        };
        assert!(matches!(
            ipw_effect(&design, Some(0.2)),
            Err(AdjustError::PositivityViolated { score, .. })
            if score > 0.8 || score < 0.2
        ));
    }

    #[test]
    fn separated_designs_refuse_the_propensity_fit() {
        // Covariate = treatment exactly: logistic separation.
        let n = 20;
        let treatment: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
        let design = AdjustDesign {
            treatment: treatment.clone(),
            covariates: vec![treatment.clone()],
            outcome: vec![1.0; n],
        };
        assert!(matches!(
            ipw_effect(&design, None),
            Err(AdjustError::PropensityNotConverged)
        ));
    }

    #[test]
    fn effect_modification_recovers_the_interaction_exactly() {
        // y = 2t + 3tg with balanced orthogonal binary columns.
        let n = 40;
        let mut treatment = Vec::with_capacity(n);
        let mut subgroup = Vec::with_capacity(n);
        let mut outcome = Vec::with_capacity(n);
        for i in 0..n {
            let t = (i % 2) as f64;
            let g = ((i / 2) % 2) as f64;
            treatment.push(t);
            subgroup.push(g);
            outcome.push(2.0 * t + 3.0 * t * g);
        }
        let report = effect_modification(&treatment, &subgroup, &outcome).unwrap();
        assert!(
            (report.effect_baseline - 2.0).abs() < 1e-9,
            "was {}",
            report.effect_baseline
        );
        assert!(
            (report.effect_delta - 3.0).abs() < 1e-9,
            "was {}",
            report.effect_delta
        );
        assert!((report.effect_subgroup - 5.0).abs() < 1e-9);
    }

    #[test]
    fn malformed_adjustment_designs_are_refused() {
        let mut design = confounded_study(30, 21).0;
        design.outcome[0] = f64::NAN;
        assert!(matches!(
            ipw_effect(&design, None),
            Err(AdjustError::NotFinite {
                field: "outcome",
                index: 0
            })
        ));
        let mut design = confounded_study(30, 22).0;
        design.treatment[1] = 0.5;
        assert!(matches!(
            ipw_effect(&design, None),
            Err(AdjustError::NotBinary { index: 1, .. })
        ));
        // 2 rows cannot support intercept + covariate + one spare row.
        let tiny = AdjustDesign {
            treatment: vec![0.0, 1.0],
            covariates: vec![vec![1.0, 0.0]],
            outcome: vec![1.0, 2.0],
        };
        assert!(matches!(
            ipw_effect(&tiny, None),
            Err(AdjustError::TooFewRows { .. })
        ));
        // Nonsense floor.
        let design = confounded_study(30, 23).0;
        assert!(matches!(
            ipw_effect(&design, Some(0.7)),
            Err(AdjustError::BadFloor(0.7))
        ));
        // Effect modification: non-binary subgroup (5 rows - the row
        // contract passes, the binary scan fires).
        let g: Vec<f64> = vec![0.0, 1.0, 0.5, 1.0, 0.0];
        assert!(matches!(
            effect_modification(&[0.0, 1.0, 0.0, 1.0, 0.0], &g, &[1.0, 2.0, 3.0, 4.0, 5.0]),
            Err(AdjustError::NotBinary { index: 2, .. })
        ));
        // Below the interaction row contract.
        assert!(matches!(
            effect_modification(
                &[0.0, 1.0, 0.0, 1.0],
                &[0.0, 1.0, 0.5, 1.0],
                &[1.0, 2.0, 3.0, 4.0]
            ),
            Err(AdjustError::TooFewRows { rows: 4, params: 4 })
        ));
    }
}
