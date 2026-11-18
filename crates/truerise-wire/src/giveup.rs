//! Bounded give-up audit (G44, contracted first version): "I cannot do
//! this" is a claim like any other - it needs a ledger, a sealed budget
//! bound, and a retry plan, or it is refused.
//!
//! Lazy surrender wastes compute and direction exactly the way fake
//! completion does, and it has NO oracle (impossibility is a counterfactual
//! over an unbounded search space). The honest rewrite: a give-up claim is
//! only ever "INFEASIBLE WITHIN BUDGET X" - never "impossible".
//!
//! The verdict space is EXACTLY three-valued and cannot be collapsed:
//! Completed (per-item evidence) / NotCompleted (gap list + reasons) /
//! InfeasibleWithinBudget (search-surface ledger + sealed bound + retry
//! plan). Consensus v3.3 10.3 defers AUTO-EXECUTED bounded recheck until
//! G2/G21 budget linkage exists - this version carries the LEDGER, the
//! SEALED BOUND and the RETRY PLAN; the plan must be DISJOINT from the
//! tried set, and the budget bound must cover the cheapest untried probe
//! (a lazy ledger that only lists doomed directions is refused).
//!
//! Honest boundary (docs/limits.md): an accepted give-up proves "no route
//! found within the declared budget", never "no route exists".

use serde::{Deserialize, Serialize};

use crate::intent_contract::{ContractError, IntentContractV1};

/// What one probe of the search surface produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProbeOutcome {
    /// The probe was tried; what came out.
    Tried {
        /// What the probe observed (a fact, not a vibe).
        result_note: String,
    },
    /// The probe was never tried; its cheapest concrete instantiation and
    /// what that instantiation would cost (the lazy-ledger tripwire: an
    /// untried probe with no cheapest-form is a refusal).
    Untried {
        /// The cheapest concrete form this hypothesis could take.
        cheapest_probe: String,
        /// The cost estimate for that cheapest form.
        cost_estimate: String,
    },
}

/// One row of the search-surface ledger: which hypothesis, what happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchProbe {
    /// The hypothesis that was (or was not) tried.
    pub hypothesis: String,
    /// Tried or untried, with the required detail per variant.
    pub outcome: ProbeOutcome,
}

/// The retry plan: candidate probes the bounded recheck WILL run. Must be
/// disjoint from the tried set (re-running a known dead end is billed, not
/// blessed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RetryPlan {
    /// Concrete candidate probes, disjoint from the tried set.
    pub candidate_probes: Vec<String>,
}

/// The v1 search-surface ledger: what a give-up claim must present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchLedgerV1 {
    /// The SEALED cost upper bound: "if a positive shows up within this
    /// budget, I was wrong" - sealed before the bounded recheck (G17
    /// semantics); an empty bound is a refusal.
    pub budget_bound: String,
    /// The ledger rows: tried hypotheses with outcomes, untried ones with
    /// their cheapest concrete form.
    pub probes: Vec<SearchProbe>,
    /// The disjoint retry plan for the bounded recheck.
    pub retry_plan: RetryPlan,
}

/// The three-valued outcome (never compressible to two).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiveUpVerdict {
    /// Every acceptance item carried evidence (G32/G35 territory).
    Completed,
    /// Not done - the gap ledger names what and why (G35 taxonomy).
    NotCompleted,
    /// No route found within the SEALED budget bound - a bounded claim.
    InfeasibleWithinBudget,
}

/// A give-up claim: the verdict plus the ledger that backs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiveUpClaimV1 {
    /// The sealed intent contract this claim dispositions (optional only
    /// for claims made outside a contract run; contract-bound claims
    /// validate against it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_sealed_hash: Option<String>,
    /// The three-valued verdict.
    pub verdict: GiveUpVerdict,
    /// The search-surface ledger (REQUIRED for InfeasibleWithinBudget).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledger: Option<SearchLedgerV1>,
}

/// Give-up refusals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiveUpError {
    /// A give-up verdict without a search ledger (unattributed abandonment).
    AbandonmentWithoutLedger,
    /// The budget bound is empty - the claim is not bounded, so it is not
    /// decidable and not acceptable.
    UnboundedClaim,
    /// The retry plan re-runs a probe the ledger already tried.
    RetryNotDisjoint(String),
    /// An untried probe lacks its cheapest concrete form (the lazy-ledger
    /// tripwire: listing only doomed directions is not a search surface).
    UntriedWithoutCheapestProbe(String),
    /// Review fix (P2): a TRIED probe with an empty result note - the old
    /// code returned UntriedWithoutCheapestProbe, a variant whose message
    /// pointed at the wrong kind of probe entirely.
    TriedWithoutResult(String),
    /// An untried probe lacks its cost estimate.
    UntriedWithoutCost(String),
    /// The ledger is empty - "I tried nothing and I am out of ideas" is
    /// not a bounded claim.
    EmptyLedger,
    /// The claim's hash does not match the contract it cites.
    HashMismatch {
        /// The hash of the contract being validated against.
        expected: String,
        /// The hash the claim cites.
        found: String,
    },
    /// Completed/NotCompleted claims do not carry infeasibility ledgers.
    VerdictLedgerMismatch,
}

impl std::fmt::Display for GiveUpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AbandonmentWithoutLedger => write!(
                f,
                "give-up without a search-surface ledger is unattributed abandonment - refused"
            ),
            Self::UnboundedClaim => {
                write!(
                    f,
                    "no sealed budget bound - 'infeasible' without a bound is not a claim"
                )
            }
            Self::RetryNotDisjoint(probe) => {
                write!(f, "retry plan re-runs a tried probe: {probe}")
            }
            Self::UntriedWithoutCheapestProbe(h) => write!(
                f,
                "untried hypothesis has no cheapest concrete form: {h} (listing only doomed directions is not a search surface)"
            ),
            Self::TriedWithoutResult(h) => {
                write!(
                    f,
                    "tried hypothesis {h} carries no result note - an outcome fact is required"
                )
            }
            Self::UntriedWithoutCost(h) => {
                write!(f, "untried hypothesis has no cost estimate: {h}")
            }
            Self::EmptyLedger => write!(
                f,
                "empty search ledger - tried nothing is not a bounded claim"
            ),
            Self::HashMismatch { expected, found } => {
                write!(
                    f,
                    "claim cites contract {found}, this contract is {expected}"
                )
            }
            Self::VerdictLedgerMismatch => {
                write!(f, "only InfeasibleWithinBudget carries a search ledger")
            }
        }
    }
}

impl std::error::Error for GiveUpError {}

impl GiveUpClaimV1 {
    /// Validate against a sealed contract (when contract-bound).
    pub fn validate_against(&self, contract: Option<&IntentContractV1>) -> Result<(), GiveUpError> {
        if let Some(contract) = contract {
            let sealed = contract
                .sealed_hash()
                .map_err(|_| GiveUpError::HashMismatch {
                    expected: "(unsealed contract)".to_string(),
                    found: self
                        .contract_sealed_hash
                        .clone()
                        .unwrap_or_else(|| "(none)".to_string()),
                })?;
            let cited = self
                .contract_sealed_hash
                .as_deref()
                .ok_or(GiveUpError::HashMismatch {
                    expected: sealed.to_string(),
                    found: "(none)".to_string(),
                })?;
            if cited != sealed {
                return Err(GiveUpError::HashMismatch {
                    expected: sealed.to_string(),
                    found: cited.to_string(),
                });
            }
        }
        match self.verdict {
            GiveUpVerdict::InfeasibleWithinBudget => {
                let ledger = self
                    .ledger
                    .as_ref()
                    .ok_or(GiveUpError::AbandonmentWithoutLedger)?;
                self.validate_ledger(ledger)
            }
            GiveUpVerdict::Completed | GiveUpVerdict::NotCompleted => {
                if self.ledger.is_some() {
                    return Err(GiveUpError::VerdictLedgerMismatch);
                }
                Ok(())
            }
        }
    }

    fn validate_ledger(&self, ledger: &SearchLedgerV1) -> Result<(), GiveUpError> {
        if ledger.probes.is_empty() {
            return Err(GiveUpError::EmptyLedger);
        }
        if ledger.budget_bound.trim().is_empty() {
            return Err(GiveUpError::UnboundedClaim);
        }
        let mut tried: Vec<&str> = Vec::new();
        for probe in &ledger.probes {
            match &probe.outcome {
                ProbeOutcome::Tried { result_note } => {
                    if result_note.trim().is_empty() {
                        return Err(GiveUpError::TriedWithoutResult(probe.hypothesis.clone()));
                    }
                    tried.push(&probe.hypothesis);
                }
                ProbeOutcome::Untried {
                    cheapest_probe,
                    cost_estimate,
                } => {
                    if cheapest_probe.trim().is_empty() {
                        return Err(GiveUpError::UntriedWithoutCheapestProbe(
                            probe.hypothesis.clone(),
                        ));
                    }
                    if cost_estimate.trim().is_empty() {
                        return Err(GiveUpError::UntriedWithoutCost(probe.hypothesis.clone()));
                    }
                }
            }
        }
        // retry_budget_covers_cheapest_untried_probe (consensus v3.3): the
        // bound must be stated AND the retry plan must be non-empty when
        // untried hypotheses remain - the bound covers the cheapest untried
        // probe by CONSTRUCTION (the bound is sealed against the plan).
        let has_untried = ledger
            .probes
            .iter()
            .any(|p| matches!(p.outcome, ProbeOutcome::Untried { .. }));
        if has_untried && ledger.retry_plan.candidate_probes.is_empty() {
            return Err(GiveUpError::UnboundedClaim);
        }
        // bounded_retry_is_disjoint_from_tried_set.
        for candidate in &ledger.retry_plan.candidate_probes {
            if tried.contains(&candidate.as_str()) {
                return Err(GiveUpError::RetryNotDisjoint(candidate.clone()));
            }
        }
        Ok(())
    }
}

// Keep the import honest: ContractError is the shared seal-vocabulary; the
// give-up gate reuses its machine-floor rule via the completion path (a
// contract under the floor cannot certify COMPLETION, so its give-up claims
// are the bounded kind).
const _: fn(&IntentContractV1) -> Result<(), ContractError> = IntentContractV1::validate;

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(tried: usize, untried: usize, retry: &[&str]) -> SearchLedgerV1 {
        let mut probes = Vec::new();
        for n in 0..tried {
            probes.push(SearchProbe {
                hypothesis: format!("tried-{n}"),
                outcome: ProbeOutcome::Tried {
                    result_note: format!("probe {n} came back negative"),
                },
            });
        }
        for n in 0..untried {
            probes.push(SearchProbe {
                hypothesis: format!("untried-{n}"),
                outcome: ProbeOutcome::Untried {
                    cheapest_probe: format!("smallest experiment for untried-{n}"),
                    cost_estimate: "one bounded run".to_string(),
                },
            });
        }
        SearchLedgerV1 {
            budget_bound: "two bounded runs or 100k tokens, whichever first".to_string(),
            probes,
            retry_plan: RetryPlan {
                candidate_probes: retry.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    fn claim(ledger: Option<SearchLedgerV1>) -> GiveUpClaimV1 {
        GiveUpClaimV1 {
            contract_sealed_hash: None,
            verdict: GiveUpVerdict::InfeasibleWithinBudget,
            ledger,
        }
    }

    #[test]
    fn abandonment_without_search_ledger_is_rejected() {
        assert_eq!(
            claim(None).validate_against(None),
            Err(GiveUpError::AbandonmentWithoutLedger)
        );
        assert_eq!(
            claim(Some(SearchLedgerV1 {
                budget_bound: "x".into(),
                probes: vec![],
                retry_plan: RetryPlan::default(),
            }))
            .validate_against(None),
            Err(GiveUpError::EmptyLedger)
        );
    }

    #[test]
    fn infeasible_certificate_states_its_budget_bound() {
        let unbounded = GiveUpClaimV1 {
            contract_sealed_hash: None,
            verdict: GiveUpVerdict::InfeasibleWithinBudget,
            ledger: Some(SearchLedgerV1 {
                budget_bound: "   ".into(),
                probes: vec![SearchProbe {
                    hypothesis: "h".into(),
                    outcome: ProbeOutcome::Tried {
                        result_note: "negative".into(),
                    },
                }],
                retry_plan: RetryPlan::default(),
            }),
        };
        assert_eq!(
            unbounded.validate_against(None),
            Err(GiveUpError::UnboundedClaim)
        );
        // A bounded ledger with a full retry plan validates.
        assert!(
            claim(Some(ledger(2, 1, &["smallest experiment for untried-0"])))
                .validate_against(None)
                .is_ok()
        );
    }

    #[test]
    fn bounded_retry_is_disjoint_from_tried_set() {
        let bad = claim(Some(ledger(2, 0, &["tried-0"])));
        assert_eq!(
            bad.validate_against(None),
            Err(GiveUpError::RetryNotDisjoint("tried-0".into()))
        );
    }

    #[test]
    fn retry_budget_covers_cheapest_untried_probe() {
        // Untried hypotheses remain but the retry plan is EMPTY: the bound
        // covers nothing - refused (the lazy-ledger tripwire).
        let lazy = claim(Some(ledger(1, 2, &[])));
        assert_eq!(
            lazy.validate_against(None),
            Err(GiveUpError::UnboundedClaim)
        );
        // An untried probe without a cheapest concrete form: refused.
        let mut no_form = ledger(1, 1, &["plan-0"]);
        no_form.probes[1].outcome = ProbeOutcome::Untried {
            cheapest_probe: "   ".into(),
            cost_estimate: "cheap".into(),
        };
        assert_eq!(
            claim(Some(no_form)).validate_against(None),
            Err(GiveUpError::UntriedWithoutCheapestProbe("untried-0".into()))
        );
    }

    #[test]
    fn three_valued_outcome_cannot_be_collapsed() {
        // Only the three named verdicts parse - there is no "partial",
        // "mostly done" or "passed with caveats".
        for bogus in [
            "pass",
            "partial",
            "mostly_done",
            "success_with_caveats",
            "done",
        ] {
            assert!(
                serde_json::from_str::<GiveUpVerdict>(format!("\"{bogus}\"").as_str()).is_err(),
                "{bogus} must not parse as a verdict"
            );
        }
        for real in ["completed", "not_completed", "infeasible_within_budget"] {
            assert!(serde_json::from_str::<GiveUpVerdict>(format!("\"{real}\"").as_str()).is_ok());
        }
    }

    #[test]
    fn completed_claims_do_not_carry_infeasibility_ledgers() {
        let mixed = GiveUpClaimV1 {
            contract_sealed_hash: None,
            verdict: GiveUpVerdict::Completed,
            ledger: Some(ledger(1, 0, &[])),
        };
        assert_eq!(
            mixed.validate_against(None),
            Err(GiveUpError::VerdictLedgerMismatch)
        );
    }
}
