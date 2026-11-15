# RFC 0003: Generation Lineage + the Approval-Gated Apply Path

- **Status**: Approved (single-maintainer approval, recorded honestly per
  the RFC policy; external review invited — see RFC 0002's review note)
- **Started**: 2026-11-16
- **Implementation**: `crates/truerise-rsi/src/apply.rs` (the ONLY apply
  path), `crates/truerise-cli/src/evolve_cmd.rs` (assembly)

## Motivation

The slogan's "Recursive Self-Improvement" obligation is: at least one
generation `gen0 -> gen1` runs end to end (propose -> human approval ->
apply -> re-evaluate -> ratchet -> rollback on regression), with the whole
cycle recomputable offline. Today the loop is BROKEN at "apply": the
workspace can propose (never auto-merge) but nothing can apply, and no
generation lineage exists (no parent hash, no diff, no cost, no verdict
pointer). The M4 spec gate `no_merge_path_exists` documented exactly this
gap and required an RFC for the change — this is that RFC.

## Non-negotiable boundaries (each one has a named test)

1. **No apply without a human approval token.** There is NO code path from
   proposal to applied change that does not require an approval. Not on a
   timer, not on a score, not "when confident".
2. **The approval token binds to the EXACT proposal bytes.** The token IS
   the sha256 of the child content; applying content whose hash differs
   from the approved hash is refused (defeats "approve A, apply B").
3. **The approval must be LIVE in the queue** (Approved, not Revoked, not
   unknown). Revoked proposals never apply — the existing approval-queue
   semantics (approve is the only transition into Approved; revoke is
   terminal) are reused, not re-implemented.
4. **The snapshot write is fenced.** Apply writes through the queue's
   epoch fence (`fenced_write`); a stale epoch is refused (optimistic
   concurrency, reusing the M2 semantics).
5. **The ratchet can always undo.** An applied generation whose re-eval
   regresses past the baseline is ROLLED BACK to the byte-exact snapshot,
   and the rollback reason lands on the chain.
6. **The verdict is honest in both directions.** The generation
   certificate must be able to express `FAIL + rolled back` — a completed
   closed loop that found a regression is a SUCCESS of the machinery, not
   a failure of the certificate.

## What is added

- `rsi::apply::apply_proposal(skill_path, child_content, approval_hash,
  observed_epoch, queue, chain, write)` — the single apply path. The file
  write itself is INJECTED by the boundary (the CLI wires the sandbox
  executor), so the apply path stays executor-agnostic and the write goes
  through the W2 policy + W3 budget enforcement.
- `rsi::apply::rollback(...)` — restore the byte-exact snapshot + chain
  the reason.
- The generation certificate: `{generation, parent{content_hash},
  child{content_hash}, proposal_hash, diff_stat, cost, verdict, rollback,
  replay}` — integers only (float rule). Committed under `readings/` by
  the demo; recomputable offline via `evolve --replay` (both skill
  contents are embedded in the certificate, so replay needs no files and
  no network).
- Chain rows: `evolve.proposal`, `evolve.approval`, `evolve.apply`,
  `evolve.rollback`, `evolve.certificate` (all `audit_event.v2` rows; no
  wire change).

## Alternatives considered

- **Auto-apply on score improvement** — violates boundary 1 and the
  project's own anti-cheat thesis. Rejected.
- **Apply in the CLI only (no rsi module)** — the apply semantics (token
  binding, fence, snapshot, rollback) are domain logic; leaving them in
  the IO shell makes them untestable as a unit and re-implementable. The
  rsi crate is the right home; the M4 gate comment anticipated an RFC.
- **Certificates without embedded contents** — replay would depend on
  mutable files or a network model call; "offline recompute" would be
  weaker than the word Verifiable. Rejected.

## Migration

- `evolve_propose` / `no_merge_path_exists` unchanged: propose still never
  merges; apply is a NEW, separate, approval-gated path (the gate's
  comment anticipated this RFC).
- `truerise-cli evolve [--demo] [--approve <hash>] [--replay --cert <p>]`
  is the only CLI surface; REPL/run paths are untouched.

## Named acceptance tests

- `evolve_gen0_to_gen1_produces_lineage_certificate`
- `apply_without_human_approval_token_is_refused`
- `approval_hash_mismatch_with_proposal_is_refused`
- `injected_regression_rolls_back_and_chains_the_reason`
- `generation_certificate_recomputes_identically_offline`
- `stale_or_revoked_epoch_write_rejected` (fence + revoked approval)
