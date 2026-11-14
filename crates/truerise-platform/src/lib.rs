//! truerise-platform: the ONLY platform boundary of the harness.
//!
//! Spec (§5): every module must reach platform primitives through this crate
//! (file locks, monotonic clocks, process surface) - reaching them directly
//! elsewhere is a CI red. Modules:
//!
//! - [`monotonic`]: the only clock for leases/timeouts (wall-clock forbidden
//!   in that domain by construction).
//! - [`file_lock`]: OS-level exclusive file lock (ledger append critical
//!   section).
//! - [`process`]: spawn/wait/kill surface with integer-millisecond metering.

#![deny(clippy::undocumented_unsafe_blocks)]

pub mod file_lock;
pub mod monotonic;
pub mod process;
