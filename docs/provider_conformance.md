# Sandbox provider conformance

[English](provider_conformance.md) | [简体中文](provider_conformance.zh-CN.md)

Every code execution in TrueRise goes through a conformance-certified sandbox
provider. This page is how you add one — your own container, microVM, jail or
in-process fence — and how it gets judged.

Two rules up front, because they are the design, not policy text:

1. **Self-check produces facts; only `truerise-eval` produces verdicts.**
   A provider may never mark itself conformant.
2. **Providers live in their own repositories.** This repository hosts the
   port and the conformance suite and nothing else (see
   [`../providers/README.md`](../providers/README.md)).

## 1. The port

`crates/truerise-sandbox/src/provider.rs` — five methods, no optionals:

| Method | Contract |
|---|---|
| `create(id, workdir) -> SandboxHandle` | instantiate a fenced workspace |
| `execute(handle, action, payload) -> ExecutionOutput` | run one action; output carries the capability level **in force for this execution** |
| `destroy(handle)` | release resources; idempotent per handle |
| `metrics(handle) -> HashMap<String, u64>` | integer-unit counters |
| `audit_handle(handle) -> String` | the chain-row reference prefix for this sandbox |

Capability bits are declarative (`CapabilityBits`: snapshot / fork / pause /
resume), and the observed level has **three states** — all-declared-working,
degraded, or unusable. Degradation is never silent, and "unusable" refuses
execution rather than pretending to run.

## 2. The three must-fail regions

`self_check(root)` in `crates/truerise-sandbox/src/conformance.rs` probes
exactly three regions. A correct provider FAILS all three probes, meaning the
attack is refused:

| Region | Probe | Refusal required |
|---|---|---|
| ESCAPE (`conformance.rs:77`) | a path that leaves the fence | `SandboxError` refusal, not sanitisation into compliance |
| OVERSELL (`conformance.rs:106`) | an ability you did not declare | refused **as undeclared** |
| RESIDUE (`conformance.rs:119`) | filesystem state after `destroy` | nothing remains on disk |

The escape probe earned its keep during construction: the first run caught a
real escape in the shipped in-process provider, because `Path::starts_with`
is lexical and does not resolve `..`. The fence became component-level
(`ParentDir` / `RootDir` / `Prefix` refused). That is the point of a must-fail
probe — it tests your provider, not your README.

## 3. From facts to a verdict

```rust
use truerise_sandbox::self_check;
use truerise_eval::judge;

let report = self_check(&root);          // facts only
let verdict = judge(&report);            // PASS only if every region passed
```

Enforced in `cargo test -p truerise-eval`:

- `all_regions_pass_yields_pass`
- `any_region_fail_yields_fail`
- `verdict_is_recorded_and_reproducible` — a verdict that cannot be written to
  the audit chain is an **error**, not a warning (`judge_and_record`)

## 4. Checklist for a new provider

```bash
cargo test -p truerise-sandbox                                  # in-process reference
cargo test -p truerise-eval                                     # judging authority
python tools/lint_deny_isolation.py                             # license isolation
cargo cyclonedx --format json --spec-version 1.5 && python tools/check_sbom.py
#   (the SBOM step needs cargo-cyclonedx 0.5.10, version-pinned in CI)
```

Then, in your repository:

- [ ] implement `SandboxProvider` (all five methods; no `unwrap`/`expect` in
      production paths — the workspace denies those lints)
- [ ] declare capability bits truthfully; probe them and report the observed
      level instead of the declared one
- [ ] run `self_check` against your provider and publish the report
- [ ] pin dependencies to exact versions (`[bans] wildcards = "deny"` upstream)
- [ ] license Apache-2.0 or MIT — BSL/GPL/AGPL cannot enter the tree, and the
      CI asserts it (`tools/lint_deny_isolation.py`)
- [ ] add `unsafe` only with per-block `// SAFETY:` rationale, and expect the
      same bar as `truerise-platform`, the only crate where it is allowed

## 5. Getting listed

Open an issue from the `feature_request` template and attach:

1. the `self_check` report (all three regions),
2. your capability declarations plus the probed levels,
3. the crate path or repository URL,
4. the license of everything you link.

The maintainer records accepted providers in
`CHANGELOG.md` with the commit each verdict was measured at.
This process is deliberately thin while the project has one maintainer; the
conformance verdict — not the listing — is the thing that carries trust.

## 6. What is out of scope for providers

- Conformance verdicts inside your own crate (only `truerise-eval` judges).
- Model routing and memory backends have their own port traits
  (`ModelRouter` in `crates/truerise-core/src/router.rs`, `MemoryBackend` in
  `crates/truerise-memory/src/lib.rs`), and tool exposure is currently carried
  by `truerise-mcp` plus the three-state policy — see the open `ToolProvider`
  gap noted in [`../providers/README.md`](../providers/README.md). Changing any
  port trait requires an RFC (`docs/rfc/README.md`).
