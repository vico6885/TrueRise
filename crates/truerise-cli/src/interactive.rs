//! The interactive surface (REPL + onboarding): an IO adapter ONLY - the
//! multi-turn semantics reuse the same loop path (no bare-face entry).

use std::io::{BufRead, Write};
use std::path::PathBuf;

use truerise_governance::HashChain;

use crate::demo::{demo_chain_path, run_demo};
use crate::{find_default_baseline, run_books, run_commandline};

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
                let baseline = find_default_baseline();
                let _code = run_commandline(text, &[], None, baseline.as_deref(), None);
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
    println!(
        "  3. tool execution runs through the sandbox port (write_file is Ask by default: unapproved writes are audited, never executed)"
    );
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
