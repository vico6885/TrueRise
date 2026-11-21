//! Process-level grading (G10): "did the run EARN the pass, or did it get
//! lucky?" - read from the signed trajectory (G9), not just the terminal
//! state.
//!
//! Four detectors over the VERIFIED trajectory steps, each a pure function
//! with a declared step-kind vocabulary:
//!
//! 1. `RetryLoop`: the same (kind, content) repeated back-to-back - a
//!    regression-style loop with no new information between attempts.
//! 2. `VerificationGap`: a completion declared with NO self-verification
//!    executed after the last content change - the pass cannot be caused
//!    by a verified state.
//! 3. `CriteriaMissing`: a completion declared without any acceptance-
//!    criteria execution at all - "判据未执行却宣布完成".
//! 4. `StaleCriteria`: the criteria ran BEFORE the last content change -
//!    the declared criteria were executed against a state that no longer
//!    exists ("通过前最后一次变更与通过无因果关系").
//!
//! Verdicts: `Solid` (no signals), `Lucky` (a pass the process does not
//! support - it MAY still be correct, but nothing in the trajectory shows
//! why), `Suspicious` (retry loops - the process actively smells). The
//! scoring contract is the point of the whole gate: LUCKY PASSES ARE NOT
//! SCORED. They feed a public lucky-pass rate instead (that number is what
//! a buyer wants to see; inflating a score is what a seller wants).
//!
//! Grading consumes ONLY verified steps (G9): an unsigned or tampered step
//! contributes no evidence, so the grade is reproducible from the
//! trajectory by construction (acceptance:
//! `process_grade_reproducible_from_trajectory`).

use truerise_observe::trajectory::Trajectory;

/// The step-kind vocabulary the grader recognizes (kinds are free-form
/// strings on the trajectory; the vocabulary maps them onto roles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    /// Kinds that CHANGE content (build/patch/apply).
    pub build: Vec<String>,
    /// Kinds that run self-verification.
    pub verify: Vec<String>,
    /// Kinds that execute acceptance criteria.
    pub criteria: Vec<String>,
    /// Kinds that declare completion (the pass claim itself).
    pub declare: Vec<String>,
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            build: vec!["build".to_string(), "patch".to_string()],
            verify: vec!["verify".to_string(), "self_check".to_string()],
            criteria: vec!["criteria".to_string(), "acceptance".to_string()],
            declare: vec!["declare".to_string(), "complete".to_string()],
        }
    }
}

impl Vocabulary {
    fn is_any(&self, kind: &str, list: &[String]) -> bool {
        list.iter().any(|k| k == kind)
    }
}

/// The process-grade verdict over one trajectory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessGrade {
    /// No process signals: the pass is supported by the trajectory.
    Solid,
    /// A pass the process does not support (no criteria, or criteria ran
    /// against a stale state). Not scored - see [`LuckyRate`].
    Lucky,
    /// The process actively smells (regression loops).
    Suspicious,
}

/// One detector signal with its evidence (step indices into the VERIFIED
/// step list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The same (kind, content) ran back-to-back `count` times.
    RetryLoop {
        /// Index of the first step of the run.
        start: usize,
        /// Repeated kind.
        kind: String,
        /// How many identical back-to-back attempts.
        count: usize,
    },
    /// A completion was declared with no verification after the last
    /// content change.
    VerificationGap {
        /// Last content-changing step index.
        last_build: Option<usize>,
        /// The declaring step index.
        declared_at: usize,
    },
    /// A completion was declared with no criteria execution at all.
    CriteriaMissing {
        /// The declaring step index.
        declared_at: usize,
    },
    /// The criteria ran BEFORE the last content change: they judged a
    /// state that no longer exists.
    StaleCriteria {
        /// Last criteria-execution index.
        last_criteria: usize,
        /// The later content-change index.
        last_build: usize,
    },
}

/// The grading report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessReport {
    /// The verdict.
    pub grade: ProcessGrade,
    /// The signals behind the verdict (empty for Solid).
    pub signals: Vec<Signal>,
    /// How many verified steps the grade consumed (the trajectory may
    /// carry more, unsigned - those proved nothing).
    pub evidence_steps: usize,
}

/// Grade one trajectory. Pure function over the verified steps: the same
/// trajectory always yields the same report.
#[must_use]
pub fn grade_trajectory(trajectory: &Trajectory, vocab: &Vocabulary) -> ProcessReport {
    let verified = trajectory.evidence_steps();
    let kinds: Vec<&str> = verified.iter().map(|s| s.kind.as_str()).collect();
    let mut signals: Vec<Signal> = Vec::new();

    // Detector 1: consecutive identical (kind, content_hash) runs.
    let mut i = 0usize;
    while i < verified.len() {
        let mut run = 1usize;
        while i + run < verified.len()
            && verified[i + run].kind == verified[i].kind
            && verified[i + run].content_hash == verified[i].content_hash
        {
            run += 1;
        }
        if run >= 2 {
            signals.push(Signal::RetryLoop {
                start: i,
                kind: verified[i].kind.clone(),
                count: run,
            });
        }
        i += run;
    }

    // Last indices per role.
    let last_of = |list: &[String]| -> Option<usize> {
        (0..verified.len())
            .rev()
            .find(|&idx| vocab.is_any(kinds[idx], list))
    };
    let last_build = last_of(&vocab.build);
    let last_criteria = last_of(&vocab.criteria);

    // Detectors 2-4 are about COMPLETION declarations.
    let declare_positions: Vec<usize> = (0..verified.len())
        .filter(|&idx| vocab.is_any(kinds[idx], &vocab.declare))
        .collect();
    for &declared_at in &declare_positions {
        // Detector 3: no criteria execution anywhere before the declaration.
        if last_criteria.is_none() {
            signals.push(Signal::CriteriaMissing { declared_at });
        }
        // Detector 2 (independent of 3): no verification after the last
        // content change - the pass cannot be caused by a verified state.
        let verify_after_build = last_build.is_none_or(|lb| {
            (lb + 1..=declared_at).any(|idx| vocab.is_any(kinds[idx], &vocab.verify))
        });
        if !verify_after_build {
            signals.push(Signal::VerificationGap {
                last_build,
                declared_at,
            });
        }
        // Detector 4: the criteria ran before the last content change -
        // they judged a state that no longer exists.
        if let (Some(lc), Some(lb)) = (last_criteria, last_build)
            && lc < lb
        {
            signals.push(Signal::StaleCriteria {
                last_criteria: lc,
                last_build: lb,
            });
        }
    }
    // An EMPTY verified trajectory proves nothing: there is no process to
    // bless, so it can never be Solid (a "pass" with zero evidence is the
    // definition of lucky).
    if verified.is_empty() && !signals.is_empty() {
        // fall through - signals alone decide below
    }
    if verified.is_empty() && !declare_positions.is_empty() {
        // declared without any evidence at all
    }

    let grade = if signals
        .iter()
        .any(|s| matches!(s, Signal::RetryLoop { .. }))
    {
        ProcessGrade::Suspicious
    } else if verified.is_empty()
        || signals.iter().any(|s| {
            matches!(
                s,
                Signal::CriteriaMissing { .. }
                    | Signal::StaleCriteria { .. }
                    | Signal::VerificationGap { .. }
            )
        })
    {
        // No evidence at all, or process signals: the pass is not
        // supported by its own trajectory - Lucky (never Solid on empty).
        ProcessGrade::Lucky
    } else {
        ProcessGrade::Solid
    };
    ProcessReport {
        grade,
        signals,
        evidence_steps: verified.len(),
    }
}

/// The score over a set of graded trajectories (acceptance:
/// `lucky_pass_is_not_scored` - Lucky contributes ZERO).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    /// Scored (Solid) trajectories.
    pub solid: usize,
    /// Lucky trajectories - present in the report, absent from the score.
    pub lucky: usize,
    /// Suspicious trajectories - also unscored (and flagged for review).
    pub suspicious: usize,
}

impl Score {
    /// Aggregate the scores over graded reports.
    #[must_use]
    pub fn from_grades(grades: &[ProcessGrade]) -> Self {
        Self {
            solid: grades
                .iter()
                .filter(|g| matches!(g, ProcessGrade::Solid))
                .count(),
            lucky: grades
                .iter()
                .filter(|g| matches!(g, ProcessGrade::Lucky))
                .count(),
            suspicious: grades
                .iter()
                .filter(|g| matches!(g, ProcessGrade::Suspicious))
                .count(),
        }
    }

    /// The SCORE: Solid passes only. A lucky pass is worth exactly nothing
    /// here - by design.
    #[must_use]
    pub fn value(&self) -> usize {
        self.solid
    }
}

/// The public lucky-pass rate over a batch (acceptance:
/// `lucky_pass_rate_is_reported`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuckyRate {
    /// Lucky trajectories.
    pub lucky: usize,
    /// Total graded trajectories.
    pub total: usize,
}

impl LuckyRate {
    /// Aggregate from graded reports.
    #[must_use]
    pub fn from_grades(grades: &[ProcessGrade]) -> Self {
        Self {
            lucky: grades
                .iter()
                .filter(|g| matches!(g, ProcessGrade::Lucky))
                .count(),
            total: grades.len(),
        }
    }

    /// The rate in per-mille (integer; 0 when the batch is empty).
    #[must_use]
    pub fn per_mille(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.lucky as u64 * 1000) / self.total as u64
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use ed25519_dalek::SigningKey;
    use truerise_observe::trajectory::TrajectoryBuilder;

    fn build(actions: &[(&str, &[u8])]) -> Trajectory {
        let key = SigningKey::from_bytes(&[42; 32]);
        let mut builder = TrajectoryBuilder::new(&key);
        for (kind, content) in actions {
            builder.append(kind, content);
        }
        builder.finish()
    }

    fn grade(actions: &[(&str, &[u8])]) -> ProcessReport {
        grade_trajectory(&build(actions), &Vocabulary::default())
    }

    #[test]
    fn a_clean_build_verify_criteria_declare_is_solid() {
        let report = grade(&[
            ("build", b"patch the harness"),
            ("verify", b"self-check green"),
            ("criteria", b"acceptance criteria executed"),
            ("declare", b"complete"),
        ]);
        assert_eq!(report.grade, ProcessGrade::Solid);
        assert!(report.signals.is_empty());
        assert_eq!(report.evidence_steps, 4);
    }

    #[test]
    fn criteria_missing_makes_the_pass_lucky() {
        // Declared complete with NO criteria execution anywhere.
        let report = grade(&[
            ("build", b"patch"),
            ("verify", b"self-check green"),
            ("declare", b"complete"),
        ]);
        assert_eq!(report.grade, ProcessGrade::Lucky);
        assert!(
            report
                .signals
                .iter()
                .any(|s| matches!(s, Signal::CriteriaMissing { .. }))
        );
    }

    #[test]
    fn a_verification_gap_is_lucky_not_solid() {
        // Verify ran BEFORE the last build: the verified state is gone.
        let report = grade(&[
            ("build", b"patch v1"),
            ("verify", b"self-check green"),
            ("build", b"patch v2 - unrelated change"),
            ("declare", b"complete"),
        ]);
        assert_eq!(report.grade, ProcessGrade::Lucky);
        assert!(
            report
                .signals
                .iter()
                .any(|s| matches!(s, Signal::VerificationGap { .. }))
        );
    }

    #[test]
    fn retry_loops_make_the_run_suspicious() {
        // The same patch retried identically twice: no new information.
        let report = grade(&[
            ("build", b"same patch"),
            ("build", b"same patch"),
            ("verify", b"green"),
            ("criteria", b"criteria"),
            ("declare", b"complete"),
        ]);
        assert_eq!(report.grade, ProcessGrade::Suspicious);
        assert!(
            report
                .signals
                .iter()
                .any(|s| matches!(s, Signal::RetryLoop { count: 2, .. }))
        );
    }

    #[test]
    fn stale_criteria_flags_the_pass() {
        // Criteria ran, THEN the content changed, THEN declared: the
        // criteria judged a dead state (and no verify after the change).
        let report = grade(&[
            ("build", b"patch v1"),
            ("criteria", b"criteria executed"),
            ("build", b"patch v2"),
            ("declare", b"complete"),
        ]);
        assert_eq!(report.grade, ProcessGrade::Lucky);
        assert!(
            report
                .signals
                .iter()
                .any(|s| matches!(s, Signal::StaleCriteria { .. }))
        );
    }

    /// THE scoring acceptance: a lucky pass contributes ZERO to the score.
    #[test]
    fn lucky_pass_is_not_scored() {
        let grades = vec![
            ProcessGrade::Solid,
            ProcessGrade::Lucky,
            ProcessGrade::Solid,
            ProcessGrade::Suspicious,
        ];
        let score = Score::from_grades(&grades);
        assert_eq!(score.value(), 2, "only Solid passes score");
        assert_eq!(score.lucky, 1);
        assert_eq!(score.suspicious, 1);
    }

    /// THE reporting acceptance: the lucky-pass rate is a public number.
    #[test]
    fn lucky_pass_rate_is_reported() {
        let grades = vec![
            ProcessGrade::Lucky,
            ProcessGrade::Lucky,
            ProcessGrade::Solid,
            ProcessGrade::Solid,
        ];
        let rate = LuckyRate::from_grades(&grades);
        assert_eq!(rate.per_mille(), 500);
        assert_eq!(LuckyRate::from_grades(&[]).per_mille(), 0);
    }

    /// THE reproducibility acceptance: the grade is a pure function of the
    /// trajectory - re-deriving it from the persisted steps reproduces the
    /// identical report, and unsigned steps cannot change it.
    #[test]
    fn process_grade_reproducible_from_trajectory() {
        let actions: &[(&str, &[u8])] = &[
            ("build", b"patch"),
            ("verify", b"green"),
            ("criteria", b"criteria"),
            ("declare", b"complete"),
        ];
        let trajectory = build(actions);
        let first = grade_trajectory(&trajectory, &Vocabulary::default());
        // Offline path: reassemble from the persisted steps WITH the
        // executor key that travels with the trajectory (a fresh seed would
        // be a different executor and would verify nothing).
        let rebuilt = Trajectory::from_steps(trajectory.steps().to_vec(), *trajectory.executor());
        let second = grade_trajectory(&rebuilt, &Vocabulary::default());
        assert_eq!(first, second);
        // Determinism: grading twice on the same handle agrees.
        assert_eq!(grade_trajectory(&trajectory, &Vocabulary::default()), first);
    }

    #[test]
    fn an_empty_trajectory_cannot_be_solid() {
        // Nothing happened: there is no evidence to call Solid (the
        // evidence-step count is zero, and the grade refuses to bless it).
        let report = grade(&[]);
        assert_ne!(report.grade, ProcessGrade::Solid);
        assert_eq!(report.evidence_steps, 0);
        assert!(matches!(report.grade, ProcessGrade::Lucky));
    }
}
