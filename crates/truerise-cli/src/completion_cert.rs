//! G35/G36 certificate integration: an intent contract bound to a
//! generation certificate MUST carry a closing completion ledger.
//!
//! The rule (RFC 0003 extension): when a generation is issued under a
//! sealed intent contract, the certificate embeds a `completion` section
//! whose report closes against THAT contract - every acceptance item is
//! dispositioned exactly once (done / not_done / not_verified), the report
//! hash matches the contract seal, and the machine-check floor holds. A
//! certificate that would cite a contract without a complete ledger is
//! REFUSED, not downgraded (silence is not coverage, at certificate level).

use std::path::PathBuf;

use serde_json::json;
use truerise_wire::CompletionReportV1;
use truerise_wire::IntentContractV1;

/// The binding between a generation run and its intent contract:
/// `--intent <contract.toml>` plus the (required) `--completion <report.json>`.
pub struct CompletionBinding {
    /// The intent contract file.
    pub intent_path: PathBuf,
    /// The completion report file (required whenever intent is given).
    pub report_path: Option<PathBuf>,
    /// Review fix (P0): the per-item adjudication token. Human-judged items
    /// marked `done` REQUIRE an Accept decision here; a token carrying any
    /// Reject/Edit refuses the whole binding (G32 red line).
    pub token_path: Option<PathBuf>,
}

/// Parse `--intent <toml>` / `--completion <json>` out of evolve args.
/// `Err` = usage error (exit 2 at the CLI boundary).
pub fn parse_completion_args(args: &[String]) -> Result<Option<CompletionBinding>, String> {
    let mut intent: Option<String> = None;
    let mut report: Option<String> = None;
    let mut token: Option<String> = None;
    let mut idx = 0;
    while idx < args.len() {
        let arg = args[idx].as_str();
        if arg == "--intent" {
            match args.get(idx + 1) {
                Some(p) if !p.starts_with("--") => {
                    if intent.is_some() {
                        return Err("--intent given twice".to_string());
                    }
                    intent = Some(p.clone());
                    idx += 2;
                }
                _ => return Err("--intent needs a path to a contract TOML".to_string()),
            }
        } else if arg == "--intent-token" {
            match args.get(idx + 1) {
                Some(p) if !p.starts_with("--") => {
                    if token.is_some() {
                        return Err("--intent-token given twice".to_string());
                    }
                    token = Some(p.clone());
                    idx += 2;
                }
                _ => return Err("--intent-token needs a path to a token JSON".to_string()),
            }
        } else if arg == "--completion" {
            match args.get(idx + 1) {
                Some(p) if !p.starts_with("--") => {
                    if report.is_some() {
                        return Err("--completion given twice".to_string());
                    }
                    report = Some(p.clone());
                    idx += 2;
                }
                _ => return Err("--completion needs a path to a report JSON".to_string()),
            }
        } else {
            idx += 1;
        }
    }
    match intent {
        Some(intent_path) => {
            // Review fix (P2): a missing --completion is a USAGE error
            // (exit 2), not a run failure (exit 1) - the old code conflated
            // "user forgot the report" with "the ledger refused".
            let Some(report_path) = report else {
                return Err("--intent needs --completion <report.json>".to_string());
            };
            Ok(Some(CompletionBinding {
                intent_path: PathBuf::from(intent_path),
                report_path: Some(PathBuf::from(report_path)),
                token_path: token.map(PathBuf::from),
            }))
        }
        None => {
            if report.is_some() {
                Err("--completion needs --intent <contract.toml>".to_string())
            } else {
                Ok(None)
            }
        }
    }
}

/// Validate the binding and build the certificate's `completion` section:
/// load the contract, validate, SEAL it, parse the report, and close the
/// report against the contract. `Err` = the certificate must NOT be issued.
pub fn validate_binding(binding: &CompletionBinding) -> Result<serde_json::Value, String> {
    let contract_text = std::fs::read_to_string(&binding.intent_path).map_err(|e| {
        format!(
            "cannot read intent contract {}: {e}",
            binding.intent_path.display()
        )
    })?;
    let mut contract: IntentContractV1 = toml::from_str(&contract_text)
        .map_err(|e| format!("intent contract is not valid v1 TOML: {e}"))?;
    contract
        .validate()
        .map_err(|e| format!("intent contract refused: {e}"))?;
    contract.seal();
    let seal = contract
        .sealed_hash()
        .map_err(|e| format!("intent contract refused: {e}"))?
        .to_string();
    let Some(report_path) = binding.report_path.as_ref() else {
        return Err(
            "a sealed intent contract requires a completion ledger - a certificate citing a \
             contract without a three-way ledger is refused (silence is not coverage)"
                .to_string(),
        );
    };
    let report_text = std::fs::read_to_string(report_path).map_err(|e| {
        format!(
            "cannot read completion report {}: {e}",
            report_path.display()
        )
    })?;
    let report: CompletionReportV1 = serde_json::from_str(&report_text)
        .map_err(|e| format!("completion report is not valid v1 JSON: {e}"))?;
    report
        .validate_against(&contract)
        .map_err(|e| format!("completion ledger refused: {e}"))?;
    // G32 red line (review P0): the adjudication token gates HUMAN items.
    let accepted_human: Vec<String> = match binding.token_path.as_ref() {
        Some(token_path) => {
            let token_text = std::fs::read_to_string(token_path)
                .map_err(|e| format!("cannot read intent token {}: {e}", token_path.display()))?;
            let token: truerise_wire::IntentToken = serde_json::from_str(&token_text)
                .map_err(|e| format!("intent token is not valid JSON: {e}"))?;
            token
                .validate_against(&contract)
                .map_err(|e| format!("intent token refused: {e}"))?;
            if token
                .decisions
                .iter()
                .any(|d| d.decision != truerise_wire::AdjudicationDecision::Accept)
            {
                return Err(
                    "a contract with reject/edit decisions in its token cannot certify completion - redraft and reseal"
                        .to_string(),
                );
            }
            token.decisions.iter().map(|d| d.item_id.clone()).collect()
        }
        None => Vec::new(),
    };
    for item in &contract.acceptance_items {
        if item.kind != truerise_wire::AcceptanceKind::Human {
            continue;
        }
        let Some(entry) = report.entries.iter().find(|e| e.item_id == item.id) else {
            continue; // closure already refused missing entries
        };
        if let truerise_wire::CompletionStatus::Done { .. } = entry.status {
            if !accepted_human.iter().any(|acc| acc == &item.id) {
                return Err(format!(
                    "human-judged item {} is marked done WITHOUT a user token - a human item is done+token or not_verified, never silently done",
                    item.id
                ));
            }
        }
    }
    let items: Vec<serde_json::Value> = contract
        .acceptance_items
        .iter()
        .map(|i| {
            json!({
                "id": i.id,
                "kind": match i.kind {
                    truerise_wire::AcceptanceKind::Machine => "machine",
                    truerise_wire::AcceptanceKind::Human => "human",
                }
            })
        })
        .collect();
    Ok(json!({
        "contract_sealed_hash": seal,
        "machine_items": contract.machine_item_count(),
        "items_total": contract.acceptance_items.len(),
        "items": items,
        "report": report,
    }))
}

/// Replay-side recomputation (offline, no contract file): the embedded
/// report must be hash-consistent and close against the embedded item list.
/// `Err` = the completion section does not recompute.
pub fn replay_completion(completion: &serde_json::Value) -> Result<(), String> {
    let seal = completion["contract_sealed_hash"]
        .as_str()
        .ok_or("completion section carries no contract seal")?;
    let report: CompletionReportV1 = serde_json::from_value(completion["report"].clone())
        .map_err(|e| format!("embedded report unparsable: {e}"))?;
    if report.contract_sealed_hash != seal {
        return Err(format!(
            "report cites {}, the certificate binds {}",
            report.contract_sealed_hash, seal
        ));
    }
    let items = completion["items"]
        .as_array()
        .ok_or("completion section carries no item list")?;
    let machine_floor = truerise_wire::MACHINE_CHECK_FLOOR;
    let mut machine = 0u32;
    let mut seen: Vec<&str> = Vec::new();
    for item in items {
        let id = item["id"].as_str().ok_or("item without an id")?;
        if seen.contains(&id) {
            return Err(format!("duplicate item in the binding: {id}"));
        }
        seen.push(id);
        if item["kind"].as_str() == Some("machine") {
            machine += 1;
        }
    }
    if machine < machine_floor {
        return Err(format!(
            "only {machine} machine-checkable items (floor {machine_floor})"
        ));
    }
    let mut closed: Vec<&str> = Vec::new();
    for entry in &report.entries {
        if !seen.contains(&entry.item_id.as_str()) {
            return Err(format!("report names unknown item {}", entry.item_id));
        }
        if closed.contains(&entry.item_id.as_str()) {
            return Err(format!("report names {} twice", entry.item_id));
        }
        closed.push(&entry.item_id);
    }
    for id in seen {
        if !closed.contains(&id) {
            return Err(format!("report is silent about item {id}"));
        }
    }
    Ok(())
}
