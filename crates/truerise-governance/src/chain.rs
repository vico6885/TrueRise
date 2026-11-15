//! The append-only audit hash chain (spec §6 audit-chain row - the single
//! authority for "what happened", zero exemptions).
//!
//! Row shape matches [`truerise_wire::AuditEventV2`]:
//! `{seq, ts, kind, severity, detail, prev_hash, hash}` with
//! `hash = sha256(canonical json of all fields except hash)`.
//!
//! Invariants (all enforced):
//! - append holds the file lock, reads the tail inside the lock, and flushes
//!   before releasing (the crash window is the OS flush granularity, declared
//!   per §6.7: single-writer + append-only + file lock);
//! - verify recomputes the WHOLE chain: seq continuity, prev-hash linkage,
//!   content hashes - tampering is located by row number and fails closed;
//! - idempotent replay: a row with the same canonical key (kind + detail key)
//!   is written once, replays skip;
//! - legacy-prefixed rows are legal to store but never govern.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use truerise_platform::file_lock::lock_exclusive;
use truerise_wire::{AuditEventV2, Severity};

/// Genesis previous-hash: 64 zeros.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A governance error from the chain layer.
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    /// The chain failed verification; the offending row numbers are listed.
    #[error("chain verification failed at rows {rows:?}: {reason}")]
    Tampered {
        /// Offending row numbers (0-based).
        rows: Vec<usize>,
        /// Aggregate reason.
        reason: String,
    },
    /// An I/O or lock error while appending.
    #[error("chain io error: {0}")]
    Io(#[from] std::io::Error),
    /// A float appeared in a chain row (canonical-JSON float rule,
    /// docs/determinism.md): chain rows carry integers or strings ONLY -
    /// `10.0` vs `10` divergence is a real cross-implementation bug class.
    #[error("chain: float forbidden in row detail at {0} (integer or string only)")]
    FloatForbidden(String),
}

/// Walk a JSON tree and report the FIRST float value's path (the
/// canonical-JSON float rule's enforcement point).
fn find_float(value: &serde_json::Value, path: &str) -> Option<String> {
    match value {
        serde_json::Value::Number(n) => {
            if n.as_f64()
                .is_some_and(|f| f.fract() != 0.0 || n.to_string().contains('.'))
            {
                Some(path.to_string())
            } else {
                None
            }
        }
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| find_float(v, &format!("{path}[{i}]"))),
        serde_json::Value::Object(map) => map
            .iter()
            .find_map(|(k, v)| find_float(v, &format!("{path}.{k}"))),
        _ => None,
    }
}

/// One in-memory chain row (thin wrapper over the wire type + legacy flag).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The wire-shaped event.
    pub event: AuditEventV2,
    /// Legacy rows (``legacy:`` prefixed kinds) never participate in governance.
    pub legacy: bool,
}

impl Row {
    fn new(
        seq: u64,
        kind: &str,
        severity: Severity,
        detail: &serde_json::Value,
        prev_hash: &str,
        legacy: bool,
    ) -> Self {
        let event = AuditEventV2 {
            seq,
            ts: String::new(), // filled by the chain at append (monotonic counter, not wall clock)
            kind: kind.to_string(),
            severity,
            detail: detail.clone(),
            prev_hash: prev_hash.to_string(),
            hash: String::new(),
        };
        Self { event, legacy }
    }

    fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        // Canonical order: the struct field order, detail pre-serialized.
        let detail = self.event.detail.to_string();
        hasher.update(format!(
            "{{\"detail\":{},\"hash\":null,\"kind\":{},\"prev_hash\":{},\"seq\":{},\"severity\":{},\"ts\":{}}}",
            detail,
            serde_json::to_string(&self.event.kind).unwrap_or_default(),
            serde_json::to_string(&self.event.prev_hash).unwrap_or_default(),
            self.event.seq,
            serde_json::to_string(severity_name(self.event.severity)).unwrap_or_default(),
            serde_json::to_string(&self.event.ts).unwrap_or_default(),
        ));
        let out = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in out.iter() {
            hex.push_str(&format!("{b:02x}"));
        }
        hex
    }

    fn finalize(mut self) -> Self {
        self.event.hash = self.content_hash();
        self
    }
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

/// The append-only chain: file-backed, lock-guarded, single writer per file.
#[derive(Debug)]
pub struct HashChain {
    path: PathBuf,
    inner: Mutex<ChainState>,
}

#[derive(Debug)]
struct ChainState {
    rows: Vec<Row>,
}

impl HashChain {
    /// Open (or create) a chain at ``path``; the existing file is verified
    /// before any append (fail-closed on a tampered history).
    ///
    /// # Errors
    /// [`ChainError::Tampered`] when the persisted rows fail verification;
    /// [`ChainError::Io`] on file/lock errors.
    pub fn open(path: &Path) -> Result<Self, ChainError> {
        let rows = if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let mut rows = Vec::new();
            for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
                let ev: AuditEventV2 =
                    serde_json::from_str(line).map_err(|e| ChainError::Tampered {
                        rows: vec![i],
                        reason: format!("unparsable row {i}: {e}"),
                    })?;
                let legacy = ev.kind.starts_with("legacy:");
                rows.push(Row { event: ev, legacy });
            }
            let problems = verify_rows(&rows);
            if !problems.is_empty() {
                return Err(ChainError::Tampered {
                    rows: problems.iter().map(|(i, _)| *i).collect(),
                    reason: problems
                        .iter()
                        .map(|(_, r)| r.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
            rows
        } else {
            Vec::new()
        };
        Ok(Self {
            path: path.to_path_buf(),
            inner: Mutex::new(ChainState { rows }),
        })
    }

    /// Append one event. Full-chain verification runs inside the lock before
    /// the write (tamper between calls fails closed).
    ///
    /// Idempotent replay: when ``replay_key`` is provided and a governing row
    /// already carries ``detail.key == replay_key``, this call is a no-op.
    ///
    /// # Errors
    /// See [`Self::open`]; also fails closed on mid-session tampering.
    pub fn append(
        &self,
        kind: &str,
        severity: Severity,
        detail: &serde_json::Value,
        replay_key: Option<&str>,
    ) -> Result<Row, ChainError> {
        let mut state = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        // Lock the FILE, then read/write ONLY through the locked handle:
        // Windows byte-range locks are process-wide, so a second handle
        // (e.g. fs::read_to_string) would collide with our own lock.
        let mut guard = lock_exclusive(&self.path)?;
        let text = String::from_utf8_lossy(&guard.read_all()?).to_string();
        let count = text.lines().filter(|l| !l.trim().is_empty()).count();
        if count != state.rows.len() {
            // Another writer appended; reload and verify before continuing.
            let mut rows = Vec::new();
            for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
                let ev: AuditEventV2 =
                    serde_json::from_str(line).map_err(|e| ChainError::Tampered {
                        rows: vec![i],
                        reason: format!("unparsable row {i}: {e}"),
                    })?;
                let legacy = ev.kind.starts_with("legacy:");
                rows.push(Row { event: ev, legacy });
            }
            let problems = verify_rows(&rows);
            if !problems.is_empty() {
                return Err(ChainError::Tampered {
                    rows: problems.iter().map(|(i, _)| *i).collect(),
                    reason: problems
                        .iter()
                        .map(|(_, r)| r.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
            state.rows = rows;
        }

        // The float rule (docs/determinism.md): chain rows never carry
        // floats - integers or strings only, enforced at THE write gate.
        if let Some(path) = find_float(detail, "detail") {
            return Err(ChainError::FloatForbidden(path));
        }

        let seq = state.rows.last().map(|r| r.event.seq + 1).unwrap_or(0);
        let prev_hash = state
            .rows
            .last()
            .map(|r| r.event.hash.clone())
            .unwrap_or_else(|| GENESIS.to_string());
        let legacy = kind.starts_with("legacy:");

        // Idempotent replay against governing rows (in-lock, post-reload).
        if let Some(key) = replay_key {
            if state.rows.iter().any(|r| {
                r.event.governs() && r.event.detail.get("key").and_then(|k| k.as_str()) == Some(key)
            }) {
                if let Some(existing) = state.rows.last().cloned() {
                    return Ok(existing);
                }
                return Err(ChainError::Tampered {
                    rows: vec![],
                    reason: "dedup hit but chain empty".into(),
                });
            }
        }

        let mut row = Row::new(seq, kind, severity, detail, &prev_hash, legacy);
        // ts: a per-chain monotonic counter rendered as a string (wall-clock
        // stays OUT of the chain by design; ordering authority is seq+hash).
        row.event.ts = format!("seq@{seq}");
        let row = row.finalize();
        state.rows.push(row.clone());

        let mut line = serde_json::to_string(&row.event).map_err(|e| ChainError::Tampered {
            rows: vec![],
            reason: e.to_string(),
        })?;
        line.push('\n');
        guard.write_all(line.as_bytes())?;
        drop(guard);
        Ok(row)
    }

    /// Full verification (all three checks), locating offending rows.
    pub fn verify(&self) -> Result<(), ChainError> {
        let state = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let problems = verify_rows(&state.rows);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(ChainError::Tampered {
                rows: problems.iter().map(|(i, _)| *i).collect(),
                reason: problems
                    .iter()
                    .map(|(_, r)| r.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            })
        }
    }

    /// Snapshot of governing (non-legacy) rows.
    pub fn governing_rows(&self) -> Vec<Row> {
        let state = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        state
            .rows
            .iter()
            .filter(|r| r.event.governs() && !r.legacy)
            .cloned()
            .collect()
    }

    /// Total row count.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .rows
            .len()
    }

    /// True when the chain has no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Three-check verification over an in-memory row list: seq continuity,
/// prev-hash linkage, content hash. Returns offending (row, reason) pairs.
fn verify_rows(rows: &[Row]) -> Vec<(usize, String)> {
    let mut problems = Vec::new();
    let mut prev = GENESIS.to_string();
    for (i, row) in rows.iter().enumerate() {
        if row.event.seq != i as u64 {
            problems.push((
                i,
                format!("seq discontinuity (got {}, want {i})", row.event.seq),
            ));
        }
        if row.event.prev_hash != prev {
            problems.push((i, "prev_hash mismatch (chain broken here)".to_string()));
        }
        if row.event.hash != row.content_hash() {
            problems.push((i, "content hash mismatch (tampered)".to_string()));
        }
        prev = row.event.hash.clone();
    }
    problems
}
