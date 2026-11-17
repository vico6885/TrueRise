//! truerise-eval: the conformance JUDGING AUTHORITY, ratchet, fingerprints,
//! and judge-agreement statistics (pure numbers, no authority).

pub mod fingerprint;
pub mod interception;
pub mod judge;
pub mod ratchet;
pub mod derivable;
pub mod run_verdict;
pub mod significance;
pub mod stability;

pub use fingerprint::{CompareForbidden, Fingerprint, FingerprintRegistry};
pub use judge::{AgreementStats, Verdict, judge, judge_agreement, judge_and_record};
pub use ratchet::{Ratchet, RatchetViolation};
pub use run_verdict::{EvalError, RunFacts, RunVerdict, judge_run, judge_tasks};
pub use significance::{ArmResult, CompareError, DISCORDANT_FLOOR, Significance, mcnemar_exact};
pub use derivable::{check_all, DerivableClaim, DerivableVerdict};
pub use interception::{mode_for, Appeal, Interception, InterceptionLedger, Mode, Outcome};
pub use stability::{certificate_row as stability_row, check as check_stability, Resample, Stability};
