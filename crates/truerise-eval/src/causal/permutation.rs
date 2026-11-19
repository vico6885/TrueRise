//! Randomization inference (CA4): significance for small agent studies.
//!
//! Agent-run comparisons are small (tens of runs), so asymptotic tests are
//! dishonest machinery. Randomization inference needs NO distributional
//! assumption: the p-value is computed over the assignment distribution
//! itself - "if the arms were labels drawn at random, how often would a
//! contrast at least this large appear?".
//!
//! Two tests, both seeded and reproducible:
//! - [`two_sample`]: unpaired group contrast, exact when the combination
//!   count fits the compute contract, Monte-Carlo otherwise.
//! - [`paired_flip`]: paired sign-flip test for within-subject designs
//!   (crossover pairs).
//!
//! The Monte-Carlo branch uses the add-one estimator
//! `(1 + hits) / (1 + draws)` so the observed arrangement is always counted
//! and a p-value of exactly zero is impossible (a zero p-value is a lie
//! about a finite sample).

/// Compute contract: exact enumeration is used while the number of distinct
/// rearrangements stays at or under this bound; above it the test falls to
/// seeded Monte-Carlo and says so in the outcome.
pub const MAX_EXACT_ARRANGEMENTS: u64 = 200_000;

/// Test-refusal reasons.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PermError {
    /// Both groups (or the paired series) must be non-empty.
    #[error("permutation test refused: {0}")]
    TooFewUnits(&'static str),
    /// Non-finite observations would poison every statistic downstream.
    #[error("permutation test refused: non-finite value at index {index}")]
    NotFinite {
        /// Position of the offending value.
        index: usize,
    },
    /// Finite inputs can still produce an infinite AGGREGATE (a subset sum
    /// over 1e308-scale values); a p-value built on inf comparisons is a
    /// lie, so the test refuses instead (review P1-1).
    #[error("permutation test refused: aggregate statistic overflowed to non-finite")]
    Overflow,
}

/// The outcome of one randomization-inference test.
#[derive(Debug, Clone, PartialEq)]
pub struct PermOutcome {
    /// The observed contrast (mean difference; signed for two-sample).
    pub statistic: f64,
    /// Two-sided assignment-space p-value (never below one arrangement).
    pub p_value: f64,
    /// True when the p-value came from exact enumeration, false for
    /// seeded Monte-Carlo.
    pub exact: bool,
    /// Arrangements evaluated (exact: the full combination count; MC: the
    /// draw count).
    pub draws: u64,
}

fn validate(values: &[f64], what: &'static str) -> Result<(), PermError> {
    if values.is_empty() {
        return Err(PermError::TooFewUnits(what));
    }
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(PermError::NotFinite { index: i });
        }
    }
    Ok(())
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Number of k-subsets of an n-set, saturated at u64 (the caller only
/// compares against [`MAX_EXACT_ARRANGEMENTS`], so saturation is safe).
fn combinations(n: u64, k: u64) -> u64 {
    let k = k.min(n - k);
    let mut result: u64 = 1;
    for i in 0..k {
        result = result.saturating_mul(n - i).saturating_div(i + 1);
    }
    result
}

/// Two-sample randomization test on the mean difference.
///
/// Exact branch: every way of splitting the pooled values into groups of
/// the observed sizes is enumerated (combinadic order), so the observed
/// arrangement is always part of the reference set.
pub fn two_sample(x: &[f64], y: &[f64], seed: u64) -> Result<PermOutcome, PermError> {
    validate(x, "group x")?;
    validate(y, "group y")?;
    let (m, n) = (x.len(), y.len());
    let mut pool: Vec<f64> = Vec::with_capacity(m + n);
    pool.extend_from_slice(x);
    pool.extend_from_slice(y);
    let total = (m + n) as u64;
    let arrangements = combinations(total, m as u64);

    if arrangements <= MAX_EXACT_ARRANGEMENTS {
        // The observed statistic MUST come from the same arithmetic path as
        // the enumerated ones (pool sums minus x-sum), so the observed
        // arrangement always counts as its own reference point - a one-ulp
        // discrepancy here would silently drop it from the tail.
        let mut hits: u64 = 0;
        let total_sum: f64 = pool.iter().sum();
        let x_sum_obs: f64 = pool[..m].iter().sum();
        let observed = (total_sum - x_sum_obs) / n as f64 - x_sum_obs / m as f64;
        if !observed.is_finite() {
            return Err(PermError::Overflow);
        }
        // Combinadic subset iteration: each k-subset indexes which pooled
        // slots belong to group x, in lexicographic order.
        let mut subset: Vec<usize> = (0..m).collect();
        loop {
            let x_sum: f64 = subset.iter().map(|&i| pool[i]).sum();
            let y_sum = total_sum - x_sum;
            let stat = y_sum / n as f64 - x_sum / m as f64;
            if !stat.is_finite() {
                return Err(PermError::Overflow);
            }
            if stat.abs() >= observed.abs() {
                hits += 1;
            }
            // Advance to the next k-subset lexicographically.
            let mut done = true;
            for idx in (0..m).rev() {
                if subset[idx] < (m + n) - (m - idx) {
                    subset[idx] += 1;
                    for nxt in (idx + 1)..m {
                        subset[nxt] = subset[nxt - 1] + 1;
                    }
                    done = false;
                    break;
                }
            }
            if done {
                break;
            }
        }
        return Ok(PermOutcome {
            statistic: observed,
            p_value: hits as f64 / arrangements as f64,
            exact: true,
            draws: arrangements,
        });
    }

    let observed = mean(y) - mean(x);
    if !observed.is_finite() {
        return Err(PermError::Overflow);
    }

    // Monte-Carlo branch: permute the pool, keep the group sizes.
    let draws: u64 = 10_000;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0x5EED_5EED);
    let mut hits: u64 = 0;
    for _ in 0..draws {
        rng.shuffle(&mut pool);
        let x_mean = mean(&pool[..m]);
        let y_mean = mean(&pool[m..]);
        let stat = y_mean - x_mean;
        if !stat.is_finite() {
            return Err(PermError::Overflow);
        }
        if stat.abs() >= observed.abs() {
            hits += 1;
        }
    }
    Ok(PermOutcome {
        statistic: observed,
        p_value: (1.0 + hits as f64) / (1.0 + draws as f64),
        exact: false,
        draws,
    })
}

/// Paired sign-flip test on within-subject differences (crossover pairs).
///
/// The reference set is the 2^n sign assignments of the differences; the
/// statistic is the mean. Exact while `2^n` fits the compute contract.
pub fn paired_flip(diffs: &[f64], seed: u64) -> Result<PermOutcome, PermError> {
    validate(diffs, "paired differences")?;
    let observed = mean(diffs);
    if !observed.is_finite() {
        return Err(PermError::Overflow);
    }
    let n = diffs.len() as u64;
    let arrangements = if n >= 63 { u64::MAX } else { 1u64 << n };
    if arrangements <= MAX_EXACT_ARRANGEMENTS {
        let mut hits: u64 = 0;
        for mask in 0..(1u64 << n) {
            let mut total = 0.0;
            for (i, &d) in diffs.iter().enumerate() {
                let sign = if (mask >> i) & 1 == 1 { -1.0 } else { 1.0 };
                total += sign * d;
            }
            let stat = total / diffs.len() as f64;
            if !stat.is_finite() {
                return Err(PermError::Overflow);
            }
            if stat.abs() >= observed.abs() {
                hits += 1;
            }
        }
        return Ok(PermOutcome {
            statistic: observed,
            p_value: hits as f64 / arrangements as f64,
            exact: true,
            draws: arrangements,
        });
    }
    // Monte-Carlo: seeded random sign patterns (the all-actual pattern is
    // counted by the add-one estimator).
    let draws: u64 = 10_000;
    let mut rng = super::rng::SplitMix64::new(seed ^ 0xF11F_11F1);
    let mut hits: u64 = 0;
    for _ in 0..draws {
        let mut total = 0.0;
        for &d in diffs {
            let sign = if rng.next_f64() < 0.5 { -1.0 } else { 1.0 };
            total += sign * d;
        }
        let stat = total / diffs.len() as f64;
        if !stat.is_finite() {
            return Err(PermError::Overflow);
        }
        if stat.abs() >= observed.abs() {
            hits += 1;
        }
    }
    Ok(PermOutcome {
        statistic: observed,
        p_value: (1.0 + hits as f64) / (1.0 + draws as f64),
        exact: false,
        draws,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn exact_permutation_separates_distributions() {
        // Fully separated groups (means 2 vs 5, observed |diff| = 3): the
        // observed arrangement and its mirror are the only 2 of C(6,3)=20
        // splits with |diff| >= 3.
        let out = two_sample(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], 1).unwrap();
        assert!(out.exact);
        assert_eq!(out.draws, 20);
        assert!((out.p_value - 0.1).abs() < 1e-12, "p was {}", out.p_value);
    }

    /// Review P1-1 regression: per-value finiteness is not enough - finite
    /// inputs at 1e308 scale produce an infinite AGGREGATE, and a p-value
    /// built on inf comparisons must be refused, never reported as 0.0.
    #[test]
    fn overflowing_aggregates_are_refused_not_reported_as_significant() {
        let x = [1e308, 1e308, 1e308];
        let y = [-1e308, -1e308, -1e308];
        let out = two_sample(&x, &y, 1);
        assert!(matches!(out, Err(PermError::Overflow)), "got {out:?}");
        let out = paired_flip(&[1e308, 1e308, -1e308], 1);
        assert!(matches!(out, Err(PermError::Overflow)), "got {out:?}");
    }

    #[test]
    fn identical_groups_give_the_maximal_p_value() {
        let out = two_sample(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0], 1).unwrap();
        assert!(
            (out.p_value - 1.0).abs() < 1e-12,
            "no split can beat a zero contrast"
        );
    }

    #[test]
    fn paired_sign_flip_rejects_all_positive_differences() {
        // All three pairs favour one arm: only the actual pattern and the
        // full flip reach the observed |mean|; p = 2/8.
        let out = paired_flip(&[1.0, 2.0, 3.0], 1).unwrap();
        assert!(out.exact);
        assert!((out.p_value - 0.25).abs() < 1e-12, "p was {}", out.p_value);
    }

    #[test]
    fn symmetric_differences_cannot_be_significant() {
        let out = paired_flip(&[1.0, -1.0, 1.0, -1.0], 1).unwrap();
        assert!((out.p_value - 1.0).abs() < 1e-12);
    }

    #[test]
    fn monte_carlo_branch_is_seeded_and_counts_the_observed() {
        // 25 vs 25 pushes the combination count past the exact contract
        // (C(50,25) is astronomically above 200k) - the MC branch runs.
        let x: Vec<f64> = (0..25).map(f64::from).collect();
        let y: Vec<f64> = (25..50).map(f64::from).collect();
        let a = two_sample(&x, &y, 123).unwrap();
        let b = two_sample(&x, &y, 123).unwrap();
        assert!(!a.exact);
        assert_eq!(a, b, "same seed must reproduce the MC p-value");
        assert!(
            a.p_value > 0.0,
            "add-one estimator: zero p-values are impossible"
        );
        assert!(
            a.p_value < 0.05,
            "fully separated groups must be significant, p={}",
            a.p_value
        );
    }

    #[test]
    fn refusals_are_typed_not_panics() {
        assert!(matches!(
            two_sample(&[], &[1.0], 1),
            Err(PermError::TooFewUnits(_))
        ));
        let mut bad = vec![1.0, f64::NAN];
        assert!(matches!(
            two_sample(&bad, &[1.0], 1),
            Err(PermError::NotFinite { index: 1 })
        ));
        bad = vec![f64::INFINITY];
        assert!(matches!(
            paired_flip(&bad, 1),
            Err(PermError::NotFinite { index: 0 })
        ));
    }
}
