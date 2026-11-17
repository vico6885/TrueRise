//! The approval-gated APPLY path (RFC 0003) - the piece that closes the RSI
//! loop without ever opening an auto-merge door:
//!
//! propose (never merges) -> HUMAN approval token -> apply -> re-eval ->
//! ratchet -> rollback on regression. Every refusal is audited; every
//! generation leaves a lineage row on the chain.
//!
//! The file write itself is INJECTED (``write`` closure) - the boundary
//! wires the sandbox executor, so the write goes through the W2 policy and
//! the W3 budget like any other action.

use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Apply/rollback errors. Every variant is a REFUSAL - nothing was written.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// No approval token was presented (there is NO default-apply path).
    #[error("apply refused: no human approval token")]
    ApprovalRequired,
    /// The token does not bind to THESE proposal bytes (approve A, apply B).
    #[error("apply refused: approval hash mismatch (token {token} != proposal {proposal})")]
    HashMismatch {
        /// The hash the token carries.
        token: String,
        /// The hash of the content actually being applied.
        proposal: String,
    },
    /// The approval is not LIVE in the queue (revoked or unknown).
    #[error("apply refused: approval not live in the queue")]
    NotApproved,
    /// The snapshot write failed the epoch fence (stale epoch).
    #[error("apply refused: stale epoch (fenced write rejected)")]
    StaleEpoch,
    /// Review fix (P1): the CURRENT file could not be READ (permissions,
    /// non-UTF-8) - its snapshot would be a silent empty string, faking the
    /// lineage (parent = hash of nothing) and arming a rollback that wipes
    /// the file. Refusal instead.
    #[error("apply refused: current file unreadable (snapshot would be fabricated): {0}")]
    SnapshotUnreadable(String),
    /// The injected write failed (the boundary refused or io failed).
    #[error("apply refused: write failed: {0}")]
    WriteFailed(String),
    /// Chain refusal.
    #[error("apply: chain: {0}")]
    Chain(String),
}

/// One applied generation's lineage facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedGeneration {
    /// sha256 of the content BEFORE the apply (the snapshot).
    pub parent_hash: String,
    /// sha256 of the content AFTER the apply.
    pub child_hash: String,
    /// The byte-exact snapshot (for the ratchet's rollback).
    pub snapshot: String,
}

/// sha256 hex of ``content`` (the proposal/lineage identity).
#[must_use]
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in out.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// The single path from proposal to applied change (RFC 0003). Refuses -
/// and writes NOTHING - unless every gate passes, in this order:
/// 1. an approval token is present,
/// 2. the token's hash == sha256(``child_content``) byte-exact,
/// 3. the approval is LIVE in ``queue`` (Approved, not revoked/unknown),
/// 4. the snapshot write survives the epoch fence (``observed_epoch`` must
///    be the queue's current epoch - stale is refused),
/// 5. the injected ``write`` succeeds (sandbox/policy/budget at the
///    boundary).
///
/// # Errors
/// [`ApplyError`] - every variant means NOTHING was written.
pub fn apply_proposal(
    skill_path: &Path,
    child_content: &str,
    approval_hash: Option<&str>,
    observed_epoch: u64,
    queue: &mut truerise_governance::ApprovalQueue,
    chain: &truerise_governance::HashChain,
    write: &dyn Fn(&str) -> Result<(), String>,
) -> Result<AppliedGeneration, ApplyError> {
    let child_hash = content_hash(child_content);
    let refuse = |kind: &str, detail: serde_json::Value| {
        let _ = chain.append(
            "evolve.apply_refused",
            truerise_wire::Severity::Warn,
            &json!({ "reason": kind, "detail": detail, "path": skill_path.display().to_string() }),
            None,
        );
    };
    // Gate 1: no token, no apply. Ever.
    let Some(token_hash) = approval_hash else {
        refuse(
            "approval_required",
            json!({ "path": skill_path.display().to_string() }),
        );
        return Err(ApplyError::ApprovalRequired);
    };
    // Gate 2: the token binds to THESE bytes (defeats approve-A-apply-B).
    if token_hash != child_hash {
        refuse(
            "approval_hash_mismatch",
            json!({ "token": token_hash, "proposal": child_hash }),
        );
        return Err(ApplyError::HashMismatch {
            token: token_hash.to_string(),
            proposal: child_hash,
        });
    }
    // Gate 3: the approval must be LIVE (revoked/unknown never apply).
    use truerise_governance::RequestState;
    if queue.state(token_hash) != Some(RequestState::Approved) {
        refuse("approval_not_live", json!({ "token": token_hash }));
        return Err(ApplyError::NotApproved);
    }
    // Gate 4: the fenced snapshot write (stale epoch refused - the SAME
    // fence the approval queue has used since M2).
    // Review fix (P1): read errors are NOT silently mapped to an empty
    // snapshot - that fakes the lineage and arms a wiping rollback.
    let snapshot = match std::fs::read_to_string(skill_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(ApplyError::SnapshotUnreadable(e.to_string())),
    };
    let parent_hash = content_hash(&snapshot);
    if queue
        .fenced_write(observed_epoch, &format!("snapshot:{parent_hash}"))
        .is_err()
    {
        refuse("stale_epoch", json!({ "observed_epoch": observed_epoch }));
        return Err(ApplyError::StaleEpoch);
    }
    // Gate 5: the injected write (sandbox-backed at the boundary).
    if let Err(e) = write(child_content) {
        refuse("write_failed", json!({ "error": e }));
        return Err(ApplyError::WriteFailed(e));
    }
    if let Err(e) = chain.append(
        "evolve.apply",
        truerise_wire::Severity::Info,
        &json!({
            "path": skill_path.display().to_string(),
            "parent_hash": parent_hash,
            "child_hash": child_hash,
            "epoch": observed_epoch,
            "snapshot_bytes": snapshot.len()
        }),
        None,
    ) {
        // Review fix (P1): the FILE is already written at this point - the
        // old plain refusal lied ("nothing was written"). Compensate: roll
        // the file back to the byte-exact snapshot, record the
        // compensation, and only then return the refusal.
        let _ = std::fs::write(skill_path, snapshot.as_bytes());
        let _ = chain.append(
            "evolve.apply_compensated",
            truerise_wire::Severity::Error,
            &json!({
                "path": skill_path.display().to_string(),
                "reason": e.to_string(),
                "restored_bytes": snapshot.len()
            }),
            None,
        );
        return Err(ApplyError::Chain(e.to_string()));
    }
    Ok(AppliedGeneration {
        parent_hash,
        child_hash,
        snapshot,
    })
}

/// Roll an applied generation back to its byte-exact snapshot and chain the
/// reason (the ratchet's undo; the reason is the AUDITABLE fact).
///
/// # Errors
/// [`ApplyError`] - write or chain refusal.
pub fn rollback(
    skill_path: &Path,
    snapshot: &str,
    reason: &str,
    chain: &truerise_governance::HashChain,
    write: &dyn Fn(&str) -> Result<(), String>,
) -> Result<(), ApplyError> {
    write(snapshot).map_err(ApplyError::WriteFailed)?;
    chain
        .append(
            "evolve.rollback",
            truerise_wire::Severity::Warn,
            &json!({
                "path": skill_path.display().to_string(),
                "restored_hash": content_hash(snapshot),
                "reason": reason
            }),
            None,
        )
        .map(|_| ())
        .map_err(|e| ApplyError::Chain(e.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "truerise_apply_{tag}_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn chain(dir: &std::path::Path) -> truerise_governance::HashChain {
        truerise_governance::HashChain::open(&dir.join("a.jsonl")).unwrap()
    }

    fn fs_write(path: PathBuf) -> impl Fn(&str) -> Result<(), String> {
        move |content: &str| std::fs::write(&path, content).map_err(|e| e.to_string())
    }

    const GEN0: &str = r#"{"schema_version":"truerise.answers.v1","answers":{"task-1":"a"}}"#;
    const GEN1: &str = r#"{"schema_version":"truerise.answers.v1","answers":{"task-1":"b"}}"#;

    #[test]
    fn apply_without_human_approval_token_is_refused() {
        let dir = temp_dir("no_token");
        let chain = chain(&dir);
        let mut queue = truerise_governance::ApprovalQueue::new();
        let path = dir.join("skill.json");
        let err = apply_proposal(
            &path,
            GEN1,
            None,
            queue.current_epoch(),
            &mut queue,
            &chain,
            &fs_write(path.clone()),
        )
        .unwrap_err();
        assert!(matches!(err, ApplyError::ApprovalRequired));
        assert!(!path.exists(), "nothing was written");
    }

    #[test]
    fn approval_hash_mismatch_with_proposal_is_refused() {
        let dir = temp_dir("mismatch");
        let chain = chain(&dir);
        let mut queue = truerise_governance::ApprovalQueue::new();
        let other = content_hash(GEN0);
        let _ = queue.request(&other);
        queue.approve(&other).unwrap();
        let path = dir.join("skill.json");
        let err = apply_proposal(
            &path,
            GEN1,
            Some(&other), // approved the WRONG bytes (approve A, apply B)
            queue.current_epoch(),
            &mut queue,
            &chain,
            &fs_write(path.clone()),
        )
        .unwrap_err();
        assert!(matches!(err, ApplyError::HashMismatch { .. }), "{err}");
        assert!(!path.exists());
    }

    #[test]
    fn stale_or_revoked_epoch_write_rejected() {
        let dir = temp_dir("fence");
        let chain = chain(&dir);
        let mut queue = truerise_governance::ApprovalQueue::new();
        let child_hash = content_hash(GEN1);
        let _ = queue.request(&child_hash);
        queue.approve(&child_hash).unwrap();
        let path = dir.join("skill.json");
        // STALE epoch: the caller observed epoch 0, the queue moved on.
        queue.next_epoch();
        let err = apply_proposal(
            &path,
            GEN1,
            Some(&child_hash),
            0, // stale
            &mut queue,
            &chain,
            &fs_write(path.clone()),
        )
        .unwrap_err();
        assert!(matches!(err, ApplyError::StaleEpoch), "{err}");
        assert!(!path.exists());

        // REVOKED approval: live-queue gate refuses.
        let mut queue2 = truerise_governance::ApprovalQueue::new();
        let _ = queue2.request(&child_hash);
        queue2.approve(&child_hash).unwrap();
        queue2.revoke(&child_hash);
        let err = apply_proposal(
            &path,
            GEN1,
            Some(&child_hash),
            queue2.current_epoch(),
            &mut queue2,
            &chain,
            &fs_write(path.clone()),
        )
        .unwrap_err();
        assert!(matches!(err, ApplyError::NotApproved), "{err}");
        assert!(!path.exists());
    }

    #[test]
    fn approved_and_fenced_apply_writes_and_records_lineage() {
        let dir = temp_dir("ok");
        let chain = chain(&dir);
        let mut queue = truerise_governance::ApprovalQueue::new();
        std::fs::write(dir.join("skill.json"), GEN0).unwrap();
        let path = dir.join("skill.json");
        let child_hash = content_hash(GEN1);
        let _ = queue.request(&child_hash);
        queue.approve(&child_hash).unwrap();
        let applied = apply_proposal(
            &path,
            GEN1,
            Some(&child_hash),
            queue.current_epoch(),
            &mut queue,
            &chain,
            &fs_write(path.clone()),
        )
        .unwrap();
        assert_eq!(applied.child_hash, child_hash);
        assert_eq!(applied.parent_hash, content_hash(GEN0));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            GEN1,
            "the new content is live"
        );
        assert!(
            chain
                .governing_rows()
                .iter()
                .any(|r| r.event.kind == "evolve.apply"),
            "lineage row on the chain"
        );
    }
}
