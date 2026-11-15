# providers

[English](README.md) | [简体中文](README.zh-CN.md)

Third-party providers live in **their own repositories**. This directory hosts
only the port traits and the conformance suites that judge implementations:

| Port | Defined in | Judged by |
|---|---|---|
| `SandboxProvider` | `crates/truerise-sandbox/src/provider.rs` | three must-fail regions (escape / oversell / residue) + `truerise-eval` verdict |
| `ModelRouter` | `crates/truerise-core/src/router.rs` | round pinning: a response from a non-pinned model voids the round (`foreign_model_voids_the_round`) |
| `MemoryBackend` | `crates/truerise-memory/src/lib.rs` | fusion weights, determinism and tie-break tests |
| `ResourcePolicy` | `crates/truerise-governance/src/resource_policy.rs` | three gates in order; unknown policy refused |

> **Decided position on the fourth port.** The construction spec names a
> fourth port trait, `ToolProvider`. This workspace implements **three** port
> traits plus one policy port, and tool exposure is carried by three existing
> mechanisms instead: `hat.tool_subset` (data, `crates/truerise-hats`), the
> three-state policy (`Auto` / `Ask` / `Deny`, approval required on file) and
> `truerise-mcp` (zero-exposure server + write-class hard denial).
> The docs therefore state the implemented contract, not the spec's intent.
> Introducing a `ToolProvider` trait later is an RFC-gated change
> (`docs/rfc/README.md`) — it is not a documentation fix, it is new surface.

To add a provider, follow [`../docs/provider_conformance.md`](../docs/provider_conformance.md):
implement the port, run `self_check`, publish the report, open an issue.

A provider may never declare itself conformant — self-check produces facts,
only `truerise-eval` produces verdicts.
