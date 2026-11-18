//! G32 CLI wiring: `run --intent <contract.toml> [--intent-token <token.json>]`.
//!
//! The intent contract is loaded, validated, SEALED, and (when a token is
//! presented) adjudicated BEFORE the governed run starts; the seal lands on
//! the run chain as an `intent.seal` row, so every later verdict can name
//! the exact contract it was measured against. A refused contract stops the
//! run with a usage error - an unanchored goal never silently proceeds.

use std::path::{Path, PathBuf};

use serde_json::json;
use truerise_governance::HashChain;
use truerise_wire::Severity;
use truerise_wire::{AdjudicationDecision, IntentContractV1, IntentToken};

/// The outcome of a successful intent load: everything the run banner and
/// the chain row need (the sealed contract itself stays on disk).
#[derive(Debug)]
pub struct IntentSeal {
    /// The goal sentence (verbatim from the contract).
    pub goal: String,
    /// Where the contract file lives (the post-run check re-reads it).
    pub path: PathBuf,
    /// The computed seal (SHA-256 hex).
    pub sealed_hash: String,
    /// Whether a valid per-item adjudication token was presented.
    pub adjudicated: bool,
    /// Review fix (P0): did the token contain any Reject/Edit decision? A
    /// rejected contract is NOT a completion basis (G32 red line) - the run
    /// proceeds honestly, but no certificate may ever cite it.
    pub rejected: bool,
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
    let mut contract: IntentContractV1 =
        toml::from_str(&text).map_err(|e| format!("intent contract is not valid v1 TOML: {e}"))?;
    contract
        .validate()
        .map_err(|e| format!("intent contract refused: {e}"))?;
    // Review fix (P2): if the FILE carries its own seal, recompute and
    // MATCH it - the old code just overwrote the field, making the seal a
    // write-only decoration (SealMismatch was dead code). Anchor: a file
    // whose claimed seal does not match its own content is not loadable.
    let claimed = contract.sealed_hash.clone();
    contract.seal();
    if let Some(claimed) = claimed {
        let recomputed = contract.sealed_hash().map_err(|e| e.to_string())?;
        if claimed != recomputed {
            return Err(format!(
                "intent contract seal mismatch: file claims {claimed}, content hashes to {recomputed}"
            ));
        }
    }
    let sealed_hash = contract
        .sealed_hash()
        .map_err(|e| format!("intent contract refused: {e}"))?
        .to_string();
    let mut adjudicated = false;
    let mut rejected = false;
    if let Some(token_path) = token_path {
        let token_text = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("cannot read intent token {token_path}: {e}"))?;
        let token: IntentToken = serde_json::from_str(&token_text)
            .map_err(|e| format!("intent token is not valid JSON: {e}"))?;
        token
            .validate_against(&contract)
            .map_err(|e| format!("intent token refused: {e}"))?;
        adjudicated = true;
        rejected = token
            .decisions
            .iter()
            .any(|d| d.decision != AdjudicationDecision::Accept);
    }
    Ok(Some(IntentSeal {
        goal: contract.goal.clone(),
        path: PathBuf::from(path.clone()),
        sealed_hash,
        adjudicated,
        rejected,
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
                "rejected": seal.rejected,
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
        if seal.adjudicated {
            "yes"
        } else {
            "no - the contract cannot certify completion until a per-item token is presented"
        }
    );
    if seal.rejected {
        println!(
            "  note: the token contains reject/edit decisions - this contract CANNOT certify completion (redraft and reseal)"
        );
    }
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

/// The shared run-chain dir (overridable for tests via TRUERISE_RUN_DIR).
pub fn run_chain_dir() -> std::path::PathBuf {
    std::env::var("TRUERISE_RUN_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("truerise_cli_run"))
}

/// G32 mid-run enforcement: re-read the contract file and re-derive its
/// seal. Any change since the pre-run seal voids the run's completion
/// basis - a changed intent is a NEW contract, and the run that started
/// under the old one cannot silently keep its verdict.
pub fn verify_intent_unchanged(intent_path: &Path, sealed_hash: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(intent_path)
        .map_err(|e| format!("intent contract unreadable after the run ({e}) - the completion basis cannot be re-derived"))?;
    let mut contract: IntentContractV1 =
        toml::from_str(&text).map_err(|e| format!("intent contract no longer parses ({e})"))?;
    contract
        .validate()
        .map_err(|e| format!("intent contract no longer validates ({e})"))?;
    contract.seal();
    let now = contract
        .sealed_hash()
        .map_err(|e| format!("intent contract refused: {e}"))?;
    if now != sealed_hash {
        return Err(format!(
            "intent contract CHANGED mid-run: sealed {sealed_hash}, now {now} - the run's completion basis is void"
        ));
    }
    Ok(())
}

/// Record the mid-run invalidation on the chain (the voided run is visible,
/// not silent).
fn record_intent_changed(dir: &Path, seal: &IntentSeal, reason: &str) -> Result<(), String> {
    let chain = HashChain::open(&dir.join("run.jsonl"))
        .map_err(|e| format!("intent invalidation unrecordable: {e}"))?;
    chain
        .append(
            "intent.changed",
            Severity::Warn,
            &json!({
                "sealed_hash": seal.sealed_hash,
                "reason": reason,
            }),
            None,
        )
        .map_err(|e| format!("intent invalidation unrecordable: {e}"))?;
    Ok(())
}

/// The whole intent flow of one `run` invocation: load/seal/record BEFORE
/// the run, verify-unchanged AFTER it. `run` is the governed-run closure
/// (single-track: everything else in the run branch is untouched).
/// `Err(usage)` = exit 2; `Ok(code)` = the run's exit code (a voided run
/// exits 1 with the intent.changed row on the chain).
pub fn intent_guarded_run(args: &[String], run: &dyn Fn(&str) -> i32) -> Result<i32, String> {
    let seal = load_intent(args)?;
    let fallback_goal = args
        .iter()
        .position(|a| a == "--goal")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "(no goal given)".into());
    let Some(seal) = seal else {
        // Review fix (P1): a run with NO goal and NO contract silently
        // executed a governed run with a placeholder goal and exited 0 -
        // an anchorless PASS. Refusal now.
        if fallback_goal == "(no goal given)" {
            return Err(
                "run needs --goal <text> (or --intent <contract.toml>) - an anchorless run never starts".to_string(),
            );
        }
        return Ok(run(&fallback_goal));
    };
    print_intent_banner(&seal);
    let dir = run_chain_dir();
    record_intent_on_chain(&dir, &seal).map_err(|e| format!("run aborted before start: {e}"))?;
    let code = run(&seal.goal);
    if code == 0 {
        if let Err(reason) = verify_intent_unchanged(&seal.path, &seal.sealed_hash) {
            // Best-effort record; the invalidation itself must still surface.
            let _ = record_intent_changed(&dir, &seal, &reason);
            eprintln!("run: {reason}");
            return Ok(1);
        }
    }
    Ok(code)
}
