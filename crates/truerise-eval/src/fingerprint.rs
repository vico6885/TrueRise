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
    /// G9: pin a trajectory evidence digest INTO the fingerprint. The
    /// trajectory pins into the evaluator dimension (the judging context
    /// includes the signed trajectory it judged): two fingerprints with
    /// different trajectories are different fingerprints (acceptance:
    /// `trajectory_digest_pinned_in_fingerprint`).
    ///
    /// The digest MUST come from
    /// `truerise_observe::trajectory::Trajectory::evidence_digest` - an
    /// unsigned or tampered trajectory has none to pin (review B3-P2-9:
    /// the previous wording named a crate outside this dependency graph).
    /// Pinning is IDEMPOTENT: an already-pinned evaluator is re-pinned
    /// from its base (review A3-P2-6 - a double pin produced two stacked
    /// markers). The pinned digest is recoverable via
    /// [`Self::trajectory_digest`], the last-marker convention.
    #[must_use]
    pub fn pinned_with_trajectory(&self, trajectory_digest: &str) -> Fingerprint {
        let mut pinned = self.clone();
        pinned.evaluator = format!("{}|trajectory={}", self.base_evaluator(), trajectory_digest);
        pinned
    }

    /// The pinned trajectory digest, if this fingerprint carries one
    /// (the last `|trajectory=` marker wins).
    #[must_use]
    pub fn trajectory_digest(&self) -> Option<&str> {
        self.evaluator
            .rsplit("|trajectory=")
            .next()
            .filter(|rest| !rest.contains('|'))
    }

    /// The evaluator id with any trajectory pin stripped.
    fn base_evaluator(&self) -> &str {
        match self.evaluator.rsplit_once("|trajectory=") {
            Some((base, _)) => base,
            None => &self.evaluator,
        }
    }

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

    /// G9 acceptance: the trajectory digest is pinned INTO the fingerprint.
    /// Same four parts with a different trajectory digest produce a
    /// different fingerprint digest, and the pinned digest stays
    /// recoverable from the evaluator id.
    #[test]
    fn trajectory_digest_pinned_in_fingerprint() {
        let base = Fingerprint {
            model: "m".to_string(),
            harness: "h".to_string(),
            environment: "e".to_string(),
            evaluator: "v".to_string(),
        };
        let pinned_a = base.pinned_with_trajectory("aaaaaaaa");
        let pinned_b = base.pinned_with_trajectory("bbbbbbbb");
        assert_ne!(
            pinned_a.digest(),
            pinned_b.digest(),
            "trajectory must change the fingerprint"
        );
        assert_ne!(
            pinned_a.digest(),
            base.digest(),
            "pinning must change the fingerprint"
        );
        // Recoverability: the pinned digest is part of the evaluator id.
        assert!(pinned_a.evaluator.ends_with("|trajectory=aaaaaaaa"));
        // Registry-compatible: the pinned fingerprint registers as itself.
        let registry = FingerprintRegistry::new();
        let digest = registry.register(&pinned_a);
        assert_eq!(registry.get(&digest).unwrap().evaluator, pinned_a.evaluator);
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
