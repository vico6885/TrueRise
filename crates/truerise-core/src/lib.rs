//! truerise-core: the cognitive core.
//!
//! M1 scope (spec §7 M1 row): modes / errors / state machine / timecore.
//! - [`errors`]: the five-variant domain error tree with structured context.
//! - [`mode`]: the four run modes, injected and frozen; research never
//!   grants execution; live requires the affirmation.
//! - [`timecore`]: half-open slices, fold windows with enforced purge,
//!   overlap detection (LeakGuard vectors 1-2 foundations).
//! - [`state_machine`]: typestate + table-driven run machine (§6.7.1).

pub mod compaction;
pub mod errors;
pub mod loop_;
pub mod mode;
pub mod router;
pub mod state_machine;
pub mod timecore;
pub mod tool;

pub use compaction::{CompactionError, Entry, Level, PreCompactEvent, compact, total_tokens};
pub use errors::{CoreError, DataConsistency, ModeViolation, RiskGate, StaleFence, TimeLeak};
pub use loop_::{AuditSink, Loop, LoopConfig, LoopError, RunBooks, ToolHooks};
pub use mode::{ExecutionGrant, Mode, ModeSet};
pub use router::{
    ModelRouter, RetryPolicy, RouterError, TurnRequest, TurnResponse, backoff_ms, check_pinned,
};
pub use state_machine::{Event, Machine, State, TransitionLog, transition};
pub use timecore::{FoldWindow, TimeSlice, any_overlap};
pub use tool::{
    ActionPolicy, GateDecision, ToolError, ToolOutcome, ToolProvider, ToolRequest, parse_tool_call,
};
