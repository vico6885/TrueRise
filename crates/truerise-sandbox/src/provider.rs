//! The SandboxProvider port + capability bits (spec §6 sandbox row).
//!
//! Core contract: five methods (create / execute / destroy / metrics /
//! audit_handle) MUST pass conformance. Optional abilities (snapshot / fork /
//! pause / resume) are DECLARED via capability bits - overselling a bit the
//! provider cannot honour is exactly what the conformance suite catches.
//! Every execution records the capability level (full / degraded / none) as
//! an event - silent degradation to "none" is forbidden.

use std::collections::HashMap;

/// Declared optional abilities (spec: snapshot/fork/pause/resume = capability
/// bits, declarative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapabilityBits(pub u8);

impl CapabilityBits {
    /// No optional abilities.
    pub const NONE: CapabilityBits = CapabilityBits(0);
    /// Bit: the provider can snapshot its state.
    pub const SNAPSHOT: u8 = 1 << 0;
    /// Bit: the provider can fork a running sandbox.
    pub const FORK: u8 = 1 << 1;
    /// Bit: the provider can pause execution.
    pub const PAUSE: u8 = 1 << 2;
    /// Bit: the provider can resume execution.
    pub const RESUME: u8 = 1 << 3;

    /// True when ALL of ``required`` are declared.
    pub fn has_all(self, required: u8) -> bool {
        self.0 & required == required
    }
}

/// The observed capability level (lands in config and on every execution
/// event; three states, never silently degraded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityLevel {
    /// All declared abilities work.
    Full,
    /// Some declared abilities failed probing (degraded, writes need
    /// explicit allowance).
    Degraded,
    /// No usable sandbox (execution refused, never silent).
    None,
}

impl CapabilityLevel {
    /// Wire name.
    pub fn as_str(&self) -> &'static str {
        match self {
            CapabilityLevel::Full => "full",
            CapabilityLevel::Degraded => "degraded",
            CapabilityLevel::None => "none",
        }
    }
}

/// Handle to a created sandbox instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxHandle(pub u64);

/// Output of one sandboxed execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOutput {
    /// Exit-style code (0 = ok).
    pub code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// The capability level in force for THIS execution.
    pub capability: CapabilityLevel,
}

/// Errors from the sandbox surface.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// The requested ability is not declared by this provider.
    #[error("sandbox: capability not declared (bit {0})")]
    NotDeclared(u8),
    /// The handle does not exist (destroyed or never created).
    #[error("sandbox: unknown handle {0}")]
    UnknownHandle(u64),
    /// The sandbox refused the operation (e.g. path escapes the fence).
    #[error("sandbox: refused: {0}")]
    Refused(String),
    /// Underlying io error.
    #[error("sandbox io: {0}")]
    Io(#[from] std::io::Error),
}

/// The port every sandbox provider implements (spec: five core methods).
pub trait SandboxProvider: Send + Sync {
    /// Create a sandbox instance; returns its handle.
    fn create(&self, id: u64, workdir: &std::path::Path) -> Result<SandboxHandle, SandboxError>;
    /// Execute one action inside the sandbox.
    fn execute(
        &self,
        handle: SandboxHandle,
        action: &str,
        payload: &HashMap<String, String>,
    ) -> Result<ExecutionOutput, SandboxError>;
    /// Destroy the sandbox instance (release resources; idempotent per handle).
    fn destroy(&self, handle: SandboxHandle) -> Result<(), SandboxError>;
    /// Metrics snapshot (integer units).
    fn metrics(&self, handle: SandboxHandle) -> Result<HashMap<String, u64>, SandboxError>;
    /// The audit-trail reference for this sandbox (chain row kind prefix).
    fn audit_handle(&self, handle: SandboxHandle) -> Result<String, SandboxError>;
}
