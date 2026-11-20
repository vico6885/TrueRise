//! The refuter battery (CA3): challenges an observational effect estimate
//! must survive before it can carry a causal-observational claim.
//!
//! Three refuters, all seeded and deterministic:
//! - [`placebo_test`]: the treatment labels are permuted; if permuted
//!   treatments produce contrasts as large as the observed one, the
//!   observed effect is indistinguishable from a placebo and the claim is
//!   challenged (a large `placebo_p` is the CHALLENGE, a small one means
//!   the effect survived).
//! - [`subset_stability`]: seeded bootstrap over rows; the percentile
//!   interval shows how far the estimate wanders under resampling
//!   (the G27 family applied to a coefficient).
//! - [`confounder_injection`]: a synthetic binary unmeasured factor with
//!   caller-chosen loadings on treatment and outcome is injected; the
//!   resulting coefficient shift PRICES the fragility (the experimental
//!   cousin of the E-value in [`super::evalue`]).
//!
//! Boundary: a refuter that passes does not prove causality - it removes
//! ONE class of boring explanations. Refuters fail individually; that is
//! why they are a battery.

/// Design for a two-arm effect study: one treatment column, optional
/// covariate columns, one outcome column - all row-aligned.
#[derive(Debug, Clone, PartialEq)]
pub struct Design {
    /// Treatment/exposure value per row (binary or continuous).
    pub treatment: Vec<f64>,
    /// Covariate columns, row-aligned with `treatment`.
    pub covariates: Vec<Vec<f64>>,
    /// Outcome value per row.
    pub outcome: Vec<f64>,
}

/// Refusal reasons for estimation and refutation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RefuteError {
    /// Row vectors disagree in length - the design is not a matrix.
    #[error("refuter refused: {field} has {rows} rows, outcome has {outcome_rows}")]
    LengthMismatch {
        /// The offending field name.
        field: &'static str,
        /// Its row count.
        rows: usize,
        /// The outcome row count.
        outcome_rows: usize,
    },
    /// Too few rows for the requested columns (no residual df).
    #[error(
        "refuter refused: {rows} rows cannot support {columns} columns - at least {min} rows are required"
    )]
    TooFewRows {
        /// Available rows.
        rows: usize,
        /// Requested column count (treatment + covariates).
        columns: usize,
        /// The minimum contract (columns + 1).
        min: usize,
    },
    /// NaN or infinite values poison every estimate.
    #[error("refuter refused: non-finite value in {field} at index {index}")]
    NotFinite {
        /// The offending field name.
        field: &'static str,
        /// Position of the offending value.
        index: usize,
    },
    /// The normal equations are singular (duplicated or collinear columns).
    #[error("refuter refused: design matrix is singular - duplicated or collinear columns")]
    SingularDesign,
    /// A draw count below 2 cannot produce an interval or a distribution.
    #[error("refuter refused: {count} draws cannot form a distribution - at least 2 are required")]
    BadDrawCount {
        /// The offending draw count.
        count: u64,
    },
}

impl Design {
    /// Validate the design: alignment, finiteness, residual df.
    pub fn validate(&self) -> Result<usize, RefuteError> {
        let rows = self.outcome.len();
        if self.treatment.len() != rows {
            return Err(RefuteError::LengthMismatch {
                field: "treatment",
                rows: self.treatment.len(),
                outcome_rows: rows,
            });
        }
        for col in &self.covariates {
            if col.len() != rows {
                return Err(RefuteError::LengthMismatch {
                    field: "covariate",
                    rows: col.len(),
                    outcome_rows: rows,
                });
            }
        }
        // The estimation contract includes the intercept column (see
        // estimate_effect), so a design needs one row beyond its columns.
        let params = 2 + self.covariates.len();
        if rows < params + 1 {
            return Err(RefuteError::TooFewRows {
                rows,
                columns: params,
                min: params + 1,
            });
        }
        for (i, v) in self.outcome.iter().enumerate() {
            if !v.is_finite() {
                return Err(RefuteError::NotFinite {
                    field: "outcome",
                    index: i,
                });
            }
        }
        for (i, v) in self.treatment.iter().enumerate() {
            if !v.is_finite() {
                return Err(RefuteError::NotFinite {
                    field: "treatment",
                    index: i,
                });
            }
        }
        for col in &self.covariates {
            for (i, v) in col.iter().enumerate() {
                if !v.is_finite() {
                    return Err(RefuteError::NotFinite {
                        field: "covariate",
                        index: i,
                    });
                }
            }
        }
        Ok(rows)
    }

    /// The regression columns in order: treatment first, covariates after.
    fn columns(&self) -> Vec<&[f64]> {
        let mut cols = vec![self.treatment.as_slice()];
        cols.extend(self.covariates.iter().map(Vec::as_slice));
        cols
    }
}

/// Ordinary least squares via normal equations and Gaussian elimination
/// with partial pivoting.
///
/// Returns the coefficient per column, or `None` when the design matrix is
/// singular (rank-deficient designs are a caller bug, not a rounding
/// accident - duplicated columns are refused, not resolved arbitrarily).
#[must_use]
pub fn ols_fit(columns: &[&[f64]], outcome: &[f64]) -> Option<Vec<f64>> {
    let k = columns.len();
    if k == 0 {
        return Some(Vec::new());
    }
    let n = outcome.len();
    if columns.iter().any(|c| c.len() != n) {
        return None;
    }
    // A = X'X (k x k), b = X'y.
    let mut a = vec![0.0; k * k];
    let mut b = vec![0.0; k];
    for col_j in 0..k {
        for col_i in 0..k {
            let dot: f64 = (0..n).map(|r| columns[col_i][r] * columns[col_j][r]).sum();
            a[col_i * k + col_j] = dot;
        }
        let dot: f64 = (0..n).map(|r| columns[col_j][r] * outcome[r]).sum();
        b[col_j] = dot;
    }
    // Gaussian elimination with partial pivoting on the augmented system.
    // The pivot threshold is RELATIVE to the matrix scale (review P2-5):
    // an absolute floor of 1e-12 refused uniformly tiny but perfectly
    // conditioned systems (all columns at 1e-8 scale and below).
    let scale: f64 = a.iter().fold(0.0f64, |m, &v| m.max(v.abs()));
    if scale == 0.0 {
        return None; // an all-zero normal matrix is rank 0 by definition
    }
    for pivot in 0..k {
        let (best_row, best_val) = (pivot..k)
            .map(|r| (r, a[r * k + pivot].abs()))
            .fold((pivot, 0.0), |acc, x| if x.1 > acc.1 { x } else { acc });
        if best_val <= 1e-12 * scale {
            return None; // rank-deficient
        }
        if best_row != pivot {
            // Two disjoint rows (best_row != pivot): swap via split_at_mut
            // to satisfy the borrow checker without copying.
            if best_row > pivot {
                let (head, tail) = a.split_at_mut(best_row * k);
                head[pivot * k..pivot * k + k].swap_with_slice(&mut tail[0..k]);
            } else {
                let (head, tail) = a.split_at_mut(pivot * k);
                head[best_row * k..best_row * k + k].swap_with_slice(&mut tail[0..k]);
            }
            b.swap(pivot, best_row);
        }
        for row in (pivot + 1)..k {
            let factor = a[row * k + pivot] / a[pivot * k + pivot];
            if factor == 0.0 {
                continue;
            }
            for col in pivot..k {
                a[row * k + col] -= factor * a[pivot * k + col];
            }
            b[row] -= factor * b[pivot];
        }
    }
    // Back substitution.
    let mut beta = vec![0.0; k];
    for row in (0..k).rev() {
        let mut acc = b[row];
        for col in (row + 1)..k {
            acc -= a[row * k + col] * beta[col];
        }
        beta[row] = acc / a[row * k + row];
    }
    Some(beta)
}

/// Estimate the treatment effect: the OLS coefficient on the treatment
/// column, with an intercept and the covariates adjusted. The intercept
/// matters: without it, arm-level level differences leak into the
/// treatment coefficient and every refuter downstream inherits the bias.
pub fn estimate_effect(design: &Design) -> Result<f64, RefuteError> {
    design.validate()?;
    let ones = vec![1.0; design.outcome.len()];
    let mut columns = vec![ones.as_slice()];
    columns.extend(design.columns());
    let beta = ols_fit(&columns, &design.outcome);
    beta.map(|b| b[1]).ok_or(RefuteError::SingularDesign)
}

/// The placebo refuter's report.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceboReport {
    /// The observed treatment coefficient.
    pub observed: f64,
    /// Assignment-space p-value of the observed contrast WITHIN the placebo
    /// distribution: small = the observed effect survived the placebo
    /// challenge; large = permuted labels do as well, and the claim is
    /// challenged.
    pub placebo_p: f64,
    /// Placebo permutations whose contrast reached the observed one.
    pub hits: u64,
    /// The VALID permutation count behind the report: singular permuted
    /// refits are skipped and never counted (review P2-7).
    pub permutations: u64,
}

/// Permute the treatment labels and re-estimate, seeded.
pub fn placebo_test(
    design: &Design,
    permutations: u64,
    seed: u64,
) -> Result<PlaceboReport, RefuteError> {
    design.validate()?;
    if permutations < 2 {
        return Err(RefuteError::BadDrawCount {
            count: permutations,
        });
    }
    let observed = estimate_effect(design)?;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0x9147_0B3D);
    let mut permuted_treatment = design.treatment.clone();
    let mut hits: u64 = 0;
    let mut valid: u64 = 0;
    for _ in 0..permutations {
        rng.shuffle(&mut permuted_treatment);
        let shuffled = Design {
            treatment: permuted_treatment.clone(),
            covariates: design.covariates.clone(),
            outcome: design.outcome.clone(),
        };
        if let Ok(effect) = estimate_effect(&shuffled) {
            valid += 1;
            if effect.abs() >= observed.abs() {
                hits += 1;
            }
        }
    }
    // Review P2-7: a singular permuted refit is skipped, and the add-one
    // denominator must follow the VALID draws - otherwise collinear
    // covariates silently thin the null distribution while the report
    // claims the full permutation count.
    Ok(PlaceboReport {
        observed,
        placebo_p: (1.0 + hits as f64) / (1.0 + valid as f64),
        hits,
        permutations: valid,
    })
}

/// The subset-stability refuter's report.
#[derive(Debug, Clone, PartialEq)]
pub struct StabilityReport {
    /// The point estimate over the full design.
    pub point: f64,
    /// Bootstrap 2.5th percentile.
    pub ci_lo: f64,
    /// Bootstrap 97.5th percentile.
    pub ci_hi: f64,
    /// The bootstrap count behind the interval.
    pub boots: u64,
}

/// Seeded bootstrap over rows; the percentile interval of the re-estimated
/// effect is the stability footprint.
pub fn subset_stability(
    design: &Design,
    boots: u64,
    seed: u64,
) -> Result<StabilityReport, RefuteError> {
    design.validate()?;
    if boots < 2 {
        return Err(RefuteError::BadDrawCount { count: boots });
    }
    let point = estimate_effect(design)?;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0xB007_5744);
    let rows = design.outcome.len();
    let cols = design.columns();
    let mut estimates = Vec::with_capacity(boots as usize);
    for _ in 0..boots {
        // Resample row indices with replacement.
        let idx: Vec<usize> = (0..rows)
            .map(|_| rng.next_below(rows).unwrap_or(0))
            .collect();
        let treatment: Vec<f64> = idx.iter().map(|&i| design.treatment[i]).collect();
        let outcome: Vec<f64> = idx.iter().map(|&i| design.outcome[i]).collect();
        let covariates: Vec<Vec<f64>> = cols
            .iter()
            .skip(1)
            .map(|c| idx.iter().map(|&i| c[i]).collect())
            .collect();
        let resampled = Design {
            treatment,
            covariates,
            outcome,
        };
        if let Ok(effect) = estimate_effect(&resampled) {
            estimates.push(effect);
        }
    }
    if estimates.len() < 2 {
        return Err(RefuteError::SingularDesign);
    }
    estimates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let last = estimates.len() - 1;
    let lo = estimates[((last as f64) * 0.025).floor() as usize];
    let hi = estimates[((last as f64) * 0.975).ceil() as usize];
    Ok(StabilityReport {
        point,
        ci_lo: lo,
        ci_hi: hi,
        boots,
    })
}

/// The confounder-injection report.
#[derive(Debug, Clone, PartialEq)]
pub struct InjectionReport {
    /// The estimate without the synthetic factor.
    pub base: f64,
    /// The smallest re-estimate across injection simulations.
    pub shifted_min: f64,
    /// The largest re-estimate across injection simulations.
    pub shifted_max: f64,
    /// The simulation count behind the range.
    pub sims: u64,
}

/// Inject a synthetic binary unmeasured factor with the given loadings.
///
/// Per simulation: ONE binary factor `u` is drawn per row (balanced to
/// half-ones by seeded shuffle, so the factor has no free mass to hide in)
/// and it moves BOTH channels: treatment by `u_on_treatment * (2u - 1)`
/// and the outcome by `u_on_outcome * (2u - 1)`. The shift range over
/// simulations prices how badly an unmeasured factor with THOSE loadings
/// would distort the estimate. Zero loadings are identity: the report must
/// reproduce the base estimate exactly.
pub fn confounder_injection(
    design: &Design,
    u_on_treatment: f64,
    u_on_outcome: f64,
    sims: u64,
    seed: u64,
) -> Result<InjectionReport, RefuteError> {
    design.validate()?;
    if sims == 0 {
        return Err(RefuteError::BadDrawCount { count: 0 });
    }
    let base = estimate_effect(design)?;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0x1AB7_ABBE);
    let rows = design.outcome.len();
    let mut shifted_min = f64::INFINITY;
    let mut shifted_max = f64::NEG_INFINITY;
    for _ in 0..sims {
        // ONE balanced factor per simulation, shared by both channels.
        let mut flags: Vec<bool> = (0..rows).map(|i| i < rows / 2).collect();
        rng.shuffle(&mut flags);
        let treatment: Vec<f64> = design
            .treatment
            .iter()
            .zip(flags.iter())
            .map(|(&t, &u)| t + u_on_treatment * (if u { 1.0 } else { -1.0 }))
            .collect();
        let outcome: Vec<f64> = design
            .outcome
            .iter()
            .zip(flags.iter())
            .map(|(&y, &u)| y + u_on_outcome * (if u { 1.0 } else { -1.0 }))
            .collect();
        let injected = Design {
            treatment,
            covariates: design.covariates.clone(),
            outcome,
        };
        let effect = estimate_effect(&injected)?;
        shifted_min = shifted_min.min(effect);
        shifted_max = shifted_max.max(effect);
    }
    Ok(InjectionReport {
        base,
        shifted_min,
        shifted_max,
        sims,
    })
}

/// The negative-control-outcome refuter's report.
#[derive(Debug, Clone, PartialEq)]
pub struct NegativeControlReport {
    /// The observed effect of the treatment on the NEGATIVE-CONTROL
    /// outcome (should be ~0 in a clean design).
    pub control_effect: f64,
    /// Assignment-space p-value of that effect within its own placebo
    /// distribution: small = the treatment "moves" an outcome it must not
    /// move.
    pub control_p: f64,
    /// True when the primary effect estimate must NOT be trusted as
    /// confounding-free: the treatment leaked into an outcome it is known
    /// not to affect. Fires at `control_p < 0.05` (declared; review P2-11).
    pub flags_residual_confounding: bool,
}

/// Negative-control refuter (review family B): if the treatment is truly
/// confounded, the confounding also leaks into a NEGATIVE-CONTROL outcome
/// N that treatment cannot causally affect. A significant treatment
/// association with N therefore FLAGS the primary estimate.
///
/// The flag fires at `control_p < 0.05` (review P2-11: the cutoff is a
/// declared constant of the report, not an implicit default). Zero-draw
/// calls are refused: a "clean" verdict from zero evidence is the exact
/// fail-open lie this battery exists to prevent (review P1-1).
///
/// Semantics: this refuter can only FLAG, never clear - passing it means
/// "no leak found in this outcome", which is evidence for the ledger's
/// exchangeability defense, not proof of no confounding.
pub fn negative_control(
    design: &Design,
    nc_outcome: &[f64],
    permutations: u64,
    seed: u64,
) -> Result<NegativeControlReport, RefuteError> {
    design.validate()?;
    if permutations < 2 {
        return Err(RefuteError::BadDrawCount {
            count: permutations,
        });
    }
    if nc_outcome.len() != design.outcome.len() {
        return Err(RefuteError::LengthMismatch {
            field: "negative-control outcome",
            rows: nc_outcome.len(),
            outcome_rows: design.outcome.len(),
        });
    }
    for (i, v) in nc_outcome.iter().enumerate() {
        if !v.is_finite() {
            return Err(RefuteError::NotFinite {
                field: "negative-control outcome",
                index: i,
            });
        }
    }
    let control_design = Design {
        treatment: design.treatment.clone(),
        covariates: design.covariates.clone(),
        outcome: nc_outcome.to_vec(),
    };
    let observed = estimate_effect(&control_design)?;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0x9E6A_71CE);
    let mut permuted_treatment = design.treatment.clone();
    let mut hits: u64 = 0;
    let mut valid: u64 = 0;
    for _ in 0..permutations {
        rng.shuffle(&mut permuted_treatment);
        let shuffled = Design {
            treatment: permuted_treatment.clone(),
            covariates: design.covariates.clone(),
            outcome: nc_outcome.to_vec(),
        };
        if let Ok(effect) = estimate_effect(&shuffled) {
            valid += 1;
            if effect.abs() >= observed.abs() {
                hits += 1;
            }
        }
    }
    let control_p = (1.0 + hits as f64) / (1.0 + valid as f64);
    Ok(NegativeControlReport {
        control_effect: observed,
        control_p,
        flags_residual_confounding: control_p < 0.05,
    })
}
