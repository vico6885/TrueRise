//! The conformance JUDGING AUTHORITY (spec §6 conformance row + P0-4 split):
//! the sandbox self-check produces FACTS; THIS is the only place verdicts are
//! produced and recorded. A verdict lands in the eval-side ledger (hash
//! chained via governance) - re-running the judge over the same facts must
//! reproduce the same verdict (offline re-runnable).
//!
//! Judge agreement statistics (spec §6 judge 一致率) are PURE statistics over
//! pairs of verdicts - no authority, just numbers.

use serde_json::json;
use truerise_governance::HashChain;
use truerise_sandbox::ConformanceReport;
use truerise_wire::Severity;

/// A conformance verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// All three must-fail regions pass.
    Pass,
    /// At least one must-fail region failed.
    Fail,
}

impl Verdict {
    /// Wire name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
        }
    }
}

/// Judge a conformance report: ALL three must-fail regions (escape /
/// oversell / residue) must pass for a PASS verdict. Deterministic over the
/// facts - same report in, same verdict out (offline re-runnable).
pub fn judge(report: &ConformanceReport) -> Verdict {
    if report.all_must_fail_regions_pass() {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

/// Judge + RECORD into the eval-side ledger (the verdict becomes part of the
/// hash chain; the recording is the judging authority's act).
///
/// # Errors
/// Chain errors propagate (a verdict that cannot be recorded is an error -
/// unrecorded verdicts would break auditability).
pub fn judge_and_record(
    report: &ConformanceReport,
    chain: &HashChain,
) -> Result<Verdict, truerise_governance::ChainError> {
    let verdict = judge(report);
    let failed_regions: Vec<String> = ["escape", "oversell", "residue"]
        .iter()
        .filter(|r| report.region(r).iter().any(|f| !f.passed))
        .map(|r| (*r).to_string())
        .collect();
    chain.append(
        "eval.conformance.verdict",
        if verdict == Verdict::Pass {
            Severity::Info
        } else {
            Severity::Warn
        },
        &json!({
            "provider": report.provider,
            "verdict": verdict.as_str(),
            "failed_regions": failed_regions,
            "capability": report.capability.as_str(),
        }),
        None,
    )?;
    Ok(verdict)
}

/// Judge agreement: pure statistics over paired verdicts.
#[derive(Debug, Clone, PartialEq)]
pub struct AgreementStats {
    /// Number of pairs.
    pub n: usize,
    /// Raw agreement rate in [0,1].
    pub agreement: f64,
    /// Cohen's kappa (0 = chance agreement, 1 = perfect).
    pub kappa: f64,
}

/// Compute raw agreement + Cohen's kappa over paired verdicts
/// (judge_a[i] vs judge_b[i]); verdicts as booleans (true = PASS).
#[must_use]
pub fn judge_agreement(pairs: &[(bool, bool)]) -> AgreementStats {
    let n = pairs.len();
    if n == 0 {
        return AgreementStats {
            n: 0,
            agreement: 0.0,
            kappa: 0.0,
        };
    }
    let agree = pairs.iter().filter(|(a, b)| a == b).count();
    let a_pass = pairs.iter().filter(|(a, _)| *a).count();
    let b_pass = pairs.iter().filter(|(_, b)| *b).count();
    let agreement = agree as f64 / n as f64;
    let p_a = a_pass as f64 / n as f64;
    let p_b = b_pass as f64 / n as f64;
    let p_e = p_a * p_b + (1.0 - p_a) * (1.0 - p_b);
    let kappa = if (1.0 - p_e).abs() < 1e-9 {
        1.0
    } else {
        (agreement - p_e) / (1.0 - p_e)
    };
    AgreementStats {
        n,
        agreement,
        kappa,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use truerise_sandbox::ProbeFact;

    fn report_with(failing_region: Option<&str>) -> ConformanceReport {
        let mk = |region: &str, passed: bool| ProbeFact {
            region: region.to_string(),
            probe: "probe".to_string(),
            passed,
            detail: String::new(),
        };
        let facts = match failing_region {
            None => vec![
                mk("escape", true),
                mk("oversell", true),
                mk("residue", true),
            ],
            Some(r) => vec![
                mk("escape", true),
                mk("oversell", true),
                mk("residue", true),
            ]
            .into_iter()
            .map(|mut f| {
                if f.region == r {
                    f.passed = false;
                }
                f
            })
            .collect(),
        };
        ConformanceReport {
            provider: "inprocess".into(),
            capability: truerise_sandbox::CapabilityLevel::Full,
            declared_bits: 0,
            facts,
            generated_at_dir: std::path::PathBuf::from("."),
        }
    }

    #[test]
    fn all_regions_pass_yields_pass() {
        assert_eq!(judge(&report_with(None)), Verdict::Pass);
    }

    #[test]
    fn any_region_fail_yields_fail() {
        assert_eq!(judge(&report_with(Some("residue"))), Verdict::Fail);
        assert_eq!(judge(&report_with(Some("escape"))), Verdict::Fail);
    }

    #[test]
    fn verdict_is_recorded_and_reproducible() {
        let dir = std::env::temp_dir().join(format!("truerise_judge_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let chain = HashChain::open(&dir.join("eval.jsonl")).unwrap();
        let report = report_with(Some("oversell"));
        let v1 = judge_and_record(&report, &chain).unwrap();
        let v2 = judge_and_record(&report, &chain).unwrap();
        assert_eq!(v1, v2, "same facts -> same verdict (offline re-runnable)");
        assert_eq!(chain.len(), 2);
        chain.verify().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agreement_stats_and_kappa() {
        // Perfect agreement:
        let perfect = [(true, true), (false, false), (true, true)];
        let s = judge_agreement(&perfect);
        assert!((s.agreement - 1.0).abs() < 1e-9);
        assert!((s.kappa - 1.0).abs() < 1e-9);
        // Chance-level agreement:
        let mixed = [(true, true), (true, false), (false, true), (false, false)];
        let s2 = judge_agreement(&mixed);
        assert!((s2.agreement - 0.5).abs() < 1e-9);
        assert!(
            s2.kappa.abs() < 1e-9,
            "kappa ~0 at chance, got {}",
            s2.kappa
        );
    }
}
