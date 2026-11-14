//! Four-part fingerprint (model x harness x environment x evaluator) with
//! cross-environment comparison FORBIDDEN (spec §6 ratchet/指纹 row).

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

/// The four parts of an evaluation context.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint {
    /// Model id.
    pub model: String,
    /// Harness id.
    pub harness: String,
    /// Environment id.
    pub environment: String,
    /// Evaluator id.
    pub evaluator: String,
}

impl Fingerprint {
    /// SHA-256 over the canonical ``k=v|k=v`` serialization.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "model={}|harness={}|environment={}|evaluator={}",
            self.model, self.harness, self.environment, self.evaluator
        ));
        let out = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in out.iter() {
            hex.push_str(&format!("{b:02x}"));
        }
        hex
    }
}

/// The known fingerprint registry. Cross-REGISTRY comparison is refused:
/// two registries are two environments, and the spec forbids comparing
/// scores across them (a score from registry A is meaningless in B).
#[derive(Debug, Default)]
pub struct FingerprintRegistry {
    entries: Mutex<HashMap<String, Fingerprint>>,
}

impl FingerprintRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fingerprint; returns its digest (idempotent).
    pub fn register(&self, fp: &Fingerprint) -> String {
        let d = fp.digest();
        self.entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(d.clone(), fp.clone());
        d
    }

    /// Look up by digest.
    pub fn get(&self, digest: &str) -> Option<Fingerprint> {
        self.entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(digest)
            .cloned()
    }

    /// FORBIDDEN operation: comparing a score across two fingerprints
    /// (environments). Always an error - the comparison itself is illegal,
    /// regardless of the values.
    ///
    /// # Errors
    /// Always: cross-environment score comparison is forbidden by spec.
    pub fn cross_environment_compare(&self, _a: &str, _b: &str) -> Result<(), CompareForbidden> {
        Err(CompareForbidden)
    }
}

/// The cross-environment comparison error (the operation itself is illegal).
#[derive(Debug, thiserror::Error)]
#[error("cross-environment score comparison is forbidden (fingerprints are not comparable)")]
pub struct CompareForbidden;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn digest_is_stable_and_part_sensitive() {
        let a = Fingerprint {
            model: "m".into(),
            harness: "h".into(),
            environment: "e".into(),
            evaluator: "v".into(),
        };
        let b = Fingerprint {
            model: "m2".into(),
            harness: "h".into(),
            environment: "e".into(),
            evaluator: "v".into(),
        };
        assert_eq!(a.digest(), a.digest(), "stable");
        assert_ne!(a.digest(), b.digest(), "any part change -> new fingerprint");
    }

    #[test]
    fn cross_environment_comparison_always_fails() {
        let reg = FingerprintRegistry::new();
        let d1 = reg.register(&Fingerprint {
            model: "m".into(),
            harness: "h".into(),
            environment: "e1".into(),
            evaluator: "v".into(),
        });
        let d2 = reg.register(&Fingerprint {
            model: "m".into(),
            harness: "h".into(),
            environment: "e2".into(),
            evaluator: "v".into(),
        });
        assert!(reg.cross_environment_compare(&d1, &d2).is_err());
        // Even comparing a fingerprint with ITSELF is forbidden (the operation,
        // not the values, is the violation).
        assert!(reg.cross_environment_compare(&d1, &d1).is_err());
    }
}
