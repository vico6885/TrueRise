//! Self-service verification bundle (G39): the "can you see it yourself"
//! form of a completion claim.
//!
//! A bundle names ONE sealed intent contract and one expected observation
//! per machine-checkable acceptance item. Coverage is the law here: a
//! bundle that skips a machine item is refused (the author must not choose
//! which items are easy to prove), a duplicated or unknown item is refused,
//! and the contract hash must match. Items that CANNOT be verified by the
//! bundle's observation kinds are not silent - the runner routes them into
//! the not-verified bucket, which the report prints (silence is not
//! coverage, G36's rule applied to bundles).

use serde::{Deserialize, Serialize};

use crate::intent_contract::{AcceptanceKind, IntentContractV1};

/// What one observation checks (v1 kinds are deterministic and offline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    /// A JSON artifact file must exist and carry a field equal to `expected`.
    ArtifactField,
    /// A file (any format) must exist at `target`.
    FileExists,
}

/// One expected observation bound to one acceptance item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleObservation {
    /// The acceptance item id (must exist in the sealed contract).
    pub item_id: String,
    /// What to check.
    pub kind: ObservationKind,
    /// Artifact path (for artifact_field) or file path (for file_exists).
    pub target: String,
    /// The expected value (field equality for artifact_field; unused for
    /// file_exists, where it may be empty).
    pub expected: String,
}

/// The v1 verification bundle: per-machine-item observations bound to one
/// sealed contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyBundleV1 {
    /// The sealed intent contract hash this bundle verifies.
    pub contract_sealed_hash: String,
    /// One observation per machine-checkable acceptance item.
    pub observations: Vec<BundleObservation>,
}

/// Bundle-level refusals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    /// The bundle skips a machine-checkable item (cherry-picking refused).
    MissingObservation(String),
    /// The bundle names the same item twice.
    DuplicateObservation(String),
    /// The bundle names an item that is not in the contract.
    UnknownItem(String),
    /// The bundle was built against a different contract hash.
    HashMismatch {
        /// The hash of the contract being validated against.
        expected: String,
        /// The hash the bundle was built against.
        found: String,
    },
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingObservation(id) => write!(
                f,
                "bundle has no observation for machine item {id} - coverage is not optional"
            ),
            Self::DuplicateObservation(id) => write!(f, "bundle names item {id} twice"),
            Self::UnknownItem(id) => write!(f, "bundle names unknown item {id}"),
            Self::HashMismatch { expected, found } => {
                write!(
                    f,
                    "bundle was built against contract {found}, this contract is {expected}"
                )
            }
        }
    }
}

impl std::error::Error for BundleError {}

impl VerifyBundleV1 {
    /// Validate the bundle against a SEALED contract: the hash must match
    /// and EVERY machine-checkable acceptance item must carry exactly one
    /// observation. Human-judged items are NOT bundle business - they route
    /// through adjudication tokens (G32), never through a script.
    pub fn validate_against(&self, contract: &IntentContractV1) -> Result<(), BundleError> {
        let sealed = contract
            .sealed_hash()
            .map_err(|_| BundleError::HashMismatch {
                expected: "(unsealed contract)".to_string(),
                found: self.contract_sealed_hash.clone(),
            })?;
        if self.contract_sealed_hash != sealed {
            return Err(BundleError::HashMismatch {
                expected: sealed.to_string(),
                found: self.contract_sealed_hash.clone(),
            });
        }
        let machine_ids: Vec<&str> = contract
            .acceptance_items
            .iter()
            .filter(|i| i.kind == AcceptanceKind::Machine)
            .map(|i| i.id.as_str())
            .collect();
        let mut seen: Vec<&str> = Vec::new();
        for observation in &self.observations {
            if !machine_ids.contains(&observation.item_id.as_str()) {
                return Err(BundleError::UnknownItem(observation.item_id.clone()));
            }
            if seen.contains(&observation.item_id.as_str()) {
                return Err(BundleError::DuplicateObservation(
                    observation.item_id.clone(),
                ));
            }
            seen.push(&observation.item_id);
        }
        for id in machine_ids {
            if !seen.contains(&id) {
                return Err(BundleError::MissingObservation(id.to_string()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_contract::{AcceptanceItem, IntentContractV1};

    fn sealed_contract(machine: u32, human: u32) -> IntentContractV1 {
        let mut items: Vec<AcceptanceItem> = Vec::new();
        for n in 0..machine {
            items.push(AcceptanceItem {
                id: format!("m{n}"),
                text: format!("machine item {n}"),
                kind: AcceptanceKind::Machine,
            });
        }
        for n in 0..human {
            items.push(AcceptanceItem {
                id: format!("h{n}"),
                text: format!("human item {n}"),
                kind: AcceptanceKind::Human,
            });
        }
        let mut contract = IntentContractV1 {
            contract_version: "v1".into(),
            goal: "g".into(),
            acceptance_items: items,
            non_goals: vec![],
            exclusions: vec![],
            sealed_hash: None,
        };
        contract.seal();
        contract
    }

    fn full_bundle(contract: &IntentContractV1) -> VerifyBundleV1 {
        VerifyBundleV1 {
            contract_sealed_hash: contract.sealed_hash().unwrap_or("unsealed").into(),
            observations: contract
                .acceptance_items
                .iter()
                .filter(|i| i.kind == AcceptanceKind::Machine)
                .map(|i| BundleObservation {
                    item_id: i.id.clone(),
                    kind: ObservationKind::FileExists,
                    target: format!("{}.txt", i.id),
                    expected: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn bundle_expected_observations_sealed_from_contract_items() {
        let contract = sealed_contract(3, 1);
        let bundle = full_bundle(&contract);
        assert!(bundle.validate_against(&contract).is_ok());
    }

    #[test]
    fn bundle_skipping_a_machine_item_is_refused() {
        let contract = sealed_contract(3, 1);
        let mut bundle = full_bundle(&contract);
        bundle.observations.pop();
        assert_eq!(
            bundle.validate_against(&contract),
            Err(BundleError::MissingObservation("m2".into()))
        );
    }

    #[test]
    fn bundle_human_items_are_never_observations() {
        let contract = sealed_contract(3, 1);
        let mut bundle = full_bundle(&contract);
        bundle.observations.push(BundleObservation {
            item_id: "h0".into(),
            kind: ObservationKind::FileExists,
            target: "h0.txt".into(),
            expected: String::new(),
        });
        assert_eq!(
            bundle.validate_against(&contract),
            Err(BundleError::UnknownItem("h0".into())),
            "a script must never adjudicate a human item"
        );
    }

    #[test]
    fn bundle_bound_to_another_contract_is_refused() {
        let contract = sealed_contract(3, 1);
        let bundle = VerifyBundleV1 {
            contract_sealed_hash: "deadbeef".into(),
            observations: full_bundle(&contract).observations,
        };
        assert!(matches!(
            bundle.validate_against(&contract),
            Err(BundleError::HashMismatch { .. })
        ));
    }

    #[test]
    fn bundle_duplicate_observation_is_refused() {
        let contract = sealed_contract(3, 1);
        let mut bundle = full_bundle(&contract);
        let first = bundle.observations[0].clone();
        bundle.observations.push(first);
        assert_eq!(
            bundle.validate_against(&contract),
            Err(BundleError::DuplicateObservation("m0".into()))
        );
    }
}
