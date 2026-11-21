//! Trajectory stepwise signing (G9): every action in the run tape carries
//! the executor's signature plus a content hash, chained to the previous
//! step - judging can read the PROCESS, not just the terminal state, and
//! offline replay keeps the tape semantics.
//!
//! Evidence semantics (the acceptance contract): a step that is unsigned
//! or signature-INVALID contributes NOTHING to the evidence digest - an
//! unsigned step is as good as absent. The digest of the trajectory is
//! computed over verified steps only, which makes it the dependency root
//! for the downstream consumers (process grading, counterfactual replay).
//!
//! Signature domain: each step signs
//! `sha256(index_be || kind || content_hash || prev_step_hash)`, where
//! `prev_step_hash` is the previous VERIFIED step's digest - tampering
//! with any step invalidates it AND everything after it.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in out.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// One trajectory step. `signature` signs the step's position, kind,
/// content hash and the previous verified step's hash - together they bind
/// the WHOLE prefix, not just this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    /// Position in the trajectory (0-based; gaps break the chain).
    pub index: u64,
    /// What kind of action this is (tool call, gate decision, ...).
    pub kind: String,
    /// sha256 of the action's content bytes.
    pub content_hash: String,
    /// The executor's ed25519 signature over the step domain. `None` =
    /// unsigned: the step carries NO evidence.
    pub signature: Option<Signature>,
}

impl StepRecord {
    /// The step-hash domain: everything the signature commits to,
    /// LENGTH-FRAMED (review A3-P2-2: without field lengths a legitimately
    /// signed step re-splits into a different (kind, content_hash) pair
    /// and still verifies - the same bug class as byte-string truncation).
    fn domain(&self, prev_hash: &str) -> Vec<u8> {
        let frame = |field: &str| -> Vec<u8> {
            let mut out = (field.len() as u64).to_be_bytes().to_vec();
            out.extend_from_slice(field.as_bytes());
            out
        };
        let mut hasher = Sha256::new();
        hasher.update(self.index.to_be_bytes());
        hasher.update(frame(&self.kind));
        hasher.update(frame(&self.content_hash));
        hasher.update(frame(prev_hash));
        hasher.finalize().to_vec()
    }

    /// The step's own hash (what the evidence digest folds over).
    fn step_hash(&self, prev_hash: &str) -> String {
        sha256_hex(&self.domain(prev_hash))
    }
}

/// A signed trajectory under construction.
#[derive(Debug)]
pub struct TrajectoryBuilder<'a> {
    key: &'a SigningKey,
    steps: Vec<StepRecord>,
    prev_hash: String,
}

impl<'a> TrajectoryBuilder<'a> {
    /// Start an empty trajectory signed by the executor's key.
    #[must_use]
    pub fn new(key: &'a SigningKey) -> Self {
        Self {
            key,
            steps: Vec::new(),
            prev_hash: String::new(),
        }
    }

    /// Append and SIGN one action. The content bytes are hashed here - the
    /// caller passes the raw action content, never a pre-computed hash.
    pub fn append(&mut self, kind: &str, content: &[u8]) -> u64 {
        let index = self.steps.len() as u64;
        let content_hash = sha256_hex(content);
        let domain = StepRecord {
            index,
            kind: kind.to_string(),
            content_hash: content_hash.clone(),
            signature: None,
        }
        .domain(&self.prev_hash);
        let signature = self.key.sign(&domain);
        self.prev_hash = sha256_hex(&domain);
        self.steps.push(StepRecord {
            index,
            kind: kind.to_string(),
            content_hash,
            signature: Some(signature),
        });
        index
    }

    /// Freeze the trajectory (the executor's public key rides along: the
    /// verification path needs it, and it identifies WHO signed).
    #[must_use]
    pub fn finish(self) -> Trajectory {
        Trajectory {
            steps: self.steps,
            executor: self.key.verifying_key().to_bytes(),
        }
    }
}

/// A frozen trajectory: the signed record of WHAT happened, plus the
/// executor's public key (verification needs it; it identifies WHO signed).
#[derive(Debug, Clone, Default)]
pub struct Trajectory {
    steps: Vec<StepRecord>,
    executor: [u8; 32],
}

impl PartialEq for Trajectory {
    fn eq(&self, other: &Self) -> bool {
        self.steps == other.steps && self.executor == other.executor
    }
}
impl Eq for Trajectory {}

impl Trajectory {
    /// Assemble from persisted records (offline verification path): the
    /// executor's public key bytes MUST travel with the steps.
    #[must_use]
    pub fn from_steps(steps: Vec<StepRecord>, executor: [u8; 32]) -> Self {
        Self { steps, executor }
    }

    /// The verified steps, re-derived on the spot: index chain intact,
    /// signature valid, prefix binding intact. An unsigned or tampered
    /// step is EXCLUDED - it contributes no evidence, and everything after
    /// it inherits the broken prefix binding. An unresolvable executor key
    /// means NO step carries evidence.
    #[must_use]
    pub fn evidence_steps(&self) -> Vec<&StepRecord> {
        let Ok(key) = VerifyingKey::from_bytes(&self.executor) else {
            return Vec::new();
        };
        let mut verified: Vec<&StepRecord> = Vec::new();
        let mut prev_hash = String::new();
        for step in &self.steps {
            let Some(signature) = &step.signature else {
                continue; // unsigned: no evidence
            };
            let domain = step.domain(&prev_hash);
            if key.verify_strict(&domain, signature).is_ok() {
                prev_hash = step.step_hash(&prev_hash);
                verified.push(step);
            }
        }
        verified
    }

    /// Verify the whole trajectory (every step signed and valid).
    #[must_use]
    pub fn verify(&self) -> bool {
        self.evidence_steps().len() == self.steps.len()
    }

    /// The evidence digest: sha256 over the executor public key PLUS the
    /// VERIFIED steps' step-hashes ONLY. Provenance binds WHO attests, so
    /// identical content signed by different executors digests differently
    /// (a signature swap cannot hide). Unsigned or tampered steps
    /// contribute nothing. `None` when no step carries evidence (an empty
    /// or fully unsigned trajectory proves nothing).
    #[must_use]
    pub fn evidence_digest(&self) -> Option<String> {
        let verified = self.evidence_steps();
        if verified.is_empty() {
            return None;
        }
        let mut hasher = Sha256::new();
        hasher.update(self.executor);
        let mut prev_hash = String::new();
        for step in &verified {
            let hash = step.step_hash(&prev_hash);
            hasher.update(hash.as_bytes());
            prev_hash = hash;
        }
        Some(sha256_hex(&hasher.finalize()))
    }

    /// All persisted steps, including the unsigned ones (for tooling that
    /// reports WHY a trajectory is incomplete).
    #[must_use]
    pub fn steps(&self) -> &[StepRecord] {
        &self.steps
    }

    /// The executor's public key bytes - the offline reconstruction path
    /// (`from_steps`) needs them to verify the signatures, so they travel
    /// with the steps.
    #[must_use]
    pub fn executor(&self) -> &[u8; 32] {
        &self.executor
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use ed25519_dalek::SigningKey;

    fn signed_trajectory(seed: [u8; 32], actions: &[(&str, &[u8])]) -> Trajectory {
        let key = SigningKey::from_bytes(&seed);
        let mut builder = TrajectoryBuilder::new(&key);
        for (kind, content) in actions {
            builder.append(kind, content);
        }
        builder.finish()
    }

    #[test]
    fn a_signed_trajectory_fully_verifies_and_digests() {
        let t = signed_trajectory(
            [7; 32],
            &[("gate", b"allow tool:read"), ("run", b"step content")],
        );
        assert!(t.verify());
        assert!(t.evidence_digest().is_some());
        assert_eq!(t.evidence_steps().len(), t.steps().len());
    }

    #[test]
    fn tampering_with_any_step_voids_it_and_the_prefix_after_it() {
        let mut t = signed_trajectory(
            [7; 32],
            &[
                ("gate", b"allow tool:read"),
                ("run", b"step content"),
                ("judge", b"pass"),
            ],
        );
        // Rewrite step 0's content hash (the signature no longer matches).
        t.steps[0].content_hash = "deadbeef".to_string();
        let verified = t.evidence_steps();
        // Step 0 is excluded, and steps AFTER it inherit the broken prefix
        // binding - their signatures were computed over the original chain.
        assert!(verified.iter().all(|s| s.index != 0));
        assert!(!t.verify(), "tampering must break verification");
    }

    /// THE acceptance: an unsigned step contributes NO evidence - the
    /// evidence digest of a trajectory with an injected unsigned step is
    /// IDENTICAL to the clean trajectory without it.
    #[test]
    fn unsigned_step_contributes_no_evidence() {
        let clean = signed_trajectory([9; 32], &[("gate", b"allow"), ("run", b"content")]);
        let mut with_interloper = clean.clone();
        with_interloper.steps.push(StepRecord {
            index: 2,
            kind: "judge".to_string(),
            content_hash: sha256_hex(b"fabricated verdict"),
            signature: None, // the interloper: present, unsigned
        });
        let _ = &with_interloper.executor;
        assert_eq!(
            with_interloper.evidence_digest(),
            clean.evidence_digest(),
            "an unsigned step changed the evidence digest"
        );
        assert_eq!(with_interloper.evidence_steps().len(), 2);
        assert_eq!(with_interloper.steps().len(), 3);
        assert!(
            !with_interloper.verify(),
            "an unsigned step is not verified"
        );
        // A fully unsigned trajectory proves nothing at all.
        let mut unsigned = clean.clone();
        for step in &mut unsigned.steps {
            step.signature = None;
        }
        assert!(unsigned.evidence_digest().is_none());
    }

    #[test]
    fn a_different_executor_produces_a_different_digest() {
        let a = signed_trajectory([1; 32], &[("gate", b"allow")]);
        let b = signed_trajectory([2; 32], &[("gate", b"allow")]);
        assert_ne!(a.evidence_digest(), b.evidence_digest());
    }
}
