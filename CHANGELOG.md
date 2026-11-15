# Changelog

All notable changes to TrueRise are documented here. Format based on
Keep a Changelog; versioning is a single axis shared with the Python-track
publications (the Rust core is the authoritative implementation).

## [0.2.0-dev.0] - unreleased

### Added (P0-P3 construction waves, 2026-11-16)

- Tool execution port (RFC 0002): `ActionPolicy` + `ToolProvider` in core,
  the fenced `SandboxToolProvider`, the governed run path
  (decide -> execute -> audit; capability = the executor's value; denial is
  an audited outcome; no hooks = nothing executes).
- Budget enforcement: the `Budget` port stops a live run (round + tool
  consumption; exhaustion audited, exit 1); rsi `RunBudget` binds the
  singleton ledger.
- Run verdicts: `judge_run` / `judge_tasks` from the judging authority
  (RunVerdict has no public constructor); baseline ratchet on the run path;
  `grade_exit` keeps PASS=0/FAIL=1.
- Hats on the run path: `--hat` registers the content hash + layers the
  tool_subset as the OUTER policy gate (unknown tool refuses the run).
- Memory + observe on the run path: `ContextRecall` port (audited evidence,
  per-mille scores), one-way span export.
- MCP server as a process (`mcp --serve`, zero exposure by default, every
  call on the chain).
- RSI closed loop (RFC 0003): the approval-gated apply path, generation
  lineage, byte-deterministic certificates (`readings/v0.2/`), offline
  replay; injected regression rolls back with the reason on the chain.
- Certificate signing identity: ed25519 sign-on-issue + `verify --cert`
  (unsigned = unverified, never silent trust).
- Significance: exact integer McNemar (p = k/2^n); unequal-cost refused;
  discordant floor voids claims; wired into the certificates.
- OpenAI-compatible provider adapter + the recording/replay TAPE
  (committed readings replay offline); `trace_digest` in tape headers.
- The canonical-JSON float rule (Rust + Python write gates refuse floats);
  `docs/determinism.md` (three scoped claims + the honest boundary).
- Long-horizon closed-loop task family (v0.2) + the external-solver
  protocol (executor-signed grades; self-report capped L0).
- Python SDK (`truerise.sdk`) + the PyO3 native backend
  (`truerise-native`, optional); integration patterns (LangGraph /
  OpenAI Agents SDK) + OTel span projection (`otel` feature).
- Gates: wiring (islands are a permanent red), claim terms (the title is
  held to the evidence ledger, 5/5 met), action SHAs pinned.


### Added (M0 — repository scaffold)

- Workspace with 12 `truerise-*` crates (skeletons, `publish = false`).
- CI: fmt / clippy (deny) / build / MSRV 1.85 / cargo-deny / SBOM / DCO /
  file-length gate / schema gate / timesafety placeholder job.
- Leak-prevention double doors: workspace banned-token gate + git
  history-object gate (token list stored hex-encoded, zero literals in-repo).
- License isolation: cargo-deny whitelist excludes the legacy Python-edition
  license (BSL-1.1) and copyleft GPL/AGPL, machine-asserted in CI.
- Governance docs: README, CONTRIBUTING (IP-clean declarations), SECURITY,
  GOVERNANCE, CODE_OF_CONDUCT, PROGRESS.

### Added (M1 — wire + platform + core)

- Error tree (wire), four modes (platform), typestate machine + timecore
  (core); the LeakGuard suite enforces all four timesafety vectors
  (declaration + enforcing tests asserted, never skippable).

### Added (M2 — governance + sandbox)

- Hash-chain audit with dual-writer gate, three-state policy, fenced
  approvals, resource-policy port, sandbox conformance self-check.

### Added (M3 — loop + compaction + memory)

- Cognitive loop (transcript + transition log + audit chain kept mutually
  consistent), five-level compaction (monotonic token decrease, chained
  L1->L4), three-way memory fusion (BM25 + recency + importance).

### Added (M4 — eval + rsi + hats)

- Conformance judging authority (`judge_and_record`: a verdict that cannot
  be recorded is an error), score ratchet (tighten-only; regression maps to
  a non-zero exit), RSI budget singleton, evolve surface that never
  auto-merges (proposals + human-review reports only), hats as data with
  chain-aligned content hashes.

### Added (M5 — mcp + observe + cli + release surface)

- MCP zero-exposure server (empty allowlist) and client with write-class
  hard-denial (JSON-RPC 2.0 strict frames).
- Observe: jsonl span export with sensitive-key redaction; one-way (export
  never mutates the chain).
- CLI binary + REPL + secret-free onboarding skeleton; REPL golden test.
- Release surface: cargo-dist config (five targets, shell/powershell
  installers), M5-grade SBOM gate, spec-mapping gate (23 rows / 67
  identifiers), criterion baselines.

### Added (W3 — the Anti-Cheat Bench, Python track)

- `truerise.evals`: generated task packs (six-defect taxonomy, sealed answer
  surfaces with isolation canary), prereg seal gate, runner-counted
  multiplicity accountant, evidence ladder (L0-L6), four-part fingerprints,
  claim certificates; 60-second three-scenario demo (honest PASS, cheater
  FAIL, mutator FAIL) with offline `--replay`.
- Committed reference readings (`readings/v0.1/`), technical report
  (`docs/report/anti_cheat_bench_v0.1.md`), certificate-schema RFC 0001,
  data license (`LICENSE-DATA.md`), dual-implementation cross-certification
  gate (`tools/check_dual_impl.py`).

### Added (documentation surface — bilingual + repository hygiene)

- Bilingual pairing across the tree: `README.zh-CN.md`, `docs/README{,.zh-CN}.md`
  index, `docs/quickstart`, `docs/cli`, `docs/task_pack_guide`,
  `docs/provider_conformance`, `docs/anti_cheat_bench`,
  `docs/report/anti_cheat_bench_v0.1`, `providers/README`, `python/README`,
  `tests/README` — each pair cross-linked in its header, English authoritative.
- `tools/check_docs.py` documentation gate (CI job `docs`): registered
  pair existence, header switch links, relative-link resolution, and every
  cited `tools/<script>.py` / `python -m truerise*` invocation must exist.
  A document that claims to be bilingual but carries no Chinese now fails CI.
- `.github/PULL_REQUEST_TEMPLATE.md` (gate table, spec-row linkage,
  contributor declarations), `.github/CODEOWNERS` (routing intent plus the
  provisional-handle warning), `.editorconfig`, `RELEASING.md` (pre-tag
  checklist and the decisions that block the first public tag).
- Makefile parity with CI: `gates-py` runs all nine repo gates, `verify` is
  the full local mirror; `demo`, `replay`, `dual`, `doc-links` targets.
- crates.io discovery metadata (`keywords`, `categories`) for all 12 crates.
- `.gitignore` covers stray local asset directories (`/vibe_images/`,
  `readings/local/`).

### Fixed (behaviour — the exit code now carries the verdict)

- `truerise-cli`: `demo` returns `1` on a FAIL verdict and `0` on PASS
  (previously `0` unconditionally, so a CI lane gating on the Rust binary's
  exit code could never go red). Unknown command still `2`.
- `tools/check_dual_impl.py`: the exit code joined the compared contract
  fields, plus a per-side check that the code agrees with its **own** verdict
  (equality alone would let both sides agree on a useless constant).
- New test target `crates/truerise-cli/tests/exit_codes.rs` (4 tests);
  workspace count moved 97 -> 101 and every document quoting the count was
  updated in the same commit.

### Corrected (documentation accuracy, found while auditing claims against code)

- Port count stated as implemented: **three port traits** (`ModelRouter`,
  `SandboxProvider`, `MemoryBackend`) plus `ResourcePolicy`; tool exposure is
  carried by `hat.tool_subset`, the three-state policy and `truerise-mcp`.
  The spec-named fourth trait `ToolProvider` does not exist in this workspace,
  so `docs/arch.md`, `GOVERNANCE.md` and `docs/rfc/README.md` no longer assert
  it — introducing it later is an RFC-gated addition, not a doc fix.
- `python/README.md`: `truerise.evals` documented (it was absent), and the
  library-level `DEFAULT_THRESHOLDS` export gap registered.
- `tests/README.md`: placeholder "M1+ will land" text replaced with the actual
  inventory (101 Rust / 25 Python tests, gate list, and what is not tested).

## [0.1.0] - 2026-11-14 (Python track)

- Integrity-evaluation CLI: claim ledger, prereg gate, evidence gate,
  multiplicity accountant, 60-second perturbation demo (`python -m truerise
  demo`), offline replay.
