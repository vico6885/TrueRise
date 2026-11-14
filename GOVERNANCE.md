# TrueRise Governance

## Neutrality

TrueRise is a general-purpose runtime foundation with no vertical industry
coupling. Industry-specific logic lives outside this repository, in separate
provider/plugin projects that consume the port traits.

## Decision making

- Port traits (ModelRouter / ToolProvider / SandboxProvider / MemoryBackend)
  change only through RFCs (`docs/rfc/`): motivation, alternatives, wire
  compatibility, migration. Two maintainer approvals required.
- The spec-mapping gate rules: a behaviour without a spec row and a named
  test does not exist.

## Admission

Contribution admission is non-discriminatory: gates are technical only
(CI gates, DCO, IP-clean declarations) and apply identically to all
contributors.

## Zero-exemption audit

The audit hash chain is the single authority for "what happened". No
component — including built-in providers and the tooling itself — may write
outside it, skip it, or edit it. Judgement authority rests with
truerise-eval; sandbox self-checks produce facts, never verdicts.
