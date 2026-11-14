//! The core error taxonomy (spec: thiserror domain tree + structured context).
//!
//! Five domain errors, each carrying structured context (asof / window /
//! mode - generic harness terms). Boundary layers wrap these into richer
//! reports; domain code matches on these variants only.

use thiserror::Error;

/// Time-domain violations (leakage, non-monotonic use, window misuse).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TimeLeak {
    /// An evaluation window touched the training span (leakage by overlap).
    #[error(
        "time leak: eval window [{eval_start}, {eval_end}) overlaps training span ending at {train_end}"
    )]
    EvalOverlapsTrain {
        /// First instant of the evaluation window.
        eval_start: u64,
        /// Last instant of the evaluation window (exclusive).
        eval_end: u64,
        /// End of the training span.
        train_end: u64,
    },
    /// A wall-clock read was attempted in the monotonic domain.
    #[error("time leak: wall-clock read attempted in the monotonic domain (asof={asof})")]
    WallClockInMonotonicDomain {
        /// Where the attempt happened (free-form tag).
        asof: String,
    },
    /// A purge requirement was violated.
    #[error("time leak: purge of {required_ms}ms required, got {got_ms}ms (window {window})")]
    InsufficientPurge {
        /// Required purge in ms.
        required_ms: u64,
        /// Actual purge in ms.
        got_ms: u64,
        /// Window label.
        window: String,
    },
}

/// Risk/governance gate violations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RiskGate {
    /// Live mode was attempted without the affirmation.
    #[error("risk gate: live mode requires TRUERISE_LIVE_AFFIRM=yes (mode={mode})")]
    LiveWithoutAffirmation {
        /// The mode in which the attempt happened.
        mode: String,
    },
    /// Execution capability requested in a mode that never grants it.
    #[error("risk gate: execution capability not grantable in mode={mode}")]
    ExecutionNotGrantable {
        /// The mode that refused.
        mode: String,
    },
    /// A required approval is missing.
    #[error("risk gate: approval required for action={action}")]
    ApprovalRequired {
        /// The action needing approval.
        action: String,
    },
}

/// Data consistency violations (ledger/chain/state stores).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DataConsistency {
    /// A hash-chain row failed verification.
    #[error("data consistency: chain broken at row {row}: {reason}")]
    ChainBroken {
        /// Offending row number.
        row: usize,
        /// Why it broke.
        reason: String,
    },
    /// A monotonic counter went backwards.
    #[error("data consistency: counter {name} went backwards ({from} -> {to})")]
    CounterReversed {
        /// Counter name.
        name: String,
        /// Previous value.
        from: u64,
        /// New value.
        to: u64,
    },
    /// A fingerprint collision or mismatch.
    #[error("data consistency: fingerprint mismatch for {name}")]
    FingerprintMismatch {
        /// What mismatched.
        name: String,
    },
}

/// Mode/state machine violations (undefined transitions, terminal misuse).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ModeViolation {
    /// An undefined (state, event) pair - the table has no next state.
    #[error("mode violation: undefined transition {state} x {event}")]
    UndefinedTransition {
        /// Current state name.
        state: String,
        /// Event name.
        event: String,
    },
    /// An event fired on a terminal state (terminal states absorb).
    #[error("mode violation: terminal state {state} absorbed event {event}")]
    TerminalAbsorbed {
        /// Terminal state name.
        state: String,
        /// Event name.
        event: String,
    },
    /// Mode switch refused (frozen injection violated).
    #[error("mode violation: mode switch refused (from={from}, to={to})")]
    SwitchRefused {
        /// Source mode.
        from: String,
        /// Target mode.
        to: String,
    },
}

/// Lease fencing violations (stale writes after a new lease epoch).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum StaleFence {
    /// A write carried an outdated epoch.
    #[error("stale fence: write with epoch {write_epoch} rejected, current epoch {current_epoch}")]
    OutdatedEpoch {
        /// Epoch on the rejected write.
        write_epoch: u64,
        /// Current epoch.
        current_epoch: u64,
    },
    /// A lease expired before its write landed.
    #[error("stale fence: lease expired after {held_ms}ms (timeout {timeout_ms}ms)")]
    LeaseExpired {
        /// How long the lease was held.
        held_ms: u64,
        /// Its timeout.
        timeout_ms: u64,
    },
}

/// The unified core error: every fallible core operation returns this.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CoreError {
    /// Time-domain violation.
    #[error(transparent)]
    TimeLeak(#[from] TimeLeak),
    /// Risk/governance gate violation.
    #[error(transparent)]
    RiskGate(#[from] RiskGate),
    /// Data consistency violation.
    #[error(transparent)]
    DataConsistency(#[from] DataConsistency),
    /// Mode/state violation.
    #[error(transparent)]
    ModeViolation(#[from] ModeViolation),
    /// Stale fence violation.
    #[error(transparent)]
    StaleFence(#[from] StaleFence),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn errors_carry_structured_context() {
        let e = CoreError::from(TimeLeak::EvalOverlapsTrain {
            eval_start: 10,
            eval_end: 20,
            train_end: 15,
        });
        assert!(e.to_string().contains("[10, 20)"));
        assert!(e.to_string().contains("15"));
    }

    #[test]
    fn transparency_preserves_source_variant() {
        let e = CoreError::from(StaleFence::OutdatedEpoch {
            write_epoch: 1,
            current_epoch: 2,
        });
        match e {
            CoreError::StaleFence(StaleFence::OutdatedEpoch {
                write_epoch,
                current_epoch,
            }) => {
                assert_eq!((write_epoch, current_epoch), (1, 2));
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }
}
