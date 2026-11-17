//! Three-state policy (ask / deny / auto) with scope caching
//! (spec §6 policy row: semantics match the Python authority).

use std::collections::HashMap;

/// The three policy states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyState {
    /// Ask the operator every time (never cached as a yes).
    Ask,
    /// Refuse outright.
    Deny,
    /// Auto-approve within the granted scope.
    Auto,
}

impl PolicyState {
    /// Wire name.
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyState::Ask => "ask",
            PolicyState::Deny => "deny",
            PolicyState::Auto => "auto",
        }
    }
}

/// A decision for one action request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Proceed (auto, or a recorded ask-approval).
    Allow,
    /// Operator must approve; the request is queued, NOT granted.
    NeedsApproval,
    /// Refused.
    Deny,
}

/// Scope-keyed policy table with a decision cache. DENY short-circuits
/// before any cache lookup (spec: deny 短路优先; 缺省拒).
#[derive(Debug, Default)]
pub struct PolicyTable {
    rules: HashMap<String, PolicyState>,
    cache: HashMap<String, Decision>,
}

impl PolicyTable {
    /// An empty table: default-deny for every scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the policy for a scope (later calls overwrite; audit trail is the
    /// chain's job, not the table's).
    pub fn set(&mut self, scope: &str, state: PolicyState) {
        self.rules.insert(scope.to_string(), state);
        self.cache.remove(scope);
    }

    /// Evaluate ``scope``: Deny rules short-circuit; Auto allows and caches
    /// the decision; Ask requires an explicit approval record for the exact
    /// scope before it may become Allow (approvals are recorded, never
    /// implied by timeout).
    pub fn evaluate(&mut self, scope: &str, approval_on_file: bool) -> Decision {
        let state = self.rules.get(scope).copied();
        if let Some(PolicyState::Deny) = state {
            return Decision::Deny;
        }
        if let Some(cached) = self.cache.get(scope) {
            return cached.clone();
        }
        match state {
            Some(PolicyState::Auto) => {
                self.cache.insert(scope.to_string(), Decision::Allow);
                Decision::Allow
            }
            Some(PolicyState::Ask) => {
                // Review fix (P1): an Ask ALLOW is for THIS evaluation only
                // and is NEVER cached - caching it turned a revocable
                // approval into permanent consent (the tool-port contract
                // says approvals never expire into consent; the old cache
                // violated that on the next approval_on_file=false call).
                if approval_on_file {
                    Decision::Allow
                } else {
                    Decision::NeedsApproval
                }
            }
            Some(PolicyState::Deny) => Decision::Deny, // unreachable (short-circuit above), exhaustive for the compiler
            None => Decision::Deny,                    // default-deny
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn default_deny() {
        let mut t = PolicyTable::new();
        assert_eq!(t.evaluate("anything", false), Decision::Deny);
        assert_eq!(t.evaluate("anything", true), Decision::Deny);
    }

    #[test]
    fn deny_short_circuits_even_with_approval() {
        let mut t = PolicyTable::new();
        t.set("rm", PolicyState::Deny);
        assert_eq!(t.evaluate("rm", true), Decision::Deny);
    }

    #[test]
    fn auto_allows_and_caches() {
        let mut t = PolicyTable::new();
        t.set("read", PolicyState::Auto);
        assert_eq!(t.evaluate("read", false), Decision::Allow);
        assert_eq!(t.evaluate("read", false), Decision::Allow, "cached");
    }

    #[test]
    fn ask_allow_is_per_evaluation_never_cached() {
        // Review fix (P1): the old test PINNED the bug - an Ask allow was
        // cached, turning a revocable approval into permanent consent. The
        // contract is "approvals never expire into consent": each
        // evaluation re-checks approval_on_file.
        let mut t = PolicyTable::new();
        t.set("write", PolicyState::Ask);
        assert_eq!(t.evaluate("write", false), Decision::NeedsApproval);
        assert_eq!(t.evaluate("write", true), Decision::Allow);
        assert_eq!(
            t.evaluate("write", false),
            Decision::NeedsApproval,
            "the allow was for THAT evaluation - no consent leakage"
        );
        assert_eq!(t.evaluate("write", true), Decision::Allow);
    }

    #[test]
    fn set_overwrites_and_invalidates_cache() {
        let mut t = PolicyTable::new();
        t.set("x", PolicyState::Auto);
        assert_eq!(t.evaluate("x", false), Decision::Allow);
        t.set("x", PolicyState::Deny);
        assert_eq!(t.evaluate("x", false), Decision::Deny);
    }
}
