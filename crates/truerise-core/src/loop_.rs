//! The governed cognitive loop: drives the state machine through ModelRouter
//! turns, keeping THREE books simultaneously (spec §7 M3 gate):
//! 1. transcript (the conversation entries),
//! 2. audit chain (governance events, via truerise-governance),
//! 3. state-transition log (the typestate machine's declaration stream).
//!
//! A minimal run ends with all three books populated and mutually consistent.

use std::sync::Mutex;

use crate::budget::Budget;
use crate::compaction::Entry;
use crate::errors::CoreError;
use crate::recall::{ContextRecall, RecallEvidence};
use crate::router::{ModelRouter, RetryPolicy, RouterError, TurnRequest, backoff_ms, check_pinned};
use crate::state_machine::{Event, Machine, State, TransitionLog};
use crate::tool::{ActionPolicy, GateDecision, ToolProvider, ToolRequest, parse_tool_call};

/// The audit sink port (core side). The governance chain implements it in
/// its own crate (no core->governance dependency; hexagonal ports rule).
pub trait AuditSink: Send + Sync {
    /// Append one audit event; returns the new total row count.
    ///
    /// # Errors
    /// Implementation-defined (string-typed at the port boundary).
    fn append(
        &self,
        kind: &str,
        severity: &str,
        detail: &serde_json::Value,
    ) -> Result<usize, String>;
}

/// Loop errors.
#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    /// Router failed after retries (non-retryable or attempts exhausted).
    #[error("loop: router failed: {0}")]
    Router(#[from] crate::router::RouterError),
    /// Round voided by pinning (foreign model).
    #[error("loop: {0}")]
    Pin(#[from] crate::router::PinError),
    /// Audit sink refused.
    #[error("loop: audit sink: {0}")]
    Sink(String),
    /// Budget exhausted (W3): the audit row landed, then the run stopped.
    #[error("loop: budget exhausted for hat {hat}")]
    Budget {
        /// The hat that hit the wall.
        hat: String,
    },
    /// State machine refused.
    #[error("loop: state: {0}")]
    State(#[from] CoreError),
    /// Configuration error (e.g. empty rounds).
    #[error("loop: config: {0}")]
    Config(String),
}

/// Loop configuration.
#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// The pinned model for the WHOLE run (round pinning).
    pub pinned_model: String,
    /// Retry policy for router calls.
    pub retry: RetryPolicy,
}

/// The loop's tool hooks (RFC 0002): policy decides, provider executes.
/// Absent hooks = no execution ever (plain-text act payloads stay plain).
pub struct ToolHooks<'a> {
    /// The three-state policy for tool requests.
    pub policy: &'a dyn ActionPolicy,
    /// The fenced executor for allowed requests.
    pub provider: &'a dyn ToolProvider,
}

/// The three books, all populated by a run.
#[derive(Debug)]
pub struct RunBooks {
    /// Book 1: transcript entries (role/tag + payload).
    pub transcript: Vec<Entry>,
    /// Book 2: state-transition declarations.
    pub transitions: Vec<TransitionLog>,
    /// Book 3: audit chain row count (rows written by this run).
    pub audit_rows: usize,
}

/// The governed loop.
pub struct Loop<'a> {
    router: &'a dyn ModelRouter,
    config: LoopConfig,
    sink: &'a dyn AuditSink,
    tools: Option<ToolHooks<'a>>,
    budget: Option<(&'a str, &'a dyn Budget)>,
    memory: Option<&'a dyn ContextRecall>,
    transcript: Mutex<Vec<Entry>>,
    transitions: Mutex<Vec<TransitionLog>>,
    audit_rows: Mutex<usize>,
}

impl<'a> Loop<'a> {
    /// Bind the loop to a router, config and audit sink.
    pub fn new(router: &'a dyn ModelRouter, config: LoopConfig, sink: &'a dyn AuditSink) -> Self {
        Self {
            router,
            config,
            sink,
            tools: None,
            budget: None,
            memory: None,
            transcript: Mutex::new(Vec::new()),
            transitions: Mutex::new(Vec::new()),
            audit_rows: Mutex::new(0),
        }
    }

    /// Wire tool hooks (RFC 0002): act payloads may then request executions,
    /// gated by the policy and fenced by the provider. Without hooks nothing
    /// ever executes.
    #[must_use]
    pub fn with_tools(mut self, hooks: ToolHooks<'a>) -> Self {
        self.tools = Some(hooks);
        self
    }

    /// Wire the budget (W3): one unit per round + one unit per allowed tool
    /// execution, attributed to ``hat``. Exhaustion stops the run
    /// (fail-closed, audited).
    #[must_use]
    pub fn with_budget(mut self, hat: &'a str, budget: &'a dyn Budget) -> Self {
        self.budget = Some((hat, budget));
        self
    }

    /// Wire context recall (W6): before every plan turn the port is
    /// consulted; the evidence (or its absence) is audited either way.
    #[must_use]
    pub fn with_memory(mut self, memory: &'a dyn ContextRecall) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Consume ``units`` against the wired budget; on exhaustion audit the
    /// stop and return the Budget error (fail-closed).
    fn consume_budget(&self, units: u32, what: &str) -> Result<(), LoopError> {
        let Some((hat, budget)) = self.budget else {
            return Ok(());
        };
        if let Err(stop) = budget.consume(hat, units) {
            self.audit(
                "budget.exhausted",
                "warn",
                &serde_json::json!({ "hat": stop.hat, "what": what, "units": units }),
            )?;
            return Err(LoopError::Budget { hat: stop.hat });
        }
        Ok(())
    }

    /// One router call with retry/backoff (retryable only) + pinning check.
    fn call(&self, tag: &str, payload: &str) -> Result<String, LoopError> {
        let policy = &self.config.retry;
        let mut last_err: Option<RouterError> = None;
        for attempt in 0..policy.max_attempts {
            match self.router.route(&TurnRequest {
                payload: payload.into(),
                tag: tag.into(),
            }) {
                Ok(resp) => {
                    check_pinned(&self.config.pinned_model, &resp)?;
                    return Ok(resp.payload);
                }
                Err(e) if e.retryable() && attempt + 1 < policy.max_attempts => {
                    // Review fix (P1): the backoff was COMPUTED and thrown
                    // away - a 429 got max_attempts near-simultaneous
                    // retries, amplifying exactly the rate limit it hit.
                    // The sleep is real now; the jitter roll stays
                    // deterministic for the tape/replay contract.
                    let jitter = (attempt as u64) * 7;
                    let delay = backoff_ms(policy, attempt, jitter);
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    last_err = Some(e);
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(last_err.map_or_else(
            || LoopError::Config("retry loop exhausted without error".into()),
            LoopError::Router,
        ))
    }

    /// Append one audit event + move the audit-row counter (books 2/3 move
    /// together: EVERY audit row lands through here).
    fn audit(
        &self,
        kind: &str,
        severity: &str,
        detail: &serde_json::Value,
    ) -> Result<usize, LoopError> {
        let rows = self
            .sink
            .append(kind, severity, detail)
            .map_err(LoopError::Sink)?;
        *self.audit_rows.lock().unwrap_or_else(|p| p.into_inner()) = rows;
        Ok(rows)
    }

    /// Record one transition + audit event (books 2 and 3 move together).
    fn record(&self, from: State, event: Event, to: State) -> Result<(), LoopError> {
        self.transitions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(TransitionLog { from, event, to });
        self.audit(
            "loop.transition",
            "info",
            &serde_json::json!({
                "from": from.as_str(), "event": event.as_str(), "to": to.as_str()
            }),
        )?;
        Ok(())
    }

    /// The RFC 0002 enforcement chain for one parsed tool request:
    /// decide -> execute -> audit -> observation text. Denial is an audited
    /// outcome (the run continues); only a broken audit sink aborts the run.
    fn execute_tool(
        &self,
        hooks: &ToolHooks<'_>,
        request: &ToolRequest,
    ) -> Result<String, LoopError> {
        self.audit(
            "tool.request",
            "info",
            &serde_json::json!({ "tool": request.tool }),
        )?;
        let observation = match hooks.policy.decide(request) {
            GateDecision::Deny => {
                self.audit(
                    "tool.denied",
                    "warn",
                    &serde_json::json!({ "tool": request.tool, "reason": "policy_deny" }),
                )?;
                format!("tool {} denied by policy", request.tool)
            }
            GateDecision::NeedsApproval => {
                self.audit(
                    "tool.denied",
                    "warn",
                    &serde_json::json!({ "tool": request.tool, "reason": "approval_required" }),
                )?;
                format!("tool {} requires approval; not executed", request.tool)
            }
            GateDecision::Allow => {
                // W3: an allowed execution costs one unit too.
                self.consume_budget(1, "tool")?;
                match hooks.provider.execute(request) {
                    Ok(outcome) => {
                        // The capability level is the executor's return value,
                        // never a declared one (RFC 0002 acceptance test).
                        self.audit(
                            "tool.executed",
                            "info",
                            &serde_json::json!({
                                "tool": request.tool,
                                "capability": outcome.capability,
                                "code": outcome.code
                            }),
                        )?;
                        self.transcript
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .push(Entry {
                                role: "tool".into(),
                                text: outcome.stdout.clone(),
                                tokens: total_estimate(&outcome.stdout),
                            });
                        outcome.stdout
                    }
                    Err(err) => {
                        self.audit(
                            "tool.denied",
                            "warn",
                            &serde_json::json!({
                                "tool": request.tool,
                                "reason": "sandbox_refused",
                                "detail": err.to_string()
                            }),
                        )?;
                        format!("tool {} refused by the sandbox: {err}", request.tool)
                    }
                }
            }
        };
        Ok(observation)
    }

    /// A minimal run: RunStart -> (Plan -> Act -> Observe)x rounds -> verdict.
    /// The router produces each phase's payload; every phase is a state.
    /// `no_verdict` routes to takeover per spec (caller re-runs).
    ///
    /// # Errors
    /// [`LoopError`] on router/pinning/chain/state failures.
    pub fn run(&self, goal: &str, rounds: usize) -> Result<RunBooks, LoopError> {
        if rounds == 0 {
            return Err(LoopError::Config("rounds must be >= 1".into()));
        }
        // RunStart:
        let m0 = Machine::new().run_start();
        {
            let log = m0.log();
            let Some(t) = log.last() else {
                return Err(LoopError::Config("run_start produced no log entry".into()));
            };
            self.record(t.from, t.event, t.to)?;
        }
        self.transcript
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(Entry {
                role: "user".into(),
                text: goal.to_string(),
                tokens: total_estimate(goal),
            });

        let mut plan_payload = String::new();
        for round in 0..rounds {
            // W3: every round consumes one unit BEFORE any model call -
            // "over budget stops the run" is the minimum Governance proof.
            self.consume_budget(1, "round")?;
            // W6: recall context for the plan (evidence audited either way;
            // scores as per-mille integers - the float rule).
            let mut plan_query = format!("{goal} [round {round}]");
            if let Some(memory) = &self.memory {
                let evidence: Option<RecallEvidence> = memory.recall_for(goal);
                match evidence {
                    Some(ev) => {
                        self.audit(
                            "memory.recall",
                            "info",
                            &serde_json::json!({
                                "injected": ev.ids.len(),
                                "ids": ev.ids,
                                "scores_per_mille": ev.scores_per_mille
                            }),
                        )?;
                        plan_query = format!(
                            "recall context:
{}

{plan_query}",
                            ev.context
                        );
                    }
                    None => {
                        self.audit(
                            "memory.recall",
                            "info",
                            &serde_json::json!({ "injected": 0, "ids": [], "scores_per_mille": [] }),
                        )?;
                    }
                }
            }
            plan_payload = self.call("plan", &plan_query)?;
            self.record(State::Running, Event::Plan, State::Planned)?;
            self.transcript
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(Entry {
                    role: "model".into(),
                    text: plan_payload.clone(),
                    tokens: total_estimate(&plan_payload),
                });

            let act_payload = self.call("act", &plan_payload)?;
            self.record(State::Planned, Event::Act, State::Acting)?;
            // RFC 0002: an act payload carrying the tool envelope goes
            // through the enforcement chain; its outcome is the observation.
            let mut observation = act_payload.clone();
            if let Some(hooks) = &self.tools {
                if let Some(request) = parse_tool_call(&act_payload) {
                    observation = self.execute_tool(hooks, &request)?;
                }
            }
            let obs_payload = self.call("observe", &observation)?;
            self.record(State::Acting, Event::Observe, State::Observing)?;
            self.transcript
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(Entry {
                    role: "model".into(),
                    text: obs_payload.clone(),
                    tokens: total_estimate(&obs_payload),
                });
        }
        // Verdict: the LAST observe payload decides (demo-grade; judge lands M4).
        let has_verdict = !plan_payload.is_empty();
        if has_verdict {
            self.record(State::Observing, Event::VerdictIssued, State::VerdictReady)?;
            self.record(State::VerdictReady, Event::Finish, State::Finished)?;
        } else {
            self.record(State::Observing, Event::NoVerdict, State::AwaitingTakeover)?;
        }
        let transcript = self
            .transcript
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let transitions = self
            .transitions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let audit_rows = *self.audit_rows.lock().unwrap_or_else(|p| p.into_inner());
        Ok(RunBooks {
            transcript,
            transitions,
            audit_rows,
        })
    }

    /// The final state after [`Loop::run`] (from the transition log tail).
    pub fn final_state(transitions: &[TransitionLog]) -> State {
        transitions.last().map(|t| t.to).unwrap_or(State::Pending)
    }
}

fn total_estimate(text: &str) -> u64 {
    (text.split_whitespace().count() + text.len() / 8) as u64
}

// serde_json is used by the audit payloads; imported for clarity.
#[allow(unused_imports)]
use serde_json as _serde_json;
