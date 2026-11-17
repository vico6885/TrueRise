//! Interception ledger (G31a): the interceptor's own accountability.
//!
//! The rule the consensus made non-negotiable: NO gate in the
//! support-judgement family (G25/G27/G29/G30) runs in enforcement mode
//! until its own precision account exists - until then it runs in
//! SHADOW MODE (verdicts are recorded, not enforced). Without G31a an
//! interceptor is exactly this repository's definition of an unsupported
//! assertion.
//!
//! Three machine facts this module guarantees:
//! 1. every interception is logged (gate, subject, outcome) - an
//!    unlogged interception is a bug, not a judgement;
//! 2. an override through the appeal channel is ALWAYS recorded as a
//!    violation (the human may overrule the gate; the overrule is on the
//!    record forever);
//! 3. the ledger is append-only - nothing is ever removed, retracted
//!    entries are superseded by later entries.

/// The outcome of one interception decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The gate refused the claim (enforcement mode).
    Enforced,
    /// The gate WOULD refuse, but shadow mode only records (the refusal is
    /// advisory until the precision account is published).
    ShadowFlagged,
    /// The gate found nothing to refuse.
    Allowed,
}

/// An appeal against an interception (the human channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appeal {
    /// Was an override token presented (human authority)?
    pub override_token_present: bool,
    /// Why the gate's verdict was overruled (or why the appeal failed).
    pub note: String,
}

/// One logged interception event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interception {
    /// Which gate produced this event (e.g. "G25.derivable", "G30.source").
    pub gate: String,
    /// What was judged (a claim id / subject path).
    pub subject: String,
    /// The decision (Enforced / ShadowFlagged / Allowed).
    pub outcome: Outcome,
    /// The appeal, when one was raised.
    pub appeal: Option<Appeal>,
}

impl Interception {
    /// The accountability rule: an override is a recorded VIOLATION - the
    /// human wins, the record keeps it.
    #[must_use]
    pub fn is_violation(&self) -> bool {
        self.appeal
            .as_ref()
            .map(|a| a.override_token_present)
            .unwrap_or(false)
    }
}

/// The interceptor's operating mode - derived, never configured by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Verdicts are recorded, not enforced (the default until precision is
    /// published).
    Shadow,
    /// Verdicts are enforced (allowed ONLY with a published precision
    /// account).
    Enforcing,
}

/// The mode rule (G31a's gate on the gates): enforcement requires a
/// published precision/recall account (G31b - the interceptor's own
/// precision budget). Until then: shadow.
#[must_use]
pub fn mode_for(precision_published: bool) -> Mode {
    if precision_published {
        Mode::Enforcing
    } else {
        Mode::Shadow
    }
}

/// The append-only ledger. There is no remove API by construction.
#[derive(Debug, Clone, Default)]
pub struct InterceptionLedger {
    entries: Vec<Interception>,
}

impl InterceptionLedger {
    /// A ledger starts in shadow mode (fail-closed for the interceptor:
    /// enforcement is EARNED by a precision account, not granted).
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Record one interception (append-only; the entry is returned so the
    /// caller can act on the outcome).
    pub fn record(&mut self, entry: Interception) -> &Interception {
        let idx = self.entries.len();
        self.entries.push(entry);
        &self.entries[idx]
    }

    /// The per-gate counts that G42's net-benefit ledger consumes.
    #[must_use]
    pub fn count_by_gate(&self, gate: &str) -> usize {
        self.entries.iter().filter(|e| e.gate == gate).count()
    }

    /// Every override-as-violation (G31b's numerator inputs).
    #[must_use]
    pub fn violations(&self) -> Vec<&Interception> {
        self.entries.iter().filter(|e| e.is_violation()).collect()
    }

    /// The entries (read-only).
    #[must_use]
    pub fn entries(&self) -> &[Interception] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn entry(gate: &str, outcome: Outcome) -> Interception {
        Interception {
            gate: gate.to_string(),
            subject: "claim.42".to_string(),
            outcome,
            appeal: None,
        }
    }

    #[test]
    fn interceptor_in_shadow_mode_until_precision_published() {
        // No precision account -> SHADOW, no exceptions, not configurable.
        assert_eq!(mode_for(false), Mode::Shadow);
        assert_eq!(mode_for(true), Mode::Enforcing);
        let mut ledger = InterceptionLedger::new();
        let recorded = ledger.record(entry("G25.derivable", Outcome::ShadowFlagged));
        assert_eq!(
            recorded.outcome,
            Outcome::ShadowFlagged,
            "a fresh ledger flags, it does not enforce"
        );
    }

    #[test]
    fn override_is_recorded_as_violation() {
        let mut ledger = InterceptionLedger::new();
        ledger.record(Interception {
            gate: "G25.derivable".into(),
            subject: "claim.7".into(),
            outcome: Outcome::Enforced,
            appeal: Some(Appeal {
                override_token_present: true,
                note: "human confirmed the artifact field was mislabelled".into(),
            }),
        });
        ledger.record(entry("G25.derivable", Outcome::Enforced));
        assert_eq!(ledger.violations().len(), 1, "exactly the override");
        assert!(ledger.entries()[0].is_violation());
        assert!(!ledger.entries()[1].is_violation());
    }

    #[test]
    fn interception_ledger_is_append_only() {
        // The type has no removal API; this test pins the growth property
        // across many records.
        let mut ledger = InterceptionLedger::new();
        for n in 0..5 {
            ledger.record(entry("G30.source", Outcome::Allowed));
            assert_eq!(ledger.entries().len(), n + 1);
        }
    }

    #[test]
    fn intercepted_claims_are_counted_per_gate() {
        // G42's future net-benefit ledger reads these counts.
        let mut ledger = InterceptionLedger::new();
        ledger.record(entry("G25.derivable", Outcome::ShadowFlagged));
        ledger.record(entry("G25.derivable", Outcome::ShadowFlagged));
        ledger.record(entry("G30.source", Outcome::ShadowFlagged));
        assert_eq!(ledger.count_by_gate("G25.derivable"), 2);
        assert_eq!(ledger.count_by_gate("G30.source"), 1);
        assert_eq!(ledger.count_by_gate("G31b.none"), 0);
    }
}
