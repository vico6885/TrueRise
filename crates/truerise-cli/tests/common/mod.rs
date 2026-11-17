//! Shared test helpers (compiled into EACH test binary - the cross-binary
//! serialization below works because the lock is a FILE, not a process
//! mutex: Windows byte-range locks are process-wide, so two test binaries
//! appending to the same run chain would collide without it).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A unique temp dir per call (pid + counter, like the per-file pattern).
pub fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_test_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Released on Drop (even on panic - Drop runs while unwinding).
pub struct ChainGuard {
    path: PathBuf,
}

impl Drop for ChainGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Cross-BINARY lock around the shared run chain: a create_new spin-lock on
/// a well-known file next to the chain. Review fix (flake root cure): the
/// deadline branch used to DELETE the live lock unconditionally - after 60s
/// every waiter became a remover and two tests could both "hold" the lock
/// (the revolving-door race). Now: only an AGE-STALE lock (>60s, i.e. a
/// crashed holder) is taken over; a live contention past the deadline
/// panics fail-fast instead of dismantling someone else's lock.
pub fn chain_guard(tag: &str) -> ChainGuard {
    let dir = std::env::temp_dir().join("truerise_cli_run");
    std::fs::create_dir_all(&dir).expect("chain dir");
    let path = dir.join(".test_chain_lock");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return ChainGuard { path },
            Err(_) => {
                let age_over_60s = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|m| m.elapsed().ok())
                    .map(|e| e.as_secs() > 60)
                    .unwrap_or(false);
                if age_over_60s {
                    // a crashed holder's lock: takeover is safe
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                if Instant::now() > deadline {
                    panic!(
                        "chain lock contention past 120s (holder is alive; refusing to dismantle it) - test: {tag}"
                    );
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}
