//! truerise-governance: audit hash chain (singleton semantics), three-state
//! policy, fenced approval queue, and the resource-policy port.
//!
//! Spec (§6): the chain is the single authority with zero exemptions;
//! approvals never expire into consent; deletion flow is generic (three
//! gates), policy pluggable.

pub mod approval;
pub mod chain;
pub mod policy;
pub mod resource_policy;
pub mod write_gate;

pub use approval::{ApprovalQueue, RequestState};
pub use chain::{ChainError, GENESIS, HashChain, Row};
pub use policy::{Decision, PolicyState, PolicyTable};
pub use resource_policy::{
    DeletionDecision, DeletionGate, DeletionTarget, ResourcePolicy, StrictPolicy,
};
pub use write_gate::{
    SupportRef, UnverifiedAssumption, UnverifiedLedger, WriteDecision, WriteGate, WriteGateError,
    WritePath,
};

// Governance errors re-exported from the core taxonomy (chain adds its own).
pub use truerise_core::{CoreError, RiskGate, StaleFence};
