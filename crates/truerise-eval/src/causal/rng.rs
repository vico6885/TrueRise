//! Deterministic seeded randomness for the experiment primitives (CA4).
//!
//! Every randomised primitive in the [`super`] family takes an explicit
//! `seed: u64` and produces the identical sequence on every platform and
//! every run: an assignment drawn for a study must be re-derivable from its
//! seed alone, because the audit chain records seeds, not draws. No external
//! RNG dependency is taken - the generator below is the standard SplitMix64
//! (Steele, Lea & Mincer), whose output quality is ample for assignment,
//! permutation and sampling workloads and whose implementation is short
//! enough to audit by eye.

/// SplitMix64: a seeded counter-style generator with a 64-bit state.
///
/// The stream is fully determined by the seed: two generators built with the
/// same seed always agree bit-for-bit (asserted by
/// [`tests::same_seed_reproduces_the_stream_exactly`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

/// The golden-ratio increment that drives the state forward.
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl SplitMix64 {
    /// Build a generator. Seed 0 is a legal seed (the mixing makes it a
    /// perfectly ordinary stream).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit draw. Cheap, branchless, and exhaustive over the
    /// state space: consecutive outputs are unrelated even for sequential
    /// seeds.
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform f64 in `[0, 1)`. The draw uses the top 53 bits, so every f64
    /// mantissa value in range is reachable and none outside is.
    #[must_use]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// An unbiased index in `[0, n)`; `None` only when `n == 0`.
    ///
    /// Rejection sampling over the full 64-bit range: the acceptance zone is
    /// the largest multiple of `n` inside `u64::MAX`, so every index has
    /// exactly the same number of preimages (no modulo bias, no float
    /// rounding).
    #[must_use]
    pub fn next_below(&mut self, n: usize) -> Option<usize> {
        if n == 0 {
            return None;
        }
        let n64 = n as u64;
        let zone = u64::MAX - (u64::MAX % n64);
        loop {
            let draw = self.next_u64();
            if draw < zone {
                return Some((draw % n64) as usize);
            }
        }
    }

    /// In-place Fisher-Yates shuffle. Every permutation of the input is
    /// equally likely (each swap position draws via [`Self::next_below`]).
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.next_below(i + 1).unwrap_or(0);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn same_seed_reproduces_the_stream_exactly() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..1_000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        // A different seed must not accidentally align with the first draws.
        let mut c = SplitMix64::new(43);
        let mut d = SplitMix64::new(42);
        assert_ne!(c.next_u64(), d.next_u64());
    }

    #[test]
    fn f64_draws_stay_in_the_half_open_unit_interval() {
        let mut g = SplitMix64::new(7);
        for _ in 0..10_000 {
            let v = g.next_f64();
            assert!((0.0..1.0).contains(&v), "draw {v} outside [0, 1)");
        }
    }

    #[test]
    fn next_below_is_balanced_across_buckets() {
        let mut g = SplitMix64::new(2026);
        let buckets = [0usize; 7];
        let mut counts = buckets;
        for _ in 0..70_000 {
            let idx = g.next_below(7).unwrap();
            counts[idx] += 1;
        }
        // Exact expectation 10_000 per bucket; a fair generator stays within
        // a few hundred. (A modulo-biased 64-bit stream would NOT drift
        // detectably at this sample size - the discriminating uniformity
        // oracle is the permutation census below.)
        for c in counts {
            assert!((9_500..=10_500).contains(&c), "bucket count {c} drifted");
        }
    }

    /// Review P2-3: a bucket-count test cannot catch a biased Fisher-Yates
    /// (e.g. swap positions drawn from [0, len) instead of [0, i]), but a
    /// census over ALL 4! permutations of a 4-element sequence can - every
    /// one of the 24 orders must appear at its expected frequency.
    #[test]
    fn shuffle_census_over_all_24_permutations_is_flat() {
        let target: Vec<u32> = vec![10, 20, 30, 40];
        let canonical = |v: &[u32]| -> usize {
            // Lehmer rank: digit_i = how many REMAINING elements are smaller
            // than the one at position i; the factorial-base accumulation
            // maps every permutation bijectively onto 0..24 (lexicographic).
            let order: Vec<usize> = v
                .iter()
                .map(|x| target.iter().position(|&t| t == *x).unwrap())
                .collect();
            let mut rank = 0usize;
            let mut base = order.len();
            for i in 0..order.len() {
                let smaller_to_the_right = order[i + 1..].iter().filter(|&&s| s < order[i]).count();
                rank = rank * base + smaller_to_the_right;
                base -= 1;
            }
            rank
        };
        let mut counts = [0usize; 24];
        let runs = 48_000;
        let mut g = SplitMix64::new(424_242);
        for _ in 0..runs {
            let mut v = target.clone();
            g.shuffle(&mut v);
            counts[canonical(&v)] += 1;
        }
        // Expected 2_000 each; an off-by-one Fisher-Yates biases whole
        // permutation classes far outside a +/-20% band.
        for (rank, c) in counts.iter().enumerate() {
            assert!(
                (1_600..=2_400).contains(c),
                "permutation rank {rank} appeared {c} times (expected ~2000)"
            );
        }
    }

    #[test]
    fn next_below_refuses_an_empty_range() {
        let mut g = SplitMix64::new(1);
        assert!(g.next_below(0).is_none());
        assert_eq!(g.next_below(1).unwrap(), 0);
    }

    #[test]
    fn shuffle_preserves_the_multiset_and_is_seeded() {
        let mut g1 = SplitMix64::new(99);
        let mut g2 = SplitMix64::new(99);
        let mut v1: Vec<u32> = (0..50).collect();
        let mut v2: Vec<u32> = (0..50).collect();
        g1.shuffle(&mut v1);
        g2.shuffle(&mut v2);
        assert_eq!(v1, v2, "same seed must give the same permutation");
        let mut sorted = v1.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..50).collect::<Vec<u32>>());
        // The identity order surviving 50 draws is astronomically unlikely
        // but not impossible; assert only that SOME order moved.
        assert_ne!(v1, (0..50).collect::<Vec<u32>>());
    }
}
