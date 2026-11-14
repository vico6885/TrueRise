//! truerise-eval: the conformance JUDGING AUTHORITY, ratchet, fingerprints,
//! and judge-agreement statistics (pure numbers, no authority).

pub mod fingerprint;
pub mod judge;
pub mod ratchet;

pub use fingerprint::{CompareForbidden, Fingerprint, FingerprintRegistry};
pub use judge::{AgreementStats, Verdict, judge, judge_agreement, judge_and_record};
pub use ratchet::{Ratchet, RatchetViolation};
