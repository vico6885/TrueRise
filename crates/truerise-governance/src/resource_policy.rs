//! The resource-policy port (spec §6 ResourcePolicy row): deletion-flow
//! semantics are generic, the POLICY is pluggable and industry-specific
//! policies live out of this repo.
//!
//! Three gates, all mandatory, in order:
//! 1. risk analysis (impact + recoverability) - the caller supplies it;
//! 2. itemized target list (post-glob expansion) - the caller supplies it;
//! 3. human approval token - recorded, verifiable, NEVER implied.

use std::collections::HashMap;

/// One itemized deletion target (post-glob expansion, one entry per hit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionTarget {
    /// Absolute or repo-relative path of the actual hit.
    pub path: String,
    /// Recoverability note (e.g. "regenerable", "gone after prune").
    pub recoverability: String,
}

/// The outcome of a policy review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletionDecision {
    /// All three gates passed; the caller may proceed (and MUST log to the
    /// audit chain - the chain is the authority, not this decision).
    Approved,
    /// Refused with the failing gate listed.
    Refused {
        /// Which of the three gates refused.
        gate: &'static str,
        /// Why it refused.
        reason: String,
    },
}

/// The pluggable policy port. Implementations decide HOW gates 1-2 are
/// validated and WHAT counts as an approval token; the three-gate ORDER is
/// fixed here (flow is generic, policy is pluggable).
pub trait ResourcePolicy: Send + Sync {
    /// Gate 1: is the risk analysis acceptable for these targets?
    fn review_risk(&self, analysis: &str, targets: &[DeletionTarget]) -> Result<(), String>;
    /// Gate 2: is the itemized list complete and well-formed?
    fn review_targets(&self, targets: &[DeletionTarget]) -> Result<(), String>;
    /// Gate 3: is this approval token valid (recorded by a human, not implied)?
    fn review_approval(&self, token: &str) -> Result<(), String>;
}

/// The generic three-gate runner. Any policy plugs in; the order is fixed.
#[derive(Default)]
pub struct DeletionGate {
    policies: HashMap<String, Box<dyn ResourcePolicy>>,
}

impl DeletionGate {
    /// Register a named policy (e.g. "default", "audit-artifacts").
    pub fn register(&mut self, name: &str, policy: Box<dyn ResourcePolicy>) {
        self.policies.insert(name.to_string(), policy);
    }

    /// Run all three gates in order for the named policy.
    ///
    /// # Errors
    /// [`DeletionDecision::Refused`] converted by the caller; returns the
    /// decision value (this layer never panics on policy errors).
    pub fn review(
        &self,
        policy_name: &str,
        analysis: &str,
        targets: &[DeletionTarget],
        approval_token: &str,
    ) -> DeletionDecision {
        let Some(policy) = self.policies.get(policy_name) else {
            return DeletionDecision::Refused {
                gate: "registered-policy",
                reason: format!("no policy named {policy_name}"),
            };
        };
        if let Err(reason) = policy.review_risk(analysis, targets) {
            return DeletionDecision::Refused {
                gate: "risk-analysis",
                reason,
            };
        }
        if let Err(reason) = policy.review_targets(targets) {
            return DeletionDecision::Refused {
                gate: "itemized-targets",
                reason,
            };
        }
        if let Err(reason) = policy.review_approval(approval_token) {
            return DeletionDecision::Refused {
                gate: "human-approval",
                reason,
            };
        }
        DeletionDecision::Approved
    }
}

/// The built-in strict policy: nothing is exempt, approval tokens must be
/// non-empty and carry a reviewer tag ("approved-by:<who>").
#[derive(Debug, Default)]
pub struct StrictPolicy;

impl ResourcePolicy for StrictPolicy {
    fn review_risk(&self, analysis: &str, _targets: &[DeletionTarget]) -> Result<(), String> {
        if analysis.trim().len() < 20 {
            return Err(
                "risk analysis too thin (<20 chars): impact + recoverability required".into(),
            );
        }
        Ok(())
    }

    fn review_targets(&self, targets: &[DeletionTarget]) -> Result<(), String> {
        if targets.is_empty() {
            return Err("itemized target list is empty: expand the glob first".into());
        }
        for t in targets {
            if t.path.trim().is_empty() {
                return Err("target with empty path in itemized list".into());
            }
        }
        Ok(())
    }

    fn review_approval(&self, token: &str) -> Result<(), String> {
        if let Some(who) = token.strip_prefix("approved-by:") {
            if !who.trim().is_empty() {
                return Ok(());
            }
        }
        Err("approval token must be 'approved-by:<reviewer>' (human, recorded)".into())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn targets() -> Vec<DeletionTarget> {
        vec![DeletionTarget {
            path: "output/tmp/old.txt".into(),
            recoverability: "regenerable".into(),
        }]
    }

    #[test]
    fn three_gates_in_order() {
        let mut gate = DeletionGate::default();
        gate.register("strict", Box::new(StrictPolicy));
        // Gate 1 fails (thin analysis):
        assert!(matches!(
            gate.review("strict", "delete it", &targets(), "approved-by:vico"),
            DeletionDecision::Refused {
                gate: "risk-analysis",
                ..
            }
        ));
        // Gate 2 fails (empty list):
        assert!(matches!(
            gate.review(
                "strict",
                "impact: tmp files are regenerable",
                &[],
                "approved-by:vico"
            ),
            DeletionDecision::Refused {
                gate: "itemized-targets",
                ..
            }
        ));
        // Gate 3 fails (no reviewer tag):
        assert!(matches!(
            gate.review(
                "strict",
                "impact: tmp files are regenerable",
                &targets(),
                "yes i guess"
            ),
            DeletionDecision::Refused {
                gate: "human-approval",
                ..
            }
        ));
        // All three pass:
        assert_eq!(
            gate.review(
                "strict",
                "impact: tmp files are regenerable",
                &targets(),
                "approved-by:vico"
            ),
            DeletionDecision::Approved
        );
    }

    #[test]
    fn unknown_policy_refused() {
        let gate = DeletionGate::default();
        assert!(matches!(
            gate.review("ghost", "impact: n/a", &targets(), "approved-by:x"),
            DeletionDecision::Refused {
                gate: "registered-policy",
                ..
            }
        ));
    }
}
