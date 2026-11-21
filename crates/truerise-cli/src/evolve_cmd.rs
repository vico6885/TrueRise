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
use truerise_rsi::{apply_proposal_gated, content_hash, evolve_propose, rollback};
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
    tape_path: Option<&Path>,
) -> Vec<(String, bool)> {
    // W14: when a tape path is wired, THIS generation's calls are recorded
    // (a fresh recorder per generation, seeded from the existing tape so
    // the whole evolve run lands in ONE file).
    let _tape_router =
        tape_path.map(|p| crate::tape::TapeRouter::recording(router, p.to_path_buf()));
    let taped: &dyn truerise_core::ModelRouter = match &_tape_router {
        Some(t) => t,
        None => router,
    };
    TASKS
        .iter()
        .map(|(id, goal, expected)| {
            let books = crate::run_books_in(RunParams {
                chain_dir: run_dir,
                router: taped,
                hooks: None,
                budget: Some(("evolve", budget)),
                hat_toml: None,
                memory: None,
                goal: &format!("solve {id}: {goal}"),
                rounds: 1,
                pinned_model: None,
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
/// Certificate attachments: completion section + G29 unverified rows.
struct CertAttachments<'a> {
    completion: Option<&'a serde_json::Value>,
    unverified_rows: &'a [String],
}

fn certificate(
    applied: Option<&truerise_rsi::AppliedGeneration>,
    parent_content: &str,
    child_content: &str,
    verdict: Option<&truerise_eval::RunVerdict>,
    rollback_reason: Option<&str>,
    significance_json: serde_json::Value,
    attachments: CertAttachments<'_>,
) -> serde_json::Value {
    let (added, removed) = diff_stat(parent_content, child_content);
    let mut cert = json!({
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
        "significance": significance_json,
        "replay": { "command": "truerise evolve --replay --cert gen1_certificate.json" },
        "contents": { "parent": parent_content, "child": child_content }
    });
    // G35/G36: the completion section appears ONLY when an intent
    // contract was bound (bound without ledger never ships).
    if let Some(section) = attachments.completion {
        cert["completion"] = section.clone();
    }
    // G29: unverified rows ride in EVERY certificate (gen-0 mark visible
    // at gen n, washed or not - acceptance: survives_all_later_generations).
    cert["unverified_assumptions"] = serde_json::json!(attachments.unverified_rows);
    cert
}

/// `evolve --demo`: one full generation.
///
/// Exit codes: 0 = the closed loop COMPLETED (a FAIL verdict + rollback is
/// a completed loop that caught a regression - the machinery worked);
/// 1 = the loop did not complete (refusals, chain failures).
pub fn evolve_demo(
    run_dir: &Path,
    approve: Option<&str>,
    inject_regression: bool,
    tape_path: Option<&Path>,
    completion: Option<&crate::completion_cert::CompletionBinding>,
) -> i32 {
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
    // G29: marks survive by MACHINERY (ledger reconstructs from chain rows).
    let mut unverified_ledger =
        truerise_governance::UnverifiedLedger::reconstruct_from_chain(&chain);

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
    let results0 = evaluate_generation(run_dir, &gen0_router, &budget, &chain, tape_path);
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
    // Review fix (batch 3b follow-up): an unrecorded request is an error,
    // not a warning (the apply gate reads from THIS record).
    if let Err(err) = queue.request(&proposal_hash) {
        eprintln!("evolve: approval request refused: {err}");
        return 1;
    }
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
    let applied = apply_proposal_gated(
        &skill_path,
        child_content,
        approve,
        queue.current_epoch(),
        &mut queue,
        &write,
        &mut truerise_rsi::GateContext {
            chain: &chain,
            support: None, // the evolve flow cites recompute evidence post-apply
            ledger: &mut unverified_ledger,
            generation: 0,
        },
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
    let results1 = evaluate_generation(run_dir, &child_router, &budget, &chain, tape_path);
    let verdict = match truerise_eval::judge_tasks(&results1, &baseline, &chain) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("evolve: gen1 verdict unrecordable: {e}");
            return 1;
        }
    };

    // W9: the improvement claim must be statistically DEFENSIBLE or it is
    // refused. Paired per-trial outcomes: gen0 vs gen1 over the SAME task
    // pack at the SAME budget. A 4-task pack can never meet the discordant
    // floor - the certificate says VOIDED, honestly.
    let arm_a = truerise_eval::ArmResult {
        outcomes: results0.iter().map(|(_, ok)| *ok).collect(),
        budget_units: TASKS.len() as u64,
    };
    let arm_b = truerise_eval::ArmResult {
        outcomes: results1.iter().map(|(_, ok)| *ok).collect(),
        budget_units: TASKS.len() as u64,
    };
    let significance = match truerise_eval::mcnemar_exact(&arm_a, &arm_b) {
        Ok(sig) => sig,
        Err(e) => {
            eprintln!("evolve: significance comparison refused: {e}");
            return 1;
        }
    };
    let significance_json = match &significance {
        truerise_eval::Significance::Significant {
            a_better,
            discordant,
            p_numerator,
            p_pow2,
        } => json!({
            "verdict": "significant", "a_better": a_better,
            "discordant": discordant, "p_numerator": p_numerator, "p_pow2": p_pow2
        }),
        truerise_eval::Significance::NotSignificant {
            discordant,
            p_numerator,
            p_pow2,
        } => json!({
            "verdict": "not_significant", "discordant": discordant,
            "p_numerator": p_numerator, "p_pow2": p_pow2
        }),
        truerise_eval::Significance::Voided { discordant, floor } => json!({
            "verdict": "voided", "reason": "discordant pairs below the floor",
            "discordant": discordant, "floor": floor
        }),
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

    // G35/G36: a generation issued under a sealed intent contract MUST
    // carry a closing completion ledger - the certificate is REFUSED
    // otherwise (silence is not coverage, at certificate level).
    let completion_json = match completion {
        Some(binding) => {
            let section = match crate::completion_cert::validate_binding(binding) {
                Ok(section) => section,
                Err(reason) => {
                    eprintln!("evolve: certificate refused: {reason}");
                    return 1;
                }
            };
            let _ = chain.append(
                "intent.seal",
                Severity::Info,
                &json!({
                    "sealed_hash": section["contract_sealed_hash"],
                    "bound_to": "generation-1"
                }),
                None,
            );
            Some(section)
        }
        None => None,
    };
    // The certificate: chain row carries hashes; the FILE embeds contents.
    let cert = certificate(
        Some(&applied),
        &applied.snapshot,
        child_content,
        Some(&verdict),
        rollback_reason.as_deref(),
        significance_json,
        CertAttachments {
            completion: completion_json.as_ref(),
            unverified_rows: &unverified_ledger.certificate_rows(),
        },
    );
    let cert_path = run_dir.join("gen1_certificate.json");
    // W10: sign when the issuer key is in the environment (keyless = unsigned).
    let mut cert_out = cert.clone();
    if let Some(key) = crate::identity::load_signing_key() {
        crate::identity::sign_certificate(&mut cert_out, &key);
    }
    let cert_text = match serde_json::to_string_pretty(&cert_out) {
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
    let sig_word = match &significance {
        truerise_eval::Significance::Significant { .. } => "significant",
        truerise_eval::Significance::NotSignificant { .. } => "not-significant",
        truerise_eval::Significance::Voided { .. } => "voided",
    };
    println!(
        "evolve gen0->gen1: verdict {} (failures {}) rolled_back={} significance={sig_word} cert={}",
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
    let results = evaluate_generation(&throwaway, &router, &budget, &chain, None);
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
    // 4. G35/G36: an embedded completion section must recompute offline -
    // hash-consistent and closed against the embedded item list.
    if cert["completion"].is_object() {
        if let Err(reason) = crate::completion_cert::replay_completion(&cert["completion"]) {
            eprintln!("replay: completion ledger does not recompute: {reason}");
            ok = false;
        }
    }
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
