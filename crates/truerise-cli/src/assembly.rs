//! Boundary assembly for the governed tool path (RFC 0002): the governance
//! policy and the fenced sandbox executor live HERE, at the boundary, so
//! `truerise-core` stays port-only and `truerise-sandbox` stays
//! executor-only. The loop sees traits; this module owns the real objects.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use truerise_core::{ActionPolicy, GateDecision, ToolRequest};
use truerise_governance::{ApprovalQueue, Decision, PolicyState, PolicyTable, RequestState};
use truerise_hats::Hat;
use truerise_sandbox::{InProcessProvider, SandboxToolProvider};

/// The read_file tool name (the only Auto-scoped tool in run defaults).
pub const READ_FILE: &str = "read_file";
/// The write_file tool name (Ask by default: unapproved writes are audited,
/// never executed - fail-closed).
pub const WRITE_FILE: &str = "write_file";

/// The governance-backed `ActionPolicy`: the three-state table plus the
/// approval queue (deny short-circuits; ask never expires into consent).
pub struct GovernedPolicy {
    table: Mutex<PolicyTable>,
    queue: Mutex<ApprovalQueue>,
}

impl GovernedPolicy {
    /// The run defaults: `read_file` Auto, `write_file` Ask, every other
    /// tool default-deny (an unlisted tool is an unknown tool).
    #[must_use]
    pub fn run_defaults() -> Self {
        Self::with_rules(&[
            (READ_FILE, PolicyState::Auto),
            (WRITE_FILE, PolicyState::Ask),
        ])
    }

    /// Build from explicit scope->state rules (unknown scopes stay deny).
    #[must_use]
    pub fn with_rules(rules: &[(&str, PolicyState)]) -> Self {
        let mut table = PolicyTable::new();
        for (scope, state) in rules {
            table.set(scope, *state);
        }
        Self {
            table: Mutex::new(table),
            queue: Mutex::new(ApprovalQueue::new()),
        }
    }

    /// Record one approval ON FILE for a scope (the ONLY transition into
    /// ask-allowed; never implied by time).
    pub fn approve(&self, scope: &str) {
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        if queue.state(scope).is_none() {
            // Review fix (batch 3b follow-up): request() can now refuse
            // (duplicate id); an unrecorded approval request must not pass
            // silently - the refusal is surfaced (best-effort print, the
            // scope stays unapproved either way).
            if let Err(e) = queue.request(scope) {
                eprintln!("approval request refused: {e}");
                return;
            }
        }
        let _ = queue.approve(scope);
    }
}

impl ActionPolicy for GovernedPolicy {
    fn decide(&self, request: &ToolRequest) -> GateDecision {
        let on_file = {
            let queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            queue.state(&request.tool) == Some(RequestState::Approved)
        };
        let mut table = self.table.lock().unwrap_or_else(|p| p.into_inner());
        match table.evaluate(&request.tool, on_file) {
            Decision::Allow => GateDecision::Allow,
            Decision::NeedsApproval => GateDecision::NeedsApproval,
            Decision::Deny => GateDecision::Deny,
        }
    }
}

/// Owned tool assembly for one run directory: the governed policy plus the
/// fenced executor (fence = `<chain_dir>/sandbox/run`).
pub struct RunTooling {
    /// The policy the run's tool requests are judged by.
    pub policy: GovernedPolicy,
    /// The fenced executor the allowed requests run through.
    pub provider: SandboxToolProvider,
}

impl RunTooling {
    /// Assemble over ``chain_dir`` (created on demand).
    ///
    /// # Errors
    /// String-typed at the CLI boundary: fence creation or sandbox
    /// handle creation refused (fail-closed - no executor, no executions).
    pub fn new(chain_dir: &Path) -> Result<Self, String> {
        let fence = chain_dir.join("sandbox").join("run");
        std::fs::create_dir_all(&fence).map_err(|e| e.to_string())?;
        let provider = SandboxToolProvider::new(InProcessProvider::new(&fence), &fence)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            policy: GovernedPolicy::run_defaults(),
            provider,
        })
    }

    /// Where a sandboxed write lands relative to ``chain_dir`` (tests and
    /// demos assert on this; the fence layout is assembly-owned).
    #[must_use]
    pub fn sandbox_path(chain_dir: &Path, rel: &str) -> PathBuf {
        chain_dir.join("sandbox").join("run").join(rel)
    }
}

/// The tools this workspace's executor actually knows (load-time registry:
/// a hat naming anything outside this set is REFUSED at load, not silently
/// ignored at run time - W5).
pub const KNOWN_TOOLS: &[&str] = &["read_file", "write_file"];

/// Validate a hat's tool_subset against the known-tool registry.
///
/// # Errors
/// String-typed at the CLI boundary: the subset names an unknown tool.
pub fn validate_hat_tools(hat: &Hat) -> Result<(), String> {
    for tool in &hat.tool_subset {
        if !KNOWN_TOOLS.contains(&tool.as_str()) {
            return Err(format!(
                "unknown tool in hat {}: {tool} (known: {KNOWN_TOOLS:?})",
                hat.name
            ));
        }
    }
    Ok(())
}

/// The hat's tool_subset as the OUTER policy gate (W5): anything outside the
/// subset is denied before the three-state table is even consulted; inside
/// the subset, the inner policy decides as usual.
pub struct StackedPolicy<'a> {
    /// The run's hat (data, not a trait).
    pub hat: Hat,
    /// The inner policy (the governance table + approvals).
    pub inner: &'a dyn ActionPolicy,
}

impl ActionPolicy for StackedPolicy<'_> {
    fn decide(&self, request: &ToolRequest) -> GateDecision {
        if !self.hat.tool_subset.iter().any(|t| t == &request.tool) {
            return GateDecision::Deny;
        }
        self.inner.decide(request)
    }
}
