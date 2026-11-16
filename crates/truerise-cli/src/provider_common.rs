//! Shared HTTP-to-router error classification for the provider adapters:
//! the loop's retry contract (only rate-limit/transport classes are
//! retryable, everything else is fatal) maps onto HTTP status codes here,
//! ONCE, so both adapters classify identically.

use truerise_core::RouterError;

/// Classify an HTTP status into the loop's retry contract.
#[must_use]
pub fn classify_status(code: u16) -> RouterError {
    match code {
        429 => RouterError::RateLimited,
        500..=599 => RouterError::Unavailable,
        _ => RouterError::Fatal(format!("provider http {code}")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn provider_error_classification_matches_the_retry_contract() {
        // The loop's contract: ONLY rate-limit/transport classes retry.
        assert!(
            RouterError::RateLimited.retryable(),
            "429 is the canonical retryable class"
        );
        assert!(classify_status(429).retryable());
        assert!(
            classify_status(503).retryable(),
            "5xx = transport-class, retryable"
        );
        assert!(
            !classify_status(400).retryable(),
            "4xx is fatal, never retried"
        );
        assert!(!classify_status(401).retryable());
        assert!(matches!(classify_status(429), RouterError::RateLimited));
        assert!(matches!(classify_status(503), RouterError::Unavailable));
        assert!(matches!(classify_status(400), RouterError::Fatal(_)));
    }
}
