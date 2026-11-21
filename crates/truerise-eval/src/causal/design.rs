//! Experiment-design primitives (CA4): assignment schemas for agent-run
//! studies.
//!
//! Agent runs interfere with each other through shared memory sessions,
//! shared caches and provider connections, so run-level randomisation
//! silently violates SUTVA. The schemas here are the fix: blocked
//! assignment for exact arm balance, cluster assignment so every member of
//! an interfering group receives the SAME arm, and a crossover scheduler
//! for paired within-subject comparisons. All schemas take an explicit seed
//! and are byte-reproducible (see [`super::rng`]).
//!
//! Honesty boundary: an assignment schema identifies nothing by itself. It
//! only makes the DESIGN auditable - who got which arm, decided by an
//! unambiguous seeded rule, never by convenience.

/// The two arms of a two-arm study.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// Arm A (conventionally the candidate change).
    A,
    /// Arm B (conventionally the incumbent).
    B,
}

/// Assignment-refusal reasons. Every variant is a design error, not a
/// warning: an unbalanced or ambiguous design cannot be repaired after the
/// runs happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DesignError {
    /// A zero-size study has no contrast to randomise.
    #[error("design refused: study needs at least 2 units, got {0}")]
    TooFewUnits(usize),
    /// Block size must be even and divide the subject count exactly, or the
    /// balance guarantee is void.
    #[error("design refused: block size {block} must be even and divide {units} units exactly")]
    BadBlock {
        /// Requested block size.
        block: usize,
        /// Subject count.
        units: usize,
    },
    /// An odd subject count cannot balance two arms exactly.
    #[error(
        "design refused: {0} units cannot split into two exactly balanced arms - pad or use a simple assignment and report the realised imbalance"
    )]
    OddUnits(usize),
    /// Fewer than two clusters leaves no between-cluster contrast.
    #[error("design refused: cluster assignment needs at least 2 distinct clusters, got {0}")]
    TooFewClusters(usize),
}

/// Blocked randomisation: `block`-sized blocks, each split exactly in half
/// between the arms.
///
/// Guarantees: total balance is EXACT (`n/2` per arm) and balance holds
/// inside every block, so a mid-study look cannot drift by chance alone.
/// `n` must be divisible by `block`, and `block` must be even.
pub fn assign_blocked(n: usize, block: usize, seed: u64) -> Result<Vec<Arm>, DesignError> {
    if n < 2 {
        return Err(DesignError::TooFewUnits(n));
    }
    if block < 2 || !block.is_multiple_of(2) || !n.is_multiple_of(block) {
        return Err(DesignError::BadBlock { block, units: n });
    }
    let mut rng = super::rng::SplitMix64::new(seed);
    let mut arms = Vec::with_capacity(n);
    for start in (0..n).step_by(block) {
        // Choose which positions inside the block take arm A: exactly half.
        let mut positions: Vec<usize> = (start..start + block).collect();
        rng.shuffle(&mut positions);
        let mut block_arms = vec![Arm::B; block];
        for pos in positions.into_iter().take(block / 2) {
            block_arms[pos - start] = Arm::A;
        }
        arms.extend(block_arms);
    }
    Ok(arms)
}

/// Simple randomisation: an independent coin per unit.
///
/// Balance is APPROXIMATE and the realised imbalance is the caller's to
/// report; this is the escape hatch for subject counts that refuse the
/// exact-balance contracts of [`assign_blocked`] / [`plan_crossover`].
pub fn assign_simple(n: usize, seed: u64) -> Result<Vec<Arm>, DesignError> {
    if n < 2 {
        return Err(DesignError::TooFewUnits(n));
    }
    let mut rng = super::rng::SplitMix64::new(seed ^ 0xDA7A_C0DE);
    Ok((0..n)
        .map(|_| if rng.next_f64() < 0.5 { Arm::A } else { Arm::B })
        .collect())
}

/// Cluster randomisation: one draw per CLUSTER, inherited by every member.
///
/// This is the SUTVA repair for interfering units (a shared memory session,
/// a shared cache namespace): members of one cluster always land on the
/// same arm, so no run in the cluster can contaminate an arm it does not
/// belong to.
///
/// The cluster count must be even (exact between-cluster balance) and at
/// least two (a one-cluster study has no contrast).
pub fn assign_clusters(cluster_ids: &[u64], seed: u64) -> Result<Vec<Arm>, DesignError> {
    let mut distinct: Vec<u64> = cluster_ids.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() < 2 {
        return Err(DesignError::TooFewClusters(distinct.len()));
    }
    if !distinct.len().is_multiple_of(2) {
        return Err(DesignError::OddUnits(distinct.len()));
    }
    let mut rng = super::rng::SplitMix64::new(seed);
    rng.shuffle(&mut distinct);
    // First half of the shuffled cluster list -> A, second half -> B.
    let arm_of: std::collections::HashMap<u64, Arm> = distinct
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                *id,
                if i < distinct.len() / 2 {
                    Arm::A
                } else {
                    Arm::B
                },
            )
        })
        .collect();
    Ok(cluster_ids.iter().map(|id| arm_of[id]).collect())
}

/// A crossover plan: every subject receives BOTH arms in a seeded,
/// order-randomised sequence.
///
/// Crossover controls subject-level heterogeneity (the same task pack under
/// both arms), which is the honest design against nondeterministic
/// providers. The washout obligation is part of the plan: agents learn, so
/// period 2 runs on an agent whose state was reset - the caller owns that
/// reset, the plan only records the order contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossoverPlan {
    /// One (period-1 arm, period-2 arm) pair per subject.
    pub per_subject: Vec<(Arm, Arm)>,
    /// Subjects starting on arm A (the AB group).
    pub ab_group: usize,
    /// Subjects starting on arm B (the BA group).
    pub ba_group: usize,
}

/// Plan a 2x2 crossover. Subject count must be even; half the subjects get
/// AB, half BA, and WHICH subjects get which order is seeded, not
/// positional.
pub fn plan_crossover(subjects: usize, seed: u64) -> Result<CrossoverPlan, DesignError> {
    if subjects < 2 {
        return Err(DesignError::TooFewUnits(subjects));
    }
    if !subjects.is_multiple_of(2) {
        return Err(DesignError::OddUnits(subjects));
    }
    let mut rng = super::rng::SplitMix64::new(seed ^ 0xCAFE_BABE);
    let mut order: Vec<u8> = vec![0; subjects]; // 0 = AB, 1 = BA
    for slot in order.iter_mut().take(subjects / 2) {
        *slot = 1;
    }
    rng.shuffle(&mut order);
    let per_subject = order
        .iter()
        .map(|&o| {
            if o == 0 {
                (Arm::A, Arm::B)
            } else {
                (Arm::B, Arm::A)
            }
        })
        .collect();
    let ba_group = order.iter().filter(|&&o| o == 1).count();
    Ok(CrossoverPlan {
        per_subject,
        ab_group: subjects - ba_group,
        ba_group,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn count(arms: &[Arm], which: Arm) -> usize {
        arms.iter().filter(|a| **a == which).count()
    }

    #[test]
    fn blocked_assignment_is_exactly_balanced_inside_every_block() {
        let arms = assign_blocked(24, 6, 11).unwrap();
        assert_eq!(count(&arms, Arm::A), 12);
        assert_eq!(count(&arms, Arm::B), 12);
        for chunk in arms.chunks(6) {
            assert_eq!(
                count(chunk, Arm::A),
                3,
                "block imbalance is a silent design drift"
            );
        }
    }

    #[test]
    fn blocked_assignment_is_seeded_and_reproducible() {
        let a = assign_blocked(20, 4, 77).unwrap();
        let b = assign_blocked(20, 4, 77).unwrap();
        assert_eq!(a, b);
        let c = assign_blocked(20, 4, 78).unwrap();
        assert_ne!(a, c, "different seeds must be able to disagree");
    }

    #[test]
    fn bad_block_geometry_is_refused() {
        assert!(
            assign_blocked(10, 3, 1).is_err(),
            "odd block cannot split in half"
        );
        assert!(assign_blocked(10, 4, 1).is_err(), "4 does not divide 10");
        assert!(assign_blocked(1, 2, 1).is_err());
    }

    #[test]
    fn cluster_assignment_unifies_members_and_balances_clusters() {
        // Six distinct clusters (even - the exact-balance contract).
        let ids = [9, 9, 9, 4, 4, 7, 7, 2, 2, 5, 5, 8];
        let arms = assign_clusters(&ids, 5).unwrap();
        let arm_of = |id: u64| arms[ids.iter().position(|&x| x == id).unwrap()];
        for id in [9u64, 4, 7, 2, 5, 8] {
            let first = arm_of(id);
            for (i, &other) in ids.iter().enumerate() {
                if other == id {
                    assert_eq!(
                        arms[i], first,
                        "cluster {id} split across arms - SUTVA violated"
                    );
                }
            }
        }
        let a_clusters = [9u64, 4, 7, 2, 5, 8]
            .iter()
            .filter(|id| arm_of(**id) == Arm::A)
            .count();
        assert_eq!(a_clusters, 3, "cluster-level balance must be exact");
    }

    #[test]
    fn one_or_odd_cluster_counts_are_refused() {
        assert!(assign_clusters(&[1, 1, 1], 3).is_err());
        assert!(assign_clusters(&[1, 2, 3], 3).is_err());
    }

    #[test]
    fn crossover_orders_balance_exactly() {
        let plan = plan_crossover(16, 2026).unwrap();
        assert_eq!(plan.ab_group, 8);
        assert_eq!(plan.ba_group, 8);
        for (p1, p2) in &plan.per_subject {
            assert_ne!(p1, p2, "a crossover subject must switch arms");
        }
        let again = plan_crossover(16, 2026).unwrap();
        assert_eq!(plan, again);
    }

    #[test]
    fn odd_subject_counts_are_refused_by_exact_balance_schemas() {
        assert!(plan_crossover(7, 1).is_err());
        assert!(
            assign_simple(7, 1).is_ok(),
            "simple assignment is the documented escape hatch"
        );
    }
}
