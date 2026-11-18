//! Intent contract (G32): the object that carries what the user actually
//! wants, sealed before any run may cite it as a completion basis.
//!
//! Before G32 the `run --goal` string was the whole intent surface - it fed
//! plan queries and memory recall and nothing else (no per-item evidence
//! mapping was possible). This module upgrades the goal into a structured,
//! sealable contract:
//!
//! - every acceptance item is EXPLICITLY `machine` or `human` (no default
//!   kind: a missing kind is a parse error, not an assumption);
//! - the contract seals with a SHA-256 over its canonical JSON (sorted
//!   keys, sealed_hash excluded) - any post-hoc edit is a different contract
//!   (intent change mid-run starts a NEW contract, never silently extends);
//! - the user confirmation token is a PER-ITEM adjudication list - a blanket
//!   confirmation is structurally impossible, and a token that skips one
//!   item is refused;
//! - a contract with fewer than [`MACHINE_CHECK_FLOOR`] machine-checkable
//!   items cannot certify completion (the completion report refuses it).
//!
//! Honest boundary (docs/limits.md): the contract enforces that acceptance
//! items exist and are adjudicated; it does NOT judge whether the items
//! measure what the user truly wants (construct validity is grade C).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The minimum number of machine-checkable acceptance items a contract must
/// carry before any completion claim may cite it (Vico ruling, N=3).
pub const MACHINE_CHECK_FLOOR: u32 = 3;

/// How an acceptance item may be verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceKind {
    /// Verifiable by a deterministic command or artifact field.
    Machine,
    /// Verifiable only by an explicit human token (never machine).
    Human,
}

/// One acceptance item of the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceItem {
    /// Stable id referenced by adjudication tokens and completion reports.
    pub id: String,
    /// What "done" means for this item.
    pub text: String,
    /// Explicit verification kind - a missing kind is a parse error.
    pub kind: AcceptanceKind,
}

/// The v1 intent contract (G32). Sealed before first use; the seal hash
/// covers every field except `sealed_hash` itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IntentContractV1 {
    /// Wire version; only `v1` exists.
    #[serde(default = "default_version")]
    pub contract_version: String,
    /// The goal sentence (what the user wants, verbatim).
    pub goal: String,
    /// Acceptance items - the finish line, item by item.
    pub acceptance_items: Vec<AcceptanceItem>,
    /// Explicit non-goals (silence here is not scope freedom).
    #[serde(default)]
    pub non_goals: Vec<String>,
    /// Explicit exclusions the user carved out.
    #[serde(default)]
    pub exclusions: Vec<String>,
    /// The seal (SHA-256 hex over the canonical JSON of the bare contract).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_hash: Option<String>,
}

fn default_version() -> String {
    "v1".to_string()
}

/// Contract-level refusals (fail-closed; every message names the fix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// The goal sentence is empty.
    EmptyGoal,
    /// The wire version is not v1 (the schema enum is law on both sides).
    UnsupportedVersion(String),
    /// No acceptance items - the contract cannot anchor anything.
    NoAcceptanceItems,
    /// An item id is empty.
    EmptyItemId,
    /// An item text is empty.
    EmptyItemText,
    /// Two items share an id (completion mapping would be ambiguous).
    DuplicateItemId(String),
    /// The contract was used before it was sealed.
    NotSealed,
    /// The presented seal does not match the recomputed one.
    SealMismatch {
        /// The seal carried by the object under test.
        expected: String,
        /// The seal recomputed from the payload.
        found: String,
    },
    /// The token carries no decisions at all (blanket confirmations refused).
    TokenEmpty,
    /// The token skips an item - partial adjudication is not confirmation.
    TokenMissingDecision(String),
    /// The token adjudicates the same item twice.
    TokenDuplicateDecision(String),
    /// The token adjudicates an id that is not in the contract.
    TokenUnknownDecision(String),
    /// The token was sealed against a different contract hash.
    TokenHashMismatch {
        /// The hash of the contract being validated against.
        expected: String,
        /// The hash the token was sealed against.
        found: String,
    },
    /// Fewer machine-checkable items than [`MACHINE_CHECK_FLOOR`].
    MachineFloor {
        /// The contract's machine-checkable item count.
        found: u32,
        /// The required floor ([`MACHINE_CHECK_FLOOR`]).
        minimum: u32,
    },
    /// The completion report skips an item (silence is not coverage).
    CompletionMissingItem(String),
    /// The completion report names an item twice.
    CompletionDuplicateItem(String),
    /// The completion report names an item outside the contract.
    CompletionUnknownItem(String),
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGoal => write!(f, "goal is empty - state what the user wants"),
            Self::UnsupportedVersion(v) => {
                write!(
                    f,
                    "contract_version {v} is not v1 - only the v1 wire exists"
                )
            }
            Self::NoAcceptanceItems => {
                write!(
                    f,
                    "contract has no acceptance items - ask the user, do not assume"
                )
            }
            Self::EmptyItemId => write!(f, "acceptance item id is empty"),
            Self::EmptyItemText => write!(f, "acceptance item text is empty"),
            Self::DuplicateItemId(id) => write!(f, "duplicate acceptance item id: {id}"),
            Self::NotSealed => write!(f, "contract used before sealing - seal it first"),
            Self::SealMismatch { expected, found } => {
                write!(f, "seal mismatch: expected {expected}, recomputed {found}")
            }
            Self::TokenEmpty => {
                write!(f, "confirmation token is empty - adjudicate every item")
            }
            Self::TokenMissingDecision(id) => {
                write!(
                    f,
                    "confirmation token skips item {id} - per-item adjudication required"
                )
            }
            Self::TokenDuplicateDecision(id) => write!(f, "token adjudicates {id} twice"),
            Self::TokenUnknownDecision(id) => write!(f, "token adjudicates unknown item {id}"),
            Self::TokenHashMismatch { expected, found } => {
                write!(
                    f,
                    "token was sealed against contract {found}, this contract is {expected}"
                )
            }
            Self::MachineFloor { found, minimum } => write!(
                f,
                "only {found} machine-checkable items (floor {minimum}) - this contract cannot certify completion"
            ),
            Self::CompletionMissingItem(id) => {
                write!(
                    f,
                    "completion report is silent about item {id} - silence is not coverage"
                )
            }
            Self::CompletionDuplicateItem(id) => write!(f, "completion report names {id} twice"),
            Self::CompletionUnknownItem(id) => {
                write!(f, "completion report names unknown item {id}")
            }
        }
    }
}

impl std::error::Error for ContractError {}

impl IntentContractV1 {
    /// Structural validation (fail-closed before sealing).
    pub fn validate(&self) -> Result<(), ContractError> {
        // Review fix (P2): the schema pins contract_version to "v1" - the
        // Rust side used to accept "v99" silently (a cross-implementation
        // inconsistency in a dual-impl project).
        if self.contract_version != "v1" {
            return Err(ContractError::UnsupportedVersion(
                self.contract_version.clone(),
            ));
        }
        if self.goal.trim().is_empty() {
            return Err(ContractError::EmptyGoal);
        }
        if self.acceptance_items.is_empty() {
            return Err(ContractError::NoAcceptanceItems);
        }
        let mut seen: Vec<&str> = Vec::new();
        for item in &self.acceptance_items {
            if item.id.trim().is_empty() {
                return Err(ContractError::EmptyItemId);
            }
            if item.text.trim().is_empty() {
                return Err(ContractError::EmptyItemText);
            }
            if seen.contains(&item.id.as_str()) {
                return Err(ContractError::DuplicateItemId(item.id.clone()));
            }
            seen.push(&item.id);
        }
        Ok(())
    }

    /// How many items are machine-checkable.
    #[must_use]
    pub fn machine_item_count(&self) -> u32 {
        self.acceptance_items
            .iter()
            .filter(|i| i.kind == AcceptanceKind::Machine)
            .count() as u32
    }

    /// The canonical JSON of the bare contract (sorted keys, no seal).
    #[must_use]
    pub fn signing_payload(&self) -> String {
        let mut bare = self.clone();
        bare.sealed_hash = None;
        match serde_json::to_value(&bare) {
            Ok(value) => serde_json::to_string(&value).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    /// Seal the contract: compute and store the SHA-256 over the canonical
    /// JSON of the bare contract. Idempotent - resealing an unchanged
    /// contract yields the same hash.
    pub fn seal(&mut self) {
        let mut hasher = Sha256::new();
        hasher.update(self.signing_payload().as_bytes());
        let digest = hasher.finalize();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        self.sealed_hash = Some(hex);
    }

    /// The seal, refusing to hand out an unsealed contract.
    pub fn sealed_hash(&self) -> Result<&str, ContractError> {
        self.sealed_hash.as_deref().ok_or(ContractError::NotSealed)
    }
}

/// One per-item decision of the confirmation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjudicationDecision {
    /// The user accepts the item as written.
    Accept,
    /// The user rejects the item (the contract must be redrafted).
    Reject,
    /// The user edits the item (the contract must be resealed).
    Edit,
}

/// One adjudication row: which item, which decision, optional note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adjudication {
    /// The acceptance item id this decision applies to.
    pub item_id: String,
    /// The user's decision.
    pub decision: AdjudicationDecision,
    /// Optional free-text note from the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The user's confirmation token: a PER-ITEM adjudication list bound to one
/// sealed contract hash. A blanket confirmation cannot be expressed - the
/// token is a list, and a list that skips any item is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentToken {
    /// The sealed contract hash this token adjudicates.
    pub contract_sealed_hash: String,
    /// One decision per acceptance item (no skips, no duplicates).
    pub decisions: Vec<Adjudication>,
}

impl IntentToken {
    /// Validate the token against a SEALED contract.
    pub fn validate_against(&self, contract: &IntentContractV1) -> Result<(), ContractError> {
        let sealed = contract.sealed_hash()?;
        contract.validate()?;
        if self.contract_sealed_hash != sealed {
            return Err(ContractError::TokenHashMismatch {
                expected: sealed.to_string(),
                found: self.contract_sealed_hash.clone(),
            });
        }
        if self.decisions.is_empty() {
            return Err(ContractError::TokenEmpty);
        }
        let mut seen: Vec<&str> = Vec::new();
        for decision in &self.decisions {
            if !contract
                .acceptance_items
                .iter()
                .any(|item| item.id == decision.item_id)
            {
                return Err(ContractError::TokenUnknownDecision(
                    decision.item_id.clone(),
                ));
            }
            if seen.contains(&decision.item_id.as_str()) {
                return Err(ContractError::TokenDuplicateDecision(
                    decision.item_id.clone(),
                ));
            }
            seen.push(&decision.item_id);
        }
        for item in &contract.acceptance_items {
            if !seen.contains(&item.id.as_str()) {
                return Err(ContractError::TokenMissingDecision(item.id.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: AcceptanceKind) -> AcceptanceItem {
        AcceptanceItem {
            id: id.to_string(),
            text: format!("item {id} is verifiably done"),
            kind,
        }
    }

    fn sealed_contract(machine: u32, human: u32) -> IntentContractV1 {
        let mut items: Vec<AcceptanceItem> = Vec::new();
        for n in 0..machine {
            items.push(item(&format!("m{n}"), AcceptanceKind::Machine));
        }
        for n in 0..human {
            items.push(item(&format!("h{n}"), AcceptanceKind::Human));
        }
        let mut contract = IntentContractV1 {
            contract_version: "v1".into(),
            goal: "ship the honest ledger".into(),
            acceptance_items: items,
            non_goals: vec!["no extra tooling".into()],
            exclusions: vec![],
            sealed_hash: None,
        };
        contract.seal();
        contract
    }

    fn accept_all(contract: &IntentContractV1) -> IntentToken {
        IntentToken {
            contract_sealed_hash: contract.sealed_hash().unwrap_or("unsealed").into(),
            decisions: contract
                .acceptance_items
                .iter()
                .map(|i| Adjudication {
                    item_id: i.id.clone(),
                    decision: AdjudicationDecision::Accept,
                    note: None,
                })
                .collect(),
        }
    }

    #[test]
    fn sealed_hash_detects_post_hoc_edit() {
        let mut contract = sealed_contract(3, 0);
        let first = contract.sealed_hash().unwrap_or("unsealed").to_string();
        contract.goal = "a different goal entirely".into();
        contract.seal();
        let second = contract.sealed_hash().unwrap_or("unsealed").to_string();
        assert_ne!(
            first, second,
            "any intent change must start a NEW contract, never extend the old one"
        );
    }

    #[test]
    fn intent_change_midrun_starts_new_contract() {
        // The named scenario: the same items in a different ORDER are a
        // different contract too (the seal covers the item list, order
        // included) - there is no in-place contract mutation.
        let mut a = sealed_contract(3, 0);
        let before = a.sealed_hash().unwrap_or("unsealed").to_string();
        a.acceptance_items.swap(0, 1);
        a.seal();
        let after = a.sealed_hash().unwrap_or("unsealed").to_string();
        assert_ne!(before, after);
    }

    #[test]
    fn contract_confirmation_requires_per_item_adjudication() {
        let contract = sealed_contract(2, 1);
        // A token that skips one item is refused (the blanket case cannot
        // even be built: the token is a list, and every list shorter than
        // the item set lands here).
        let mut token = accept_all(&contract);
        token.decisions.pop();
        assert_eq!(
            token.validate_against(&contract),
            Err(ContractError::TokenMissingDecision("h0".into()))
        );
        // An empty token is refused.
        let empty = IntentToken {
            contract_sealed_hash: contract.sealed_hash().unwrap_or("unsealed").into(),
            decisions: vec![],
        };
        assert_eq!(
            empty.validate_against(&contract),
            Err(ContractError::TokenEmpty)
        );
        // A token bound to a different contract hash is refused. The hash
        // must be foreign - not just another seal of the SAME content, for
        // the seal is deterministic and identical contracts share a hash.
        let mut cross = accept_all(&contract);
        cross.contract_sealed_hash = "deadbeef".into();
        assert!(matches!(
            cross.validate_against(&contract),
            Err(ContractError::TokenHashMismatch { .. })
        ));
    }

    #[test]
    fn missing_acceptance_item_forces_question_not_assumption() {
        let contract = IntentContractV1 {
            contract_version: "v1".into(),
            goal: "just do it".into(),
            acceptance_items: vec![],
            non_goals: vec![],
            exclusions: vec![],
            sealed_hash: None,
        };
        assert_eq!(
            contract.validate(),
            Err(ContractError::NoAcceptanceItems),
            "no items means ASK, never assume"
        );
    }

    #[test]
    fn duplicate_item_ids_are_refused() {
        let mut contract = IntentContractV1 {
            contract_version: "v1".into(),
            goal: "g".into(),
            acceptance_items: vec![
                item("m0", AcceptanceKind::Machine),
                item("m0", AcceptanceKind::Human),
            ],
            non_goals: vec![],
            exclusions: vec![],
            sealed_hash: None,
        };
        contract.seal();
        assert_eq!(
            contract.validate(),
            Err(ContractError::DuplicateItemId("m0".into()))
        );
    }

    #[test]
    fn human_judged_item_is_explicit_not_default() {
        let text = r#"{"id":"x","text":"t"}"#;
        let parsed: Result<AcceptanceItem, _> = serde_json::from_str(text);
        assert!(
            parsed.is_err(),
            "a missing kind must be a parse error, not a silent default"
        );
    }
}
