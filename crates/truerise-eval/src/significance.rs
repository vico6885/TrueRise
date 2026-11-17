//! Significance (W9): "the score went up" is upgraded to "the improvement
//! is statistically defensible" - or the claim is REFUSED. Exact McNemar
//! over PAIRED per-trial outcomes; the p-value is an exact integer
//! fraction (k / 2^n), so the whole test is float-free and the
//! significance decision is an INTEGER comparison (20k <= 2^n for p<=0.05).
//!
//! Hard preconditions (refusal, not a warning):
//! - equal-cost: both arms must have the SAME trial count AND the SAME
//!   budget units (comparing across unequal costs is exactly the
//!   best-of-N cheat this project exists to catch);
//! - discordant floor: fewer than [`DISCORDANT_FLOOR`] discordant pairs
//!   VOIDS the claim (one lucky task is not an improvement).

/// Minimum discordant pairs for a defensible McNemar claim.
pub const DISCORDANT_FLOOR: u32 = 10;

/// One arm of a paired comparison (per-trial outcomes + what it cost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmResult {
    /// Per-trial outcomes, PAIRED by index with the other arm.
    pub outcomes: Vec<bool>,
    /// The budget units the arm consumed (the equal-cost precondition).
    pub budget_units: u64,
}

/// Comparison errors. Every variant is a refusal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompareError {
    /// The two arms cost different amounts - the comparison is refused.
    #[error(
        "comparison refused: unequal cost (trials {a_trials}/{b_trials}, budget {a_budget}/{b_budget})"
    )]
    UnequalCost {
        /// Arm A trials.
        a_trials: u32,
        /// Arm B trials.
        b_trials: u32,
        /// Arm A budget units.
        a_budget: u64,
        /// Arm B budget units.
        b_budget: u64,
    },
    /// The two arms have different trial counts (cannot pair).
    #[error("comparison refused: outcomes cannot pair ({a} vs {b})")]
    Unpairable {
        /// Arm A outcome count.
        a: usize,
        /// Arm B outcome count.
        b: usize,
    },
}

/// The verdict over one comparison. `Voided` is an HONEST outcome: the data
/// cannot support ANY claim, in either direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Significance {
    /// The improvement is defensible at p <= 0.05 (two-sided exact McNemar).
    Significant {
        /// True when arm A is the better one.
        a_better: bool,
        /// Discordant pairs (b + c).
        discordant: u32,
        /// Exact p numerator: k (p = k / 2^n).
        p_numerator: u128,
        /// Exact p exponent: n (denominator = 2^n).
        p_pow2: u32,
    },
    /// No defensible difference (p > 0.05).
    NotSignificant {
        /// Discordant pairs (b + c).
        discordant: u32,
        /// Exact p numerator: k (p = k / 2^n).
        p_numerator: u128,
        /// Exact p exponent: n (denominator = 2^n).
        p_pow2: u32,
    },
    /// The data voids the claim: too few discordant pairs.
    Voided {
        /// Discordant pairs observed.
        discordant: u32,
        /// The floor that was not met.
        floor: u32,
    },
}

/// The exact two-sided McNemar verdict over paired outcomes.
///
/// # Errors
/// [`CompareError`] - unequal cost or unpairable arms (refusal, never a
/// downgrade).
pub fn mcnemar_exact(a: &ArmResult, b: &ArmResult) -> Result<Significance, CompareError> {
    // Equal-cost precondition FIRST (trials AND budget).
    if a.outcomes.len() != b.outcomes.len() {
        return Err(CompareError::Unpairable {
            a: a.outcomes.len(),
            b: b.outcomes.len(),
        });
    }
    // Equal-cost precondition: the two ARMS must have spent the SAME budget
    // (a best-of-N arm that paid for more trials is exactly the hidden-
    // denominator cheat the bench exists to catch).
    if a.budget_units != b.budget_units {
        return Err(CompareError::UnequalCost {
            a_trials: a.outcomes.len() as u32,
            b_trials: b.outcomes.len() as u32,
            a_budget: a.budget_units,
            b_budget: b.budget_units,
        });
    }
    // Discordant pairs: A pass/B fail (b) and A fail/B pass (c).
    let mut b_count: u32 = 0;
    let mut c_count: u32 = 0;
    for (pa, pb) in a.outcomes.iter().zip(b.outcomes.iter()) {
        match (*pa, *pb) {
            (true, false) => b_count += 1,
            (false, true) => c_count += 1,
            _ => {}
        }
    }
    let discordant = b_count + c_count;
    if discordant < DISCORDANT_FLOOR {
        return Ok(Significance::Voided {
            discordant,
            floor: DISCORDANT_FLOOR,
        });
    }
    // Exact two-sided binomial: p = 2 * sum_{i<=min} C(n,i) / 2^n, clamped
    // to 1. All in integers: k = 2 * sum_{i<=min} C(n,i); significant iff
    // k / 2^n <= 1/20  <=>  20k <= 2^n.
    let smaller = b_count.min(c_count);
    let n = discordant;
    // Review fix (P1): u128 arithmetic overflows at large discordant counts
    // (debug: panic; release: silent wrap -> arbitrary verdict). Guard: for
    // n >= 128, 2^n does not fit u128, so p can never be <= 1/20 - the
    // honest verdict is NotSignificant, computed BEFORE any multiply.
    if n >= 128 {
        return Ok(Significance::NotSignificant {
            discordant: n,
            p_numerator: u128::MAX,
            p_pow2: n,
        });
    }
    let k = 2u128.saturating_mul(sum_binomial_upto(n, smaller));
    let two_pow_n = 1u128 << n;
    if u128::from(20u32).checked_mul(k).map(|lhs| lhs <= two_pow_n) == Some(true) {
        Ok(Significance::Significant {
            a_better: b_count > c_count,
            discordant,
            p_numerator: k,
            p_pow2: n,
        })
    } else {
        Ok(Significance::NotSignificant {
            discordant,
            p_numerator: k,
            p_pow2: n,
        })
    }
}

/// sum_{i=0..=m} C(n, i) in exact integer arithmetic (Pascal's rule);
/// saturates at u128::MAX when the recursion would overflow (the caller's
/// n>=128 guard makes that path unreachable in practice, checked anyway).
fn sum_binomial_upto(n: u32, m: u32) -> u128 {
    let mut row: u128 = 1; // C(n, 0)
    let mut total: u128 = 1;
    for i in 0..m {
        // Review fix (P1): checked arithmetic - saturation is the caller's
        // overflow signal, never a debug panic or a release wrap.
        let next = row
            .checked_mul(u128::from(n - i))
            .and_then(|v| v.checked_div(u128::from(i + 1)));
        let Some(next) = next else {
            return u128::MAX;
        };
        row = next;
        total = total.saturating_add(row);
    }
    total
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn arm(outcomes: &[bool]) -> ArmResult {
        ArmResult {
            outcomes: outcomes.to_vec(),
            budget_units: outcomes.len() as u64,
        }
    }

    #[test]
    fn one_task_improvement_is_not_significant() {
        // 100 vs 100 trials; exactly ONE discordant pair (the "improvement").
        let mut a = vec![true; 100];
        let b = vec![true; 100];
        a[0] = false; // A fails where B passes -> B looks better
        let verdict = mcnemar_exact(&arm(&a), &arm(&b)).unwrap();
        assert!(
            !matches!(verdict, Significance::Significant { .. }),
            "one lucky task must never announce an improvement: {verdict:?}"
        );
        // With a single discordant pair the claim is not even testable -
        // the floor VOIDS it outright:
        assert!(matches!(
            verdict,
            Significance::Voided {
                discordant: 1,
                floor: 10
            }
        ));
    }

    #[test]
    fn unequal_budget_comparison_is_refused() {
        let a = ArmResult {
            outcomes: vec![true; 50],
            budget_units: 50,
        };
        let b = ArmResult {
            outcomes: vec![true; 50],
            budget_units: 200, // the SAME trials, THREE times the budget
        };
        let err = mcnemar_exact(&a, &b).unwrap_err();
        assert!(matches!(err, CompareError::UnequalCost { .. }));
        // Different trial counts are refused as unpairable:
        let b2 = arm(&[true; 40]);
        assert!(matches!(
            mcnemar_exact(&a, &b2),
            Err(CompareError::Unpairable { .. })
        ));
    }

    #[test]
    fn discordant_pair_count_below_floor_voids_claim() {
        // 4 discordant pairs, all one-directional: LOOKS like an improvement,
        // but the floor voids the claim (the judge-saturation failure mode).
        let a: Vec<bool> = [vec![true; 96], vec![false; 4]].concat();
        let b: Vec<bool> = [vec![true; 96], vec![true; 4]].concat();
        let verdict = mcnemar_exact(&arm(&a), &arm(&b)).unwrap();
        assert!(matches!(
            verdict,
            Significance::Voided {
                discordant: 4,
                floor: 10
            }
        ));
    }

    #[test]
    fn defensible_improvement_is_significant_and_antisymmetric() {
        // 30 discordant pairs, all one-directional (plus 70 agreements).
        let a: Vec<bool> = [vec![true; 70], vec![false; 30]].concat();
        let b: Vec<bool> = [vec![true; 100]].concat();
        let verdict = mcnemar_exact(&arm(&a), &arm(&b)).unwrap();
        assert!(matches!(
            verdict,
            Significance::Significant {
                a_better: false,
                discordant: 30,
                ..
            }
        ));
        // Swapping the arms FLIPS the direction and keeps the statistic:
        let swapped = mcnemar_exact(&arm(&b), &arm(&a)).unwrap();
        assert!(matches!(
            swapped,
            Significance::Significant {
                a_better: true,
                discordant: 30,
                ..
            }
        ));
        // Same p (same numerator/exponent), mirrored direction - the
        // statistic is antisymmetric under label swap.
        if let (
            Significance::Significant {
                p_numerator: k1,
                p_pow2: n1,
                ..
            },
            Significance::Significant {
                p_numerator: k2,
                p_pow2: n2,
                ..
            },
        ) = (&verdict, &swapped)
        {
            assert_eq!(k1, k2);
            assert_eq!(n1, n2);
        }
    }

    #[test]
    fn property_swap_is_antisymmetric_across_seeds() {
        // Deterministic pseudo-random pairs: swapping labels flips b/c and
        // keeps |b-c| decisions consistent.
        for seed in 0..20u64 {
            let n = 12 + (seed as usize % 9);
            let mut a = Vec::with_capacity(n);
            let mut b = Vec::with_capacity(n);
            for i in 0..n {
                let bit = (seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add((i as u64).wrapping_mul(1442695040888963407))
                    >> 33)
                    & 3;
                a.push(bit != 0);
                b.push(bit < 2);
            }
            let v1 = mcnemar_exact(&arm(&a), &arm(&b)).unwrap();
            let v2 = mcnemar_exact(&arm(&b), &arm(&a)).unwrap();
            assert!(
                eq_mirror(&v1, &v2),
                "label swap must mirror: {v1:?} vs {v2:?}"
            );
        }
    }

    fn eq_mirror(x: &Significance, y: &Significance) -> bool {
        match (x, y) {
            (Significance::Voided { .. }, Significance::Voided { .. }) => true,
            (
                Significance::Significant {
                    a_better: d1,
                    discordant: n1,
                    p_numerator: k1,
                    p_pow2: e1,
                },
                Significance::Significant {
                    a_better: d2,
                    discordant: n2,
                    p_numerator: k2,
                    p_pow2: e2,
                },
            ) => d1 != d2 && n1 == n2 && k1 == k2 && e1 == e2,
            (
                Significance::NotSignificant {
                    discordant: n1,
                    p_numerator: k1,
                    p_pow2: e1,
                },
                Significance::NotSignificant {
                    discordant: n2,
                    p_numerator: k2,
                    p_pow2: e2,
                },
            ) => n1 == n2 && k1 == k2 && e1 == e2,
            _ => false,
        }
    }
}
