//! Time slices and fold windows - half-open intervals with enforced purge
//! (spec §6 TimeSlice/FoldWindow row; LeakGuard vectors 1-2 live here).
//!
//! - [`TimeSlice`]: half-open `[start, end)`, `start < end` enforced.
//! - [`FoldWindow`]: train slice + purge gap + eval slice, with
//!   `eval.start >= train.end + purge` enforced at construction.
//! - [`overlap`]: interval overlap detector over slices.

use crate::errors::{CoreError, TimeLeak};

/// Half-open time slice `[start, end)`. Invariant: `start < end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSlice {
    /// First instant (inclusive).
    pub start: u64,
    /// Last instant (exclusive).
    pub end: u64,
}

impl TimeSlice {
    /// Construct with the half-open invariant enforced.
    ///
    /// # Errors
    /// [`TimeLeak`] is not raised here (empty slices are a data error, not a
    /// leak): returns [`CoreError`] via a plain validate to keep the API
    /// total - see [`TimeSlice::new`].
    pub fn new(start: u64, end: u64) -> Result<Self, CoreError> {
        if start >= end {
            return Err(CoreError::DataConsistency(
                crate::errors::DataConsistency::CounterReversed {
                    name: "slice span".into(),
                    from: start,
                    to: end,
                },
            ));
        }
        Ok(Self { start, end })
    }

    /// Does this slice overlap ``other``? (Half-open semantics: touching
    /// endpoints do NOT overlap.)
    pub fn overlaps(&self, other: &TimeSlice) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// The gap from this slice's end to ``other``'s start (0 when overlapping
    /// or touching).
    pub fn gap_to(&self, other: &TimeSlice) -> u64 {
        other.start.saturating_sub(self.end)
    }
}

/// Train + purge + eval window. The purge is ENFORCED at construction:
/// `eval.start >= train.end + purge_ms` (spec: FoldWindow 强制 purge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldWindow {
    /// Training slice.
    pub train: TimeSlice,
    /// Evaluation slice.
    pub eval: TimeSlice,
    /// Required purge gap in ms (integer units; no float seconds).
    pub purge_ms: u64,
}

impl FoldWindow {
    /// Construct with the purge invariant enforced.
    ///
    /// # Errors
    /// [`TimeLeak::EvalOverlapsTrain`] when the slices overlap (a raw gap of
    /// zero must NEVER pass - an overlap is a leak, not a zero purge);
    /// [`TimeLeak::InsufficientPurge`] when the actual gap is smaller than
    /// ``purge_ms``; data error when slices are malformed.
    pub fn new(train: TimeSlice, eval: TimeSlice, purge_ms: u64) -> Result<Self, CoreError> {
        if train.overlaps(&eval) {
            return Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain {
                eval_start: eval.start,
                eval_end: eval.end,
                train_end: train.end,
            }));
        }
        let gap = train.gap_to(&eval);
        if gap < purge_ms {
            return Err(CoreError::TimeLeak(TimeLeak::InsufficientPurge {
                required_ms: purge_ms,
                got_ms: gap,
                window: format!(
                    "[{}, {}) -> [{}, {})",
                    train.start, train.end, eval.start, eval.end
                ),
            }));
        }
        Ok(Self {
            train,
            eval,
            purge_ms,
        })
    }

    /// Leak check: eval overlapping train is the canonical leak (spec vector 1).
    pub fn check_leak(&self) -> Result<(), CoreError> {
        if self.train.overlaps(&self.eval) {
            return Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain {
                eval_start: self.eval.start,
                eval_end: self.eval.end,
                train_end: self.train.end,
            }));
        }
        Ok(())
    }
}

/// Sweep detector: does any pair among ``slices`` overlap? O(n log n).
///
/// Half-open semantics: at equal timestamps, END events are processed before
/// START events (touching endpoints do not overlap).
pub fn any_overlap(slices: &[TimeSlice]) -> Option<(usize, usize)> {
    let mut order: Vec<(u64, u8, usize)> = slices
        .iter()
        .enumerate()
        .flat_map(|(i, s)| [(s.start, 1u8, i), (s.end, 0u8, i)])
        .collect();
    order.sort();
    let mut open: Vec<usize> = Vec::new();
    for (_t, kind, idx) in order {
        if kind == 1 {
            if let Some(&other) = open.first() {
                return Some((other, idx));
            }
            open.push(idx);
        } else {
            open.retain(|&x| x != idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn half_open_touching_endpoints_do_not_overlap() {
        let a = TimeSlice::new(0, 10).unwrap();
        let b = TimeSlice::new(10, 20).unwrap();
        assert!(!a.overlaps(&b));
        assert_eq!(a.gap_to(&b), 0);
    }

    #[test]
    fn fold_window_enforces_purge() {
        let train = TimeSlice::new(0, 100).unwrap();
        let eval = TimeSlice::new(110, 200).unwrap();
        assert!(FoldWindow::new(train, eval, 10).is_ok());
        let err = FoldWindow::new(train, eval, 11).unwrap_err();
        assert!(matches!(
            err,
            CoreError::TimeLeak(TimeLeak::InsufficientPurge {
                required_ms: 11,
                got_ms: 10,
                ..
            })
        ));
    }

    #[test]
    fn leak_check_catches_overlap() {
        // The constructor now refuses overlaps up front; check_leak remains as
        // the independent second gate for windows assembled by other means.
        let train = TimeSlice::new(0, 100).unwrap();
        let eval = TimeSlice::new(90, 200).unwrap();
        let w = FoldWindow {
            train,
            eval,
            purge_ms: 0,
        };
        assert!(matches!(
            w.check_leak(),
            Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain {
                eval_start: 90,
                ..
            }))
        ));
        // And the constructor refuses the same pair directly (fail fast).
        assert!(matches!(
            FoldWindow::new(train, eval, 0),
            Err(CoreError::TimeLeak(TimeLeak::EvalOverlapsTrain { .. }))
        ));
    }

    #[test]
    fn empty_slice_rejected() {
        assert!(TimeSlice::new(5, 5).is_err());
        assert!(TimeSlice::new(6, 5).is_err());
    }

    #[test]
    fn sweep_finds_one_overlap_pair() {
        let a = TimeSlice::new(0, 10).unwrap();
        let b = TimeSlice::new(5, 15).unwrap();
        let c = TimeSlice::new(20, 30).unwrap();
        assert!(any_overlap(&[a, b, c]).is_some());
        assert_eq!(any_overlap(&[a, c]), None);
    }
}
