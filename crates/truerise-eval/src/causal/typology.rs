//! Claim typology and tier semantics (CA1): what KIND of claim a word
//! makes, and what its evidence may honestly promise.
//!
//! Evidence classes are not interchangeable: an interventional study may
//! promise interventional strength, an observational study with defended
//! assumptions may promise identified-observational strength, and a
//! temporal-precedence or correlational row may promise ASSOCIATION ONLY.
//! The enforcement rule set:
//!
//! 1. Type vocabulary: a `claim_type` outside the six-value enum is a
//!    registry error (fail-closed).
//! 2. Tier semantics: a causal-typed claim with a COMPLETE assumption
//!    ledger row ([`super::ledger::AssumptionRow::is_complete`]) carries
//!    its declared tier; without one it DOWNGRADES to [`ClaimTier::HypothesisOnly`]
//!    - the INCONCLUSIVE downgrade, never a failure, never an upgrade.
//! 3. Wording law: temporal-precedence and correlational claims may never
//!    use causal language, because their evidence class cannot carry it.
//!    "follows", "precedes" and "tracks with" are legal; "cause" is not.
//!
//! The Python claim gate (tools/check_claim_terms.py) enforces rules 1 and
//! 3 over the registry; this module carries the same rules for the runtime
//! and the tier semantics of rule 2.

/// The six-value claim-type vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimType {
    /// Association without a defended identification strategy.
    Correlational,
    /// A causal claim resting on observational data whose identification
    /// assumptions are declared and defended.
    CausalObservational,
    /// A causal claim resting on a designed intervention (assignment,
    /// ablation, crossover).
    CausalInterventional,
    /// A prediction about unseen outcomes - no causal reading.
    Predictive,
    /// A claim about an instrument's validity (detector precision, judge
    /// agreement) - measurement, not causation.
    MeasurementValidity,
    /// Order-in-time evidence only. The most over-read evidence class in
    /// existence: precedence is NOT causation, and the wording law below
    /// enforces that reading.
    TemporalPrecedence,
}

impl ClaimType {
    /// Parse the snake_case registry token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "correlational" => Some(Self::Correlational),
            "causal_observational" => Some(Self::CausalObservational),
            "causal_interventional" => Some(Self::CausalInterventional),
            "predictive" => Some(Self::Predictive),
            "measurement_validity" => Some(Self::MeasurementValidity),
            "temporal_precedence" => Some(Self::TemporalPrecedence),
            _ => None,
        }
    }

    /// The canonical token.
    #[must_use]
    pub fn token(&self) -> &'static str {
        match self {
            Self::Correlational => "correlational",
            Self::CausalObservational => "causal_observational",
            Self::CausalInterventional => "causal_interventional",
            Self::Predictive => "predictive",
            Self::MeasurementValidity => "measurement_validity",
            Self::TemporalPrecedence => "temporal_precedence",
        }
    }

    /// Whether the evidence class can carry a causal claim AT ALL (subject
    /// to the ledger).
    #[must_use]
    pub fn is_causal(self) -> bool {
        matches!(self, Self::CausalObservational | Self::CausalInterventional)
    }
}

/// The tier a claim may honestly occupy, given its type and its ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimTier {
    /// Interventional strength: a designed intervention with a complete
    /// assumption row.
    Interventional,
    /// Observational strength WITH defended identification assumptions.
    IdentifiedObservational,
    /// The downgrade: a causal-shaped claim whose assumptions are absent
    /// or incomplete. Reads as a hypothesis; must be worded as one.
    HypothesisOnly,
    /// Association strength (correlational or temporal precedence).
    Associational,
    /// Prediction about unseen outcomes.
    Predictive,
    /// Instrument validity.
    Measurement,
}

/// Causal-language markers that an association-grade row may never use.
/// ASCII markers match case-insensitively; CJK markers match as
/// substrings.
pub const CAUSAL_WORDING_MARKERS: [&str; 5] = ["cause", "causal", "导致", "因果", "归因"];

/// The tier a claim occupies given its type and (optionally) its ledger
/// row.
///
/// The core honesty rule: a causal-typed claim WITHOUT a complete row is a
/// hypothesis. The ledger's presence upgrades the WORDING obligation, not
/// the truth - truth stays a judgement.
#[must_use]
pub fn tier_for(claim_type: &ClaimType, row: Option<&super::ledger::AssumptionRow>) -> ClaimTier {
    match claim_type {
        ClaimType::CausalInterventional => {
            if row.is_some_and(super::ledger::AssumptionRow::is_complete) {
                ClaimTier::Interventional
            } else {
                ClaimTier::HypothesisOnly
            }
        }
        ClaimType::CausalObservational => {
            if row.is_some_and(super::ledger::AssumptionRow::is_complete) {
                ClaimTier::IdentifiedObservational
            } else {
                ClaimTier::HypothesisOnly
            }
        }
        ClaimType::Correlational | ClaimType::TemporalPrecedence => ClaimTier::Associational,
        ClaimType::Predictive => ClaimTier::Predictive,
        ClaimType::MeasurementValidity => ClaimTier::Measurement,
    }
}

/// Wording violations for a row's `meaning` text.
///
/// Enforced for the two ASSOCIATION-grade types only: a
/// temporal-precedence or correlational claim whose meaning uses causal
/// language is smuggling a claim its evidence class cannot carry. The
/// returned strings name the offending marker so the gate output is
/// actionable.
#[must_use]
pub fn wording_violations(claim_type: &ClaimType, meaning: &str) -> Vec<String> {
    if !matches!(
        claim_type,
        ClaimType::TemporalPrecedence | ClaimType::Correlational
    ) {
        return Vec::new();
    }
    let lowered = meaning.to_lowercase();
    CAUSAL_WORDING_MARKERS
        .iter()
        .filter(|marker| lowered.contains(&marker.to_lowercase()))
        .map(|marker| {
            format!(
                "association-grade claim type '{}' must not use the causal marker {marker:?} - \
                 reword as precedence/association or change the claim type",
                claim_type.token()
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::causal::ledger::{Assumption, AssumptionLedger};

    const COMPLETE_ROW: &str = r#"{"rows": [{
        "claim_term": "Tool Subset Lifts Pass Rate",
        "assumptions": ["sutva", "consistency", "exchangeability", "positivity"],
        "defense": "runs are cluster-randomised; assignment recorded on the chain",
        "test": "some_named_test",
        "artifact": "baselines/run_scores.json",
        "status": "defended"
    }]}"#;

    fn ledger_with(text: &str) -> AssumptionLedger {
        AssumptionLedger::from_json(text).unwrap()
    }

    #[test]
    fn causal_interventional_without_defense_downgrades_to_hypothesis() {
        let tier = tier_for(&ClaimType::CausalInterventional, None);
        assert_eq!(tier, ClaimTier::HypothesisOnly);
    }

    #[test]
    fn defended_row_upgrades_interventional_claim() {
        let ledger = ledger_with(COMPLETE_ROW);
        let row = ledger.row_for("Tool Subset Lifts Pass Rate");
        assert_eq!(
            tier_for(&ClaimType::CausalInterventional, row),
            ClaimTier::Interventional
        );
        assert_eq!(
            tier_for(&ClaimType::CausalObservational, row),
            ClaimTier::IdentifiedObservational
        );
    }

    #[test]
    fn incomplete_defended_row_still_downgrades() {
        // Drop positivity: structurally valid, semantically incomplete.
        let text =
            COMPLETE_ROW.replace(r#""exchangeability", "positivity""#, r#""exchangeability""#);
        let ledger = ledger_with(&text);
        let row = ledger.row_for("Tool Subset Lifts Pass Rate");
        assert_eq!(
            tier_for(&ClaimType::CausalInterventional, row),
            ClaimTier::HypothesisOnly,
            "three of four assumptions is a hypothesis, not an identification"
        );
    }

    #[test]
    fn temporal_precedence_wording_never_claims_causality() {
        let violations = wording_violations(
            &ClaimType::TemporalPrecedence,
            "metric drift precedes the failure-rate rise",
        );
        assert!(violations.is_empty(), "precedence wording is legal");
        let violations = wording_violations(
            &ClaimType::TemporalPrecedence,
            "drift causes the failure-rate rise",
        );
        assert_eq!(violations.len(), 1, "got {violations:?}");
        assert!(violations[0].contains("cause"));
    }

    #[test]
    fn correlational_wording_never_claims_causality_in_any_language() {
        let violations = wording_violations(
            &ClaimType::Correlational,
            "因子与收益同向变动，且滞后于收益",
        );
        assert!(violations.is_empty());
        let violations =
            wording_violations(&ClaimType::Correlational, "该因子的暴露导致了收益抬升");
        assert_eq!(violations.len(), 1, "got {violations:?}");
        // Case-insensitivity: "Caused" trips the same marker.
        let violations = wording_violations(
            &ClaimType::Correlational,
            "The Score Improvement Was Caused By The Patch",
        );
        assert_eq!(violations.len(), 1, "got {violations:?}");
    }

    #[test]
    fn causal_and_instrument_types_are_exempt_from_the_wording_law() {
        // An interventional meaning may legitimately SAY "cause".
        assert!(
            wording_violations(
                &ClaimType::CausalInterventional,
                "ablation causes the pass rate to drop"
            )
            .is_empty()
        );
        assert!(
            wording_violations(
                &ClaimType::MeasurementValidity,
                "detector precision tracks the false-positive rate"
            )
            .is_empty()
        );
    }

    #[test]
    fn the_type_vocabulary_round_trips_and_refuses_unknowns() {
        let tokens = [
            "correlational",
            "causal_observational",
            "causal_interventional",
            "predictive",
            "measurement_validity",
            "temporal_precedence",
        ];
        for token in tokens {
            let parsed = ClaimType::parse(token).unwrap();
            assert_eq!(parsed.token(), token);
        }
        assert!(ClaimType::parse("causal_period").is_none());
        assert!(ClaimType::parse("").is_none());
        // Only the two causal types carry causal-grade tiers.
        assert!(
            ClaimType::parse("causal_interventional")
                .unwrap()
                .is_causal()
        );
        assert!(
            ClaimType::parse("causal_observational")
                .unwrap()
                .is_causal()
        );
        assert!(!ClaimType::parse("temporal_precedence").unwrap().is_causal());
        assert!(!ClaimType::parse("correlational").unwrap().is_causal());
    }

    #[test]
    fn association_types_never_reach_causal_tiers_even_with_a_perfect_row() {
        // A complete ledger row cannot upgrade an association-grade type:
        // the tier follows the EVIDENCE CLASS, not the paperwork.
        let ledger = ledger_with(COMPLETE_ROW);
        let row = ledger.row_for("Tool Subset Lifts Pass Rate");
        assert_eq!(
            tier_for(&ClaimType::TemporalPrecedence, row),
            ClaimTier::Associational
        );
        assert_eq!(
            tier_for(&ClaimType::Correlational, row),
            ClaimTier::Associational
        );
        assert_eq!(tier_for(&ClaimType::Predictive, row), ClaimTier::Predictive);
        assert_eq!(
            tier_for(&ClaimType::MeasurementValidity, row),
            ClaimTier::Measurement
        );
    }

    #[test]
    fn assumption_parse_round_trips() {
        for token in ["sutva", "consistency", "exchangeability", "positivity"] {
            let a = Assumption::parse(token).unwrap();
            assert_eq!(a.token(), token);
        }
        assert!(Assumption::parse("randomization").is_none());
    }
}
