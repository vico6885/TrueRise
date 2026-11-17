//! G32 CLI wiring: `run --intent <contract.toml> [--intent-token <token.json>]`.
//!
//! The intent contract is loaded, validated, SEALED, and (when a token is
//! presented) adjudicated BEFORE the governed run starts; the seal lands on
//! the run chain as an `intent.seal` row, so every later verdict can name
//! the exact contract it was measured against. A refused contract stops the
//! run with a usage error - an unanchored goal never silently proceeds.

use std::path::Path;

use serde_json::json;
use truerise_governance::HashChain;
use truerise_wire::Severity;
use truerise_wire::{IntentContractV1, IntentToken};

/// The outcome of a successful intent load: everything the run banner and
/// the chain row need (the sealed contract itself stays on disk).
#[derive(Debug)]
pub struct IntentSeal {
    /// The goal sentence (verbatim from the contract).
    pub goal: String,
    /// The computed seal (SHA-256 hex).
    pub sealed_hash: String,
    /// Whether a valid per-item adjudication token was presented.
    pub adjudicated: bool,
    /// Machine-checkable item count (the completion floor applies to this).
    pub machine_items: u32,
    /// Total acceptance items.
    pub items_total: usize,
}

/// Parse `--intent <path.toml>` and `--intent-token <path.json>` out of the
/// run args. `Ok(None)` = no intent given (plain `--goal` run). Any refusal
/// is a `Err(usage)` that exits 2 - an invalid contract never degrades into
/// a warning.
pub fn load_intent(args: &[String]) -> Result<Option<IntentSeal>, String> {
    let intent_path = extract_flag(args, "--intent")?;
    let token_path = extract_flag(args, "--intent-token")?;
    let Some(path) = intent_path else {
        if token_path.is_some() {
            return Err("--intent-token needs --intent <contract.toml>".to_string());
        }
        return Ok(None);
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read intent contract {path}: {e}"))?;
    let mut contract: IntentContractV1 = toml::from_str(&text)
        .map_err(|e| format!("intent contract is not valid v1 TOML: {e}"))?;
    contract
        .validate()
        .map_err(|e| format!("intent contract refused: {e}"))?;
    contract.seal();
    let sealed_hash = contract
        .sealed_hash()
        .map_err(|e| format!("intent contract refused: {e}"))?
        .to_string();
    let mut adjudicated = false;
    if let Some(token_path) = token_path {
        let token_text = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("cannot read intent token {token_path}: {e}"))?;
        let token: IntentToken = serde_json::from_str(&token_text)
            .map_err(|e| format!("intent token is not valid JSON: {e}"))?;
        token
            .validate_against(&contract)
            .map_err(|e| format!("intent token refused: {e}"))?;
        adjudicated = true;
    }
    Ok(Some(IntentSeal {
        goal: contract.goal.clone(),
        sealed_hash,
        adjudicated,
        machine_items: contract.machine_item_count(),
        items_total: contract.acceptance_items.len(),
    }))
}

/// Append the `intent.seal` row to the run chain (the row names the exact
/// contract hash every later verdict is measured against).
pub fn record_intent_on_chain(dir: &Path, seal: &IntentSeal) -> Result<(), String> {
    let chain = HashChain::open(&dir.join("run.jsonl"))
        .map_err(|e| format!("intent seal unrecordable (chain open): {e}"))?;
    chain
        .append(
            "intent.seal",
            Severity::Info,
            &json!({
                "sealed_hash": seal.sealed_hash,
                "adjudicated": seal.adjudicated,
                "machine_items": seal.machine_items,
                "items_total": seal.items_total,
            }),
            None,
        )
        .map_err(|e| format!("intent seal unrecordable (chain append): {e}"))?;
    Ok(())
}

/// Print the intent banner (honest by construction: the adjudication status
/// is stated either way, never implied).
pub fn print_intent_banner(seal: &IntentSeal) {
    println!(
        "intent contract sealed: {} (items {}/{} machine-checkable, adjudicated: {})",
        seal.sealed_hash,
        seal.machine_items,
        seal.items_total,
        if seal.adjudicated { "yes" } else { "no - the contract cannot certify completion until a per-item token is presented" }
    );
    if seal.machine_items < truerise_wire::MACHINE_CHECK_FLOOR {
        println!(
            "  note: {} machine-checkable items is under the floor of {} - no completion claim may cite this contract",
            seal.machine_items,
            truerise_wire::MACHINE_CHECK_FLOOR
        );
    }
}

fn extract_flag(args: &[String], flag: &str) -> Result<Option<String>, String> {
    let mut found: Option<String> = None;
    let mut idx = 0;
    while idx < args.len() {
        if args[idx] == flag {
            match args.get(idx + 1) {
                Some(p) if !p.starts_with("--") => {
                    if found.is_some() {
                        return Err(format!("{flag} given twice"));
                    }
                    found = Some(p.clone());
                    idx += 2;
                }
                _ => return Err(format!("{flag} needs a path")),
            }
        } else {
            idx += 1;
        }
    }
    Ok(found)
}
