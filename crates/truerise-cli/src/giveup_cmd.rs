//! G44 CLI wiring: the `giveup` subcommand records a BOUNDED give-up claim
//! on the run chain.
//!
//! `truerise giveup --ledger <claim.json> [--intent <contract.toml>]`:
//! the claim is validated (ledger present, budget bound sealed, retry plan
//! disjoint from the tried set, untried probes carry their cheapest concrete
//! form) BEFORE it lands on the chain as a `giveup.claim` row. An
//! unbounded or lazy abandonment is a refusal (exit 2), not a warning.
//! AUTO-EXECUTED bounded recheck stays deferred (consensus v3.3) - this
//! records the ledger, the bound and the plan; the plan becomes executable
//! input once G2/G21 budget linkage exists.

use std::path::{Path, PathBuf};

use serde_json::json;
use truerise_governance::HashChain;
use truerise_wire::Severity;
use truerise_wire::GiveUpClaimV1;

/// The snake_case name of a verdict (chain rows speak serde names, not
/// Rust Debug names).
fn verdict_name(verdict: &truerise_wire::GiveUpVerdict) -> &'static str {
    match verdict {
        truerise_wire::GiveUpVerdict::Completed => "completed",
        truerise_wire::GiveUpVerdict::NotCompleted => "not_completed",
        truerise_wire::GiveUpVerdict::InfeasibleWithinBudget => "infeasible_within_budget",
    }
}

/// Validate the claim (against the intent contract when one is supplied)
/// and append it to the run chain as a `giveup.claim` row. Returns the
/// human-readable confirmation line.
pub fn record_giveup(
    dir: &Path,
    ledger_path: &Path,
    intent_path: Option<&Path>,
) -> Result<String, String> {
    let text = std::fs::read_to_string(ledger_path)
        .map_err(|e| format!("cannot read give-up claim {}: {e}", ledger_path.display()))?;
    let claim: GiveUpClaimV1 = serde_json::from_str(&text)
        .map_err(|e| format!("give-up claim is not valid v1 JSON: {e}"))?;
    let contract = match intent_path {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read intent contract {}: {e}", path.display()))?;
            let mut contract: truerise_wire::IntentContractV1 = toml::from_str(&text)
                .map_err(|e| format!("intent contract is not valid v1 TOML: {e}"))?;
            contract
                .validate()
                .map_err(|e| format!("intent contract refused: {e}"))?;
            contract.seal();
            Some(contract)
        }
        None => None,
    };
    claim
        .validate_against(contract.as_ref())
        .map_err(|e| format!("give-up claim refused: {e}"))?;
    let chain = HashChain::open(&dir.join("run.jsonl"))
        .map_err(|e| format!("give-up claim unrecordable (chain open): {e}"))?;
    chain
        .append(
            "giveup.claim",
            Severity::Info,
            &json!({
                "verdict": verdict_name(&claim.verdict),
                "ledger": claim.ledger.as_ref().map(|l| json!({
                    "budget_bound": l.budget_bound,
                    "probes": l.probes.len(),
                    "retry_candidates": l.retry_plan.candidate_probes.len(),
                })),
            }),
            None,
        )
        .map_err(|e| format!("give-up claim unrecordable (chain append): {e}"))?;
    let ledger = claim.ledger.as_ref();
    Ok(format!(
        "give-up claim recorded on the run chain: verdict {}, ledger {} probes, budget bound sealed, {} retry candidates",
        verdict_name(&claim.verdict),
        ledger.map(|l| l.probes.len()).unwrap_or(0),
        ledger
            .map(|l| l.retry_plan.candidate_probes.len())
            .unwrap_or(0)
    ))
}

/// The `giveup` subcommand handler (arg parsing lives here so lib.rs stays
/// under the file-length floor).
pub fn handle_command(args: &[String]) -> i32 {
    // Review fix: a missing or duplicated value must be a USAGE error, not
    // a silent fallback - `--intent` with no value used to skip the whole
    // contract binding and record an unanchored claim.
    let ledger = match flag_value(args, "--ledger") {
        Ok(v) => v,
        Err(usage) => {
            eprintln!("usage error: {usage}
usage: truerise giveup --ledger <claim.json> [--intent <contract.toml>]");
            return 2;
        }
    };
    let Some(ledger) = ledger else {
        eprintln!("usage: truerise giveup --ledger <claim.json> [--intent <contract.toml>]");
        return 2;
    };
    let intent = match flag_value(args, "--intent") {
        Ok(v) => v.map(PathBuf::from),
        Err(usage) => {
            eprintln!("usage error: {usage}");
            return 2;
        }
    };
    let dir = run_chain_dir();
    match record_giveup(&dir, Path::new(&ledger), intent.as_deref().map(Path::new)) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            eprintln!("usage error: {e}");
            2
        }
    }
}

/// Strict flag parsing: value required, `--`-prefixed values rejected,
/// duplicates rejected (same semantics as intent_cmd::extract_flag).
fn flag_value(args: &[String], flag: &str) -> Result<Option<String>, String> {
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
                _ => return Err(format!("{flag} needs a value")),
            }
        } else {
            idx += 1;
        }
    }
    Ok(found)
}

/// The shared run-chain dir (overridable for tests via TRUERISE_RUN_DIR).
fn run_chain_dir() -> PathBuf {
    std::env::var("TRUERISE_RUN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("truerise_cli_run"))
}
