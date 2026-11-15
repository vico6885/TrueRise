//! The integrity demo (Rust edition): three cheating scenarios, caught.
//! Mirrors the Python demo verdict-for-verdict (the dual-implementation
//! gate compares the two).

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use truerise_governance::HashChain;
use truerise_wire::Severity;

/// Hex-encode a sha2 digest (LowerHex is not implemented for the output array).
pub fn hex_digest(out: impl AsRef<[u8]>) -> String {
    let mut hex = String::with_capacity(64);
    for b in out.as_ref() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
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
