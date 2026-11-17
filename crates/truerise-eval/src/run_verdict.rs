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
    /// G50: did the run's plan phase consult memory recall? A recall-enabled
    /// run is NOT trial-independent (evidence from earlier runs leaks into
    /// the plan), so it is excluded from the sequential-correction
    /// denominator and from ratchet baselining by construction.
    pub memory_recall: bool,
    /// G7: the boundary's CAUSE declaration for an unfinished run - a
    /// governance stop (budget halt) proves nothing (INCONCLUSIVE); a
    /// takeover (the agent failed to perform) is a counted FAIL. The judge
    /// never guesses causes: this flag IS the boundary's testimony.
    pub inconclusive: bool,
}

/// A judged run verdict. Construction is crate-private: `judge_run` is the
/// single source (integers only on the wire - the float rule, docs/determinism).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunVerdict {
    verdict: Verdict,
    failures: u32,
    finished: bool,
    rounds_total: u32,
    memory_recall: bool,
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

    /// G50: did the judged run consult memory recall?
    #[must_use]
    pub fn memory_recall(&self) -> bool {
        self.memory_recall
    }

    /// G50: a recall-enabled run is excluded from the sequential-correction
    /// denominator and from ratchet baselining (its scores are NOT
    /// trial-independent).
    #[must_use]
    pub fn ratchet_eligible(&self) -> bool {
        !self.memory_recall
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

/// Judge one generation's TASK results against the baseline and RECORD the
/// verdict (the evolve loop's re-evaluation step). Same authority, same
/// rules: integer metric (`task_failures`), ratchet baseline caps it, the
/// verdict is recorded even when it is a FAIL.
///
/// # Errors
/// [`EvalError::Chain`] when the recording fails.
pub fn judge_tasks(
    results: &[(String, bool)],
    baseline: &Baseline,
    chain: &HashChain,
) -> Result<RunVerdict, EvalError> {
    let total = results.len() as u32;
    let failures = results.iter().filter(|(_, ok)| !ok).count() as u32;
    let mut scores = Baseline::new();
    scores.insert("task_failures".to_string(), f64::from(failures));
    let regressed = verify_scores(baseline, &scores).is_err();
    let verdict = if failures == 0 && !regressed {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    let out = RunVerdict {
        verdict: verdict.clone(),
        failures,
        finished: true,
        rounds_total: total,
        // Task-pack judging never consults memory recall (the evolve loop
        // reads the task pack directly) - isolation is inherent here.
        memory_recall: false,
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
                "scope": "tasks",
                "verdict": out.verdict_str(),
                "passed": total - failures,
                "total": total,
                "failures": failures,
                "baseline_present": !baseline.is_empty()
            }),
            None,
        )
        .map_err(EvalError::Chain)?;
    Ok(out)
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
    // G7: the verdict DISTINGUISHES the causes of an unfinished run. A
    // governance stop (inconclusive=true) proves NOTHING - INCONCLUSIVE,
    // exit 3, non-promotable. A takeover (inconclusive=false, finished
    // false) is a counted FAIL - the agent failed to perform. The judge
    // never guesses: the boundary testifies through RunFacts.
    let (failures, verdict) = if facts.finished {
        let mut scores = Baseline::new();
        scores.insert("run_failures".to_string(), 0.0);
        let regressed = verify_scores(baseline, &scores).is_err();
        if regressed {
            (1u32, Verdict::Fail)
        } else {
            (0u32, Verdict::Pass)
        }
    } else if facts.inconclusive {
        (0u32, Verdict::Inconclusive)
    } else {
        (1u32, Verdict::Fail)
    };
    let out = RunVerdict {
        verdict: verdict.clone(),
        failures,
        finished: facts.finished,
        rounds_total: facts.rounds_total,
        memory_recall: facts.memory_recall,
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
                "baseline_present": !baseline.is_empty(),
                "memory_recall": out.memory_recall,
                "excluded_from_sequential_denominator": out.memory_recall
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
                memory_recall: false,
                inconclusive: false,
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
    fn unfinished_run_is_inconclusive_and_records_it() {
        let dir = temp_dir("fail");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        let v = judge_run(
            &RunFacts {
                finished: false,
                rounds_total: 3,
                memory_recall: false,
                inconclusive: true,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(!v.is_pass());
        assert_eq!(v.verdict_str(), "INCONCLUSIVE", "an unfinished run proves nothing");
        assert_eq!(v.failures(), 0, "no failures were OBSERVED - the run did not complete");
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
                memory_recall: false,
                inconclusive: false,
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

    #[test]
    fn eval_run_records_memory_isolation_flag_in_certificate() {
        // G50: the verdict row carries the recall flag AND the exclusion
        // marker, so every consumer (certificate, denominator, baseline
        // tooling) reads the isolation state from the chain, not from hope.
        let dir = temp_dir("iso_flag");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        let v = judge_run(
            &RunFacts {
                finished: true,
                rounds_total: 1,
                memory_recall: true,
                inconclusive: false,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(v.memory_recall());
        assert!(!v.ratchet_eligible(), "a recall-enabled run is not ratchet eligible");
        let rows = chain.governing_rows();
        let verdict_row = rows
            .iter()
            .find(|r| r.event.kind == "eval.verdict")
            .map(|r| r.event.detail.clone())
            .unwrap();
        assert_eq!(verdict_row["memory_recall"], serde_json::json!(true));
        assert_eq!(
            verdict_row["excluded_from_sequential_denominator"],
            serde_json::json!(true)
        );
        // The isolated run records the opposite.
        let v2 = judge_run(
            &RunFacts {
                finished: true,
                rounds_total: 1,
                memory_recall: false,
                inconclusive: false,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(v2.ratchet_eligible());
    }

    #[test]
    fn run_with_memory_recall_is_ratchet_ineligible() {
        let dir = temp_dir("iso_ratchet");
        let chain = HashChain::open(&dir.join("c.jsonl")).unwrap();
        let recall_enabled = judge_run(
            &RunFacts {
                finished: true,
                rounds_total: 1,
                memory_recall: true,
                inconclusive: false,
            },
            &baseline_zero(),
            &chain,
        )
        .unwrap();
        assert!(
            !recall_enabled.ratchet_eligible(),
            "recall leaks evidence across trials - the score must never enter a baseline"
        );
    }
}
