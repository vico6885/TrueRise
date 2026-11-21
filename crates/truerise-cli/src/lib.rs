//! truerise-cli: the single-binary entry (REPL / run / demo / onboarding).
//
// - `demo`: the integrity three-scenario walkthrough (verdicts derive
//   from inputs, so `--honest` flips PASS).
// - `run --goal`: a governed minimal run (three books) over a scripted
//   router; without a provider it explains the onboarding path.
// - REPL: stdin loop with /demo /audit slash commands (IO adapter ONLY -
//   multi-turn reuses the same loop path, no bare-face entry). Onboarding
//   writes config.toml WITHOUT secrets (keys live in env).

use std::path::{Path, PathBuf};

use truerise_governance::HashChain;

pub mod assembly;
pub mod baseline;
pub mod completion_cert;
pub mod demo;
pub mod evolve_cmd;
pub mod giveup_cmd;
pub mod identity;
pub mod intent_cmd;
pub mod interactive;
pub mod mcp_serve;
pub mod provider_anthropic;
pub mod provider_choice;
pub mod provider_common;
pub mod provider_openai;
pub mod router_assembly;
pub mod tape;
pub mod verify_cmd;

pub use assembly::{GovernedPolicy, RunTooling};
pub use baseline::{find_default_baseline, grade_exit};
pub use demo::{demo_scenarios, run_demo};
pub use interactive::{onboarding, repl, repl_turn};
pub use provider_choice::{ProviderKind, ProviderSpec, build_provider_router};
pub use router_assembly::{ChainSinkBridge, ScriptRouter};

// truerise-cli library surface (the binary is a thin IO shell over this; the
// REPL is an IO adapter over THESE functions only, spec §6.5 - repl == loop).

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

/// The `run --goal` subcommand: one governed run over the FULL assembly
/// (scripted router by default; `--provider` swaps in a REAL model behind
/// the same port). Budget per W3; judged verdict per W4. Exit codes:
/// 0 PASS / 1 FAIL or failed run / 2 usage error.
pub fn run_commandline(
    goal: &str,
    approvals: &[String],
    budget_units: Option<u64>,
    baseline_path: Option<&Path>,
    hat_toml: Option<&str>,
    provider: Option<&ProviderSpec>,
) -> i32 {
    // Review fix (flake root cure): the run chain dir is THE shared dir
    // (env-overridable) - the intent.seal row and the run's own books must
    // land on the SAME chain, never on two divergent ledgers.
    let dir = intent_cmd::run_chain_dir();
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
    // Provider selection: a REAL model behind the same port, or the
    // scripted assembly when none is given (the default stays offline).
    let owned_router: Box<dyn truerise_core::ModelRouter>;
    let pinned: String;
    if let Some(spec) = provider {
        let (router, model) = match build_provider_router(spec) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("run: {e}");
                return 1;
            }
        };
        owned_router = router;
        pinned = model;
    } else {
        owned_router = Box::new(ScriptRouter::tool_calling("scripted"));
        pinned = "scripted".to_string();
    };
    let router = &*owned_router;
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
        router,
        hooks: Some(hooks),
        budget: Some(("cli-run", &run_budget)),
        hat_toml,
        memory: Some(&recall_bridge),
        goal,
        rounds: 1,
        pinned_model: Some(&pinned),
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
                // The CLI run always wires the recall bridge (W6) - the
                // verdict row therefore carries memory_recall: true, which
                // excludes it from sequential correction and baselining.
                memory_recall: true,
                inconclusive: false,
            };
            let baseline_loaded = match baseline::load_baseline(baseline_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("run: {e}");
                    return 1;
                }
            };
            match truerise_eval::judge_run(&facts, &baseline_loaded, &chain) {
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
    /// The pinned model for the WHOLE run (round pinning); `None` = the
    /// scripted assembly's "scripted".
    pub pinned_model: Option<&'a str>,
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
            // Validate BEFORE registering (refused hat = no chain row), then
            // the G29 skill-load gate: preconditions citing a marked-unwashed
            // assumption are rejected (ledger rebuilds from chain rows).
            let hat = truerise_hats::parse_hat(hat_toml).map_err(|e| {
                truerise_core::LoopError::Config(format!("hat refused at load: {e}"))
            })?;
            assembly::validate_hat_tools(&hat).map_err(truerise_core::LoopError::Config)?;
            let ledger = truerise_governance::UnverifiedLedger::reconstruct_from_chain(&chain);
            let hat = truerise_hats::load_hat_gated(hat_toml, &chain, &ledger)
                .map_err(|e| truerise_core::LoopError::Config(format!("hat refused: {e}")))?;
            Some(hat)
        }
        None => None,
    };
    let config = truerise_core::LoopConfig {
        pinned_model: params.pinned_model.unwrap_or("scripted").to_string(),
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
    let dir = intent_cmd::run_chain_dir();
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
        pinned_model: None,
    })
}

/// Shared run/repl summary (three books).
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

/// The binary entry (arg dispatch; the shell is three lines).
pub fn cli_main(args: Vec<String>) -> i32 {
    let cmd = args.get(1).map(String::as_str).unwrap_or("repl");
    match cmd {
        "demo" => {
            // The verdict drives the exit code (FAIL stops a CI lane).
            i32::from(!demo::run_demo(args.iter().any(|a| a == "--honest")))
        }
        "run" => {
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
            let provider = match provider_choice::parse_provider_args(&args) {
                Ok(p) => p,
                Err(usage) => {
                    eprintln!("usage error: {usage}");
                    return 2;
                }
            };
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
            // G32: the whole intent flow (load/seal/record before the run,
            // verify-unchanged after it) lives in intent_cmd - a contract
            // that changed mid-run voids the run (exit 1 + intent.changed).
            let run_with = |goal: &str| {
                run_commandline(
                    goal,
                    &approvals,
                    budget_units,
                    baseline.as_deref(),
                    hat_toml.as_deref(),
                    provider.as_ref(),
                )
            };
            match intent_cmd::intent_guarded_run(&args, &run_with) {
                Ok(code) => code,
                Err(usage) => {
                    eprintln!("usage error: {usage}");
                    2
                }
            }
        }
        "giveup" => giveup_cmd::handle_command(&args),
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
            let completion = match completion_cert::parse_completion_args(&args) {
                Ok(c) => c,
                Err(usage) => {
                    eprintln!("usage error: {usage}");
                    return 2;
                }
            };
            evolve_cmd::evolve_demo(
                &dir,
                approve.as_deref(),
                inject,
                tape.as_deref(),
                completion
                    .as_ref()
                    .map(|b| b as &completion_cert::CompletionBinding),
            )
        }
        "verify" => {
            // `verify --cert <path> [--issuer-key <hex>]` (W10): exit 0
            // verified / 1 invalid or unverified (never silent trust).
            // `verify --bundle <path> --intent <toml>` (G39): the self-service
            // acceptance package - exit 0 only when EVERY observation passed.
            let bundle = args
                .iter()
                .position(|a| a == "--bundle")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            let intent = args
                .iter()
                .position(|a| a == "--intent")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            if let Some(bundle) = bundle {
                let Some(intent) = intent else {
                    eprintln!("usage: truerise verify --bundle <path> --intent <contract.toml>");
                    return 2;
                };
                return match verify_cmd::run_bundle(&bundle, &intent) {
                    Ok(report) => {
                        println!("{}", report.render());
                        report.exit_code()
                    }
                    Err(usage) => {
                        eprintln!("usage error: {usage}");
                        2
                    }
                };
            }
            let cert = args
                .iter()
                .position(|a| a == "--cert")
                .and_then(|i| args.get(i + 1))
                .map(PathBuf::from);
            let Some(cert) = cert else {
                eprintln!("usage: truerise verify --cert <path> [--issuer-key <hex>]");
                return 2;
            };
            let trusted = match identity::parse_issuer_key_args(&args) {
                Ok(t) => t,
                Err(usage) => {
                    eprintln!("usage error: {usage}");
                    return 2;
                }
            };

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
