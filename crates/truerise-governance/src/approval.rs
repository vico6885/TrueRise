//! Fenced approval queue (spec §6 approval row: StateStore epoch 单点发放;
//! 禁"超时视为同意").
//!
//! Every write path carries an epoch handed out by THIS store (single point).
//! A write whose epoch is stale (superseded) is rejected with
//! [`StaleFence::OutdatedEpoch`]. Approvals never expire into consent: a
//! request not approved stays pending until explicitly approved or revoked.

use std::collections::HashMap;
use truerise_core::{CoreError, StaleFence};

/// A pending or decided approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestState {
    /// Waiting for a decision (no timeout-based consent, ever).
    Pending,
    /// Explicitly approved.
    Approved,
    /// Explicitly revoked.
    Revoked,
}

/// The single-point epoch store + approval queue.
#[derive(Debug, Default)]
pub struct ApprovalQueue {
    current_epoch: u64,
    requests: HashMap<String, (RequestState, u64)>,
}

impl ApprovalQueue {
    /// An empty queue at epoch 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Hand out the next epoch (THE single point of issuance).
    pub fn next_epoch(&mut self) -> u64 {
        self.current_epoch += 1;
        self.current_epoch
    }

    /// The current epoch (for read-side staleness checks).
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Fenced write: the write succeeds only when ``epoch`` is the CURRENT
    /// epoch; stale epochs are rejected (lost-update impossible by pairing
    /// this with single-point issuance).
    ///
    /// # Errors
    /// [`StaleFence::OutdatedEpoch`] when ``epoch`` != current.
    pub fn fenced_write(&mut self, epoch: u64, payload: &str) -> Result<String, CoreError> {
        if epoch != self.current_epoch {
            return Err(CoreError::StaleFence(StaleFence::OutdatedEpoch {
                write_epoch: epoch,
                current_epoch: self.current_epoch,
            }));
        }
        Ok(format!("stored:{payload}"))
    }

    /// Queue an approval request (stamped with the current epoch).
    /// Review fix (P1): an EXISTING id is refused - the old unconditional
    /// insert RESET Revoked back to Pending, making the "terminal" revoke
    /// reopenable (and the apply gate behind it bypassable).
    pub fn request(&mut self, id: &str) -> Result<(), CoreError> {
        if self.requests.contains_key(id) {
            return Err(CoreError::RiskGate(
                truerise_core::RiskGate::ApprovalRequired {
                    action: format!("request {id} already exists (revoke is terminal - a new proposal needs a new id)"),
                },
            ));
        }
        self.requests
            .insert(id.to_string(), (RequestState::Pending, self.current_epoch));
        Ok(())
    }

    /// Explicitly approve a PENDING request; anything else stays as-is.
    /// Timeouts NEVER approve (the only transition into Approved is this call).
    pub fn approve(&mut self, id: &str) -> Result<(), CoreError> {
        match self.requests.get_mut(id) {
            Some((state @ RequestState::Pending, _)) => {
                *state = RequestState::Approved;
                Ok(())
            }
            Some((RequestState::Revoked, _)) => Err(CoreError::RiskGate(
                truerise_core::RiskGate::ApprovalRequired {
                    action: format!("request {id} was revoked"),
                },
            )),
            Some((RequestState::Approved, _)) => Ok(()), // idempotent
            None => Err(CoreError::RiskGate(
                truerise_core::RiskGate::ApprovalRequired {
                    action: format!("unknown request {id}"),
                },
            )),
        }
    }

    /// Revoke a request (terminal; approval afterwards is refused).
    pub fn revoke(&mut self, id: &str) {
        if let Some(entry) = self.requests.get_mut(id) {
            *entry = (RequestState::Revoked, entry.1);
        }
    }

    /// The state of a request.
    pub fn state(&self, id: &str) -> Option<RequestState> {
        self.requests.get(id).map(|(s, _)| s.clone())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn stale_epoch_writes_rejected() {
        let mut q = ApprovalQueue::new();
        let e1 = q.next_epoch();
        assert!(q.fenced_write(e1, "a").is_ok());
        let e2 = q.next_epoch();
        // e1 is now stale:
        assert!(matches!(
            q.fenced_write(e1, "stale"),
            Err(CoreError::StaleFence(StaleFence::OutdatedEpoch {
                write_epoch: 1,
                current_epoch: 2
            }))
        ));
        assert!(q.fenced_write(e2, "fresh").is_ok());
    }

    #[test]
    fn timeout_never_approves() {
        let mut q = ApprovalQueue::new();
        q.request("r1");
        // ...time passes... (no API for implicit approval exists at all)
        assert_eq!(q.state("r1"), Some(RequestState::Pending));
        q.approve("r1").unwrap();
        assert_eq!(q.state("r1"), Some(RequestState::Approved));
    }

    #[test]
    fn revoked_then_approve_refused() {
        let mut q = ApprovalQueue::new();
        q.request("r1");
        q.revoke("r1");
        assert!(q.approve("r1").is_err());
        assert_eq!(q.state("r1"), Some(RequestState::Revoked));
    }

    #[test]
    fn revoked_request_cannot_be_reopened() {
        // Review fix (P1): the old request() unconditionally reset Revoked
        // to Pending - the "terminal" revoke was reopenable and the apply
        // gate behind it bypassable.
        let mut q = ApprovalQueue::new();
        q.request("h1").expect("fresh request");
        q.approve("h1").expect("approved");
        q.revoke("h1");
        assert!(q.request("h1").is_err(), "a revoked id is spent forever");
        assert!(q.approve("h1").is_err());
    }

    #[test]
    fn duplicate_request_id_is_refused() {
        let mut q = ApprovalQueue::new();
        q.request("h1").expect("fresh request");
        assert!(q.request("h1").is_err(), "one id, one request");
    }

    #[test]
    fn unknown_request_approval_refused() {
        let mut q = ApprovalQueue::new();
        assert!(q.approve("ghost").is_err());
    }
}
