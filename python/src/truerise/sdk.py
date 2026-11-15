"""The truerise Python SDK (W13a): the harness contracts as an importable,
zero-dependency Python surface. The Rust crates remain the judging
authority and the wire authority; the SDK is a CONSUMER of the same
contracts - every semantic here mirrors a Rust behaviour and is cross-
checked by the same test names where a Rust named test exists.

Surface:
    Session         the governed run loop (three books: transcript,
                    transitions, audit rows) over ANY callable router
    Policy          three-state table (auto/ask/deny; ask needs an approval
                    ON FILE; deny short-circuits; default-deny)
    ApprovalQueue   single-point epoch store + approvals (never expire into
                    consent; revoke is terminal)
    Budget          the shared-denominator ledger (per-hat quota; the global
                    count is THE denominator - never sharded)
    Ratchet         tighten-only baseline (regression = exit-grade)
    fingerprint / seal_thresholds / prereg_diff / evidence_gate
                    re-exported from their single sources
"""
from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field

from .evals.pack import FAMILIES  # re-export: the sealed taxonomy
from .fingerprint import Fingerprint
from .gates import EvidenceRecord, evidence_gate, prereg_diff, seal_thresholds

try:  # W13b: the PyO3 backend is OPTIONAL - the pure-Python path is complete
    import truerise_native as _native  # type: ignore[import-not-found]
except ImportError:
    _native = None  # type: ignore[assignment]

__all__ = [
    "native_available",
    "Session",
    "Policy",
    "ApprovalQueue",
    "Budget",
    "Ratchet",
    "Fingerprint",
    "EvidenceRecord",
    "evidence_gate",
    "prereg_diff",
    "seal_thresholds",
    "FAMILIES",
]


def native_available() -> bool:
    """True when the PyO3 backend (`truerise-native` wheel) is importable.
    The SDK is FUNCTIONAL without it; the native backend accelerates chain
    and ratchet operations with the SAME crate semantics."""
    return _native is not None


# -- policy ----------------------------------------------------------------
@dataclass
class Policy:
    """Three-state table with scope caching; deny short-circuits BEFORE any
    cache lookup; an unlisted scope is default-deny."""

    states: dict[str, str] = field(default_factory=dict)
    _cache: dict[str, str] = field(default_factory=dict, repr=False)

    def set(self, scope: str, state: str) -> None:
        if state not in ("auto", "ask", "deny"):
            raise ValueError(f"unknown policy state: {state!r}")
        self.states[scope] = state
        self._cache.pop(scope, None)

    def decide(self, scope: str, approval_on_file: bool) -> str:
        state = self.states.get(scope)
        if state == "deny":
            return "deny"
        cached = self._cache.get(scope)
        if cached:
            return cached
        if state == "auto":
            self._cache[scope] = "allow"
            return "allow"
        if state == "ask":
            decision = "allow" if approval_on_file else "needs_approval"
            if decision == "allow":
                self._cache[scope] = decision
            return decision
        return "deny"  # default-deny


# -- approvals ---------------------------------------------------------------
@dataclass
class ApprovalQueue:
    """Fenced approval queue: the epoch is issued HERE (single point);
    approvals never expire into consent; revoke is terminal."""

    current_epoch: int = 0
    requests: dict[str, str] = field(default_factory=dict)  # id -> state

    def next_epoch(self) -> int:
        self.current_epoch += 1
        return self.current_epoch

    def fenced_write(self, epoch: int, payload: str) -> str:
        if epoch != self.current_epoch:
            raise ValueError(f"stale epoch: {epoch} != current {self.current_epoch}")
        return f"stored:{payload}"

    def request(self, request_id: str) -> None:
        self.requests[request_id] = "pending"

    def approve(self, request_id: str) -> None:
        state = self.requests.get(request_id)
        if state == "revoked":
            raise ValueError(f"request {request_id} was revoked")
        if state is None:
            raise ValueError(f"unknown request {request_id}")
        self.requests[request_id] = "approved"

    def revoke(self, request_id: str) -> None:
        if request_id in self.requests:
            self.requests[request_id] = "revoked"

    def state(self, request_id: str) -> str | None:
        return self.requests.get(request_id)


# -- budget ------------------------------------------------------------------
@dataclass
class Budget:
    """The shared-denominator ledger (singleton semantics by identity: pass
    the SAME instance to every consumer). Per-hat quota; the global count is
    THE denominator - sequential correction divides by it, never by shards."""

    per_hat: dict[str, int] = field(default_factory=dict)
    global_count: int = 0

    def consume(self, hat: str, quota: int | None = None) -> int:
        used = self.per_hat.get(hat, 0)
        if quota is not None and used >= quota:
            raise ValueError(f"budget: hat {hat} exceeded quota {quota} (used {used})")
        used += 1
        self.per_hat[hat] = used
        self.global_count += 1
        return used

    def reconcile(self) -> int:
        return sum(self.per_hat.values()) - self.global_count


# -- ratchet -----------------------------------------------------------------
@dataclass
class Ratchet:
    """A baseline that only moves DOWN (tighten-only). Metrics are integer
    maxima (the float rule: per-mille / counts, never floats)."""

    baseline: dict[str, int] = field(default_factory=dict)

    def propose(self, candidate: dict[str, int]) -> None:
        for metric, new_value in candidate.items():
            old = self.baseline.get(metric)
            if old is not None and new_value > old:
                raise ValueError(f"ratchet: {metric} loosened ({old} -> {new_value})")
        self.baseline.update(candidate)

    def verify(self, scores: dict[str, int]) -> None:
        for metric, allowed in self.baseline.items():
            value = scores.get(metric)
            if value is not None and value > allowed:
                raise ValueError(
                    f"ratchet: {metric} regressed ({value} > allowed {allowed})"
                )


# -- session -----------------------------------------------------------------
Router = Callable[[str, str], str]
"""route(tag, payload) -> response payload (the model step; pin it yourself)."""


@dataclass
class Session:
    """The governed run loop: one session, three books, the SAME tool/policy
    contract as the Rust loop (RFC 0002): a request goes decide -> execute ->
    audit; a denial is an audited outcome; a run without tools executes
    nothing."""

    ledger: object  # truerise.ledger.Ledger (hash-chained audit book)
    router: Router
    pinned_model: str
    policy: Policy | None = None
    budget: Budget | None = None
    budget_quota: int | None = None
    tools: dict[str, Callable[[dict], str]] | None = None
    approvals: ApprovalQueue | None = None
    transcript: list[dict] = field(default_factory=list)
    transitions: list[dict] = field(default_factory=list)

    def _audit(self, kind: str, severity: str, detail: dict) -> None:
        self.ledger.append(kind, detail, severity=severity)

    def run(self, goal: str, rounds: int = 1) -> dict:
        """One governed run. Returns the three-books summary."""
        if rounds < 1:
            raise ValueError("rounds must be >= 1")
        self._audit("loop.transition", "info", {"from": "pending", "event": "run_start", "to": "running"})
        self.transitions.append({"from": "pending", "event": "run_start", "to": "running"})
        self.transcript.append({"role": "user", "text": goal})
        for round_no in range(rounds):
            if self.budget is not None:
                try:
                    self.budget.consume("session", self.budget_quota)
                except ValueError as exc:
                    self._audit("budget.exhausted", "warning", {"reason": str(exc), "round": round_no})
                    return self._summary(stopped="budget")
            plan = self.router("plan", f"{goal} [round {round_no}]")
            self._audit("loop.transition", "info", {"from": "running", "event": "plan", "to": "planned"})
            self.transitions.append({"from": "running", "event": "plan", "to": "planned"})
            self.transcript.append({"role": "model", "text": plan})
            act = self.router("act", plan)
            self._audit("loop.transition", "info", {"from": "planned", "event": "act", "to": "acting"})
            self.transitions.append({"from": "planned", "event": "act", "to": "acting"})
            observation = act
            tool_call = self._parse_tool(act)
            if tool_call is not None and self.tools is not None and self.policy is not None:
                observation = self._execute_tool(tool_call)
            observed = self.router("observe", observation)
            self._audit("loop.transition", "info", {"from": "acting", "event": "observe", "to": "observing"})
            self.transitions.append({"from": "acting", "event": "observe", "to": "observing"})
            self.transcript.append({"role": "model", "text": observed})
        self._audit("loop.transition", "info", {"from": "observing", "event": "verdict_issued", "to": "verdict_ready"})
        self.transitions.append({"from": "observing", "event": "verdict_issued", "to": "verdict_ready"})
        return self._summary()

    def _summary(self, stopped: str | None = None) -> dict:
        return {
            "transcript": len(self.transcript),
            "transitions": len(self.transitions),
            "audit_rows": len(self.ledger.rows),
            "stopped": stopped,
        }

    @staticmethod
    def _parse_tool(payload: str) -> dict | None:
        try:
            doc = json_loads(payload)
        except ValueError:
            return None
        inner = doc.get("truerise_tool") if isinstance(doc, dict) else None
        if not isinstance(inner, dict) or not isinstance(inner.get("args"), dict):
            return None
        return {"tool": inner.get("tool"), "args": inner["args"]}

    def _execute_tool(self, call: dict) -> str:
        tool = call["tool"]
        self._audit("tool.request", "info", {"tool": tool})
        approval_on_file = (
            self.approvals is not None and self.approvals.state(tool) == "approved"
        )
        decision = self.policy.decide(tool, approval_on_file) if self.policy else "deny"
        if decision == "deny":
            self._audit("tool.denied", "warning", {"tool": tool, "reason": "policy_deny"})
            return f"tool {tool} denied by policy"
        if decision == "needs_approval":
            self._audit("tool.denied", "warning", {"tool": tool, "reason": "approval_required"})
            return f"tool {tool} requires approval; not executed"
        handler = self.tools.get(tool)
        if handler is None:
            self._audit("tool.denied", "warning", {"tool": tool, "reason": "unknown_tool"})
            return f"tool {tool} is unknown to this session"
        self._audit("tool.executed", "info", {"tool": tool, "code": 0})
        return handler(call["args"])


def json_loads(payload: str) -> object:
    import json

    return json.loads(payload)
