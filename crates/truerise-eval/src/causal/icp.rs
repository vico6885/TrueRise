//! Invariance screening (CA3): ICP-lite across environments.
//!
//! Invariant Causal Prediction (Peters, Buhlmann & Meinshausen, 2016) is
//! the bridge between drift analysis and causal candidate selection: a
//! predictor whose relationship to the outcome is STABLE across
//! environments is a causal candidate, while a predictor that only rides
//! the environment shift is spurious. TrueRise's environments are exactly
//! its drift dimensions - provider versions, task domains, time slices.
//!
//! This is an LITE implementation, and the boundary is stated in the code:
//! invariance is tested with a Chow-style statistic (pooled fit vs per-
//! environment fits) calibrated by PERMUTATION of environment labels, not
//! by the original paper's GCM test. The output semantics match the paper:
//! the intersection of all accepted predictor sets lower-bounds the true
//! parent set; an EMPTY intersection means no mandatory parent can be
//! identified from these environments - an honest result, not a failure.

/// Predictor-count contract for subset enumeration (2^8 - 1 subsets).
pub const MAX_ICP_PREDICTORS: usize = 8;

/// Refusal reasons for invariance screening.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IcpError {
    /// At least two environments are the whole point.
    #[error("icp refused: {0} environments - invariance needs at least two")]
    TooFewEnvironments(usize),
    /// Predictor columns must be identical in count across environments.
    #[error(
        "icp refused: environment {index} has {columns} predictor columns, expected {expected}"
    )]
    ColumnMismatch {
        /// The offending environment index.
        index: usize,
        /// Its column count.
        columns: usize,
        /// The first environment's column count.
        expected: usize,
    },
    /// A predictor column misaligned with the outcome is a malformed
    /// design, not an invariance verdict (review P2-4).
    #[error(
        "icp refused: environment {index} column {column} has {rows} rows, outcome has {outcome_rows}"
    )]
    LengthMismatch {
        /// The offending environment index.
        index: usize,
        /// The offending predictor column.
        column: usize,
        /// The column's row count.
        rows: usize,
        /// The outcome's row count.
        outcome_rows: usize,
    },
    /// Each environment needs more rows than fitted columns (residual df).
    #[error(
        "icp refused: environment {index} has {rows} rows for {columns} columns - invariance needs residual df"
    )]
    NoResidualDegrees {
        /// The offending environment index.
        index: usize,
        /// Its row count.
        rows: usize,
        /// Its column count.
        columns: usize,
    },
    /// NaN or infinite values poison every fit.
    #[error("icp refused: non-finite value in environment {index}")]
    NotFinite {
        /// The offending environment index.
        index: usize,
    },
    /// A screening with zero predictor columns has no subsets to test.
    #[error("icp refused: no predictor columns - there is nothing to screen")]
    NoPredictors,
    /// The predictor count exceeds the subset-enumeration contract.
    #[error("icp refused: {predictors} predictors exceeds the enumeration contract of {cap}")]
    TooManyPredictors {
        /// Requested predictor count.
        predictors: usize,
        /// The contract value ([`MAX_ICP_PREDICTORS`]).
        cap: usize,
    },
    /// The significance level must be a strict interior probability.
    #[error("icp refused: significance level {0} must lie strictly inside (0, 1)")]
    BadSignificanceLevel(f64),
}

/// One environment: predictor COLUMNS (row-aligned) plus the outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    /// Predictor columns; column j holds that predictor's values per row.
    pub x: Vec<Vec<f64>>,
    /// Outcome values per row.
    pub y: Vec<f64>,
}

/// Screening configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct IcpConfig {
    /// Acceptance floor for the per-subset permutation p-value.
    pub sig_level: f64,
    /// Permutation draws per subset (the label-shuffle calibration).
    pub permutations: u64,
    /// Seed for the label permutation stream.
    pub seed: u64,
}

impl Default for IcpConfig {
    fn default() -> Self {
        Self {
            sig_level: 0.05,
            permutations: 200,
            seed: 2026,
        }
    }
}

/// One subset's screening verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct SubsetVerdict {
    /// The predictor indices in the subset (ascending).
    pub predictors: Vec<usize>,
    /// The observed Chow-style invariance statistic (0 = perfectly stable).
    pub statistic: f64,
    /// Permutation-calibrated p-value (large = stable = accepted).
    pub p_value: f64,
    /// Whether the subset passed the significance floor.
    pub accepted: bool,
}

/// The screening report.
#[derive(Debug, Clone, PartialEq)]
pub struct IcpReport {
    /// Every non-empty subset's verdict, in ascending bitmask order.
    pub subsets: Vec<SubsetVerdict>,
    /// Intersection of ALL accepted predictor sets. Empty means no
    /// mandatory parent could be identified - the data refuse to narrow
    /// the candidates, and that refusal is the honest answer.
    pub intersection: Vec<usize>,
}

fn residual_sum_squares(columns: &[&[f64]], y: &[f64]) -> Option<f64> {
    // Every fit carries an intercept: without it the Chow statistic picks
    // up plain level differences between environments and reports them as
    // coefficient drift - the classic false-positive pump.
    let ones = vec![1.0; y.len()];
    let mut with_intercept: Vec<&[f64]> = vec![&ones];
    with_intercept.extend(columns.iter().copied());
    let beta = super::refute::ols_fit(&with_intercept, y)?;
    let n = y.len();
    let mut rss = 0.0;
    for r in 0..n {
        let mut fitted = 0.0;
        for (c, &b) in beta.iter().enumerate() {
            fitted += b * with_intercept[c][r];
        }
        let residual = y[r] - fitted;
        rss += residual * residual;
    }
    Some(rss)
}

/// Chow-style invariance statistic over the CURRENT environment
/// partition: (pooled RSS - sum of per-environment RSS) / sum, clamped at
/// zero for float noise. Larger = the coefficient structure differs across
/// environments.
fn chow_statistic(rows: &[(Vec<&[f64]>, &[f64])], columns_of: &[usize]) -> Option<f64> {
    // Pooled fit over all rows.
    let mut pooled_cols: Vec<Vec<f64>> = vec![Vec::new(); columns_of.len()];
    let mut pooled_y: Vec<f64> = Vec::new();
    for (cols, y) in rows {
        for (dst, &src) in pooled_cols.iter_mut().zip(columns_of) {
            dst.extend_from_slice(cols[src]);
        }
        pooled_y.extend_from_slice(y);
    }
    let col_refs: Vec<&[f64]> = pooled_cols.iter().map(Vec::as_slice).collect();
    let rss_pooled = residual_sum_squares(&col_refs, &pooled_y)?;
    let mut rss_env_total = 0.0;
    for (cols, y) in rows {
        let env_cols: Vec<&[f64]> = columns_of.iter().map(|&c| cols[c]).collect();
        let rss = residual_sum_squares(&env_cols, y)?;
        rss_env_total += rss;
    }
    // Review P1-2: an overflowing RSS pair turns the difference into NaN,
    // and `f64::max(NaN, 0.0)` = 0.0 would read as PERFECT stability -
    // the exact lie this statistic must never tell. Non-finite RSS makes
    // the subset numerically unusable: refuse it (the caller treats None
    // as rejection).
    if !rss_pooled.is_finite() || !rss_env_total.is_finite() {
        return None;
    }
    let diff = rss_pooled - rss_env_total;
    let denominator = rss_env_total.max(1e-12);
    Some((diff.max(0.0)) / denominator)
}

/// Screen every non-empty predictor subset for cross-environment
/// invariance.
pub fn icp_lite(environments: &[Environment], config: &IcpConfig) -> Result<IcpReport, IcpError> {
    if environments.len() < 2 {
        return Err(IcpError::TooFewEnvironments(environments.len()));
    }
    if !(0.0..1.0).contains(&config.sig_level) || config.sig_level <= 0.0 {
        return Err(IcpError::BadSignificanceLevel(config.sig_level));
    }
    let p = environments[0].x.len();
    if p == 0 {
        return Err(IcpError::NoPredictors);
    }
    if p > MAX_ICP_PREDICTORS {
        return Err(IcpError::TooManyPredictors {
            predictors: p,
            cap: MAX_ICP_PREDICTORS,
        });
    }
    for (i, env) in environments.iter().enumerate() {
        if env.x.len() != p {
            return Err(IcpError::ColumnMismatch {
                index: i,
                columns: env.x.len(),
                expected: p,
            });
        }
        // Review P2-4: a predictor column misaligned with the outcome used
        // to surface as a stream of "singular -> rejected" verdicts; it is
        // a malformed DESIGN, and gets a typed refusal.
        for (c, col) in env.x.iter().enumerate() {
            if col.len() != env.y.len() {
                return Err(IcpError::LengthMismatch {
                    index: i,
                    column: c,
                    rows: col.len(),
                    outcome_rows: env.y.len(),
                });
            }
        }
        if env.y.len() <= p + 1 {
            // One df is consumed by the intercept (see residual_sum_squares).
            return Err(IcpError::NoResidualDegrees {
                index: i,
                rows: env.y.len(),
                columns: p,
            });
        }
        if env.y.iter().any(|v| !v.is_finite())
            || env.x.iter().any(|col| col.iter().any(|v| !v.is_finite()))
        {
            return Err(IcpError::NotFinite { index: i });
        }
    }
    // Precompute per-environment column views: (columns, outcome) pairs.
    let rows: Vec<(Vec<&[f64]>, &[f64])> = environments
        .iter()
        .map(|env| (env.x.iter().map(Vec::as_slice).collect(), env.y.as_slice()))
        .collect();
    // Pooled rows for label permutation: concatenated values per column.
    let mut pooled_cols: Vec<Vec<f64>> = vec![Vec::new(); p];
    let mut pooled_y: Vec<f64> = Vec::new();
    for env in environments {
        for (dst, src) in pooled_cols.iter_mut().zip(env.x.iter()) {
            dst.extend_from_slice(src);
        }
        pooled_y.extend_from_slice(&env.y);
    }
    let env_sizes: Vec<usize> = environments.iter().map(|e| e.y.len()).collect();

    let mut rng = super::rng::SplitMix64::new(config.seed ^ 0x1C9_1CE);
    let mut subsets = Vec::new();
    let mut intersection: Option<u64> = None;
    for mask in 1u64..(1u64 << p) {
        let columns_of: Vec<usize> = (0..p).filter(|c| mask & (1u64 << c) != 0).collect();
        let observed = chow_statistic(&rows, &columns_of);
        let Some(observed) = observed else {
            // A singular fit cannot defend invariance - refuse the subset.
            // A rejected subset does NOT participate in the intersection:
            // the ICP output is the intersection over ACCEPTED sets only.
            subsets.push(SubsetVerdict {
                predictors: columns_of.clone(),
                statistic: f64::INFINITY,
                p_value: 0.0,
                accepted: false,
            });
            continue;
        };
        // Label permutation: reassign pooled rows to environments of the
        // ORIGINAL sizes; recompute the statistic under each reshuffle.
        let draws = config.permutations;
        let mut order: Vec<usize> = (0..pooled_y.len()).collect();
        let mut hits: u64 = 0;
        let mut valid_draws: u64 = 0;
        let total_rows = pooled_y.len();
        for _ in 0..draws {
            rng.shuffle(&mut order);
            // Gather the pooled values THROUGH the shuffled order first -
            // slicing the unshuffled pools would make every "permutation"
            // the original partition and every p-value exactly 1.0.
            let mut perm_cols: Vec<Vec<f64>> = vec![Vec::with_capacity(total_rows); p];
            let mut perm_y: Vec<f64> = Vec::with_capacity(total_rows);
            for &src in &order {
                for (dst, src_col) in perm_cols.iter_mut().zip(pooled_cols.iter()) {
                    dst.push(src_col[src]);
                }
                perm_y.push(pooled_y[src]);
            }
            let mut perm_rows: Vec<(Vec<Vec<f64>>, Vec<f64>)> = Vec::with_capacity(env_sizes.len());
            let mut cursor = 0usize;
            for &size in &env_sizes {
                let env_x: Vec<Vec<f64>> = perm_cols
                    .iter()
                    .map(|col| col[cursor..cursor + size].to_vec())
                    .collect();
                let env_y = perm_y[cursor..cursor + size].to_vec();
                perm_rows.push((env_x, env_y));
                cursor += size;
            }
            let perm_views: Vec<(Vec<&[f64]>, &[f64])> = perm_rows
                .iter()
                .map(|(x, y)| (x.iter().map(Vec::as_slice).collect(), y.as_slice()))
                .collect();
            if let Some(stat) = chow_statistic(&perm_views, &columns_of) {
                valid_draws += 1;
                if stat >= observed {
                    hits += 1;
                }
            }
        }
        if valid_draws < draws / 2 {
            // More than half the permuted refits were singular: the subset
            // is numerically unusable, refuse it rather than guess.
            subsets.push(SubsetVerdict {
                predictors: columns_of.clone(),
                statistic: observed,
                p_value: 0.0,
                accepted: false,
            });
            continue;
        }
        let p_value = (1.0 + hits as f64) / (1.0 + valid_draws as f64);
        let accepted = p_value >= config.sig_level;
        if accepted {
            // AND into the running intersection - rejected subsets never
            // touch it (ICP semantics: intersection over accepted sets).
            intersection = Some(intersection.map_or(mask, |acc| acc & mask));
        }
        subsets.push(SubsetVerdict {
            predictors: columns_of,
            statistic: observed,
            p_value,
            accepted,
        });
    }
    let intersection = intersection.unwrap_or(0);
    Ok(IcpReport {
        subsets,
        intersection: (0..p).filter(|c| intersection & (1u64 << c) != 0).collect(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Environment builder: predictor 0 is the causal driver (coefficient
    /// `driver`), predictor 1 the spurious rider (coefficient `rider`),
    /// outcome noise seeded from `seed`. Features are drawn CENTERED
    /// (canonical ICP simulation setup): an uncentered rider shifts the
    /// conditional mean of y given x1 across environments, which is a
    /// genuine invariance violation - ICP would then correctly reject the
    /// driver-only model too.
    fn make_env(rows: usize, rider: f64, seed: u64) -> Environment {
        let mut rng = super::super::rng::SplitMix64::new(seed);
        let mut x1 = Vec::with_capacity(rows);
        let mut x2 = Vec::with_capacity(rows);
        let mut y = Vec::with_capacity(rows);
        for _ in 0..rows {
            let v1 = rng.next_f64() - 0.5;
            let v2 = rng.next_f64() - 0.5;
            x1.push(v1);
            x2.push(v2);
            y.push(2.0 * v1 + rider * v2 + (rng.next_f64() - 0.5) * 0.1);
        }
        Environment { x: vec![x1, x2], y }
    }

    #[test]
    fn icp_lite_finds_invariant_feature_not_spurious_one() {
        // Environment 0: x2 rides along (coefficient 3). Environment 1:
        // x2's association is GONE (coefficient 0) - exactly the spurious
        // fingerprint. x1 keeps coefficient 2 in both.
        let envs = vec![make_env(140, 3.0, 11), make_env(140, 0.0, 12)];
        let report = icp_lite(
            &envs,
            &IcpConfig {
                permutations: 60,
                ..IcpConfig::default()
            },
        )
        .unwrap();
        assert_eq!(report.subsets.len(), 3);
        let accepted: Vec<&SubsetVerdict> = report.subsets.iter().filter(|s| s.accepted).collect();
        assert_eq!(
            report.intersection,
            vec![0usize],
            "the causal driver must be the only mandatory parent; accepted: {accepted:?}"
        );
    }

    #[test]
    fn icp_lite_reports_no_mandatory_parent_when_everything_is_stable() {
        // Both predictors stable across environments: every subset passes,
        // and the intersection of ALL accepted sets is legitimately empty.
        let envs = vec![make_env(120, 3.0, 21), make_env(120, 3.0, 22)];
        let report = icp_lite(
            &envs,
            &IcpConfig {
                permutations: 60,
                ..IcpConfig::default()
            },
        )
        .unwrap();
        assert!(
            report.subsets.iter().all(|s| s.accepted),
            "stable data must accept all subsets"
        );
        assert!(
            report.intersection.is_empty(),
            "no mandatory parent is the honest answer when all subsets are accepted"
        );
    }

    #[test]
    fn screening_is_seeded_and_reproducible() {
        let envs = vec![make_env(100, 3.0, 31), make_env(100, 0.0, 32)];
        let config = IcpConfig {
            permutations: 40,
            ..IcpConfig::default()
        };
        let a = icp_lite(&envs, &config).unwrap();
        let b = icp_lite(&envs, &config).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn malformed_screening_requests_are_refused() {
        let env = make_env(30, 1.0, 41);
        // One environment only.
        assert!(matches!(
            icp_lite(std::slice::from_ref(&env), &IcpConfig::default()),
            Err(IcpError::TooFewEnvironments(1))
        ));
        // Column mismatch.
        let mut broken = env.clone();
        broken.x.pop();
        assert!(matches!(
            icp_lite(&[env.clone(), broken], &IcpConfig::default()),
            Err(IcpError::ColumnMismatch { .. })
        ));
        // No residual df: 3 rows for 2 columns (+1 intercept), columns
        // truncated in lockstep so the length check passes first.
        let mut starved = env.clone();
        starved.y.truncate(3);
        for col in starved.x.iter_mut() {
            col.truncate(3);
        }
        assert!(matches!(
            icp_lite(&[env.clone(), starved], &IcpConfig::default()),
            Err(IcpError::NoResidualDegrees { .. })
        ));
        // NaN.
        let mut dirty = env.clone();
        dirty.y[0] = f64::NAN;
        assert!(matches!(
            icp_lite(&[env.clone(), dirty], &IcpConfig::default()),
            Err(IcpError::NotFinite { .. })
        ));
        // Bad level.
        assert!(matches!(
            icp_lite(
                &[env.clone(), env.clone()],
                &IcpConfig {
                    sig_level: 1.5,
                    ..IcpConfig::default()
                }
            ),
            Err(IcpError::BadSignificanceLevel(_))
        ));
        // Review P2-4: a predictor column misaligned with its outcome is a
        // typed design refusal, not a stream of fake "not invariant" verdicts.
        let mut misaligned = env.clone();
        misaligned.x[0].pop();
        assert!(matches!(
            icp_lite(&[env.clone(), misaligned], &IcpConfig::default()),
            Err(IcpError::LengthMismatch { .. })
        ));
    }

    /// Review P1-2 regression: finite inputs at 1e200 scale overflow the
    /// RSS aggregate; `f64::max(NaN, 0.0)` used to read that as PERFECT
    /// stability (p = 1.0, accepted). Non-finite RSS must instead mark the
    /// subset numerically unusable (statistic = inf, rejected).
    #[test]
    fn overflowing_rss_refuses_the_subset_instead_of_praising_stability() {
        // Heterogeneous slopes at 1e200 outcome scale: every RSS overflows.
        let build = |rider: f64, seed: u64| {
            let mut rng = super::super::rng::SplitMix64::new(seed);
            let mut env = make_env(60, 0.0, seed);
            for r in 0..env.y.len() {
                let v1 = rng.next_f64() - 0.5;
                let v2 = rng.next_f64() - 0.5;
                env.x[0][r] = v1;
                env.x[1][r] = v2;
                env.y[r] = (2.0 * v1 + rider * v2) * 1e200;
            }
            env
        };
        let envs = vec![build(1.0, 51), build(4.0, 52)];
        let report = icp_lite(
            &envs,
            &IcpConfig {
                permutations: 20,
                ..IcpConfig::default()
            },
        )
        .unwrap();
        for s in &report.subsets {
            assert_eq!(
                s.statistic,
                f64::INFINITY,
                "subset {:?} survived an overflowing RSS as statistic {}",
                s.predictors,
                s.statistic
            );
            assert!(!s.accepted, "overflowing subset must never be accepted");
        }
    }
}
