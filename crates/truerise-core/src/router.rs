//! The ModelRouter port + retry/backoff + round pinning (spec §6 loop row).
//!
//! - Exponential backoff with jitter applies ONLY to retryable (rate-limit
//!   class) errors - everything else is fatal for the turn.
//! - Round pinning: a round is bound to ONE model id; any response from a
//!   different model VOIDS THE WHOLE ROUND (异族 pinning - no partial credit).

/// One turn request to a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRequest {
    /// The prompt/payload (opaque to the loop).
    pub payload: String,
    /// Turn tag (lands in the transcript).
    pub tag: String,
}

/// One turn response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResponse {
    /// The model that ACTUALLY produced the response (pinning checks this).
    pub model: String,
    /// The response payload.
    pub payload: String,
}

/// Router errors, classified for retry policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouterError {
    /// Rate-limit class: retryable with backoff.
    #[error("router: rate limited (retryable)")]
    RateLimited,
    /// Unavailable: retryable (transport class).
    #[error("router: unavailable (retryable)")]
    Unavailable,
    /// Fatal for the turn: never retried.
    #[error("router: fatal: {0}")]
    Fatal(String),
}

impl RouterError {
    /// Retryable class? (only rate-limit/transport classes).
    pub fn retryable(&self) -> bool {
        matches!(self, RouterError::RateLimited | RouterError::Unavailable)
    }
}

/// The port: route a turn to SOME model. The loop never sees a concrete
/// provider - only this trait (four-port rule).
pub trait ModelRouter: Send + Sync {
    /// Route one turn.
    ///
    /// # Errors
    /// [`RouterError`] - classified by the loop's retry policy.
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError>;
}

/// Retry policy: exponential backoff with jitter, integer ms (no float).
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Base delay in ms.
    pub base_ms: u64,
    /// Exponential factor (2 = doubling).
    pub factor: u32,
    /// Max attempts per turn (including the first).
    pub max_attempts: u32,
    /// Jitter ceiling in ms (added as 0..=jitter to every delay).
    pub jitter_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            base_ms: 100,
            factor: 2,
            max_attempts: 4,
            jitter_ms: 50,
        }
    }
}

/// Backoff schedule for attempt n (0-based): base * factor^n + jitter,
/// capped at u64 ms. Pure function - the loop sleeps between attempts.
pub fn backoff_ms(policy: &RetryPolicy, attempt: u32, jitter_roll: u64) -> u64 {
    let exp = policy
        .base_ms
        .saturating_mul(u64::from(policy.factor.saturating_pow(attempt)));
    exp.saturating_add(jitter_roll % (policy.jitter_ms + 1))
}

/// Round-level pinning verdict: any response from a model other than the
/// pinned one VOIDS the whole round.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PinError {
    /// A foreign model answered; the whole round is void.
    #[error("round voided: response model {got} != pinned {pinned}")]
    ForeignModel {
        /// The model that answered.
        got: String,
        /// The pinned model.
        pinned: String,
    },
}

/// Check one response against the pinned model.
///
/// # Errors
/// [`PinError::ForeignModel`] on mismatch.
pub fn check_pinned(pinned: &str, response: &TurnResponse) -> Result<(), PinError> {
    if response.model != pinned {
        return Err(PinError::ForeignModel {
            got: response.model.clone(),
            pinned: pinned.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn backoff_is_exponential_with_jitter_cap() {
        let p = RetryPolicy {
            base_ms: 100,
            factor: 2,
            max_attempts: 4,
            jitter_ms: 50,
        };
        assert_eq!(backoff_ms(&p, 0, 0), 100);
        assert_eq!(backoff_ms(&p, 1, 0), 200);
        assert_eq!(backoff_ms(&p, 2, 25), 425); // 400 + jitter 25
        assert_eq!(backoff_ms(&p, 3, 50), 850); // 800 + 50
    }

    #[test]
    fn jitter_roll_is_capped() {
        let p = RetryPolicy {
            base_ms: 100,
            factor: 2,
            max_attempts: 4,
            jitter_ms: 50,
        };
        assert_eq!(
            backoff_ms(&p, 0, 10_000),
            104,
            "jitter = roll % (jitter_ms+1) = 10000%51 = 4"
        );
    }

    #[test]
    fn only_rate_limit_class_is_retryable() {
        assert!(RouterError::RateLimited.retryable());
        assert!(RouterError::Unavailable.retryable());
        assert!(!RouterError::Fatal("bad request".into()).retryable());
    }

    #[test]
    fn foreign_model_voids_the_round() {
        let resp = TurnResponse {
            model: "other".into(),
            payload: "x".into(),
        };
        assert!(matches!(
            check_pinned("pinned-model", &resp),
            Err(PinError::ForeignModel { got, pinned }) if got == "other" && pinned == "pinned-model"
        ));
        let ok = TurnResponse {
            model: "pinned-model".into(),
            payload: "x".into(),
        };
        assert!(check_pinned("pinned-model", &ok).is_ok());
    }
}
