//! Interventional credit assignment (CA4): Shapley values over agent
//! components.
//!
//! "Which component carried the result?" is a distribution question, and
//! Shapley values are the unique answer satisfying efficiency, symmetry,
//! dummy and additivity. This module evaluates them over an INTERVENTIONAL
//! value function (a component switched off is a true do()-style
//! intervention: rerun without it), so a positive value here is component
//! evidence - never a standalone causal proof, because the value function
//! itself carries the identification burden.
//!
//! Two evaluators with an explicit compute contract:
//! - [`shapley_exact`]: subset dynamic programming, every weight computed
//!   exactly; refused above [`MAX_EXACT_PLAYERS`].
//! - [`shapley_sampled`]: seeded permutation sampling with standard
//!   errors, for games beyond the exact contract.

/// Exact evaluation is refused above this player count (2^14 subsets is
/// the compute contract; refusing beats pretending).
pub const MAX_EXACT_PLAYERS: usize = 14;

/// Sampling cap: a permutation vector of 64 players covers every
/// realistic component study and bounds the work per draw.
pub const MAX_SAMPLED_PLAYERS: usize = 64;

/// Refusal reasons for credit assignment.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShapleyError {
    /// Zero players have nothing to assign.
    #[error("shapley refused: at least one player is required, got {0}")]
    NoPlayers(usize),
    /// The exact contract caps the subset enumeration.
    #[error(
        "shapley refused: {players} players exceeds the exact contract of {cap} - use the sampled evaluator"
    )]
    ExactBoundExceeded {
        /// Requested player count.
        players: usize,
        /// The contract value ([`MAX_EXACT_PLAYERS`]).
        cap: usize,
    },
    /// The sampled evaluator's per-draw cost cap.
    #[error("shapley refused: {players} players exceeds the sampled contract of {cap}")]
    SampledBoundExceeded {
        /// Requested player count.
        players: usize,
        /// The contract value ([`MAX_SAMPLED_PLAYERS`]).
        cap: usize,
    },
    /// A zero-draw estimate is noise wearing a number.
    #[error("shapley refused: the sampled evaluator needs at least one draw")]
    ZeroDraws,
    /// A value function that answers with NaN poisons every downstream
    /// average - refused at first sight.
    #[error("shapley refused: value function returned a non-finite result for subset {subset}")]
    NonFiniteValue {
        /// The queried subset as a player bitmask.
        subset: u64,
    },
}

/// A value function over subsets: `subset[i]` is true when player `i` is
/// ACTIVE. The empty subset and the grand coalition must both be
/// well-defined (they are queried directly).
pub type ValueFn<'a> = &'a dyn Fn(&[bool]) -> f64;

fn eval(value: ValueFn<'_>, subset: u64, players: usize) -> Result<f64, ShapleyError> {
    let active: Vec<bool> = (0..players).map(|i| subset & (1u64 << i) != 0).collect();
    let v = value(&active);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(ShapleyError::NonFiniteValue { subset })
    }
}

fn factorial(n: usize) -> f64 {
    (1..=n).map(|i| i as f64).product()
}

/// Exact Shapley values via subset dynamic programming.
///
/// For each player `i`:
/// `phi_i = sum over S in subsets(N\{i}) of |S|! (n-1-|S|)! / n! * (v(S+i) - v(S))`.
/// Every subset's value is computed once and cached.
pub fn shapley_exact(value: ValueFn<'_>, players: usize) -> Result<Vec<f64>, ShapleyError> {
    if players == 0 {
        return Err(ShapleyError::NoPlayers(0));
    }
    if players > MAX_EXACT_PLAYERS {
        return Err(ShapleyError::ExactBoundExceeded {
            players,
            cap: MAX_EXACT_PLAYERS,
        });
    }
    let n_fact = factorial(players);
    let mut cache: Vec<Option<f64>> = vec![None; 1usize << players];
    let subset_value = |subset: usize, cache: &mut Vec<Option<f64>>| -> Result<f64, ShapleyError> {
        if let Some(v) = cache[subset] {
            return Ok(v);
        }
        let v = eval(value, subset as u64, players)?;
        cache[subset] = Some(v);
        Ok(v)
    };
    let mut phi = vec![0.0; players];
    for (i, phi_i) in phi.iter_mut().enumerate() {
        let others: Vec<usize> = (0..players).filter(|&p| p != i).collect();
        // Enumerate all subsets of `others` as bitmasks over that list.
        for mask in 0..(1usize << others.len()) {
            let mut s = 0usize;
            let mut s_size = 0usize;
            for (bit, &p) in others.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    s |= 1 << p;
                    s_size += 1;
                }
            }
            let with_i = s | (1 << i);
            let marginal = subset_value(with_i, &mut cache)? - subset_value(s, &mut cache)?;
            // Review P2-8: per-subset finiteness does not bound the
            // DIFFERENCE of two finite values - an inf marginal would
            // poison phi silently.
            if !marginal.is_finite() {
                return Err(ShapleyError::NonFiniteValue {
                    subset: with_i as u64,
                });
            }
            let weight = factorial(s_size) * factorial(players - 1 - s_size) / n_fact;
            *phi_i += weight * marginal;
        }
    }
    Ok(phi)
}

/// The sampled evaluator's outcome: per-player estimates with standard
/// errors over the permutation draws.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledShapley {
    /// Per-player value estimates.
    pub values: Vec<f64>,
    /// Per-player standard errors of the mean (same order as `values`).
    pub std_errs: Vec<f64>,
    /// The number of permutation draws behind the estimates.
    pub draws: u64,
}

/// Sampled Shapley values over seeded random permutations (Welford running
/// moments, single pass per draw).
pub fn shapley_sampled(
    value: ValueFn<'_>,
    players: usize,
    draws: u64,
    seed: u64,
) -> Result<SampledShapley, ShapleyError> {
    if players == 0 {
        return Err(ShapleyError::NoPlayers(0));
    }
    if players > MAX_SAMPLED_PLAYERS {
        return Err(ShapleyError::SampledBoundExceeded {
            players,
            cap: MAX_SAMPLED_PLAYERS,
        });
    }
    if draws == 0 {
        return Err(ShapleyError::ZeroDraws);
    }
    let mut rng = super::rng::SplitMix64::new(seed ^ 0x05BA_91E7);
    let mut order: Vec<usize> = (0..players).collect();
    // Welford accumulators. Every player receives EXACTLY ONE marginal
    // contribution per draw, so player p's sample count after draw d is
    // d+1 - the divisor is per-player, never a global counter.
    let mut mean = vec![0.0; players];
    let mut m2 = vec![0.0; players];
    for draw_index in 0..draws {
        rng.shuffle(&mut order);
        let mut active = vec![false; players];
        let mut coalition: u64 = 0;
        let mut prev_v = eval(value, coalition, players)?;
        for &player in &order {
            active[player] = true;
            coalition |= 1u64 << player;
            let v = eval(value, coalition, players)?;
            let marginal = v - prev_v;
            prev_v = v;
            if !marginal.is_finite() {
                return Err(ShapleyError::NonFiniteValue { subset: coalition });
            }
            let n = (draw_index + 1) as f64;
            let delta = marginal - mean[player];
            mean[player] += delta / n;
            m2[player] += delta * (marginal - mean[player]);
        }
    }
    let std_errs = m2
        .iter()
        .zip(mean.iter())
        .map(|(&m2i, _)| {
            let n = draws as f64;
            if n <= 1.0 {
                0.0
            } else {
                (m2i / (n - 1.0) / n).sqrt()
            }
        })
        .collect();
    Ok(SampledShapley {
        values: mean,
        std_errs,
        draws,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// The glove game: players 0,1 hold left gloves, player 2 the right
    /// glove; a pair is worth 1. Known Shapley values: R gets 2/3, each L
    /// gets 1/6 (efficiency: 2/3 + 1/6 + 1/6 = 1).
    fn glove(active: &[bool]) -> f64 {
        let left = active[0] || active[1];
        let right = active[2];
        if left && right { 1.0 } else { 0.0 }
    }

    #[test]
    fn shapley_exact_solves_the_glove_game() {
        // Known values: the right glove earns 2/3, each left glove 1/6
        // (efficiency: 2/3 + 1/6 + 1/6 = 1).
        let phi = shapley_exact(&glove, 3).unwrap();
        assert!(
            (phi[2] - 2.0 / 3.0).abs() < 1e-12,
            "right glove got {}",
            phi[2]
        );
        assert!(
            (phi[0] - 1.0 / 6.0).abs() < 1e-12,
            "left glove 0 got {}",
            phi[0]
        );
        assert!(
            (phi[1] - 1.0 / 6.0).abs() < 1e-12,
            "left glove 1 got {}",
            phi[1]
        );
    }

    #[test]
    fn shapley_is_efficient_and_dummy_players_get_zero() {
        // v(S) = sum of weights of active players in {0,1} plus a bonus only
        // when BOTH 0 and 1 are active; player 2 is a dummy.
        let value = |active: &[bool]| -> f64 {
            let base = if active[0] { 2.0 } else { 0.0 } + if active[1] { 3.0 } else { 0.0 };
            let bonus = if active[0] && active[1] { 1.0 } else { 0.0 };
            base + bonus
        };
        let phi = shapley_exact(&value, 3).unwrap();
        // Efficiency: values sum to v(N) - v(empty).
        let total = phi.iter().sum::<f64>();
        assert!((total - 6.0).abs() < 1e-12, "efficiency violated: {total}");
        assert!(
            phi[2].abs() < 1e-12,
            "dummy must receive zero, got {}",
            phi[2]
        );
        // The interaction bonus splits evenly between its two contributors.
        assert!((phi[0] - 2.5).abs() < 1e-12);
        assert!((phi[1] - 3.5).abs() < 1e-12);
    }

    #[test]
    fn sampled_estimates_track_the_exact_solution_within_noise() {
        let exact = shapley_exact(&glove, 3).unwrap();
        let sampled = shapley_sampled(&glove, 3, 20_000, 7).unwrap();
        for (e, s) in exact.iter().zip(sampled.values.iter()) {
            assert!(
                (e - s).abs() < 0.02,
                "sampled {s} drifted from exact {e} beyond 20k-draw noise"
            );
        }
        assert!(
            sampled
                .std_errs
                .iter()
                .all(|se| se.is_finite() && *se >= 0.0)
        );
    }

    #[test]
    fn compute_bounds_are_refusals_not_panics() {
        assert!(matches!(
            shapley_exact(&glove, MAX_EXACT_PLAYERS + 1),
            Err(ShapleyError::ExactBoundExceeded { .. })
        ));
        assert!(matches!(
            shapley_sampled(&glove, 0, 100, 1),
            Err(ShapleyError::NoPlayers(0))
        ));
    }

    #[test]
    fn non_finite_value_functions_are_refused() {
        let broken = |active: &[bool]| -> f64 {
            if active[0] && active[1] {
                f64::NAN
            } else {
                0.0
            }
        };
        assert!(matches!(
            shapley_exact(&broken, 2),
            Err(ShapleyError::NonFiniteValue { subset: 3 })
        ));
    }

    /// Review P2-8 regression: a finite-per-subset value function can
    /// still produce an INFINITE marginal (finite minus finite at 1e308
    /// scale); the evaluator must refuse, never emit phi = inf.
    #[test]
    fn infinite_marginals_are_refused() {
        let giant = |active: &[bool]| -> f64 {
            if active[0] && active[1] {
                1e308
            } else if active[0] {
                -1e308
            } else {
                0.0
            }
        };
        // v({0}) = -1e308, v({0,1}) = +1e308: the marginal for player 0 on
        // coalition {1} is 2e308 = inf.
        assert!(matches!(
            shapley_exact(&giant, 2),
            Err(ShapleyError::NonFiniteValue { .. })
        ));
        assert!(matches!(
            shapley_sampled(&giant, 2, 10, 1),
            Err(ShapleyError::NonFiniteValue { .. })
        ));
    }
}
