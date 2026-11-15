//! truerise-cli: the single-binary entry (REPL / run / demo / onboarding).
//
// - `demo`: the integrity three-scenario walkthrough (Rust edition, mirror
//   of the Python 0.1-series demo; verdicts derive from inputs, so
//   `--honest` flips PASS).
// - `run --goal`: a governed minimal run (three books) over a scripted
//   router; with no provider configured it explains the onboarding path.
// - REPL: stdin loop with /demo /audit slash commands (IO adapter ONLY -
//   the multi-turn semantics reuse the same loop path, no bare-face entry).
// - onboarding: writes config.toml WITHOUT any secret (keys live in env or
//   an OS keyring; the config stores nothing sensitive).

use std::path::{Path, PathBuf};

use truerise_governance::HashChain;
use truerise_wire::Severity;

pub mod assembly;
pub mod baseline;
pub mod demo;
pub mod evolve_cmd;
pub mod identity;
pub mod interactive;
pub mod mcp_serve;
pub mod provider_openai;
pub mod tape;

pub use assembly::{GovernedPolicy, RunTooling};
pub use baseline::{find_default_baseline, grade_exit};
pub use demo::{demo_scenarios, run_demo};
pub use interactive::{onboarding, repl, repl_turn};

// truerise-cli library surface (the binary is a thin IO shell over this). the REPL is an IO adapter over
// THESE functions only (spec §6.5 single-track ruling) - the golden test
// proves the repl path and the direct loop path produce identical books.

/// Print the subcommand help text.
pub fn print_help() {
    println!(
        "truerise - integrity-evaluation CLI (0.2-series dev)\n\n\
         SUBCOMMANDS:\n\
         \x20 demo [--honest]   integrity walkthrough: three cheating scenarios, caught\n\
         \x20 run --goal <text> governed minimal run (three books)\n\
         \x20 repl              interactive shell (/demo /audit /quit)\n\
         \x20 onboarding        first-run wizard (writes config.toml, NEVER stores keys)\n\
         \x20 help              this text"
    );
}

// ---------------------------------------------------------------------------
// run: minimal governed run (three books) with a scripted router
// ---------------------------------------------------------------------------

/// A scripted router: answers as the pinned model without any network (the
/// real ModelRouter providers are external projects; the port is the contract).
/// With ``tool_call`` set, the act turn emits the RFC 0002 tool envelope
/// instead of plain text (the governed demo of the enforcement chain).
pub struct ScriptRouter {
    model: String,
    tool_call: bool,
}

impl ScriptRouter {
    /// Plain-text scripted router (no tool envelope).
    #[must_use]
    pub fn plain(model: &str) -> Self {
        Self {
            model: model.into(),
            tool_call: false,
        }
    }

    /// Tool-requesting scripted router (act turns carry the envelope).
    #[must_use]
    pub fn tool_calling(model: &str) -> Self {
        Self {
            model: model.into(),
            tool_call: true,
        }
    }
}

impl truerise_core::ModelRouter for ScriptRouter {
    fn route(
        &self,
        request: &truerise_core::TurnRequest,
    ) -> Result<truerise_core::TurnResponse, truerise_core::RouterError> {
        let payload = if self.tool_call && request.tag == "act" {
            // One deterministic scripted action: write the run report under
            // the sandbox fence. The policy decides whether it happens.
            r#"{"truerise_tool":{"tool":"write_file","args":{"path":"report.md","content":"scripted report body"}}}"#.to_string()
        } else {
            format!("scripted response for: {}", request.payload)
        };
        Ok(truerise_core::TurnResponse {
            model: self.model.clone(),
            payload,
        })
    }
}

/// AuditSink bridge: the governance chain as the loop's audit port (this
/// bridge lives HERE, at the boundary, keeping core free of governance).
pub struct ChainSinkBridge<'a>(pub &'a HashChain);

impl truerise_core::AuditSink for ChainSinkBridge<'_> {
    fn append(
        &self,
        kind: &str,
        severity: &str,
        detail: &serde_json::Value,
    ) -> Result<usize, String> {
        let sev = match severity {
            "warn" => Severity::Warn,
            "error" => Severity::Error,
            "debug" => Severity::Debug,
            _ => Severity::Info,
        };
        self.0
            .append(kind, sev, detail, None)
            .map(|_| self.0.len())
            .map_err(|e| e.to_string())
    }
}

/// The `run --goal` subcommand: one governed run over the FULL assembly
/// (scripted router + governance policy + fenced sandbox executor, RFC 0002;
/// budget enforced per W3; the run ends with a judged verdict per W4).
/// Returns the process exit code (0 PASS / 1 FAIL or failed run - a budget
/// stop IS a failure, never a note).
pub fn run_commandline(
    goal: &str,
    approvals: &[String],
    budget_units: Option<u64>,
    baseline_path: Option<&Path>,
    hat_toml: Option<&str>,
) -> i32 {
    let dir = std::env::temp_dir().join("truerise_cli_run");
    std::fs::create_dir_all(&dir).ok();
    let tooling = match RunTooling::new(&dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("run: tool assembly refused (fail-closed): {e}");
            return 1;
        }
    };
    for scope in approvals {
        tooling.policy.approve(scope);
    }
    let router = ScriptRouter::tool_calling("scripted");
    let hooks = truerise_core::ToolHooks {
        policy: &tooling.policy,
        provider: &tooling.provider,
    };
    let run_budget = truerise_rsi::RunBudget::new(budget_units);
    // W6: the default run consults an EMPTY recall store - the attempt (and
    // its emptiness) is audited either way. Embedders inject a real bridge
    // through RunParams.memory.
    let recall_bridge = truerise_memory::RecallBridge::new(
        truerise_memory::InMemoryBackend::new(),
        truerise_memory::Weights::default(),
        3,
    );
    match run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: Some(hooks),
        budget: Some(("cli-run", &run_budget)),
        hat_toml,
        memory: Some(&recall_bridge),
        goal,
        rounds: 1,
    }) {
        Ok(books) => {
            println!("run complete (three books):");
            println!("  transcript entries: {}", books.transcript.len());
            println!("  transitions:        {}", books.transitions.len());
            println!("  audit rows:         {}", books.audit_rows);
            // W4: the run ends with a judged verdict from THE authority.
            let chain = match HashChain::open(&dir.join("run.jsonl")) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("run: verdict unrecordable (chain reopened): {e}");
                    return 1;
                }
            };
            let facts = truerise_eval::RunFacts {
                finished: truerise_core::Loop::final_state(&books.transitions)
                    == truerise_core::State::Finished,
                rounds_total: 1,
            };
            match truerise_eval::judge_run(&facts, &baseline::load_baseline(baseline_path), &chain)
            {
                Ok(verdict) => {
                    println!(
                        "  verdict:            {} (failures {}, baseline {})",
                        verdict.verdict_str(),
                        verdict.failures(),
                        if baseline_path.is_some() {
                            "loaded"
                        } else {
                            "none"
                        }
                    );
                    // W6: one-way span export (best-effort; the chain stays
                    // the authority - export NEVER mutates it).
                    let out = dir.join("observe").join("spans.jsonl");
                    match truerise_observe::export_jsonl(&dir.join("run.jsonl"), &out) {
                        Ok(n) => println!("  observe spans:      {n} (one-way export)"),
                        Err(e) => eprintln!("  observe export failed (non-fatal): {e}"),
                    }
                    grade_exit(&verdict)
                }
                Err(e) => {
                    // Unrecordable verdicts are ERRORS, not warnings.
                    eprintln!("run: verdict could not be recorded: {e}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("run failed: {e}");
            1
        }
    }
}

/// Direct-loop path for the golden comparison (no repl adapter at all).
pub fn direct_loop_books(goal: &str) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    run_books(goal, 1)
}

/// One governed run's assembly (single-track: everything a run can wire).
pub struct RunParams<'a> {
    /// Where the run's chain lives (created on demand).
    pub chain_dir: &'a Path,
    /// The router (scripted today; provider adapters are external projects).
    pub router: &'a dyn truerise_core::ModelRouter,
    /// RFC 0002 tool hooks; `None` = execution-free run (golden lock).
    pub hooks: Option<truerise_core::ToolHooks<'a>>,
    /// W3 budget (hat id + ledger port); `None` = unbounded.
    pub budget: Option<(&'a str, &'a dyn truerise_core::Budget)>,
    /// W5 hat TOML: registered on the chain + its tool_subset becomes the
    /// OUTER policy gate; `None` = hatless run.
    pub hat_toml: Option<&'a str>,
    /// W6 context recall (evidence audited per plan turn); `None` = no
    /// memory participation.
    pub memory: Option<&'a dyn truerise_core::ContextRecall>,
    /// The run goal.
    pub goal: &'a str,
    /// Declared rounds.
    pub rounds: usize,
}

/// The single governed-run implementation (single-track rule): BOTH the
/// repl path and the run subcommand land here.
pub fn run_books_in(
    params: RunParams<'_>,
) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    let chain_dir = params.chain_dir;
    std::fs::create_dir_all(chain_dir).ok();
    let chain = HashChain::open(&chain_dir.join("run.jsonl"))
        .map_err(|e| truerise_core::LoopError::Sink(e.to_string()))?;
    let sink = ChainSinkBridge(&chain);
    // W5: a run under a hat registers the hat (content hash on chain) and
    // validates its tool_subset AT LOAD (unknown tool -> the run refuses to
    // start; nothing is silently ignored).
    let hat_gate = match params.hat_toml {
        Some(hat_toml) => {
            // Validate BEFORE registering: a refused hat leaves no chain row.
            let hat = truerise_hats::parse_hat(hat_toml).map_err(|e| {
                truerise_core::LoopError::Config(format!("hat refused at load: {e}"))
            })?;
            assembly::validate_hat_tools(&hat).map_err(truerise_core::LoopError::Config)?;
            Some(truerise_hats::load_hat(hat_toml, &chain).map_err(|e| {
                truerise_core::LoopError::Config(format!("hat refused at load: {e}"))
            })?)
        }
        None => None,
    };
    let config = truerise_core::LoopConfig {
        pinned_model: "scripted".into(),
        retry: truerise_core::RetryPolicy::default(),
    };
    {
        let mut governed = truerise_core::Loop::new(params.router, config, &sink);
        if let Some((hat, budget)) = params.budget {
            governed = governed.with_budget(hat, budget);
        }
        if let Some(memory) = params.memory {
            governed = governed.with_memory(memory);
        }
        match (params.hooks, hat_gate.as_ref()) {
            (Some(hooks), Some(hat)) => {
                let stacked = assembly::StackedPolicy {
                    hat: hat.clone(),
                    inner: hooks.policy,
                };
                governed
                    .with_tools(truerise_core::ToolHooks {
                        policy: &stacked,
                        provider: hooks.provider,
                    })
                    .run(params.goal, params.rounds)
            }
            (Some(hooks), None) => governed.with_tools(hooks).run(params.goal, params.rounds),
            (None, _) => governed.run(params.goal, params.rounds),
        }
    }
}

/// Shared run core used by BOTH the repl adapter and the direct path
/// (single-track: there is exactly one governed run implementation).
/// Execution-free, unbounded-budget, hatless: the golden-locked baseline.
pub fn run_books(
    goal: &str,
    rounds: usize,
) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    let dir = std::env::temp_dir().join("truerise_cli_run");
    let router = ScriptRouter::plain("scripted");
    run_books_in(RunParams {
        chain_dir: &dir,
        router: &router,
        hooks: None,
        budget: None,
        hat_toml: None,
        memory: None,
        goal,
        rounds,
    })
}

/// Print the three-books summary (shared formatting for run/repl).
pub fn print_books(books: &truerise_core::RunBooks) -> String {
    format!(
        "run complete (three books):
  transcript entries: {}
  transitions:        {}
  audit rows:         {}",
        books.transcript.len(),
        books.transitions.len(),
        books.audit_rows
    )
}

/// The binary entry (arg dispatch; the shell below is three lines).
pub fn cli_main(args: Vec<String>) -> i32 {
    let cmd = args.get(1).map(String::as_str).unwrap_or("repl");
    match cmd {
        "demo" => {
            let honest = args.iter().any(|a| a == "--honest");
            // The verdict drives the exit code (same contract as the Python
            // CLI): a FAIL has to be able to stop a CI lane.
            i32::from(!demo::run_demo(honest))
        }
        "run" => {
            let goal = args
                .iter()
                .position(|a| a == "--goal")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_else(|| "(no goal given)".into());
            // Repeated --approve <tool>: records approvals on file BEFORE the
            // run (the only transition into ask-allowed; RFC 0002).
            let mut approvals: Vec<String> = Vec::new();
            let mut idx = 0;
            while idx < args.len() {
                if args[idx] == "--approve" {
                    match args.get(idx + 1) {
                        Some(tool) => approvals.push(tool.clone()),
                        None => {
                            eprintln!("usage error: --approve needs a tool name");
                            return 2;
                        }
                    }
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            // --baseline <path>: ratchet baseline for the run verdict
            // (default: the repo-root baselines/run_scores.json, walked up
            // from the cwd like git).
            let explicit_baseline: Option<PathBuf> = {
                let mut it = args.iter();
                let mut found = None;
                while let Some(arg) = it.next() {
                    if arg == "--baseline" {
                        match it.next() {
                            Some(p) => found = Some(Some(PathBuf::from(p))),
                            None => {
                                eprintln!("usage error: --baseline needs a path");
                                return 2;
                            }
                        }
                    }
                }
                found.unwrap_or(None)
            };
            // --budget <n>: per-hat quota for the run (rounds + tool units).
            // Over budget stops the run with exit 1 (a stop is a failure).
            let budget_units: Option<u64> = {
                let mut it = args.iter();
                let mut found = None;
                while let Some(arg) = it.next() {
                    if arg == "--budget" {
                        match it.next().and_then(|v| v.parse::<u64>().ok()) {
                            Some(n) => found = Some(Some(n)),
                            None => {
                                eprintln!("usage error: --budget needs a number");
                                return 2;
                            }
                        }
                    }
                }
                found.unwrap_or(None)
            };
            let baseline = explicit_baseline.or_else(find_default_baseline);
            let hat_toml: Option<String> = {
                let mut it = args.iter();
                let mut found = None;
                while let Some(arg) = it.next() {
                    if arg == "--hat" {
                        match it.next() {
                            Some(p) => match std::fs::read_to_string(p) {
                                Ok(toml_text) => found = Some(Some(toml_text)),
                                Err(e) => {
                                    eprintln!("usage error: cannot read hat {p}: {e}");
                                    return 2;
                                }
                            },
                            None => {
                                eprintln!("usage error: --hat needs a path to a hat TOML");
                                return 2;
                            }
                        }
                    }
                }
                found.unwrap_or(None)
            };
            run_commandline(
                &goal,
                &approvals,
                budget_units,
                baseline.as_deref(),
                hat_toml.as_deref(),
            )
        }
        "repl" => {
            interactive::repl();
            0
        }
        "onboarding" => {
            interactive::onboarding();
            0
        }
        "evolve" => {
            // `evolve --demo [--approve <hash>] [--inject-regression]` /
            // `evolve --replay --cert <path>` (W7, RFC 0003).
            let replay = args.iter().any(|a| a == "--replay");
            if replay {
                let cert = args
                    .iter()
                    .position(|a| a == "--cert")
                    .and_then(|i| args.get(i + 1))
                    .map(PathBuf::from);
                match cert {
                    Some(p) => return evolve_cmd::evolve_replay(&p),
                    None => {
                        eprintln!("usage error: --replay needs --cert <path>");
                        return 2;
                    }
                }
            }
            if !args.iter().any(|a| a == "--demo") {
                eprintln!(
                    "usage: truerise evolve --demo [--approve <hash>] | --replay --cert <path>"
                );
                return 2;
            }
            let approve = args
                .iter()
                .position(|a| a == "--approve")
                .and_then(|i| args.get(i + 1))
                .cloned();
            let inject = args.iter().any(|a| a == "--inject-regression");
            let tape: Option<PathBuf> = {
                let mut it = args.iter();
                let mut found = None;
                while let Some(arg) = it.next() {
                    if arg == "--tape" {
                        match it.next() {
                            Some(p) => found = Some(Some(PathBuf::from(p))),
                            None => {
                                eprintln!("usage error: --tape needs a path");
                                return 2;
                            }
                        }
                    }
                }
                found.unwrap_or(None)
            };
            let dir = std::env::temp_dir().join("truerise_cli_evolve");
            evolve_cmd::evolve_demo(&dir, approve.as_deref(), inject, tape.as_deref())
        }
        "verify" => {
            // `verify --cert <path> [--issuer-key <hex>]` (W10): exit 0
            // verified / 1 invalid or unverified (never silent trust).
            let cert = args
                .iter()
                .position(|a| a == "--cert")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            let Some(cert) = cert else {
                eprintln!("usage: truerise verify --cert <path> [--issuer-key <hex>]");
                return 2;
            };
            let trusted = args
                .iter()
                .position(|a| a == "--issuer-key")
                .and_then(|i| args.get(i + 1))
                .cloned();
            match identity::verify_certificate(&cert, trusted.as_deref()) {
                identity::VerifyOutcome::Verified { issuer } => {
                    println!("verified: issuer {issuer}");
                    0
                }
                identity::VerifyOutcome::UnverifiedIssuer => {
                    eprintln!("unverified issuer (unsigned, or no trusted key configured)");
                    1
                }
                identity::VerifyOutcome::InvalidSignature => {
                    eprintln!("invalid signature (tampered, or signed by another key)");
                    1
                }
                identity::VerifyOutcome::Unparsable => {
                    eprintln!("certificate unparsable");
                    1
                }
            }
        }
        "mcp" => {
            // `mcp --serve [--chain <dir>]`: zero-exposure server over stdio.
            let serve = args.iter().any(|a| a == "--serve");
            if !serve {
                eprintln!("usage: truerise mcp --serve [--chain <dir>]");
                return 2;
            }
            let chain_dir = args
                .iter()
                .position(|a| a == "--chain")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("truerise_cli_mcp"));
            mcp_serve::serve(&chain_dir)
        }
        "--help" | "-h" | "help" => {
            print_help();
            0
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            2
        }
    }
}
