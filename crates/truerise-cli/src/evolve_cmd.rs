//! `truerise evolve` (W7, RFC 0003): ONE generation, gen0 -> gen1, end to
//! end and recomputable offline:
//!
//! propose (never merges) -> HUMAN approval token -> apply (sandbox-backed
//! write, budget-gated) -> re-evaluate -> ratchet -> rollback on regression
//! -> generation certificate. The certificate embeds BOTH skill contents,
//! so `--replay` needs no files and no network.
//!
//! Honesty rail (RFC 0003 boundary 6): the certificate can express
//! `FAIL + rolled back` - a closed loop that CAUGHT a regression is a
//! success of the machinery.

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;
use truerise_governance::{ApprovalQueue, HashChain};
use truerise_rsi::{apply_proposal, content_hash, evolve_propose, rollback};
use truerise_wire::Severity;

use crate::RunParams;
use truerise_core::{Budget as _, ToolProvider as _};

/// The sealed mini task pack (id, goal, sealed expected answer). The goals
/// never contain the answer strings; the judge matches the exact
/// `task <id> answer: <expected>` shape, so plan-echo false positives are
/// impossible.
pub const TASKS: &[(&str, &str, &str)] = &[
    ("task-1", "state the fold window boundary rule", "half-open"),
    (
        "task-2",
        "name the conformance judging authority",
        "eval-crate",
    ),
    ("task-3", "state what the ratchet allows", "tighten-only"),
    ("task-4", "name who may apply a proposal", "human-approval"),
];

/// gen0: 3 of 4 (task-4 unknown - the capability gap the proposal closes).
pub const GEN0_ANSWERS: &str = r#"{"schema_version":"truerise.answers.v1","answers":{"task-1":"half-open","task-2":"eval-crate","task-3":"tighten-only","task-4":"unknown"}}"#;
/// gen1 (the proposal): all 4.
pub const GEN1_ANSWERS: &str = r#"{"schema_version":"truerise.answers.v1","answers":{"task-1":"half-open","task-2":"eval-crate","task-3":"tighten-only","task-4":"human-approval"}}"#;
/// An injected REGRESSION (task-1 forgotten): must be caught and rolled back.
pub const REGRESSED_ANSWERS: &str = r#"{"schema_version":"truerise.answers.v1","answers":{"task-1":"unknown","task-2":"eval-crate","task-3":"tighten-only","task-4":"human-approval"}}"#;

/// The answers router: a scripted solver whose knowledge IS the answers map
/// (the artifact that evolves between generations).
pub struct AnswersRouter {
    answers: HashMap<String, String>,
}

impl AnswersRouter {
    /// Parse an answers JSON (`truerise.answers.v1`).
    ///
    /// # Errors
    /// String-typed at the CLI boundary: unparsable JSON.
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let doc: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("answers unparsable: {e}"))?;
        let map = doc
            .get("answers")
            .and_then(|a| a.as_object())
            .ok_or("answers: missing answers object")?;
        Ok(Self {
            answers: map
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect(),
        })
    }

    fn answer_for(&self, payload: &str) -> String {
        let id = payload
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .find(|s| s.starts_with("task-"))
            .unwrap_or("task-none");
        self.answers
            .get(id)
            .cloned()
            .unwrap_or_else(|| "unknown".into())
    }
}

impl truerise_core::ModelRouter for AnswersRouter {
    fn route(
        &self,
        request: &truerise_core::TurnRequest,
    ) -> Result<truerise_core::TurnResponse, truerise_core::RouterError> {
        let payload = match request.tag.as_str() {
            "plan" => format!("plan for: {}", request.payload),
            "act" => {
                let answer = self.answer_for(&request.payload);
                format!("task-answer: {answer}")
            }
            "observe" => format!("observed: {}", request.payload),
            other => format!("scripted[{other}]"),
        };
        Ok(truerise_core::TurnResponse {
            model: "scripted".into(),
            payload,
        })
    }
}

/// Run the whole task pack through the governed path; return (id, passed).
/// A task passes iff the loop transcript carries the EXACT sealed answer
/// shape `task-answer: <expected>` (goal echo cannot fake it - goals never
/// contain that shape).
fn evaluate_generation(
    run_dir: &Path,
    router: &AnswersRouter,
    budget: &truerise_rsi::RunBudget,
    chain: &HashChain,
) -> Vec<(String, bool)> {
    TASKS
        .iter()
        .map(|(id, goal, expected)| {
            let books = crate::run_books_in(RunParams {
                chain_dir: run_dir,
                router,
                hooks: None,
                budget: Some(("evolve", budget)),
                hat_toml: None,
                memory: None,
                goal: &format!("solve {id}: {goal}"),
                rounds: 1,
            });
            let passed = match books {
                Ok(books) => books
                    .transcript
                    .iter()
                    .any(|e| e.text.contains(&format!("task-answer: {expected}"))),
                Err(_) => false,
            };
            let _ = chain; // rows already landed through the run path
            (id.to_string(), passed)
        })
        .collect()
}

/// Line diff stat (integers only; deterministic).
fn diff_stat(parent: &str, child: &str) -> (u32, u32) {
    let p: Vec<&str> = parent.lines().collect();
    let c: Vec<&str> = child.lines().collect();
    let added = c.iter().filter(|l| !p.contains(l)).count() as u32;
    let removed = p.iter().filter(|l| !c.contains(l)).count() as u32;
    (added, removed)
}

/// The generation certificate (integers only; BOTH contents embedded so
/// replay is file-free and network-free).
fn certificate(
    applied: Option<&truerise_rsi::AppliedGeneration>,
    parent_content: &str,
    child_content: &str,
    verdict: Option<&truerise_eval::RunVerdict>,
    rollback_reason: Option<&str>,
) -> serde_json::Value {
    let (added, removed) = diff_stat(parent_content, child_content);
    json!({
        "generation": 1,
        "parent": { "content_hash": content_hash(parent_content) },
        "child": { "content_hash": content_hash(child_content) },
        "proposal_hash": content_hash(child_content),
        "applied": applied.is_some(),
        "diff_stat": { "lines_added": added, "lines_removed": removed },
        "cost": { "task_runs": (TASKS.len() as u32) * 2, "budget_hat": "evolve" },
        "verdict": match verdict {
            Some(v) => json!({
                "verdict": v.verdict_str(),
                "failures": v.failures(),
                "total": v.rounds_total()
            }),
            None => json!(null)
        },
        "rollback": rollback_reason,
        "replay": { "command": "truerise evolve --replay --cert gen1_certificate.json" },
        "contents": { "parent": parent_content, "child": child_content }
    })
}

/// `evolve --demo`: one full generation.
///
/// Exit codes: 0 = the closed loop COMPLETED (a FAIL verdict + rollback is
/// a completed loop that caught a regression - the machinery worked);
/// 1 = the loop did not complete (refusals, chain failures).
pub fn evolve_demo(run_dir: &Path, approve: Option<&str>, inject_regression: bool) -> i32 {
    let _ = std::fs::create_dir_all(run_dir.join("skills"));
    let skill_path = run_dir.join("skills").join("skill.json");
    if !skill_path.exists() {
        if let Err(e) = std::fs::write(&skill_path, GEN0_ANSWERS) {
            eprintln!("evolve: could not seed gen0 skill: {e}");
            return 1;
        }
    }
    let chain = match HashChain::open(&run_dir.join("run.jsonl")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("evolve: chain unavailable: {e}");
            return 1;
        }
    };
    let mut queue = ApprovalQueue::new();
    let budget = truerise_rsi::RunBudget::new(Some(64));

    // gen0 evaluation (informational; sets the ratchet baseline).
    let gen0_raw = match std::fs::read_to_string(&skill_path) {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!("evolve: gen0 skill unreadable: {e}");
            return 1;
        }
    };
    let gen0_router = match AnswersRouter::from_json(&gen0_raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("evolve: gen0 skill unparsable: {e}");
            return 1;
        }
    };
    let results0 = evaluate_generation(run_dir, &gen0_router, &budget, &chain);
    let gen0_failures = results0.iter().filter(|(_, ok)| !ok).count() as u32;
    let mut baseline = truerise_eval::ratchet::Baseline::new();
    baseline.insert("task_failures".to_string(), f64::from(gen0_failures));
    if truerise_eval::judge_tasks(&results0, &baseline, &chain).is_err() {
        eprintln!("evolve: gen0 verdict unrecordable");
        return 1;
    }

    // The proposal (child content; the evolving artifact is the answers map).
    let child_content: &str = if inject_regression {
        REGRESSED_ANSWERS
    } else {
        GEN1_ANSWERS
    };
    let proposal_hash = content_hash(child_content);
    let _proposal = evolve_propose(
        &run_dir.join("quarantine"),
        "gen1",
        "close the task-4 capability gap (sealed answers only)",
        vec![("skill.json".into(), child_content.to_string())],
    );
    queue.request(&proposal_hash);
    let approved = approve == Some(proposal_hash.as_str());
    if approved {
        let _ = queue.approve(&proposal_hash);
    }
    let _ = chain.append(
        "evolve.proposal",
        Severity::Info,
        &json!({ "id": "gen1", "proposal_hash": proposal_hash }),
        None,
    );
    let _ = chain.append(
        "evolve.approval",
        if approved {
            Severity::Info
        } else {
            Severity::Warn
        },
        &json!({ "approved": approved, "token_present": approve.is_some() }),
        None,
    );

    // Apply: the sandbox-backed, budget-gated write (RFC 0003 gates all
    // enforced inside rsi::apply_proposal).
    let skill_for_write = skill_path.clone();
    let write = |content: &str| -> Result<(), String> {
        budget
            .consume("evolve", 1)
            .map_err(|e| format!("budget refused the apply write: {e}"))?;
        let request = truerise_core::ToolRequest {
            tool: "write_file".into(),
            args: json!({
                "path": "skills/skill.json",
                "content": content
            }),
        };
        // Byte-identical content: nothing to write.
        if std::fs::read_to_string(&skill_for_write)
            .map(|current| current == content)
            .unwrap_or(false)
        {
            return Ok(());
        }
        // The fence root IS the run dir: the sandboxed file IS the skill
        // file - the write goes through the W2 executor semantics.
        let fence = run_dir.to_path_buf();
        let executor = truerise_sandbox::SandboxToolProvider::new(
            truerise_sandbox::InProcessProvider::new(&fence),
            &fence,
        )
        .map_err(|e| e.to_string())?;
        // NO destroy here: the workdir IS the run dir - destroying it
        // would delete the run's own chain. The executor is dropped; files
        // stay (they ARE the artifacts).
        executor
            .execute(&request)
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    let applied = apply_proposal(
        &skill_path,
        child_content,
        approve,
        queue.current_epoch(),
        &mut queue,
        &chain,
        &write,
    );
    let applied = match applied {
        Ok(a) => a,
        Err(e) => {
            eprintln!("evolve: apply refused: {e}");
            return 1;
        }
    };

    // Re-evaluate the child generation against the gen0 baseline.
    let child_router = match AnswersRouter::from_json(child_content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("evolve: child skill unparsable: {e}");
            return 1;
        }
    };
    let results1 = evaluate_generation(run_dir, &child_router, &budget, &chain);
    let verdict = match truerise_eval::judge_tasks(&results1, &baseline, &chain) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("evolve: gen1 verdict unrecordable: {e}");
            return 1;
        }
    };

    // The ratchet's undo: a regression is ROLLED BACK, reason on the chain.
    let mut rollback_reason: Option<String> = None;
    if !verdict.is_pass() {
        let reason = format!(
            "ratchet: gen1 regressed (failures {} > baseline {gen0_failures})",
            verdict.failures()
        );
        match rollback(&skill_path, &applied.snapshot, &reason, &chain, &write) {
            Ok(()) => rollback_reason = Some(reason),
            Err(e) => {
                eprintln!("evolve: rollback failed: {e}");
                return 1;
            }
        }
    }

    // The generation certificate (chain row carries hashes; the FILE embeds
    // the contents so replay is offline).
    let cert = certificate(
        Some(&applied),
        &applied.snapshot,
        child_content,
        Some(&verdict),
        rollback_reason.as_deref(),
    );
    let cert_path = run_dir.join("gen1_certificate.json");
    let cert_text = match serde_json::to_string_pretty(&cert) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("evolve: certificate unserialisable: {e}");
            return 1;
        }
    };
    if let Err(e) = std::fs::write(&cert_path, cert_text) {
        eprintln!("evolve: could not write certificate: {e}");
        return 1;
    }
    let _ = chain.append(
        "evolve.certificate",
        Severity::Info,
        &json!({
            "generation": 1,
            "parent_hash": cert["parent"]["content_hash"],
            "child_hash": cert["child"]["content_hash"],
            "verdict": cert["verdict"]["verdict"],
            "rolled_back": rollback_reason.is_some()
        }),
        None,
    );
    println!(
        "evolve gen0->gen1: verdict {} (failures {}) rolled_back={} cert={}",
        verdict.verdict_str(),
        verdict.failures(),
        rollback_reason.is_some(),
        cert_path.display()
    );
    0
}

/// `evolve --replay`: recompute a certificate OFFLINE (no files beyond the
/// cert, no network). Exit 0 = every recomputed value matches.
pub fn evolve_replay(cert_path: &Path) -> i32 {
    let Ok(text) = std::fs::read_to_string(cert_path) else {
        eprintln!("replay: certificate unreadable: {}", cert_path.display());
        return 1;
    };
    let Ok(cert) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!("replay: certificate unparsable");
        return 1;
    };
    let parent = cert["contents"]["parent"].as_str().unwrap_or_default();
    let child = cert["contents"]["child"].as_str().unwrap_or_default();
    let mut ok = true;
    // 1. Content hashes recompute byte-exact.
    ok &= cert["parent"]["content_hash"] == json!(content_hash(parent));
    ok &= cert["child"]["content_hash"] == json!(content_hash(child));
    // 2. The verdict recomputes: run the task pack with the child answers
    //    over a THROWAWAY chain (replay must not mutate the original).
    let throwaway = std::env::temp_dir().join(format!("truerise_replay_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&throwaway);
    std::fs::create_dir_all(&throwaway).ok();
    let chain = match HashChain::open(&throwaway.join("replay.jsonl")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("replay: throwaway chain unavailable: {e}");
            return 1;
        }
    };
    let router = match AnswersRouter::from_json(child) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("replay: certificate child answers unparsable: {e}");
            return 1;
        }
    };
    let budget = truerise_rsi::RunBudget::new(None);
    let results = evaluate_generation(&throwaway, &router, &budget, &chain);
    let recomputed = match truerise_eval::judge_tasks(
        &results,
        &truerise_eval::ratchet::Baseline::new(),
        &chain,
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("replay: recomputed verdict unrecordable: {e}");
            return 1;
        }
    };
    ok &= cert["verdict"]["verdict"] == json!(recomputed.verdict_str());
    ok &= cert["verdict"]["failures"] == json!(recomputed.failures());
    // 3. Rollback consistency: a cert with a rollback MUST carry a FAIL.
    let rolled_back = cert["rollback"].is_string();
    ok &= (!rolled_back) || !recomputed.is_pass();
    let _ = std::fs::remove_dir_all(&throwaway);
    if ok {
        println!(
            "replay: certificate recomputes identically (verdict {}, failures {})",
            recomputed.verdict_str(),
            recomputed.failures()
        );
        0
    } else {
        eprintln!("replay: certificate does NOT recompute (tampered or stale)");
        1
    }
}
