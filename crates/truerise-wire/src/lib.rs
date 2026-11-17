//! truerise-wire: versioned wire contracts (serde).
//!
//! Spec (§6 wire row): the legacy v1 and tightened v2 editions are BOTH kept
//! explicit at the type level - merging them into one type is forbidden
//! ("禁抹平"). v1 stays readable but is marked deprecated and never
//! participates in governance decisions (mirrors the ledger's `governs()`).

pub mod audit_event;
pub mod bundle;
pub mod completion;
pub mod intent_contract;
pub use audit_event::{AuditEventV1, AuditEventV2, Severity};
pub use bundle::{BundleError, BundleObservation, ObservationKind, VerifyBundleV1};
pub use completion::{CompletionEntry, CompletionReportV1, CompletionStatus, NotDoneReason};
pub use intent_contract::{
    AcceptanceItem, AcceptanceKind, Adjudication, AdjudicationDecision, ContractError,
    IntentContractV1, IntentToken, MACHINE_CHECK_FLOOR,
};
