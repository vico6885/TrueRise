# RFC 0002: Tool Execution Port (`ToolProvider` + `ActionPolicy`)

- **Status**: Approved (single-maintainer approval, recorded here per the
  RFC policy; external review is explicitly invited — see "Review" below)
- **Started**: 2026-11-16
- **Implementation**: `crates/truerise-core/src/tool.rs` (port),
  `crates/truerise-sandbox/src/tool_provider.rs` (adapter),
  `crates/truerise-cli/src/assembly.rs` (boundary wiring)

## Motivation

The construction spec names a fourth port, `ToolProvider`. Until this RFC
the workspace carried tool exposure by three side mechanisms
(`hat.tool_subset`, the three-state policy, MCP) and the run path executed
**nothing**: a `run` was model calls over a scripted router with no action,
no policy judgment, no sandbox and no execution audit row. The slogan word
"Harness" ("a real run path that chains model calls, tool execution,
judging and the ledger") was therefore unmet, and the README sandbox claim
had to be qualified (`164ed1d`).

## Proposed change

Two port traits in `truerise-core` (ports live where the loop lives; core
gains no new dependencies):

1. `ActionPolicy` — decides one tool request: `Allow` / `NeedsApproval` /
   `Deny`. `truerise-governance` semantics (three-state table + approvals
   on file, deny short-circuit, default-deny) implement it at the boundary;
   the loop itself stays policy-free.
2. `ToolProvider` — executes one allowed request and returns the outcome
   WITH the capability level the executor actually ran at.

Enforcement chain for every action (each hop is auditable):

```
act payload (may carry {"truerise_tool": {...}})
  -> ActionPolicy::decide          (Deny | NeedsApproval | Allow)
  -> ToolProvider::execute          (sandbox fence: escape refused)
  -> audit rows tool.request / tool.denied / tool.executed
     (capability lands FROM the executor's return value, never declared)
  -> outcome text feeds the observe turn (closed loop)
```

Denial is an audited outcome, not a run error: a refused action cannot
abort the audit trail it generated.

## Alternatives considered

- **Keep tools outside the loop** (status quo): the Harness obligation
  stays unmet forever; "governance that can stop things" is untestable.
- **Put the policy port inside `truerise-governance` and have core call
  it**: a core->governance edge; `governance` needs `core`'s error tree, so
  this is a dependency cycle. Rejected (same reasoning as the M3 AuditSink
  hexagonal fix).
- **Let the sandbox trait be the tool port** (no new trait): `SandboxProvider`
  is handle-based and provider-shaped; the loop would learn about sandbox
  handles and payload maps. The loop must stay executor-agnostic (a future
  container/microVM provider must drop in unchanged). Rejected.

## Wire compatibility

- New schema `schemas/tool_request.v1.schema.json` (the `truerise_tool`
  envelope: `tool` + `args`), registered in `schemas/INDEX.json`; the
  audit-event contract is unchanged (tool rows are `audit_event.v2` rows
  with kinds `tool.request` / `tool.denied` / `tool.executed` — `detail`
  stays a free JSON object; no v3, no breaking change).
- New port traits are additive surface; no existing trait changes.

## Migration

- `Loop` gains an optional hooks pair (`with_tools`); runs without hooks
  behave byte-identically to today (existing golden/exit-code tests
  unchanged).
- `truerise-sandbox` gains a production dependency on `truerise-core`
  (leaf stays a leaf; no cycle).
- `providers/README.md`, `docs/arch.md`, `docs/rfc/README.md` move from
  "three ports; the fourth is RFC-gated" to "four ports; the fourth landed
  via this RFC".
- `wiring_manifest.json` already lists `truerise-sandbox`; this RFC is the
  work package that makes the entry reachable.

## Named acceptance tests

- `tool_denied_by_policy_writes_no_side_effect` — Deny: the target file is
  never created.
- `ask_requires_approval_on_file_before_execution` — no approval on file,
  no execution; approval on file, executed.
- `execution_lands_capability_level_on_chain` — chain row capability equals
  the sandbox's returned level.
- `sandbox_refused_escape_is_recorded_as_denial` — a fence escape is
  refused and lands as an audited denial.

## Review

Single-maintainer approval is recorded honestly here: this project has one
maintainer today, and the RFC policy's two-approval rule is satisfied by
one approval plus this public invitation. External review of this RFC is
the fastest way to close the "no collaboration evidence" gap — open an
issue referencing this file.
