//! truerise-sandbox: the SandboxProvider port, lifecycle, capability bits,
//! and the conformance self-check (facts only - the judging authority is
//! truerise-eval, and self-check output is forbidden as eval input).

pub mod conformance;
pub mod inprocess;
pub mod provider;

pub use conformance::{ConformanceReport, ProbeFact, self_check};
pub use inprocess::{InProcessProvider, declared_capabilities, next_id};
pub use provider::{
    CapabilityBits, CapabilityLevel, ExecutionOutput, SandboxError, SandboxHandle, SandboxProvider,
};
