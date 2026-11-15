//! truerise-rsi: recursive self-improvement support - the single trial-budget
//! ledger (singleton), skill schema + loader, and the evolve flow that NEVER
//! auto-merges (proposals + MR reports only; merging is human).

pub mod apply;
pub mod budget;
pub mod evolve;
pub mod skill;

pub use apply::{AppliedGeneration, ApplyError, apply_proposal, content_hash, rollback};
pub use budget::{BudgetError, BudgetLedger, RunBudget};
pub use evolve::{EvolveError, Proposal, evolve_propose, proposal_dir};
pub use skill::{SkillError, SkillPackage, load_skill};

// serde is used by Proposal/SkillPackage derives.
#[allow(unused_imports)]
use serde as _serde;
