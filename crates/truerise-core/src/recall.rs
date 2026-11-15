//! The context-recall port (W6): memory participates in plan-context
//! construction through THIS port - the loop never sees a backend, and the
//! recall evidence lands on the chain (what was injected, and its scores as
//! per-mille integers - the float rule).

/// What a recall injected into the plan context (ids + scores, no floats).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallEvidence {
    /// ids of the injected records (in injection order).
    pub ids: Vec<String>,
    /// fusion scores as per-mille integers (0..=1000+), same order as ids.
    pub scores_per_mille: Vec<i64>,
    /// The context text actually prepended to the plan query.
    pub context: String,
}

/// The port the loop consults before each plan turn. `None` = nothing to
/// inject (the attempt is still audited by the loop).
pub trait ContextRecall: Send + Sync {
    /// Recall context for ``query``.
    fn recall_for(&self, query: &str) -> Option<RecallEvidence>;
}
