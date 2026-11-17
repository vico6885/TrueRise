//! Completion report (G35/G36): the three-way ledger that makes silence
//! structurally impossible in a completion claim.
//!
//! For EVERY acceptance item of the sealed contract the report must carry
//! exactly one entry - `done` (with an evidence reference), `not_done` (with
//! a reason from a closed taxonomy), or `not_verified` (with the why and the
//! cost of checking it). There is no fourth state: a report that skips an
//! item is refused ("silence is not coverage"), and a report for a contract
//! with fewer than [`MACHINE_CHECK_FLOOR`] machine-checkable items is
//! refused outright (it cannot certify completion).
//!
//! Honest boundary: this object certifies that every item was EXPLICITLY
//! dispositioned - it does not judge whether the evidence is good (G1/G26
//! grade that separately).

use serde::{Deserialize, Serialize};

use crate::intent_contract::{ContractError, IntentContractV1, MACHINE_CHECK_FLOOR};

/// Why an acceptance item was deliberately left undone (closed taxonomy -
/// free-text reasons are not auditable across generations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotDoneReason {
    /// The measurement criterion is still undecided.
    ScopeUndecided,
    /// Doing it properly needs a refactor first.
    NeedsRefactor,
    /// The triggering condition never occurred this run.
    ConditionUnmet,
    /// A deliberate design tradeoff.
    DesignTradeoff,
    /// Cost-bound: not worth the budget now (the cost goes into the entry).
    Cost,
}

/// The disposition of ONE acceptance item. Every variant carries what a
/// reader needs to re-derive the claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CompletionStatus {
    /// Done - with a reference to the supporting evidence (a recomputed
    /// artifact field, a signed action, or a human token).
    Done {
        /// Where the evidence lives (artifact path + field, or token id).
        evidence: String,
    },
    /// Deliberately not done - with the reason category and a note.
    NotDone {
        /// Why it was left undone (closed taxonomy).
        reason: NotDoneReason,
        /// One line on what would change the decision.
        note: String,
    },
    /// Not verified this run - silence would be a lie, so this state exists.
    NotVerified {
        /// What was not checked.
        why: String,
        /// What it would cost to check it.
        cost_note: String,
    },
}

/// One ledger row: which item, which disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionEntry {
    /// The acceptance item id (must exist in the sealed contract).
    pub item_id: String,
    /// The disposition of that item.
    pub status: CompletionStatus,
}

/// The v1 completion report: a per-item three-way ledger bound to one
/// sealed contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionReportV1 {
    /// The sealed contract hash this report dispositions.
    pub contract_sealed_hash: String,
    /// Exactly one entry per acceptance item (no skips, no duplicates).
    pub entries: Vec<CompletionEntry>,
}

impl CompletionReportV1 {
    /// Validate the report against a SEALED contract: hash must match and
    /// every item must be dispositioned exactly once. A contract under the
    /// machine-check floor refuses completion claims entirely.
    pub fn validate_against(&self, contract: &IntentContractV1) -> Result<(), ContractError> {
        let sealed = contract.sealed_hash()?;
        contract.validate()?;
        if self.contract_sealed_hash != sealed {
            return Err(ContractError::TokenHashMismatch {
                expected: sealed.to_string(),
                found: self.contract_sealed_hash.clone(),
            });
        }
        let machine = contract.machine_item_count();
        if machine < MACHINE_CHECK_FLOOR {
            return Err(ContractError::MachineFloor {
                found: machine,
                minimum: MACHINE_CHECK_FLOOR,
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !contract
                .acceptance_items
                .iter()
                .any(|item| item.id == entry.item_id)
            {
                return Err(ContractError::CompletionUnknownItem(entry.item_id.clone()));
            }
            if seen.contains(&entry.item_id.as_str()) {
                return Err(ContractError::CompletionDuplicateItem(entry.item_id.clone()));
            }
            seen.push(&entry.item_id);
        }
        for item in &contract.acceptance_items {
            if !seen.contains(&item.id.as_str()) {
                return Err(ContractError::CompletionMissingItem(item.id.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent_contract::{AcceptanceItem, AcceptanceKind, IntentContractV1};

    fn item(id: &str, kind: AcceptanceKind) -> AcceptanceItem {
        AcceptanceItem {
            id: id.to_string(),
            text: format!("item {id} verifiably done"),
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
            goal: "ship it".into(),
            acceptance_items: items,
            non_goals: vec![],
            exclusions: vec![],
            sealed_hash: None,
        };
        contract.seal();
        contract
    }

    fn full_report(contract: &IntentContractV1) -> CompletionReportV1 {
        CompletionReportV1 {
            contract_sealed_hash: contract.sealed_hash().unwrap_or("unsealed").into(),
            entries: contract
                .acceptance_items
                .iter()
                .map(|i| CompletionEntry {
                    item_id: i.id.clone(),
                    status: if i.kind == AcceptanceKind::Machine {
                        CompletionStatus::Done {
                            evidence: "artifacts/run.jsonl#row.42".into(),
                        }
                    } else {
                        CompletionStatus::NotVerified {
                            why: "needs a human eye on the rendered chart".into(),
                            cost_note: "one manual review pass".into(),
                        }
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn every_acceptance_item_is_done_or_not_done_or_not_verified() {
        let contract = sealed_contract(3, 1);
        let report = full_report(&contract);
        assert!(
            report.validate_against(&contract).is_ok(),
            "a complete three-way ledger validates"
        );
    }

    #[test]
    fn silence_is_not_coverage() {
        let contract = sealed_contract(3, 1);
        let mut report = full_report(&contract);
        report.entries.pop();
        let dropped = contract
            .acceptance_items
            .last()
            .map(|i| i.id.clone())
            .unwrap_or_default();
        assert_eq!(
            report.validate_against(&contract),
            Err(ContractError::CompletionMissingItem(dropped)),
            "a skipped item is a refusal, not an implicit pass"
        );
    }

    #[test]
    fn certificate_without_gap_section_is_refused() {
        // A completion payload without the entries ledger (the gap section)
        // must not parse at all - there is no default that reads as "all done".
        let text = r#"{"contract_sealed_hash":"abc"}"#;
        let parsed: Result<CompletionReportV1, _> = serde_json::from_str(text);
        assert!(parsed.is_err(), "missing entries array is a parse error");
        // Same refusal when the array key carries the wrong shape.
        let text = r#"{"contract_sealed_hash":"abc","entries":"all good"}"#;
        let parsed: Result<CompletionReportV1, _> = serde_json::from_str(text);
        assert!(parsed.is_err());
    }

    #[test]
    fn not_done_reason_taxonomy_enforced() {
        let text = r#"{"status":"not_done","reason":"did not feel like it","note":""}"#;
        let parsed: Result<CompletionStatus, _> = serde_json::from_str(text);
        assert!(
            parsed.is_err(),
            "reasons outside the closed taxonomy are refused"
        );
        let text = r#"{"status":"not_done","reason":"cost","note":"budget exhausted"}"#;
        let parsed: Result<CompletionStatus, _> = serde_json::from_str(text);
        assert!(parsed.is_ok(), "taxonomy members parse");
    }

    #[test]
    fn completion_claim_requires_per_item_evidence() {
        let text = r#"{"status":"done"}"#;
        let parsed: Result<CompletionStatus, _> = serde_json::from_str(text);
        assert!(
            parsed.is_err(),
            "a done claim without an evidence reference does not parse"
        );
    }

    #[test]
    fn contract_with_fewer_than_three_machine_checks_cannot_certify_completion() {
        // The lock test for the N=3 ruling (Vico 2026-11-16): two machine
        // items + nine human items still refuses a completion claim.
        let contract = sealed_contract(2, 9);
        let report = full_report(&contract);
        assert_eq!(
            report.validate_against(&contract),
            Err(ContractError::MachineFloor {
                found: 2,
                minimum: 3
            })
        );
    }

    #[test]
    fn not_verified_carries_the_cost_of_checking() {
        let text = r#"{"status":"not_verified","why":"x"}"#;
        let parsed: Result<CompletionStatus, _> = serde_json::from_str(text);
        assert!(
            parsed.is_err(),
            "an unverified entry without the cost note does not parse - the reader needs it"
        );
    }
}
