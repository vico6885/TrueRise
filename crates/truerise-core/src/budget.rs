//! The budget port (W3): the loop consults it at EVERY resource point
//! (round start, every allowed tool execution). Exhaustion is a
//! fail-closed stop: an audit row lands, then the run ends with
//! [`LoopError::Budget`]. The port carries no policy - the ledger
//! (quota, shared denominator, per-hat attribution) lives in the
//! implementing crate.

/// The stop signal: budget exhausted for this hat (the run ends).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("budget exhausted for hat {hat}")]
pub struct BudgetStop {
    /// The hat whose consumption was refused.
    pub hat: String,
}

/// The consumption port. Implementations enforce their own quota /
/// denominator semantics; the loop only needs "consume or stop".
pub trait Budget: Send + Sync {
    /// Consume ``units`` against ``hat``.
    ///
    /// # Errors
    /// [`BudgetStop`] when the budget cannot absorb the units - the caller
    /// MUST stop the run (and audit the stop).
    fn consume(&self, hat: &str, units: u32) -> Result<(), BudgetStop>;
}
