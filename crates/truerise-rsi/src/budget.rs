//! The trial-budget SINGLETON ledger (spec §6 budget row): 帽 ID 作字段,
//! 按帽配额 + 全局序贯校正分母不切碎. Two instances of the handle MUST see
//! the same global counts (双实例对账测: sum of per-hat counts == global
//! readout, difference == 0).

use std::collections::HashMap;
use std::sync::Mutex;

/// Errors from the budget ledger.
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    /// The hat exceeded its quota.
    #[error("budget: hat {hat} exceeded quota {quota} (used {used})")]
    QuotaExceeded {
        /// Hat id.
        hat: String,
        /// Quota.
        quota: u64,
        /// Used so far.
        used: u64,
    },
}

/// The global singleton state (per-process; the OnceLock makes `BudgetLedger::
/// global()` the single point - two handles share the state, they are NOT two
/// ledgers).
#[derive(Debug, Default)]
struct GlobalState {
    per_hat: HashMap<String, u64>,
    global: u64,
}

/// A handle to the global trial-budget ledger (clone = same ledger).
#[derive(Debug, Clone, Default)]
pub struct BudgetLedger {
    _marker: (),
}

static GLOBAL: std::sync::OnceLock<Mutex<GlobalState>> = std::sync::OnceLock::new();

fn state() -> &'static Mutex<GlobalState> {
    GLOBAL.get_or_init(|| Mutex::new(GlobalState::default()))
}

impl BudgetLedger {
    /// A handle to the global singleton (no per-handle state exists).
    pub fn global() -> Self {
        Self { _marker: () }
    }

    /// Consume one trial for ``hat``; the hat's quota (if set) is enforced.
    ///
    /// # Errors
    /// [`BudgetError::QuotaExceeded`] when the hat has a quota and is at it.
    pub fn consume(&self, hat: &str, quota: Option<u64>) -> Result<u64, BudgetError> {
        let mut st = state().lock().unwrap_or_else(|p| p.into_inner());
        let used = st.per_hat.get(hat).copied().unwrap_or(0);
        if let Some(q) = quota {
            if used >= q {
                return Err(BudgetError::QuotaExceeded {
                    hat: hat.to_string(),
                    quota: q,
                    used,
                });
            }
        }
        let new_used = used + 1;
        st.per_hat.insert(hat.to_string(), new_used);
        st.global += 1;
        Ok(new_used)
    }

    /// Per-hat count.
    pub fn hat_count(&self, hat: &str) -> u64 {
        state()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .per_hat
            .get(hat)
            .copied()
            .unwrap_or(0)
    }

    /// The global count (the校正分母 holder: sequential correction divides by
    /// THIS number, never by per-hat shards).
    pub fn global_count(&self) -> u64 {
        state().lock().unwrap_or_else(|p| p.into_inner()).global
    }

    /// Reconciliation: sum(per-hat) - global == 0 (双实例对账口径).
    pub fn reconcile(&self) -> i64 {
        let st = state().lock().unwrap_or_else(|p| p.into_inner());
        let sum: u64 = st.per_hat.values().sum();
        sum as i64 - st.global as i64
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// Unique per-test hat names: the singleton is process-global (by
    /// design), so tests use fresh names and DELTA assertions.
    fn unique_tag() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!("hat-{}", N.fetch_add(1, Ordering::Relaxed))
    }

    /// Spec M4 gate: two handles over the "singleton" must reconcile to a
    /// difference of ZERO. The singleton is process-global (by design), so
    /// ALL consuming tests run in ONE serialised test (delta assertions over
    /// unique hat names) - parallel consumers would break delta arithmetic,
    /// which is itself proof that the two handles share one ledger.
    #[test]
    fn singleton_reconciliation_quota_and_denominator() {
        let hat_a = unique_tag();
        let hat_b = unique_tag();
        let a = BudgetLedger::global();
        let b = BudgetLedger::global();

        // Two handles, one ledger:
        let before = a.global_count();
        let _ = a.consume(&hat_a, None);
        let _ = a.consume(&hat_a, None);
        let _ = b.consume(&hat_b, None);
        assert_eq!(a.reconcile(), 0);
        assert_eq!(b.reconcile(), 0);
        assert_eq!(a.global_count() - before, 3);
        assert_eq!(b.global_count(), a.global_count());
        assert_eq!(a.hat_count(&hat_a) + a.hat_count(&hat_b), 3);

        // Per-hat quota:
        let hat = unique_tag();
        let _ = a.consume(&hat, Some(1));
        assert!(matches!(
            a.consume(&hat, Some(1)),
            Err(BudgetError::QuotaExceeded { .. })
        ));

        // Global denominator (not sharded):
        let h1 = unique_tag();
        let h2 = unique_tag();
        let before = b.global_count();
        for _ in 0..7 {
            let _ = b.consume(&h1, None);
        }
        for _ in 0..3 {
            let _ = b.consume(&h2, None);
        }
        assert_eq!(b.global_count() - before, 10);
        assert_eq!(b.reconcile(), 0);
    }
}

/// The loop-facing budget (W3): binds the singleton ledger to a hat quota
/// and implements the core `Budget` port. One instance per run; two
/// instances still share the SAME global denominator (the singleton rule).
#[derive(Debug, Clone)]
pub struct RunBudget {
    /// The underlying singleton ledger (clone = same ledger).
    pub ledger: BudgetLedger,
    /// Per-hat quota (`None` = unbounded; the global denominator still
    /// accumulates - sequential correction divides by the global count).
    pub quota: Option<u64>,
}

impl RunBudget {
    /// Bind the global ledger to ``quota``.
    #[must_use]
    pub fn new(quota: Option<u64>) -> Self {
        Self {
            ledger: BudgetLedger::global(),
            quota,
        }
    }
}

impl truerise_core::Budget for RunBudget {
    fn consume(&self, hat: &str, units: u32) -> Result<(), truerise_core::BudgetStop> {
        for _ in 0..units {
            self.ledger
                .consume(hat, self.quota)
                .map_err(|_| truerise_core::BudgetStop {
                    hat: hat.to_string(),
                })?;
        }
        Ok(())
    }
}
