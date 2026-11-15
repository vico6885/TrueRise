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

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Hex-encode a sha2 digest (LowerHex is not implemented for the output array).
pub fn hex_digest(out: impl AsRef<[u8]>) -> String {
    let mut hex = String::with_capacity(64);
    for b in out.as_ref() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}
use truerise_governance::HashChain;
use truerise_wire::Severity;

pub mod assembly;

pub use assembly::{GovernedPolicy, RunTooling};

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
// demo: S1 pay-later seal / S2 many-shots / S3 self-signed evidence
// ---------------------------------------------------------------------------

/// Derive the three scenario reasons from inputs (pure; verdict = reasons empty ? PASS : FAIL).
pub fn demo_scenarios(honest: bool) -> (Vec<String>, String) {
    let mut reasons = Vec::new();

    // S1: thresholds sealed, then (dishonestly) mutated before the verdict.
    let sealed = r#"{"min_gain_pct":10,"significance":0.05}"#;
    let mut hasher = Sha256::new();
    hasher.update(sealed);
    let seal = hex_digest(hasher.finalize());
    let current = if honest {
        sealed.to_string()
    } else {
        r#"{"min_gain_pct":10,"significance":0.03}"#.to_string()
    };
    let mut check = Sha256::new();
    check.update(current.as_bytes());
    let current_seal = hex_digest(check.finalize());
    if current_seal != seal {
        reasons.push("1) prereg diff: thresholds mutated after seal".into());
    }

    // S2: all trials counted; the corrected bar tightens with n.
    let trials: u64 = if honest { 1 } else { 1000 };
    let claim_p = if honest { 0.04 } else { 0.03 };
    let corrected = 0.05_f64 / trials as f64;
    if claim_p > corrected {
        reasons.push(format!(
            "2) trials={trials} -> corrected significance {corrected:.1e}, claim p={claim_p} does not clear it"
        ));
    }

    // S3: unsigned records are self-report = no evidence.
    if !honest {
        reasons.push("3) evidence: 0 executor-signed records (self-report only)".into());
    }

    (reasons, seal)
}

/// Print the demo walkthrough and record the verdict on the demo chain.
///
/// Returns whether the claim passed, so the binary can map the verdict onto
/// the exit code exactly like the Python CLI does (FAIL is a gate, not a note).
pub fn run_demo(honest: bool) -> bool {
    let (reasons, seal) = demo_scenarios(honest);
    let passed = reasons.is_empty();
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "CLAIM     : self-improvement +23% (n={} trials, best-of)",
        if honest { 1 } else { 1000 }
    );
    println!("FINGERPRINT: model=m* harness=h* env=e* evaluator=v*");
    println!("VERDICT   : {verdict}");
    for r in &reasons {
        println!("            {r}");
    }
    println!(
        "CERTIFICATE: sealed at {} (verdict derives from inputs; --honest flips PASS)",
        &seal[..12]
    );
    // The demo verdict lands on a chain (auditability of the demo itself).
    if let Ok(chain) = HashChain::open(&demo_chain_path()) {
        let _ = chain.append(
            "cli.demo.verdict",
            Severity::Info,
            &serde_json::json!({"verdict": verdict, "honest": honest}),
            None,
        );
    }
    passed
}

/// The per-process demo chain location (temp).
pub fn demo_chain_path() -> PathBuf {
    std::env::temp_dir().join(format!("truerise_cli_demo_{}.jsonl", std::process::id()))
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
/// budget enforced per W3 - `budget_units` is the per-hat round/tool quota).
/// Returns the process exit code (0 run completed / 1 run failed - a budget
/// stop IS a failure, never a note).
pub fn run_commandline(goal: &str, approvals: &[String], budget_units: Option<u64>) -> i32 {
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
    match run_books_in(
        &dir,
        &router,
        Some(hooks),
        Some(("cli-run", &run_budget)),
        goal,
        1,
    ) {
        Ok(books) => {
            println!("run complete (three books):");
            println!("  transcript entries: {}", books.transcript.len());
            println!("  transitions:        {}", books.transitions.len());
            println!("  audit rows:         {}", books.audit_rows);
            0
        }
        Err(e) => {
            eprintln!("run failed: {e}");
            1
        }
    }
}

// ---------------------------------------------------------------------------
// REPL + onboarding
// ---------------------------------------------------------------------------

/// The interactive read-eval loop (IO adapter over repl_turn only).
pub fn repl() {
    println!(
        "truerise repl - /demo /audit /quit (IO adapter only; multi-turn semantics reuse the loop path)"
    );
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        print!("truerise> ");
        std::io::stdout().flush().ok();
        line.clear();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        match trimmed {
            "" => continue,
            "/quit" | "/exit" => break,
            "/demo" => {
                run_demo(false);
            }
            "/audit" => {
                if let Ok(chain) = HashChain::open(&demo_chain_path()) {
                    println!(
                        "audit: {} row(s), verified: {}",
                        chain.len(),
                        chain.verify().is_ok()
                    );
                } else {
                    println!("audit: no chain yet (run /demo first)");
                }
            }
            text => {
                // Reuse the run path (single book write) - no bare-face entry.
                let _code = run_commandline(text, &[], None);
            }
        }
    }
}

/// The first-run wizard (writes a secret-free config skeleton).
pub fn onboarding() {
    let config_path = PathBuf::from("config.toml");
    if config_path.exists() {
        println!("onboarding: config.toml already exists - nothing to do");
        return;
    }
    println!("onboarding wizard (secrets NEVER land in this file):");
    println!("  1. pick a provider: openai-compatible / anthropic / custom base_url");
    println!("  2. put your key in the environment (TRUERISE_API_KEY) or an OS keyring");
    println!("     - argv and config files are forbidden for keys");
    println!("  3. tool execution through the sandbox port (planned - see docs/PROGRESS.md)");
    let sample = "[provider]\nkind = \"openai-compatible\"\nbase_url = \"https://example.invalid/v1\"\nmodel_route = \"default\"\n\n# NO keys here: TRUERISE_API_KEY env or OS keyring only.\n";
    if std::fs::write(&config_path, sample).is_ok() {
        println!("wrote config.toml (provider skeleton; no secrets). Set TRUERISE_API_KEY next.");
    } else {
        eprintln!("could not write config.toml in this directory");
    }
}

// HashMap import kept for the router signature shape used by the loop port.
#[allow(unused_imports)]
use std::collections::HashMap as _HashMap;

/// The repl's non-interactive equivalent: one line in, the SAME run path.
pub fn repl_turn(line: &str) -> String {
    let trimmed = line.trim();
    match trimmed {
        "/demo" => {
            run_demo(false);
            "demo printed".into()
        }
        "/audit" => {
            if let Ok(chain) = HashChain::open(&demo_chain_path()) {
                format!(
                    "audit: {} row(s), verified: {}",
                    chain.len(),
                    chain.verify().is_ok()
                )
            } else {
                "audit: no chain yet (run /demo first)".into()
            }
        }
        "" => String::new(),
        text => {
            // Free text reuses the run path (no bare-face entry).
            run_books(text, 1)
                .map(|b| {
                    format!(
                        "run complete (three books): transcript {} / transitions {} / audit {}",
                        b.transcript.len(),
                        b.transitions.len(),
                        b.audit_rows
                    )
                })
                .unwrap_or_else(|e| format!("run failed: {e}"))
        }
    }
}

/// Direct-loop path for the golden comparison (no repl adapter at all).
pub fn direct_loop_books(goal: &str) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    run_books(goal, 1)
}

/// The single governed-run implementation (single-track rule): BOTH the
/// repl path and the run subcommand land here. ``hooks`` wire the RFC 0002
/// enforcement chain; `None` keeps the run execution-free (byte-identical
/// to the pre-RFC behaviour, which the golden test locks). ``budget``
/// wires the W3 enforcement (hat id + ledger port); `None` = unbounded.
pub fn run_books_in(
    chain_dir: &Path,
    router: &dyn truerise_core::ModelRouter,
    hooks: Option<truerise_core::ToolHooks<'_>>,
    budget: Option<(&str, &dyn truerise_core::Budget)>,
    goal: &str,
    rounds: usize,
) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    std::fs::create_dir_all(chain_dir).ok();
    let chain = HashChain::open(&chain_dir.join("run.jsonl"))
        .map_err(|e| truerise_core::LoopError::Sink(e.to_string()))?;
    let sink = ChainSinkBridge(&chain);
    let config = truerise_core::LoopConfig {
        pinned_model: "scripted".into(),
        retry: truerise_core::RetryPolicy::default(),
    };
    let mut governed = truerise_core::Loop::new(router, config, &sink);
    if let Some(hooks) = hooks {
        governed = governed.with_tools(hooks);
    }
    if let Some((hat, budget)) = budget {
        governed = governed.with_budget(hat, budget);
    }
    governed.run(goal, rounds)
}

/// Shared run core used by BOTH the repl adapter and the direct path
/// (single-track: there is exactly one governed run implementation).
/// Execution-free and unbounded-budget: no hooks wired on this path.
pub fn run_books(
    goal: &str,
    rounds: usize,
) -> Result<truerise_core::RunBooks, truerise_core::LoopError> {
    let dir = std::env::temp_dir().join("truerise_cli_run");
    let router = ScriptRouter::plain("scripted");
    run_books_in(&dir, &router, None, None, goal, rounds)
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
            i32::from(!run_demo(honest))
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
            run_commandline(&goal, &approvals, budget_units)
        }
        "repl" => {
            repl();
            0
        }
        "onboarding" => {
            onboarding();
            0
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
