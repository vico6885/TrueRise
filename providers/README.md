# providers

[English](README.md) | [简体中文](README.zh-CN.md)

Third-party providers live in **their own repositories**. This directory hosts
only the port traits and the conformance suites that judge implementations:

| Port | Defined in | Judged by |
|---|---|---|
| `SandboxProvider` | `crates/truerise-sandbox/src/provider.rs` | three must-fail regions (escape / oversell / residue) + `truerise-eval` verdict |
| `ModelRouter` | `crates/truerise-core/src/router.rs` | round pinning: a response from a non-pinned model voids the round (`foreign_model_voids_the_round`) |
| `MemoryBackend` | `crates/truerise-memory/src/lib.rs` | fusion weights, determinism and tie-break tests |
| `ToolProvider` + `ActionPolicy` | `crates/truerise-core/src/tool.rs` (RFC 0002) | `crates/truerise-cli/tests/tool_gate.rs`: deny = audited no-op; ask = approval on file; capability = executor's value; fence escape = audited denial |
| `ResourcePolicy` | `crates/truerise-governance/src/resource_policy.rs` | three gates in order; unknown policy refused |

> **The fourth port landed via RFC 0002** (`docs/rfc/0002-tool-execution-port.md`).
> `ToolProvider` is defined in `truerise-core`, implemented by the fenced
> `SandboxToolProvider` (`truerise-sandbox`), and assembled at the boundary
> (`crates/truerise-cli/src/assembly.rs`) behind the `ActionPolicy`
> three-state judgment. A run without wired hooks executes nothing
> (fail-closed).

To add a provider, follow [`../docs/provider_conformance.md`](../docs/provider_conformance.md):
implement the port, run `self_check`, publish the report, open an issue.

A provider may never declare itself conformant — self-check produces facts,
only `truerise-eval` produces verdicts.
