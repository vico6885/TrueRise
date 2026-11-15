# PROGRESS — wave-by-wave delivery record

Format per wave: ID | files | reproducible command | output (<= 10 lines) | HEAD+UTC | not-doing/risks. A wave counts as done only with its gate evidence recorded here. Not-doing entries carry a 2-week clock: past the deadline without passing the gate, the wave rolls back.

## M0 — repository scaffold (2026-11-14)

HEAD: `158c4bd` (2026-11-14T18:29:24+08:00) — the commit whose gate transcriptions this file's live-anchor rows cite (11 commits all DCO-signed; external-review rounds 8-11 verified against it). Branch `main`; commits after `158c4bd` are documentation-alignment commits whose live counts live in the evidence bundle (era-anchored rows below + bundle = live authority). Wave M0 scaffold commit: `4e58fa6` (2026-11-14T15:59:40+08:00) — a different event, not to be conflated.

Files: root `Cargo.toml` / `rust-toolchain.toml` / `deny.toml` / `Makefile` / `.gitignore` / `.gitattributes`; `crates/truerise-{core,wire,platform,governance,sandbox,eval,memory,rsi,hats,mcp,observe,cli}/` (skeletons, `publish = false`, doc-only `lib.rs`); `tools/` (9 gate scripts + `banned_words.hex`); `.github/workflows/ci.yml` + ISSUE_TEMPLATE; `schemas/` (`audit_event.v2.schema.json` + `INDEX.json`, spec-level alignment gate); `tests/timesafety/` (four-vector contract skeleton); `README` / `CONTRIBUTING` / `SECURITY` / `GOVERNANCE` / `CODE_OF_CONDUCT` / `CHANGELOG` / `LICENSE-APACHE` / `LICENSE-MIT`.

### Gate evidence

Every row carries the commit it was measured at (era anchor). Live values = the evidence-bundle `gates/` transcriptions at the current HEAD; where a row's number differs from the bundle, the bundle is the authority.

| Gate | Command | Result | Measured at |
|---|---|---|---|
| build | `cargo check --workspace --all-targets` | Finished dev profile, 12 crates, zero warnings | `4e58fa6`; re-verified fresh (uncached) at `133bdf0` bundle gates/00+08 |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Finished, zero warnings | `4e58fa6`; re-verified at `133bdf0` gates/10 |
| fmt | `cargo fmt --all -- --check` | exit 0 | `4e58fa6`; re-verified at `133bdf0` gates/11 |
| workspace door (= naming gate) | `python tools/lint_banned_words.py` | `0 hits across 58 files (exit 0)` | `4e58fa6`; live = `158c4bd` gates/01 (87 files, 0 hits) |
| history-object door | `python tools/lint_git_history.py` | `0 hits across 59 blobs + full ref log (exit 0)` | `4e58fa6`; live = `158c4bd` gates/02 (86 blobs, floor 70 intact) |
| file length | `python tools/lint_file_length.py` | `all 20 code files <= 600 lines (exit 0)` | `4e58fa6`; re-verified at `133bdf0` gates/03 |
| license isolation | `python tools/lint_deny_isolation.py` | `allow list clean, dual core licenses present (exit 0)` | `4e58fa6`; re-verified at `133bdf0` gates/04 |
| schemas | `python tools/lint_schemas.py` | `1 schema(s) aligned at spec level (fields/required/enums) (exit 0)` | `991c1b1` (gate made real); re-verified at `133bdf0` gates/05 |
| SBOM existence+count | `cargo cyclonedx --format json` + `python tools/check_sbom.py` | `SBOM gate: 12/12 member reports parsed with specVersion (exit 0)` — existence+count assertion (one report per member dir, parse failures listed, build residue excluded); format/version/identity compliance is M5 scope | `133bdf0` gates/14 |
| cargo deny | `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` (exit 0) | `133bdf0` gates/13; re-verified at `080f1da` gates/13 (one earlier run hit a transient advisory-db fetch failure, recorded here; final transcription green) |
| timesafety placeholder | `python tools/check_leakguard_skeleton.py` | `4 vectors declared planned (exit 0)` | `4e58fa6`; re-verified at `133bdf0` gates/06 |
| DCO | `python tools/lint_dco.py` | `all 1 commit(s) signed (exit 0)` | `4e58fa6`; live = `158c4bd` gates/07 (11/11 signed) |
| naming check | crates.io API + GitHub search | crates.io `truerise`: 404 = not taken; GitHub: one unrelated personal repo (`dev-abdullah-shahid/truerise`) | `4e58fa6` |

### Self-introduced decisions (not spec-mandated; recorded per the external review round-1 ruling, item Q3)

- `edition = "2024"` + `rust-version = "1.85"` (edition floor, fail-closed minimal). `resolver = "2"` IS spec-mandated. Resolver observation: explicit `resolver = "2"` with edition-2024 members produces no warning/conflict — valid for the dependency-free M0 state (weak evidence); re-verify when the first external dependency lands.
- lint `non_ascii_idents = "deny"` (naming-discipline enforcement).
- `[workspace.lints]` inheritance implements the per-crate `#![deny(missing_docs)]` requirement with a single source of truth.
- README positioning follows the construction spec (section "定位") verbatim, bilingual; the v3 tagline "A Runtime Governance Layer for Long-Horizon RSI" kept as a secondary line.
- `schemas/audit_event.v2.schema.json` (clean-room from the audit-chain behaviour spec): the three chain-critical keys (seq / prev_hash / hash) chosen as the required set; severity enum values are this repo's own design. Both flagged for confirmation when the wire crate freezes in M1.
- `tools/lint_banned_words.py` holds the plugin-crate path exceptions as a plaintext `EXEMPT_PATHS = []` (auditable, PR-reviewed) instead of an encoded switch.
- cargo-deny advisory/ban/source severities are `warn` (not deny) at M0: the dependency tree is empty, and hard-deny would make the gate red on the first added dependency for reasons unrelated to a review decision. Upgrade to deny is mandatory at the first external dependency (paired with the resolver re-test trigger).
- `actions/checkout@v4` is tag-pinned; SHA-pinning policy and crate-version drift review land at the first remote push (vico decision item). In-CI tool installs are version-pinned (`cargo-deny@0.20.2`, `cargo-cyclonedx@0.5.10`).
- In-repo prose writes "0.1-series" instead of the spec's literal release token (0.1-series followed by the hyphenated form of a banned token; identity is recorded in the encoded wordlist): the literal contains a banned token (word-boundary match) and would trip the leak gate. **TENSION FLAGGED for ruling** — the spec names the release form with that token while banning the token workspace-wide. (The gate caught this document's own first draft containing the literal; fixed via the approved amend+prune, recorded in the external governance task log.)

### Local toolchain note

rustup attempted to re-download toolchain name `1.100.0` (locally only `stable-x86_64-pc-windows-msvc` was installed — the identical 1.100.0 binaries) and hung on the network; TUNA mirror 404s this version. Local evidence runs therefore used `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc` (same compiler, versions verified 1.100.0). MSRV floor locally verified: toolchain `1.85.1` installed and `cargo +1.85 check --workspace --all-targets` passes (see evidence). CI installs the real pinned `1.100.0` from the toolchain file; the local run is reference evidence, the authoritative CI run lands with the first pushed remote. Upgrade flow: a dedicated PR bumping `rust-toolchain.toml` only.

### History-door incident record (M0 construction, pre-remediation)

The history door caught this document's own first draft: 4 hits (3 in commit text, 1 blob) — a banned-token literal (the release-token suffix; identity in the encoded wordlist) and two occurrences of the internal reviewer role name. Remediation executed under the deletion three-gate protocol with the maintainer's explicit approval: (a) `git commit --amend` rewriting the offending commit with literal-free text; (b) `git reflog expire --expire=now --all` + `git gc --prune=now` expelling the unreachable old commit object and blobs (blob ids `14515ddf`/`fb4dd85b`). Post-remediation re-scan: 0 hits across all objects. The pre-prune hit snapshot is retained ONLY in the external governance task log — in-repo retention would trip this very gate by design. The verbal no-prune commitment is now backed by a technical control: `tools/history_floor.json` (blob-count floor; any decrease fails the history door; floor bumps require the three-gate protocol in a reviewed commit).

### Not doing / risks (clock: first seen 2026-11-14; 2-week gate deadline 2026-11-28 unless stated)

1. [M1, due 2026-11-28] timesafety placeholder → real LeakGuard suite. Dual-stage wording: M0 = existence assertion only (contract skeleton declared), M1 = substantive property tests.
2. [M1, due 2026-11-28] `schemas/` gains the legacy v1 audit-event schema beside v2 (double-version rule explicit at the wire type level); the v2 required-key and enum choices above await wire-crate freeze.
3. [remote, no clock] **Authoritative CI run pending**: no remote configured — the CI chain (fmt/clippy/MSRV/deny/SBOM/DCO) executes for real at first push; remote creation is a vico decision. Interim: the full §8 chain is verified as a LOCAL command matrix with exit codes (see evidence bundle; cargo-deny / cargo-cyclonedx installed locally for this purpose).
4. [vico, no clock] crates.io name-squatting decision (dry-run / occupy `truerise-*`); `publish = false` protects nothing against third-party registration.
5. [vico, no clock] Trademark-database manual check (automated GitHub / crates.io check done, see evidence).
6. [vico, no clock] git identity email `vico@users.noreply.github.com` is a placeholder pending vico's GitHub account decision (DCO trailers carry it).
7. [M1, no clock] benchmark-compare gate lands with M1 (applies every wave from M1 per spec).
8. [M5, no clock] SBOM format upgrade: specVersion asserted == 1.5 and component-name identity per member; the M0 gate is existence+count only (never to be read as format compliance).
9. [trigger-based, no clock] resolver observation re-test is MANDATORY when the first external dependency lands (weak evidence in the dependency-free state); record the re-test result in this file.
10. [trigger-based, no clock] history floor (`tools/history_floor.json`): blob count must never decrease; the floor file carries an `authorization` field referencing the external governance task log (path recorded out-of-repo only; reviewers cross-check it directly — in-repo floor bumps are not self-authorizable); a missing floor file or empty authorization is exit 2 (fail-closed). Current floor 88 (re-baselined at `1a5d3bc`, authorized by the same external three-gate record). This is the technical control behind the no-prune commitment.
11. [vico, no clock] External-review probe channel: the review harness jail root is hardcoded to the parent repository; interim channel = the evidence-bundle directory on the parent side (path recorded in the external governance task log). Durable fix (vico decision): harness repo-root override (1-line change, separate dispatch) or formalized evidence-bundle convention. Authoring rule (learned twice this session): never write parent-side directory names or internal role names into this repository — they contain banned tokens; refer to them descriptively.
12. [supply-chain, due first remote push (vico decision), no clock] `actions/checkout@v4` is tag-pinned, not SHA-pinned; pin to full commit SHAs when the remote exists (third-party tags can be re-pointed upstream).
13. [vico, 2026-11-15 ruling recorded] crates.io `truerise*` name occupation: APPROVED (occupy). GitHub remote creation: APPROVED (org name check + noreply email). Both are account-side actions on vico's machine; remote item 3 above executes at that push.
14. [credibility, no clock] **External model run pending**: one public model through the external-solver protocol (subprocess + sandbox conformance), readings committed under `readings/`. Registered as the required credibility step before any third-party adoption claim (report §4).

---

## P0 — wiring + claim gates + tool execution port (W1/W12/W2, 2026-11-16)

HEAD at wave start: `5d38fb1` (in-flight W3 open-source wave landed as one DCO commit first). Copy stop-bleed landed as `164ed1d` (README/zh sandbox claim qualified; onboarding step 3 says planned) BEFORE any new surface.

| Item | Command | Result |
|---|---|---|
| copy stop-bleed | `git show 164ed1d --stat` | README.md + README.zh-CN.md sandbox sentence qualified ("every executed action must flow through the sandbox port; until tool wiring lands no code executes at all"); cli onboarding step 3 -> "(planned - see docs/PROGRESS.md)" |
| W1 wiring gate | `python tools/check_wiring.py` (fresh) | RED by design: exactly 7 islands (eval/rsi/hats/memory/mcp/observe/sandbox); `--selftest` proves the gate bites BOTH directions (roster==closure green; roster+truerise-eval red) |
| W1 CI + Makefile | `.github/workflows/ci.yml` job `wiring`; `make wiring`; `make verify` | selftest runs BEFORE the check in CI; spec_map row "接线" with 2 named identifiers |
| W12 claim gate | `python tools/check_claim_terms.py` | 0 met / 5 unmet (Harness/Governance/Verifiable/Recursive Self-Improvement/Deterministic) - title-vs-capability gap is now CI-visible; negative selftest: planting the full slogan in the README H1 turns the gate red (proven once, recorded here) |
| W12 registry | `claim_terms.json` | every term carries command + named test + artifact even while unmet ("任一为空即红" enforced by the gate) |
| RFC 0002 | `docs/rfc/0002-tool-execution-port.md` | tool execution port RFC: motivation/alternatives/wire-compat/migration + named tests; honest single-maintainer approval + open external-review invitation |
| W2 tool port | `cargo test -p truerise-core --lib` | `envelope_parses_only_the_exact_shape`: the `{"truerise_tool":...}` envelope is the ONLY executable shape; plain text never parses |
| W2 sandbox executor | `cargo test -p truerise-sandbox` | writes land inside the fence with capability `full` (executor's own value); escape refused with zero side effects; non-string args refused |
| W2 governed run | `cargo test -p truerise-cli --test tool_gate` | 5 passed: deny = audited no-op (`tool_denied_by_policy_writes_no_side_effect`), ask = approval on file (`ask_requires_approval_on_file_before_execution`), capability = sandbox return value on chain (`execution_lands_capability_level_on_chain`), fence escape = audited denial (`sandbox_refused_escape_is_recorded_as_denial`), no-hooks = execution-free (golden lock intact) |
| W2 wire surface | `python tools/lint_schemas.py` | 2 schemas aligned: `tool_request.v1.schema.json` registered in INDEX (additive; audit_event.v2 untouched) |
| docs alignment | providers/README{,.zh-CN}.md, docs/arch.md, docs/rfc/README.md | three-port wording -> four ports (ToolProvider + ActionPolicy landed via RFC 0002); the old "decided position" block replaced by the landing record |
| docs gate scope | `tools/check_docs.py` | judges TRACKED markdown only (git ls-files) - CI parity; untracked developer scratch no longer turns local gates red |
| workspace | `cargo test --workspace` | 111 passed 0 failed; clippy -D warnings clean; fmt clean |

| W3 budget port | `cargo test -p truerise-cli --test budget_enforcement` | 3 passed: `budget_exhaustion_halts_run_at_step_n` (quota 2 + 3 rounds -> round 3 never starts, `budget.exhausted` on chain, ledger stopped at 2), `budget_is_shared_across_hats_not_sharded` (two hats, one global denominator, reconcile == 0), `run_exit_code_reflects_budget_stop` (--budget 0 -> exit 1; --budget 100 -> exit 0) |
| W3 wiring | core `Budget` port (`with_budget`, consumption at round start + every allowed tool execution); rsi `RunBudget` adapter (rsi gains its first internal dependency: core, per the Q4 ruling); cli `--budget <n>` | over budget = `LoopError::Budget` + audited stop; a stop IS a failure (exit 1) |
| budget test serialisation | `BUDGET_GUARD` in the test file | the ledger singleton is process-global by design; consuming tests hold a guard so delta arithmetic is never raced (same protocol the rsi unit tests document) |

| W4 run verdict | `cargo test -p truerise-cli --test run_eval_gate` | 3 passed: `run_emits_verdict_row_from_eval_authority` (one eval.verdict row, PASS/failures 0, RunVerdict has NO public constructor - the authority is a type-system fact), `score_regression_fails_the_run_with_grade_exit` (empty plan -> takeover -> failures 1 > baseline 0 -> FAIL recorded + grade_exit 1), `unrecordable_verdict_is_error_not_warning` (tampered rows -> open refuses -> exit-grade, never a warning) |
| W4 eval unit | `cargo test -p truerise-eval --lib` | run_verdict: finished passes/unfinished fails/integer metric only (run_failures); baseline `baselines/run_scores.json` (run_failures: 0) committed |
| W4 exit mapping | `grade_exit` = PASS 0 / FAIL 1 | same contract as the demo verdict; no third code introduced |
| wiring | truerise-eval reachable from the binary | 4 islands left (rsi/hats... wait: rsi reachable since W3) -> islands: hats/memory/mcp/observe |

| W5 hats into run | `cargo test -p truerise-cli --test hat_gate` | 3 passed: `critic_hat_denied_write_file_engineer_hat_allowed` (critic subset [read_file] -> write_file denied pre-table; engineer subset has write_file -> executed; both audited), `hat_content_hash_lands_on_chain_for_each_run` (hat.registered carries THIS hat's content_hash), `unknown_tool_in_hat_refused_at_load` (subset naming "teleport" refuses the RUN; no hat.registered row for a refused hat - validation happens BEFORE registration) |
| W5 honest data fix | hats/*.toml | preset subsets named repo_grep/list_dir - tools the executor does NOT have; presets now name only real tools (the data must not promise more than the runtime delivers) |
| W5 surface | RunParams (single-track assembly struct) + `--hat <path>`; hats::parse_hat split from load_hat (validate-then-register) | run chain registers the run's hat; StackedPolicy = hat subset as OUTER gate |

| W6 memory into run | `cargo test -p truerise-cli --test run_memory_observe` | `recall_row_records_scores_and_injection`: memory.recall row per plan turn (injected ids + per-mille INTEGER scores - float rule), the BM25-matching record ranks FIRST (fusion semantics: every record scores via recency/importance; the test asserts dominance, not exclusivity), the plan payload carries the recalled context |
| W6 observe into run | same file | `export_produces_one_span_per_row_and_redacts` (real run chain + a config row carrying an api_key -> span count == row count, secret absent, `<redacted>` present, row hashes exported), `export_never_mutates_chain_digest` (digest before == after) |
| W6 surface | core `ContextRecall` port (loop consults before every plan turn, evidence audited either way); memory `RecallBridge` (memory gains core dep); cli RunParams.memory; run subcommand exports spans (best-effort, one-way) | recall with an EMPTY store still audits `injected: 0` - the attempt is the auditable fact |

| W8 mcp --serve | `cargo test -p truerise-cli --test mcp_serve` | 3 passed: `mcp_server_default_empty_allowlist_over_stdio` (the REAL binary serves stdio; initialize answers identity; tools/list EMPTY by default; tools/call on a non-allowed tool errors -32602), `client_write_class_hard_denied_before_any_server_call` (a client with NO server still refuses write-class; the denial is audited), `mcp_calls_appear_on_the_chain` (mcp.jsonl carries initialize + tools/list in order, chain verifies) |
| WIRING GATE GREEN | `python tools/check_wiring.py` | 11/11 roster crates reachable from truerise-cli - the 7-island red that W1 planted is closed; islands are a permanent CI fact, not a one-off audit |

P0+P1 wiring complete. Remaining in P1: W7 (gen0->gen1 closed loop), W10 (certificate signing).

---

## P1 — the RSI closed loop (W7/W8, 2026-11-16)

HEAD at wave start: `71538d4`.

| Item | Command | Result |
|---|---|---|
| RFC 0003 | `docs/rfc/0003-generation-certificate-and-apply.md` | generation lineage + the approval-gated apply path; six non-negotiable boundaries, each with a named test; honest single-maintainer approval + external-review invitation |
| rsi apply | `cargo test -p truerise-rsi` | 10 passed: the ONLY apply path refuses without a token (`apply_without_human_approval_token_is_refused`), refuses hash mismatches (approve A / apply B), refuses STALE epochs + REVOKED approvals through the M2 fence semantics (`stale_or_revoked_epoch_write_rejected`), and records lineage on approval (`evolve.apply` row with parent/child hashes) |
| evolve demo | `cargo test -p truerise-cli --test evolve_gate` | 5 passed: `evolve_gen0_to_gen1_produces_lineage_certificate` (PASS 4/4; chain rows proposal/approval/apply/verdict/certificate), `apply_without_human_approval_token_is_refused` (gen0 untouched, refusal audited, NO certificate), `approval_hash_mismatch_with_proposal_is_refused`, `injected_regression_rolls_back_and_chains_the_reason` (FAIL verdict recorded + byte-exact rollback + reason on chain - the FAIL+rollback certificate is machinery SUCCESS), `generation_certificate_recomputes_identically_offline` (PASS + FAIL cases both replay offline; a tampered cert does NOT recompute) |
| committed artifact | `readings/v0.2/gen1_certificate.json` | byte-DETERMINISTIC across runs (two independent runs -> identical bytes); embeds both skill contents so replay is file-free; `cargo run -p truerise-cli -- evolve --replay --cert readings/v0.2/gen1_certificate.json` -> exit 0 |
| claim gate | `python tools/check_claim_terms.py` | 3 met / 2 unmet: Harness, Governance, Recursive Self-Improvement now carry command+test+artifact evidence; Verifiable (needs W10 signing) and Deterministic (needs W14 tape + W17 policy) stay unmet - the remaining title gap is CI-visible |
| workspace | `cargo test --workspace` | 138 passed 0 failed; clippy -D warnings clean; fmt clean |

| W10 signing identity | `cargo test -p truerise-cli --test cert_identity` | 3 passed: `verified_certificate_prints_issuer_and_chain_tip`, `tampered_certificate_fails_signature_check` (tampered payload AND foreign-issuer key both refused), `unsigned_certificate_reports_unverified_issuer` (unsigned = unverified, never silent trust; a signature without a trusted anchor is equally unverified) |
| W10 surface | `truerise verify --cert <p> [--issuer-key <hex>]` + sign-on-issue under `TRUERISE_SIGNING_KEY` | keys live in the environment ONLY (never repo/argv/logs, SECURITY.md); the committed readings cert is UNSIGNED (no release key on this machine) - `verify` reports unverified_issuer for it, honestly |

| W9 significance | `cargo test -p truerise-eval` | significance module (5 tests): `one_task_improvement_is_not_significant` (100/100 vs 99/100 -> ONE discordant pair -> floor VOIDS the claim outright), `unequal_budget_comparison_is_refused` (same trials + 4x budget -> refused; different trial counts -> unpairable), `discordant_pair_count_below_floor_voids_claim` (4 one-directional pairs that LOOK like improvement -> voided), defensible 30-pair improvement -> Significant + label-swap antisymmetry property test over deterministic seeds |
| W9 exactness | p = k / 2^n as INTEGERS | the significance decision is an integer comparison (20k <= 2^n); certificates carry numerator/exponent, never floats (float rule) |
| W9 into evolve | regenerate `readings/v0.2/gen1_certificate.json` | the cert now carries `significance: voided (discordant 1 < floor 10)` - the 4-task demo's "improvement" is honestly NOT claimable; replay still recomputes identically |

| W14 tape | `cargo test -p truerise-cli --test tape_replay` | 3 passed: `same_tape_replays_identical_digest` (record -> header digest derivable -> fresh-chain replay produces field-identical books; the tape does not change under replay), `tape_model_mismatch_voids_round` (a foreign-model tape voids through the EXISTING pinning check - the tape bypasses nothing), `readings_ship_tapes_and_replay_offline` (committed `readings/v0.2/gen1_tape.jsonl` replays byte-exact offline, consumed-take semantics give each call its own response) |
| W14 provider | `cargo test -p truerise-cli --lib` | `openai_compat_adapter_speaks_the_standard_shape` + `missing_key_never_sends_a_phantom_header` against a canned local HTTP server: temperature pinned to 0, Bearer auth only when configured, model self-reported for pinning |
| W17 float rule | `cargo test -p truerise-governance --test chain_gate` + pytest | `canonical_json_float_rule_test` (Rust) + `test_canonical_json_float_rule_refuses_floats_in_rows` (Python): floats refused AT THE WRITE GATE in BOTH implementations; chain rows carry integers/strings only (per-mille, basis points, p = k/2^n) |
| W17 policy | `docs/determinism.md` (bilingual, docs-gate registered) | three scoped determinism claims + the honest boundary (no cross-hardware bit claims, no deterministic-model-output claims); the `ts` field is provenance, not derivation |
| honest data fix | python demo + certify chain rows | `claim_p` -> `claim_p_bp` (basis points), `accuracy` -> `accuracy_per_mille`; the bench's own rows now obey the rule they inspired |
| claim gate | `python tools/check_claim_terms.py` | 5 met / 0 unmet: Deterministic flips (tape + float rule + policy). THE FULL SLOGAN IS NOW EVIDENCE-BACKED |

| W16 long_horizon | pytest (test_evals_bench) | `test_long_horizon_deterministic_byte_reproducible` + `test_long_horizon_honest_completes_all_and_skipping_fails`: multi-step pipelines with SEALED value chains; completion requires EVERY step (a right endpoint via a skipped step is a FAIL); integers only (per-mille completion); committed reading `readings/v0.2/long_horizon_honest.json` (8 chains, 1000/1000 steps) |
| W11a external protocol | pytest | `test_external_solver_protocol_end_to_end` (the reference external solver runs as a SUBPROCESS: public surface by env, submission.jsonl back, grades signed by the runner = executor), `test_external_solver_records_are_executor_signed_at_L4` (signed records count at the declared level), `test_self_reported_external_run_is_capped_at_L0` (the solver's own claim contributes nothing) |
| protocol surface | `truerise.evals.runner.run_external_trials(solver_cmd, pack, ...)` | one contract for any third-party solver: env TRUERISE_PUBLIC_DIR / TRUERISE_OUT_DIR, submission.jsonl in; the W11b public-model reading is now ONE command away |

| W13a Python SDK | pytest (test_sdk.py) | 8 passed: the zero-dependency `truerise.sdk` mirrors the Rust contracts as a CONSUMER (never a second authority): Session (three books + RFC 0002 enforcement chain: decide -> execute -> audit, plain text never executes), Policy (ask needs approval ON FILE; deny short-circuits; default-deny), ApprovalQueue (single-point epoch, revoke terminal), Budget (one shared denominator + reconcile==0), Ratchet (tighten-only, integer metrics). Mirrored test names documented per Rust behaviour |
| SDK boundary | `truerise.sdk.__all__` | fingerprint / seal / prereg / evidence gate RE-EXPORTED from their single sources (no copies of the authority) |

| W15 examples | `python examples/langgraph_harness.py` + `python examples/openai_agents_harness.py` | both smoke PASS WITHOUT the third-party framework installed (guarded imports); the pattern: YOUR agent loop stays yours, TrueRise owns budget + hash-chain audit + policy + ratchet |
| W15 OTel tier | `cargo test -p truerise-observe --features otel` | `export_spans_via_otlp_feature`: REAL OpenTelemetry spans from chain rows (stdout exporter in-feature; an OTLP collector wires at the boundary in one line); no global tracer state; one-way property holds (digest unchanged); sensitive keys redacted on the way out; CI checks the feature compiles |
| W15 integration doc | `docs/integrations.md` (bilingual) | the three surfaces in preference order: MCP (tool supply) -> Python SDK (harness embedding) -> external-solver protocol (evaluation); what we deliberately do NOT do (per-framework shims, a second judging authority) |

Not doing (P2): the committed tape/keystones remain unsigned until a release signing key exists (W10's verify honestly reports unverified_issuer for them).

---

## P3 — the ecosystem surface + release hardening (W13/W15/W18/W11b, 2026-11-16)

HEAD at wave start: `b23fab4`.

| Item | Command | Result |
|---|---|---|
| W13a Python SDK | pytest test_sdk.py | 8 named tests: the zero-dependency `truerise.sdk` (Session/Policy/ApprovalQueue/Budget/Ratchet) mirrors the Rust contracts as a CONSUMER; fingerprint/seal/prereg/evidence re-exported from their single sources |
| W13b native backend | `cargo check -p truerise-py` + pytest | the PyO3 cdylib (`truerise_native`) compiles as a workspace member; THIN bindings only (chain append/verify, ratchet); optional wheel (python-native/pyproject.toml, abi3-py310); CI native-wheels job on three OS; the SDK probe + parity test skip cleanly when the wheel is absent |
| W13b file-length compliance | `python tools/lint_file_length.py` | all 86 code files <= 600 lines (loop tests -> tests/loop_gate.rs; cli splits: demo.rs / interactive.rs / baseline.rs) |
| W15 integrations | smoke both examples | langgraph_harness.py + openai_agents_harness.py run WITHOUT the frameworks installed (guarded imports); docs/integrations.md (bilingual): MCP -> SDK -> external protocol, and the deliberate non-goals |
| W15 OTel tier | `cargo test -p truerise-observe --features otel` | `export_spans_via_otlp_feature`: REAL spans from chain rows (stdout exporter in-feature; OTLP at the boundary); no global tracer state; one-way (digest unchanged); secrets redacted; CI checks the feature compiles |
| W18 hardening | grep the workflows | actions/checkout @ 11bd7190... (v4.2.2) + actions/setup-python @ a26af69b... (v5) SHA-pinned in BOTH workflows (fetched from the GitHub API, not guessed); CITATION.cff + ROADMAP.md; RELEASING status updated |
| W11b external reading | env probe | NO model API key on this machine - the protocol is one command away (`run_external_trials("<solver>", pack, ...)`); ROADMAP.md item 1 owns the operational step; the README Limitations honestly still says "no external model run yet" |
| title | `python tools/check_claim_terms.py` | 5 met / 0 unmet - the README H1 now carries the FULL formal title, enforced by the claim gate |

Not doing (P3): per-framework shims; a second judging authority; any auto-apply path; improvement-magnitude promises (ROADMAP non-goals, stable).

---

## W3 — Anti-Cheat Bench (truerise.evals) + open-source construction wave (2026-11-15)

HEAD at wave start: `123f3c5`. All gates re-run at wave end (see the gate matrix below).

| Item | Command | Result |
|---|---|---|
| toolchain unpinned | `rust-toolchain.toml` -> channel `stable`; MSRV floor stays with the dedicated `msrv` CI job (1.85) | local builds resolve stable; MSRV contract unchanged |
| CI hardening | `.github/workflows/ci.yml` | build-test OS matrix (ubuntu+windows), new `python` job (pytest + demo + replay + honest), new `cross-cert` job (dual-implementation gate) |
| workspace hygiene | `git rm -r --cached` 8 tracked `.pyc` + `.gitignore` (`__pycache__/`, `.pytest_cache/`, `*.pyc`, `.venv/`) | index clean; files remain on disk, never deleted |
| dual-implementation gate | `python tools/check_dual_impl.py` | OK: verdict/claim/fingerprint/seal/bar/reason-kinds agree in default AND honest modes; **caught a real contract break during construction: the Python seal digest diverged from Rust (`10.0` vs `10` in canonical JSON) — aligned to `10`, seal digests now equal** |
| Rust example | `cargo run -p truerise-cli --example integrity_walkthrough` | honest=false caught=3 / honest=true caught=0, seal constant across flips |
| eval bench | `cargo bench -p truerise-eval` | `eval_baseline.rs`: fingerprint digest / ratchet tighten / conformance judge baselines compile + run |
| bench package | `PYTHONPATH=python/src python -m truerise evals demo` | 120 tasks (6 families x 20, seed 20261115); honest PASS 100% (120/120); cheater FAIL (trials=20 -> corrected 2.5e-03 vs claim 0.05); mutator FAIL (prereg diff names keys); acceptance exit 0 |
| pack determinism + isolation | `python -m pytest python/tests -q` | **25 passed** (13 demo + 12 bench: determinism byte-reproducible, canary fail-closed, no family field on public rows, no assembly marker leakage, cheat catches, replay + tamper fail-closed, rule coverage incl. safe-twin negatives) |
| committed readings | `readings/v0.1/*.json` | three certificates + pack manifest (machine-local paths stripped) |
| docs | `docs/anti_cheat_bench.md`, `docs/report/anti_cheat_bench_v0.1.md`, `docs/rfc/0001-claim-certificate.md`, `docs/topology.md`, `LICENSE-DATA.md` | bench spec + 4-page report (every number -> artifact path) + certificate schema RFC (draft) + one-way dependency/pin policy + data license |
| README storefront | `README.md` | integrity layer first; pain-point table with one runnable command per row; Limitations states the honest boundaries (no external model run yet; internal production deployment stays out of this repo by design; solo project) |

Construction-time failure inventory (published discipline): the reference
honest solver's FIRST full run scored 100/120 with ALL 20 misses in
`full_sample_scaling` — a solver-side rule bug (regex demanded an index token
after `mu`), surfaced by the miss inventory itself; fixed, re-run 120/120.
The reading was superseded by `readings/v0.1/honest_verdict.json`.

Not doing (W3): level-3 same-family twins (v0.2); external-solver subprocess
protocol (v0.2, pending item 14); PyO3 binding (python package remains the
pure-Python dual implementation; PyO3 tracked for the first-public-tag wave);
macOS CI leg (cargo-dist covers darwin binaries; test matrix adds darwin at
first remote push when CI minutes are visible).
13. [M1, due 2026-11-28] schema-gate coverage: M0 asserts exactly one wire contract (audit_event.v2 — the single chain contract defined at spec level). The full schema roster is defined at the M1 wire freeze; the gate's INDEX.json grows with it.

---

## M1 — wire + platform + core (2026-11-14)

HEAD: see `git log -1` (wave commit follows this entry). All gates below re-verified at the wave commit.

| Gate | Command | Result | Measured at |
|---|---|---|---|
| platform tests | `cargo test -p truerise-platform` | 7 passed (monotonic / file lock via guard-writes / process) | wave commit |
| wire tests | `cargo test -p truerise-wire` | 5 passed (v1/v2 dual edition round-trip, extra-field rejection, hash format check, schema file alignment) | wave commit |
| core tests | `cargo test -p truerise-core` | 18 passed (error tree / four modes / timecore / state machine table coverage) | wave commit |
| LeakGuard suite | `cargo test -p truerise-core --test leakguard` | 4 passed incl. 2 proptest (four vectors; proptest found and fixed TWO real defects: sweep endpoint ordering for half-open semantics, FoldWindow accepting overlap when purge=0) | wave commit |
| terminal absorption | `cargo test -p truerise-core --test terminal_absorb` (trybuild) | 1 passed - terminal transition does not compile (spec §6.7.1 judgement 2) | wave commit |
| clippy / fmt | `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all -- --check` | zero warnings / clean | wave commit |

Spec deviations / decisions (M1):
- `unsafe` is allowed ONLY in truerise-platform (OS file-lock calls), with per-block `// SAFETY:` rationales and `undocumented_unsafe_blocks` denied; the workspace-wide forbid stays for every other crate. Lints written out in full for that crate (workspace inheritance + sub-table is invalid TOML).
- process TREE collection is reduced to the spawn/wait/kill surface + current pid at M1 (the full tree walk needs a platform dependency; recorded as an extension point, not silently dropped).
- The wire crate validates hash FORMAT on the wire (64 lowercase hex); canonical-serialization equality is the ledger's job (M2).
- Proptest-found defects fixed during M1 are recorded above (they are the LeakGuard suite working as designed).

Not doing (M1): benches (criterion baselines land with M2+ hot paths); state machine wire contract export (state_transitions schema rides with M2 governance events).

---

## M2 — governance + sandbox (2026-11-14)

| Gate | Command | Result |
|---|---|---|
| chain gate tests | `cargo test -p truerise-governance --test chain_gate` | 6 passed: **dual-writer competition (2 handles, 1000 appends, 0 lost updates, seq+hash intact)**; tamper drill (content-hash tamper located at row 2; prev-hash tamper at row 3; unparsable row carries its number); idempotent replay; persistence reload; legacy rows stored-but-not-governing |
| governance lib | `cargo test -p truerise-governance` | 11 passed (policy three-state + deny short-circuit + approval-never-times-into-consent + three-gate deletion flow) |
| sandbox | `cargo test -p truerise-sandbox` | conformance self-check: three must-fail regions (escape / oversell / residue) PASS for the in-process provider |
| workspace | `cargo test --workspace` + clippy `-D warnings` + fmt | 53 passed 0 failed; clean |

Spec decisions / findings (M2):
- The conformance ESCAPE probe caught a real escape on first run: `Path::starts_with` is lexical and does not resolve `..` - the fence is now component-level (ParentDir/RootDir/Prefix refused). The self-check works on its own provider.
- Windows byte-range locks are process-wide: the chain's in-lock IO goes exclusively through the locked handle (a second handle for reading collided with our own lock - error 33). Platform-lock tests cover the guard-owned write pattern.
- Ledger key binds fingerprint AND evaluation parameters in the python demo; the Rust chain keeps the same rule via replay keys (parameter change = new key).
- deny.toml warn-severity fields (multiple-versions, unknown sources) remain warn at M2; machine assertion of those fields is deferred to M5 release hardening (recorded, not silently dropped).

Not doing (M2): conformance VERDICTS (judging authority lands with truerise-eval in M4 - self-check stays facts-only); capability-bit probing beyond declared-none (docker/microVM providers are external projects).

---

## M3 — loop + compaction + memory (2026-11-14)

| Gate | Command | Result |
|---|---|---|
| minimal run three-books gate | `cargo test -p truerise-core` (loop_ tests) | minimal run drives RunStart..Finish with transcript + transition log + audit rows populated and mutually consistent (audit_rows == transitions.len()) |
| compaction invariants | `cargo test -p truerise-core` (compaction tests) | tokens monotonically non-increase (chained L1->L4, clamped), pre_compact event emitted, recent-tail kept verbatim, tail-too-large refused, unknown u32 level refused |
| memory fusion | `cargo test -p truerise-memory` | 6 passed: weights normalise, BM25 ranks term-matches first, fusion respects weight shifts both ways, deterministic tie-break, backend upsert |
| workspace | `cargo test --workspace` + clippy -D + fmt | **71 passed 0 failed**; clean |

Spec decisions (M3):
- Hexagonal fix: core->governance would be a cycle (governance needs core's error tree) - the loop defines the `AuditSink` PORT in core; governance implements it at the boundary (dev-dependency only for the adapter test). Config file location stays pinned to the project root.
- Backoff: exponential with capped jitter, integer ms; ONLY rate-limit/transport classes retry (fatal errors never retried - asserted).
- Round pinning: any response from a non-pinned model VOIDS the whole round (loop returns Pin error; no partial credit).
- Compaction clamps per-entry tokens to the original (compaction never inflates); the monotonic-decrease test runs the REAL chained workflow (L1 output feeds L2).
- Deterministic jitter in tests (roll derived from attempt) - production injects entropy at the call site.

Not doing (M3): wall-clock-based recency decay (recency is writer-assigned in [0,1]; the decay policy is external); cross-backend recall federation (one backend per session).

---

## M4 — eval + rsi + hats (2026-11-14)

| Gate | Command | Result |
|---|---|---|
| ratchet (exit-grade) | `cargo test -p truerise-eval` | 9 passed: score regression -> RatchetViolation::Regression (callers map to non-zero exit); loosening refused + ratchet unchanged on refusal; fingerprints part-sensitive; cross-environment comparison FORBIDDEN even self-vs-self |
| budget reconciliation | `cargo test -p truerise-rsi` | 6 passed: two handles one ledger (reconcile diff == 0), per-hat quota, global denominator not sharded; singleton is process-global -> consuming tests serialised (delta arithmetic requires it - itself proof of the shared ledger) |
| evolve never auto-merges | `cargo test -p truerise-rsi` | proposals + MR reports ONLY; report states HUMAN REVIEW REQUIRED / NOT merged; no merge/apply function on the surface |
| hats + audit alignment | `cargo test -p truerise-hats` | TOML loads, schema-version enforced, content_hash (64 hex) lands on the chain as hat.registered; any content change -> new hash |
| workspace | `cargo test --workspace` + clippy -D + fmt | **89 passed 0 failed**; clean |

Spec decisions (M4):
- judge_agreement is PURE statistics (raw agreement + Cohen's kappa); it grants nothing.
- judge_and_record: a verdict that cannot be recorded is an error (unrecorded verdicts break auditability).
- Budget singleton is process-global via OnceLock: reconciling delta assertions REQUIRE serialised consuming tests - documented in the test itself.
- Preset hat packs (researcher/critic/engineer/evaluator) land as DATA in hats/*.toml (hats are data, not traits).

Not doing (M4): bench harness over live models (port + scripted tests only); skill entry resolution (host-side concern).

---

## M5 — mcp + observe + cli + release surface (2026-11-14)

| Gate | Command | Result |
|---|---|---|
| MCP | `cargo test -p truerise-mcp` | 5 passed: JSON-RPC 2.0 strict frames, zero-exposure server (empty allowlist), allow-gated tool calls, client HARD-DENIES write-class (audited), toy e2e client->server |
| observe | `cargo test -p truerise-observe` | 2 passed: jsonl export one-span-per-row with sensitive-key redaction (<redacted>), chain digest unchanged before/after export (ONE-WAY); otel feature placeholder compiles |
| CLI (live) | `cargo run -p truerise-cli -- demo` / `--honest` / `run --goal` | demo FAIL (3 reasons) -> --honest PASS; run completes with three books (3 transcript / 6 transitions / 6 audit rows) |
| workspace | `cargo test --workspace` + clippy -D + fmt | **96 passed 0 failed**; clean |

Spec decisions (M5):
- MCP server zero exposure: the allow-list starts EMPTY; initialize/tools/list work, tools/call on a non-allowed tool returns a JSON-RPC error frame.
- Client write-class hard-denial happens BEFORE any server interaction and the denial itself is audited.
- REPL is an IO adapter only: free text reuses the same run path (no bare-face entry, spec §6.5 external-review ruling 3).
- Onboarding writes a config skeleton with NO secrets (env/keyring only, fail-closed wording).
- cargo-dist release.yml is an in-repo placeholder: the five-platform matrix + SHA256 + installer scripts land with the first public tag (spec §6.5); the workflow shape is reviewable now.

Not doing (M5): authenticode/notarization signing (needs certificates); npm/pip shim distribution (needs published artifacts); OTLP SDK exporter behind the otel feature (feature placeholder compiles, SDK wiring deferred to first release); REPL golden transcript test (requires the chat-reference parity harness, M5+ post-release).

---

## M5 residual closure — spec-mapping gate + benches + enforced timesafety + golden REPL + release config (2026-11-14)

HEAD: `9011804` (2026-11-14T22:46:44+08:00), 22 commits all DCO-signed.

| Item | Command | Result |
|---|---|---|
| spec-mapping gate | `python tools/lint_spec_map.py` | 23 §6 rows / 67 identifiers all resolve (CI red on missing mapping or dangling identifier) |
| four vectors | `python tools/check_leakguard_skeleton.py` | 4 vectors ENFORCED (declaration + enforcing leakguard tests verified per-vector) |
| criterion baseline | `cargo bench -p truerise-governance` | chain/append + verify_full baselines saved; CI job saves `main` baseline, critcmp 20% gate activates from the second run |
| REPL golden | `cargo test -p truerise-cli --test golden_repl` | repl adapter path and direct loop path: transcript/transitions field-identical, audit delta per run == transitions (shared chain accumulates) |
| SBOM M5 gate | `python tools/check_sbom.py` (after `cargo cyclonedx --format json --spec-version 1.5`) | 12/12 reports, specVersion == 1.5, components >= declared EXTERNAL dependencies per member |
| cargo-dist config | `[workspace.metadata.dist]` | five targets, shell/powershell installers, github CI |
| dry-run | `cargo package --workspace --list --offline` | clean (exit 0) |
| docs | docs/arch.md, timesafety.md, governance.md, rfc/README.md | bilingual (EN+CN) |

The M5 exit gates are now: e2e toy server PASS, dry-run clean, SBOM assertions at M5 grade, release config in-repo. The ONLY remaining item that cannot run on this machine is the actual five-platform artifact build, which triggers on the first public tag (release.yml is in-repo and reviewable).
