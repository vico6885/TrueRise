# Roadmap

Status is EVIDENCE-BACKED, not aspirational: a milestone is "done" here only
when its named tests are green (spec_map rows / 324 identifiers are
machine-checked; the claim gate `tools/check_claim_terms.py` holds the
title to the same standard - 5/5 verifiable nouns currently met).

## Shipped

- 2026-11-21 — G29 persistent-state write gate (four paths, one door) + marks riding certificates; G9 trajectory stepwise signing (`write_gate.rs`, `trajectory.rs`)
- 2026-11-20 — CA1-CA5 causal identification substrate + quasi-experimental extensions + G10 process-level lucky-pass grading (`causal/*`, `process.rs`) (P0-P3, 2026-11-14 -> 2026-11-16)

- **P0 - the harness works**: tool execution through the sandbox port
  (RFC 0002), budget enforcement that STOPS runs (exit-grade), run verdicts
  from the judging authority with the ratchet on the run path, the wiring
  gate keeping islands a permanent CI red (11/11 crates reachable).
- **P1 - one RSI generation**: gen0 -> gen1 closed loop (RFC 0003) -
  propose (never merges) -> hash-bound human approval -> apply -> re-eval
  -> ratchet -> rollback on regression -> recomputable generation
  certificate (byte-deterministic, embedded contents, offline replay).
  MCP server runs as a process (zero exposure); memory recall + observe
  export on the run path; certificate signing identity (ed25519,
  fail-closed on unsigned/tampered); significance via exact integer
  McNemar (unequal-cost refused, discordant floor voids claims).
- **P2 - determinism + defensibility**: OpenAI-compatible provider adapter;
  the recording/replay TAPE (committed readings replay offline, no keys,
  no network); the canonical-JSON float rule enforced at the write gate in
  BOTH implementations; the determinism policy (docs/determinism.md) with
  its honest boundary. Long-horizon closed-loop task family (v0.2) and the
  external-solver protocol (executor-signed grades, self-report capped L0).
- **P3 - the ecosystem surface**: zero-dependency Python SDK (the contracts
  as a consumer); the PyO3 native backend (`truerise-native`, optional,
  parity-tested); MCP + SDK + external-protocol as THE integration surfaces
  (docs/integrations.md); OTel span projection behind a feature.

## Next (in order, each with its own acceptance tests)

1. **The first third-party model reading** (`readings/v0.3/`): one public
   model through the external-solver protocol + tape. The protocol is
   ready; the step is operational (an API key + a solver script honouring
   the contract). Until it lands, the bench remains a harness demo, not a
   leaderboard (README Limitations says so plainly).
2. **Level-3 same-family twins** (defect bench v0.3): direction
   discrimination, the judge-saturation failure mode's antidote.
3. **Signed readings at scale**: a release signing key ceremony (W10's
   `verify` is ready; today's committed artifacts honestly report
   unverified_issuer).
4. **~~Anthropic adapter~~** DONE (2026-11-16): both adapters behind the
   same `ModelRouter` port, selectable via `run --provider openai|anthropic
   --model <id> [--provider-url <url>]` (keys env-only; the provider's own
   model echo backs the pinning check; 429/5xx classified retryable).
   Streaming remains open when a user asks for it.
5. **First public tag**: crates.io + PyPI occupation (RELEASING.md A-H),
   cargo-dist five-platform artifacts, release-signed certificates.

## Non-goals (stable)

- No auto-merge / auto-apply. Anywhere. (RFC 0003 boundary 1.)
- No improvement-magnitude promises: the certificate says "verified not
  worse", significance says "defensible or voided".
- No per-framework shims; no second judging authority in any language.
