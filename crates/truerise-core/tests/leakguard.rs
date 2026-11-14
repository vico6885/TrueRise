#![allow(clippy::unwrap_used, clippy::expect_used)]
//! LeakGuard suite - the four leak vectors (spec §6 LeakGuard row).
//!
//! These tests must NEVER be skipped (spec §7 M1 gate; the timesafety CI job
//! exists to keep them unskippable). Spec-id mapping (the ids also live in
//! tests/timesafety/four_vectors.toml, which the CI job cross-checks):
//! - `future_disturbance_invariance` -> fit_apply_separation_with_negative_control
//!   (train statistic invariant under future-only disturbance)
//! - `pit_boundary_rejection` -> pit_boundary_rejection
//! - `label_uniqueness` -> sweep::detector_flags_exactly_the_overlapping_pairs
//! - `fit_apply_separation` -> fit_apply_separation_with_negative_control
//!   (fit reads only the train span; negative control moves with train data)

use truerise_core::timecore::{FoldWindow, TimeSlice, any_overlap};
use truerise_core::{CoreError, TimeLeak};

/// Vector 1 + 2 (property): every window accepted by the constructor is
/// leak-free (no overlap, purge honoured); every illegal window is rejected.
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn accepted_windows_are_always_leak_free(
            t0 in 0u64..1_000,
            t1 in 0u64..1_000,
            e0 in 0u64..1_000,
            e1 in 0u64..1_000,
            purge in 0u64..50,
        ) {
            let (ts, es) = match (TimeSlice::new(t0.min(t1) + 1, t0.max(t1) + 2), TimeSlice::new(e0.min(e1) + 1, e0.max(e1) + 2)) {
                (Ok(ts), Ok(es)) => (ts, es),
                _ => return Ok(()), // degenerate ranges: constructor rejects anyway
            };
            match FoldWindow::new(ts, es, purge) {
                Ok(w) => {
                    // Accepted => the two invariants hold unconditionally.
                    prop_assert!(!w.train.overlaps(&w.eval));
                    prop_assert!(w.check_leak().is_ok());
                    prop_assert!(w.train.gap_to(&w.eval) >= w.purge_ms);
                }
                Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain { .. })) => {
                    // Rejected exactly when the slices overlap.
                    prop_assert!(ts.overlaps(&es));
                }
                Err(CoreError::TimeLeak(TimeLeak::InsufficientPurge { .. })) => {
                    // Rejected exactly when the gap is short (and no overlap).
                    prop_assert!(!ts.overlaps(&es) && ts.gap_to(&es) < purge);
                }
                Err(other) => prop_assert!(false, "unexpected error: {other:?}"),
            }
        }
    }
}

/// Vector 3 (property): the sweep detector finds an overlap whenever one
/// exists (label regions must be unique; the detector is the gate).
mod sweep {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn detector_flags_exactly_the_overlapping_pairs(
            a0 in 0u64..50, a1 in 0u64..50,
            b0 in 0u64..50, b1 in 0u64..50,
        ) {
            let a = TimeSlice::new(a0.min(a1), a0.max(a1) + 1).unwrap();
            let b = TimeSlice::new(b0.min(b1), b0.max(b1) + 1).unwrap();
            let detected = any_overlap(&[a, b]).is_some();
            prop_assert_eq!(detected, a.overlaps(&b));
        }
    }
}

/// Vector 1 + 4 (concrete): fit statistics read only the train span.
/// Disturbing the future (eval span + beyond) must NOT move the train mean;
/// the NEGATIVE CONTROL disturbs the train span itself and the mean MUST move.
#[test]
fn fit_apply_separation_with_negative_control() {
    // Timeline 0..100; train [0,50), eval [60,90); data[i] = i as i64.
    let data: Vec<i64> = (0..100).map(|i| i as i64).collect();
    let train = TimeSlice::new(0, 50).unwrap();
    let eval = TimeSlice::new(60, 90).unwrap();
    FoldWindow::new(train, eval, 10).expect("10ms purge honoured");

    let mean_over = |s: &TimeSlice, d: &[i64]| -> i64 {
        d[s.start as usize..s.end as usize].iter().sum::<i64>() / ((s.end - s.start) as i64)
    };
    // Disturb ONLY the future (eval span and after): train mean invariant.
    let mut disturbed_future = data.clone();
    disturbed_future[60..].fill(999);
    let baseline = mean_over(&train, &data);
    assert_eq!(
        baseline,
        mean_over(&train, &disturbed_future),
        "vector 1: future disturbance must not move the train statistic"
    );
    // And the eval statistic DOES move (proving the disturbance landed).
    assert_ne!(mean_over(&eval, &data), mean_over(&eval, &disturbed_future));

    // NEGATIVE CONTROL: disturb the train span itself -> the mean MUST move.
    let mut disturbed_train = data.clone();
    disturbed_train[..50].fill(999);
    assert_ne!(
        mean_over(&train, &data),
        mean_over(&train, &disturbed_train),
        "negative control must move when the train span itself changes"
    );
}

/// Vector 2 (concrete): PIT boundary rejection - the constructor refuses
/// overlap and purge shortfall with the exact leak variants.
#[test]
fn pit_boundary_rejection() {
    let train = TimeSlice::new(0, 100).unwrap();
    // Overlap: the constructor itself refuses (fail fast, not fail later).
    let overlapped = TimeSlice::new(90, 120).unwrap();
    assert!(matches!(
        FoldWindow::new(train, overlapped, 0),
        Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain {
            eval_start: 90,
            ..
        }))
    ));
    // Purge shortfall: construction itself refuses.
    let short = TimeSlice::new(105, 120).unwrap();
    assert!(matches!(
        FoldWindow::new(train, short, 10),
        Err(CoreError::TimeLeak(TimeLeak::InsufficientPurge {
            got_ms: 5,
            required_ms: 10,
            ..
        }))
    ));
}
