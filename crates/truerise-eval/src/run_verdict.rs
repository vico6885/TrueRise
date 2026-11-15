//! Run-path judging authority (W4): THE only producer of run verdicts.
//!
//! A governed run ends with facts (finished? rounds?). THIS crate turns the
//! facts into a verdict and records it on the chain - and no other crate can
//! forge one: [`RunVerdict`] has private fields and no public constructor,
//! so the authority is a type-system fact, not a convention. A verdict that
//! cannot be recorded is an ERROR, never a warning (unrecorded verdicts
//! break auditability).

use serde_json::json;
use truerise_governance::HashChain;
use truerise_wire::Severity;

use crate::judge::Verdict;
use crate::ratchet::{Baseline, verify_scores};

/// The facts of one completed governed run (assembled by the boundary from
/// the three books; the judge never invents facts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFacts {
    /// Did the run reach the Finished state (verdict issued, not takeover)?
    pub finished: bool,
    /// How many rounds the run declared.
    pub rounds_total: u32,
}

/// A judged run verdict. Construction is crate-private: `judge_run` is the
/// single source (integers only on the wire - the float rule, docs/determinism).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunVerdict {
    verdict: Verdict,
    failures: u32,
    finished: bool,
    rounds_total: u32,
}

impl RunVerdict {
    /// The wire name of the verdict (PASS / FAIL).
    #[must_use]
    pub fn verdict_str(&self) -> &'static str {
        self.verdict.as_str()
    }

    /// True when the verdict is PASS.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.verdict == Verdict::Pass
    }

    /// Counted failures (0 = clean run).
    #[must_use]
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// The finished flag the verdict was judged from.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// The declared round count the verdict was judged from.
    #[must_use]
    pub fn rounds_total(&self) -> u32 {
        self.rounds_total
    }
}

/// Judge errors. Regression is a VERDICT (recorded, exit-grade), not an
/// error; errors mean the verdict could not be produced or recorded.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    /// The verdict could not be recorded on the chain (an error, never a
    /// warning - unrecorded verdicts break auditability).
    #[error("eval: verdict not recordable: {0}")]
    Chain(#[from] truerise_governance::ChainError),
}

/// Judge one run's facts against the baseline and RECORD the verdict.
/// The metric is integer-valued (`run_failures`); the baseline caps it
/// (ratchet: regression = FAIL, never a silent merge).
///
/// # Errors
/// [`EvalError::Chain`] when the recording fails - the caller MUST treat
/// the run as failed (exit-grade), because an unrecorded verdict is an
/// auditability break.
pub fn judge_run(
    facts: &RunFacts,
    baseline: &Baseline,
    chain: &HashChain,
) -> Result<RunVerdict, EvalError> {
    let failures = u32::from(!facts.finished);
    let mut scores = Baseline::new();
    scores.insert("run_failures".to_string(), f64::from(failures));
    let regressed = verify_scores(baseline, &scores).is_err();
    let verdict = if failures == 0 && !regressed {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    let out = RunVerdict {
        verdict: verdict.clone(),
        failures,
        finished: facts.finished,
        rounds_total: facts.rounds_total,
    };
    chain
        .append(
            "eval.verdict",
            if verdict == Verdict::Pass {
                Severity::Info
            } else {
                Severity::Error
            },
            &json!({
                "verdict": out.verdict_str(),
                "failures": out.failures,
                "finished": out.finished,
                "rounds_total": out.rounds_total,
                "baseline_present": !baseline.is_empty()
            }),
            None,
        )
        .map_err(EvalError::Chain)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "truerise_run_verdict_{tag}_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn baseline_zero() -> Baseline {
        Baseline::from([("run_failures".to_string(), 0.0)])
    }

    #[test]
    fn finished_run_passes_and_records() {
        let dir = temp_dir("pass");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        let v = judge_run(
            &RunFacts {
                finished: true,
                rounds_total: 3,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(v.is_pass());
        assert_eq!(v.failures(), 0);
        chain.verify().unwrap();
    }

    #[test]
    fn unfinished_run_fails_and_records_the_fail() {
        let dir = temp_dir("fail");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        let v = judge_run(
            &RunFacts {
                finished: false,
                rounds_total: 3,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(!v.is_pass());
        assert_eq!(v.failures(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unrecordable_verdict_is_error_not_warning() {
        let dir = temp_dir("unrecordable");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        judge_run(
            &RunFacts {
                finished: true,
                rounds_total: 1,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        // Tamper with the persisted rows. The run path records its verdict
        // by RE-OPENING the chain - and open verifies every persisted row,
        // refusing tampered content. The verdict is unrecordable -> an
        // ERROR (exit-grade), never a warning.
        let path = dir.join("c.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        let tampered = text.replacen("eval.verdict", "eval.verdictX", 1);
        std::fs::write(&path, tampered).unwrap();
        let err = HashChain::open(&path).unwrap_err();
        assert!(matches!(
            err,
            truerise_governance::ChainError::Tampered { .. }
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
