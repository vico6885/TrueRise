//! Pairwise comparison ratings (review family B, part 4): Bradley-Terry
//! over forced-choice comparisons between agents.
//!
//! Agent leaderboards compare arms on the same task packs pairwise; the
//! Bradley-Terry model turns win/loss records into ratings with
//! win-probability semantics: `p(i beats j) = pi_i / (pi_i + pi_j)`. Fitting
//! uses Hunter's MM algorithm (monotone convergence, no step-size tuning).
//!
//! Honest boundaries:
//! - Ties are reported, not modelled: they carry no rating information in
//!   plain BT (the Davidson extension is deliberately deferred).
//! - The comparison graph must be CONNECTED: ratings are only identified
//!   relative to opponents a player actually faced, so a disconnected
//!   component would get arbitrary relative ratings.
//! - An undefeated player (zero losses, some wins) makes the MLE diverge:
//!   refused, not clamped. The MIRROR case is accepted and must be: a
//!   winless player's MLE is the attainable boundary strength 0, reported
//!   as exactly-zero ratings and certainty probabilities (review A2-P2-4)
//!   - an asymmetry with a real mathematical reason, not an oversight.
//! - A SUBSET that never loses to its complement (e.g. two players both
//!   beating the rest) also diverges the MLE; per-player zero-loss checks
//!   cannot catch it. The MM loop then saturates at the boundary values -
//!   the report's [`Ratings::converged`] flag is the caller's honesty dial
//!   (review A2-P2-3).

/// Refusals for pairwise ratings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairwiseError {
    /// A player index outside the roster.
    #[error(
        "pairwise refused: comparison {index} cites player {player} outside the roster of {players}"
    )]
    PlayerOutOfRange {
        /// The offending comparison.
        index: usize,
        /// The offending player index.
        player: usize,
        /// The roster size.
        players: usize,
    },
    /// A player with no comparisons cannot be rated.
    #[error("pairwise refused: player {0} has no comparisons")]
    UncomparedPlayer(usize),
    /// A record where winner == loser is malformed data.
    #[error("pairwise refused: comparison {index} has the same winner and loser ({player})")]
    SelfComparison {
        /// The offending comparison.
        index: usize,
        /// The offending player.
        player: usize,
    },
    /// Ratings are only identified within a connected comparison graph.
    #[error(
        "pairwise refused: comparison graph has {components} components - cross-component ratings are arbitrary"
    )]
    DisconnectedGraph {
        /// The number of connected components found.
        components: usize,
    },
    /// An undefeated player diverges the MLE.
    #[error("pairwise refused: player {0} is undefeated (no losses) - the MLE diverges")]
    SeparatedPlayer(usize),
    /// The MM loop left the non-negative orthant (a numerical bug signal,
    /// not a data condition).
    #[error("pairwise refused: rating fit produced non-finite weights")]
    NotFinite,
}

/// One pairwise comparison record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    /// Winner's player index.
    pub winner: usize,
    /// Loser's player index.
    pub loser: usize,
}

/// The fitted rating report.
#[derive(Debug, Clone, PartialEq)]
pub struct Ratings {
    /// Fitted strengths, normalized so the strongest player has rating 1.0
    /// (pi on the multiplicative scale; p(i beats j) = pi_i/(pi_i + pi_j)).
    pub strengths: Vec<f64>,
    /// Total ties observed (excluded from the rating fit, reported here).
    pub ties: u64,
    /// Total decided comparisons behind the fit.
    pub decided: u64,
    /// MM iterations used.
    pub iterations: usize,
    /// False when the MM loop hit the iteration contract before the update
    /// delta collapsed (subset dominance diverges slowly at the boundary -
    /// the strengths are the correct limit but the caller should know the
    /// fit never settled; review A2-P2-3).
    pub converged: bool,
}

/// Fit Bradley-Terry ratings from decided comparisons.
///
/// `players` is the roster size; `comparisons` are decided records (ties
/// belong in `ties`). Ratings are identified up to a positive scale and
/// normalized so the maximum strength is exactly 1.0.
pub fn bradley_terry(
    players: usize,
    comparisons: &[Comparison],
    ties: u64,
) -> Result<Ratings, PairwiseError> {
    if players == 0 {
        return Err(PairwiseError::UncomparedPlayer(0));
    }
    for (i, c) in comparisons.iter().enumerate() {
        if c.winner >= players {
            return Err(PairwiseError::PlayerOutOfRange {
                index: i,
                player: c.winner,
                players,
            });
        }
        if c.loser >= players {
            return Err(PairwiseError::PlayerOutOfRange {
                index: i,
                player: c.loser,
                players,
            });
        }
        if c.winner == c.loser {
            return Err(PairwiseError::SelfComparison {
                index: i,
                player: c.winner,
            });
        }
    }
    // Connectivity over the undirected comparison graph (union-find).
    let mut parent: Vec<usize> = (0..players).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        let mut root = x;
        while parent[root] != root {
            root = parent[root];
        }
        let mut node = x;
        while parent[node] != node {
            let next = parent[node];
            parent[node] = root;
            node = next;
        }
        root
    }
    for c in comparisons {
        let (a, b) = (find(&mut parent, c.winner), find(&mut parent, c.loser));
        parent[a] = b;
    }
    let mut components = 0usize;
    for i in 0..players {
        if find(&mut parent, i) == i {
            components += 1;
        }
    }
    if components != 1 {
        return Err(PairwiseError::DisconnectedGraph { components });
    }
    // Per-player win counts and pair counts.
    let mut wins = vec![0u64; players];
    let mut pair_wins = vec![vec![0u64; players]; players]; // pair_wins[i][j] = times i beat j
    for c in comparisons {
        wins[c.winner] += 1;
        pair_wins[c.winner][c.loser] += 1;
    }
    for i in 0..players {
        let has_comparisons = wins[i] > 0 || (0..players).any(|j| j != i && pair_wins[j][i] > 0);
        let losses: u64 = (0..players)
            .filter(|&j| j != i)
            .map(|j| pair_wins[j][i])
            .sum();
        if !has_comparisons {
            return Err(PairwiseError::UncomparedPlayer(i));
        }
        if losses == 0 && wins[i] > 0 {
            return Err(PairwiseError::SeparatedPlayer(i));
        }
    }
    // Hunter's MM iteration: pi_i <- W_i / sum_{j != i} n_ij / (pi_i + pi_j),
    // normalized by the geometric mean each pass. Deterministic, monotone.
    let mut pi = vec![1.0; players];
    const MAX_ITERATIONS: usize = 500;
    let mut used = 0usize;
    let mut converged = false;
    for _ in 0..MAX_ITERATIONS {
        used += 1;
        let mut new_pi = vec![0.0; players];
        for i in 0..players {
            let mut denominator = 0.0;
            for j in 0..players {
                if i == j {
                    continue;
                }
                let n_ij = pair_wins[i][j] + pair_wins[j][i];
                if n_ij > 0 {
                    denominator += n_ij as f64 / (pi[i] + pi[j]);
                }
            }
            new_pi[i] = wins[i] as f64 / denominator.max(1e-300);
        }
        let geometric_mean =
            new_pi.iter().map(|p| p.max(1e-300).ln()).sum::<f64>() / players as f64;
        let norm = geometric_mean.exp();
        let mut delta = 0.0f64;
        for (old, p) in pi.iter_mut().zip(new_pi.iter_mut()) {
            *p /= norm;
            delta = delta.max((*old - *p).abs());
        }
        pi = new_pi;
        if delta < 1e-12 {
            converged = true;
            break;
        }
    }
    if pi.iter().any(|p| !p.is_finite()) {
        return Err(PairwiseError::NotFinite);
    }
    let max = pi.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let strengths = pi.iter().map(|p| p / max).collect();
    Ok(Ratings {
        strengths,
        ties,
        decided: comparisons.len() as u64,
        iterations: used,
        converged,
    })
}

/// Probability that player `i` beats player `j` under the fitted ratings.
///
/// `None` when either index is outside the roster, or when both players
/// have zero strength (two winless players in an accepted fit: the ratio
/// is 0/0 and a NaN "probability" would be a lie - review P2-9).
#[must_use]
pub fn win_probability(ratings: &Ratings, i: usize, j: usize) -> Option<f64> {
    let (a, b) = (ratings.strengths.get(i)?, ratings.strengths.get(j)?);
    if a + b <= 0.0 {
        return None;
    }
    Some(a / (a + b))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn records(pairs: &[(usize, usize)]) -> Vec<Comparison> {
        pairs
            .iter()
            .map(|(w, l)| Comparison {
                winner: *w,
                loser: *l,
            })
            .collect()
    }

    #[test]
    fn a_transitive_chain_orders_the_ratings_strictly() {
        // A dominates but takes one loss from C (an undefeated player makes
        // the MLE diverge and is refused); B beats C. Decided records only.
        let comparisons = records(&[(0, 1), (0, 1), (1, 2), (1, 2), (0, 2), (0, 2), (2, 0)]);
        let ratings = bradley_terry(3, &comparisons, 0).unwrap();
        assert!(ratings.strengths[0] > ratings.strengths[1]);
        assert!(ratings.strengths[1] > ratings.strengths[2]);
        assert!(
            (ratings.strengths[0] - 1.0).abs() < 1e-12,
            "max normalized to 1"
        );
        let p = win_probability(&ratings, 0, 2).unwrap();
        assert!(p > 0.7, "A should dominate C, p = {p}");
    }

    #[test]
    fn a_cyclic_settles_to_a_near_tie() {
        // A>B, B>C, C>A in equal counts: transitivity is violated and the
        // honest fit reports near-equal strengths, not a fake ordering.
        let comparisons = records(&[(0, 1), (1, 2), (2, 0), (0, 1), (1, 2), (2, 0)]);
        let ratings = bradley_terry(3, &comparisons, 0).unwrap();
        let (hi, lo) = (
            ratings
                .strengths
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max),
            ratings
                .strengths
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min),
        );
        assert!(
            hi / lo < 1.2,
            "cyclic records must not manufacture an ordering: {ratings:?}"
        );
    }

    #[test]
    fn ties_are_reported_but_not_modelled() {
        let comparisons = records(&[(0, 1), (0, 1), (1, 0)]);
        let ratings = bradley_terry(2, &comparisons, 7).unwrap();
        assert_eq!(ratings.ties, 7);
        assert_eq!(ratings.decided, 3);
    }

    /// Review A2-P2-3 regression: a dominant SUBSET (every member only
    /// ever beats or ties within, never loses cross-wise) saturates the MM
    /// loop at the boundary - the fit must SURFACE that via `converged`.
    #[test]
    fn subset_dominance_surfaces_the_unsettled_fit() {
        // {0,1} always beat {2,3}; 0 and 1 split their head-to-heads, and
        // so do 2 and 3 - no zero-loss player, but the MLE still diverges.
        let comparisons = records(&[
            (0, 2),
            (0, 2),
            (1, 3),
            (1, 3),
            (0, 1),
            (1, 0),
            (2, 3),
            (3, 2),
        ]);
        let ratings = bradley_terry(4, &comparisons, 0).unwrap();
        assert!(
            !ratings.converged,
            "subset dominance must surface as unsettled, iterations {}",
            ratings.iterations
        );
        // The boundary LIMIT is still meaningful: the dominant pair above.
        assert!(ratings.strengths[0] > ratings.strengths[2]);
        assert!(ratings.strengths[1] > ratings.strengths[3]);
    }

    /// Review P2-9: two winless players inside an ACCEPTED fit (they only
    /// ever lost) have zero strength both - the ratio would be NaN, and a
    /// NaN probability is refused instead of reported.
    #[test]
    fn zero_strength_pairs_return_none_not_nan() {
        // 0 beats 1, 2 and 3; 3 takes a win off 0 (else 0 would be
        // undefeated); players 1 and 2 only ever lost.
        let comparisons = records(&[(3, 0), (0, 1), (0, 2), (0, 3)]);
        let ratings = bradley_terry(4, &comparisons, 0).unwrap();
        assert!(
            win_probability(&ratings, 1, 2).is_none(),
            "0/0 must be None, not NaN"
        );
        assert!(win_probability(&ratings, 3, 1).unwrap() > 0.5);
    }

    #[test]
    fn structural_conditions_are_refused_by_name() {
        let comparisons = records(&[(0, 1)]);
        // Disconnected roster: players 2,3 never faced anyone.
        assert!(matches!(
            bradley_terry(4, &comparisons, 0),
            Err(PairwiseError::DisconnectedGraph { components: 3 })
        ));
        // Undefeated player: the MLE diverges.
        assert!(matches!(
            bradley_terry(2, &comparisons, 0),
            Err(PairwiseError::SeparatedPlayer(0))
        ));
        // Out-of-range player.
        let bad = records(&[(0, 5)]);
        assert!(matches!(
            bradley_terry(3, &bad, 0),
            Err(PairwiseError::PlayerOutOfRange {
                index: 0,
                player: 5,
                players: 3
            })
        ));
        // Self-comparison is malformed data, not a record.
        let self_comparison = records(&[(1, 1)]);
        assert!(matches!(
            bradley_terry(2, &self_comparison, 0),
            Err(PairwiseError::SelfComparison {
                index: 0,
                player: 1
            })
        ));
        assert!(matches!(
            bradley_terry(0, &[], 0),
            Err(PairwiseError::UncomparedPlayer(0))
        ));
    }
}
