//! The causal identification substrate (CA1-CA5, review-consensus batch).
//!
//! Scope: the DOMAIN-NEUTRAL methodology layer for causal claims in agent
//! evaluation - experiment-design primitives, randomization inference,
//! partial identification, invariance screening, a refuter battery, formal
//! failure attribution, and the claim-type/tier semantics. No vertical
//! industry coupling lives here (GOVERNANCE.md neutrality): estimators
//! tied to a domain (econometrics, epidemiology) belong to out-of-repo
//! plugin projects; the gate-adjacent substrate below is general.
//!
//! The families, and the review batch that introduced them:
//!
//! - CA1 (claim typology): [`typology`] - the six-value claim-type
//!   vocabulary, the tier semantics, and the wording law (association-
//!   grade claims never speak causal language).
//! - CA2 (assumption ledger): [`ledger`] - the four identification
//!   assumptions as machine-checkable rows; no complete row, no causal
//!   tier (the INCONCLUSIVE downgrade).
//! - CA3 (identification arithmetic): [`bounds`] (Manski no-assumption
//!   intervals), [`evalue`] (confounding fragility price), [`icp`]
//!   (invariance screening across environments), [`refute`] (placebo,
//!   subset stability, confounder injection).
//! - CA4 (experiment primitives): [`rng`] (seeded, reproducible streams),
//!   [`design`] (blocked/cluster/crossover assignment - the SUTVA repair),
//!   [`permutation`] (randomization inference for small studies),
//!   [`shapley`] (interventional credit assignment).
//! - CA5 (formal attribution): [`attribution`] - Halpern-Pearl actual
//!   causation with responsibility scores, over binary bounded models.
//!
//! Honest boundaries, stated once and enforced everywhere: these
//! primitives PRICE evidence and REFUSE designs; none of them PROVES
//! causality. Identification assumptions remain judgements; the substrate
//! forces them to be declared, defends their arithmetic, and downgrades
//! claims that skip the paperwork.

pub mod adjust;
pub mod attribution;
pub mod bounds;
pub mod design;
pub mod evalue;
pub mod icp;
pub mod ledger;
pub mod minimal_pair;
pub mod pairwise;
pub mod permutation;
pub mod quasi;
pub mod refute;
pub mod rng;
pub mod shapley;
pub mod synthetic;
pub mod typology;

pub use adjust::{
    AdjustDesign, AdjustError, IpwReport, ModificationReport, effect_modification, ipw_effect,
};
pub use attribution::{
    ACYCLIC_BUILD_CAP, AttributionError, BinaryScm, CAUSAL_SEARCH_CAP, CYCLIC_BUILD_CAP,
    CauseReport, MAX_CONTINGENCY, VarDef,
};
pub use bounds::{BoundsError, ManskiBounds, ManskiInput, manski_ate_bounds};
pub use design::{
    Arm, CrossoverPlan, DesignError, assign_blocked, assign_clusters, assign_simple, plan_crossover,
};
pub use evalue::{EvalueError, evalue_rr};
pub use icp::{
    Environment, IcpConfig, IcpError, IcpReport, MAX_ICP_PREDICTORS, SubsetVerdict, icp_lite,
};
pub use ledger::{Assumption, AssumptionLedger, AssumptionRow, LedgerError, RowStatus};
pub use minimal_pair::{PairError, validate_minimal_pair};
pub use pairwise::{Comparison, PairwiseError, Ratings, bradley_terry, win_probability};
pub use permutation::{MAX_EXACT_ARRANGEMENTS, PermError, PermOutcome, paired_flip, two_sample};
pub use quasi::{
    DidResult, MIN_FIRST_STAGE, QuasiError, RddResult, WaldResult, did, sharp_rdd, wald_iv,
};
pub use refute::{
    Design, InjectionReport, NegativeControlReport, PlaceboReport, RefuteError, StabilityReport,
    confounder_injection, estimate_effect, negative_control, ols_fit, placebo_test,
    subset_stability,
};
pub use rng::SplitMix64;
pub use shapley::{
    MAX_EXACT_PLAYERS, MAX_SAMPLED_PLAYERS, SampledShapley, ShapleyError, shapley_exact,
    shapley_sampled,
};
pub use synthetic::{SynthError, SynthResult, synthetic_control};
pub use typology::{CAUSAL_WORDING_MARKERS, ClaimTier, ClaimType, tier_for, wording_violations};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    /// The substrate-level invariant: an association-grade claim can never
    /// borrow a causal tier, and a causal-shaped claim without its ledger
    /// paperwork cannot keep one. This is the module-family contract the
    /// individual unit tests assert piecewise.
    #[test]
    fn tier_semantics_close_the_upgrade_loophole_end_to_end() {
        use super::ledger::RowStatus;
        use super::typology::{ClaimTier, ClaimType, tier_for};

        // A defended-but-incomplete row, built in memory (no JSON).
        let incomplete = super::ledger::AssumptionRow {
            claim_term: "X Lifts Pass Rate".to_string(),
            assumptions: vec![super::ledger::Assumption::Sutva],
            defense: "we believe it".to_string(),
            test: "some_test".to_string(),
            artifact: "some_artifact.json".to_string(),
            status: RowStatus::Defended,
        };
        let full = super::ledger::AssumptionRow {
            assumptions: super::ledger::AssumptionRow::all_assumptions().to_vec(),
            ..incomplete.clone()
        };
        assert_eq!(
            tier_for(&ClaimType::CausalInterventional, Some(&incomplete)),
            ClaimTier::HypothesisOnly
        );
        assert_eq!(
            tier_for(&ClaimType::CausalInterventional, Some(&full)),
            ClaimTier::Interventional
        );
        assert_eq!(
            tier_for(&ClaimType::TemporalPrecedence, Some(&full)),
            ClaimTier::Associational
        );
    }
}
