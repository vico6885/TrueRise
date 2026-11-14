# PROGRESS — wave-by-wave delivery record

Format per wave: ID | files | reproducible command | output (<= 10 lines) | HEAD+UTC | not-doing/risks. A wave counts as done only with its gate evidence recorded here. Not-doing entries carry a 2-week clock: past the deadline without passing the gate, the wave rolls back.

## M0 — repository scaffold (2026-11-14)

HEAD: `4e58fa6` (2026-11-14T15:59:40+08:00), branch `main`, DCO signed.

Files: root `Cargo.toml` / `rust-toolchain.toml` / `deny.toml` / `Makefile` / `.gitignore` / `.gitattributes`; `crates/truerise-{core,wire,platform,governance,sandbox,eval,memory,rsi,hats,mcp,observe,cli}/` (skeletons, `publish = false`, doc-only `lib.rs`); `tools/` (9 gate scripts + `banned_words.hex`); `.github/workflows/ci.yml` + ISSUE_TEMPLATE; `schemas/` (`audit_event.v2.schema.json` + `INDEX.json`, spec-level alignment gate); `tests/timesafety/` (four-vector contract skeleton); `README` / `CONTRIBUTING` / `SECURITY` / `GOVERNANCE` / `CODE_OF_CONDUCT` / `CHANGELOG` / `LICENSE-APACHE` / `LICENSE-MIT`.

### Gate evidence

| Gate | Command | Result |
|---|---|---|
| build | `cargo check --workspace --all-targets` | Finished dev profile in 0.37s, 12 crates, zero warnings |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Finished, zero warnings |
| fmt | `cargo fmt --all -- --check` | exit 0 |
| workspace door (= naming gate) | `python tools/lint_banned_words.py` | `0 hits across 58 files (exit 0)` |
| history-object door | `python tools/lint_git_history.py` | `0 hits across 59 blobs + full ref log (exit 0)` |
| file length | `python tools/lint_file_length.py` | `all 20 code files <= 600 lines (exit 0)` |
| license isolation | `python tools/lint_deny_isolation.py` | `allow list clean, dual core licenses present (exit 0)` |
| schemas | `python tools/lint_schemas.py` | `1 schema(s) aligned at spec level (fields/required/enums) (exit 0)` |
| SBOM format | `python tools/check_sbom.py` (CI, after `cargo cyclonedx --all`) | asserts report present + specVersion declared + components >= workspace members |
| timesafety placeholder | `python tools/check_leakguard_skeleton.py` | `4 vectors declared planned (exit 0)` |
| DCO | `python tools/lint_dco.py` | `all 1 commit(s) signed (exit 0)` |
| naming check | crates.io API + GitHub search | crates.io `truerise`: 404 = not taken; GitHub: one unrelated personal repo (`dev-abdullah-shahid/truerise`) |

### Self-introduced decisions (not spec-mandated; recorded per the external review round-1 ruling, item Q3)

- `edition = "2024"` + `rust-version = "1.85"` (edition floor, fail-closed minimal). `resolver = "2"` IS spec-mandated. Resolver observation: explicit `resolver = "2"` with edition-2024 members produces no warning/conflict — valid for the dependency-free M0 state (weak evidence); re-verify when the first external dependency lands.
- lint `non_ascii_idents = "deny"` (naming-discipline enforcement).
- `[workspace.lints]` inheritance implements the per-crate `#![deny(missing_docs)]` requirement with a single source of truth.
- README positioning follows the construction spec (section "定位") verbatim, bilingual; the v3 tagline "A Runtime Governance Layer for Long-Horizon RSI" kept as a secondary line.
- `schemas/audit_event.v2.schema.json` (clean-room from the audit-chain behaviour spec): the three chain-critical keys (seq / prev_hash / hash) chosen as the required set; severity enum values are this repo's own design. Both flagged for confirmation when the wire crate freezes in M1.
- `tools/lint_banned_words.py` holds the plugin-crate path exceptions as a plaintext `EXEMPT_PATHS = []` (auditable, PR-reviewed) instead of an encoded switch.
- In-repo prose writes "0.1-series" instead of the spec's literal release token (0.1-series followed by the hyphenated form of a banned token; identity is recorded in the encoded wordlist): the literal contains a banned token (word-boundary match) and would trip the leak gate. **TENSION FLAGGED for ruling** — the spec names the release form with that token while banning the token workspace-wide. (The gate caught this document's own first draft containing the literal; fixed via the approved amend+prune, recorded in the external governance task log.)

### Local toolchain note

rustup attempted to re-download toolchain name `1.100.0` (locally only `stable-x86_64-pc-windows-msvc` was installed — the identical 1.100.0 binaries) and hung on the network; TUNA mirror 404s this version. Local evidence runs therefore used `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc` (same compiler, versions verified 1.100.0). MSRV floor locally verified: toolchain `1.85.1` installed and `cargo +1.85 check --workspace --all-targets` passes (see evidence). CI installs the real pinned `1.100.0` from the toolchain file; the local run is reference evidence, the authoritative CI run lands with the first pushed remote. Upgrade flow: a dedicated PR bumping `rust-toolchain.toml` only.

### History-door incident record (M0 construction, pre-remediation)

The history door caught this document's own first draft: 4 hits (3 in commit text, 1 blob) — a banned-token literal (the release-token suffix; identity in the encoded wordlist) and two occurrences of the internal reviewer role name. Remediation executed under the deletion three-gate protocol with the maintainer's explicit approval: (a) `git commit --amend` rewriting the offending commit with literal-free text; (b) `git reflog expire --expire=now --all` + `git gc --prune=now` expelling the unreachable old commit object and blobs (blob ids `14515ddf`/`fb4dd85b`). Post-remediation re-scan: 0 hits across all objects. The pre-prune hit snapshot is retained ONLY in the external governance task log — in-repo retention would trip this very gate by design. **Commitment: no further history-pruning operations before the next external review round.**

### Not doing / risks (2-week clock where applicable)

1. [M1] timesafety placeholder → real LeakGuard suite. Dual-stage wording: M0 = existence assertion only (contract skeleton declared), M1 = substantive property tests. Clock started at M0 HEAD.
2. [M1] `schemas/` gains the legacy v1 audit-event schema beside v2 (double-version rule explicit at the wire type level); the v2 required-key and enum choices above await wire-crate freeze.
3. cargo-deny / cargo-cyclonedx not installed locally; deny + SBOM jobs execute in CI. **No remote configured yet — the authoritative CI run (fmt/clippy/MSRV/deny/SBOM) lands with the first push; remote creation is a vico decision.**
4. crates.io name-squatting decision (dry-run / occupy `truerise-*`) pending vico; `publish = false` protects nothing against third-party registration.
5. Trademark-database manual check pending vico (automated GitHub / crates.io check done, see evidence).
6. git identity email `vico@users.noreply.github.com` is a placeholder pending vico's GitHub account decision (DCO trailers carry it).
7. benchmark-compare gate intentionally absent (applies from M1 per spec).
8. External-review read-only probes cannot leave the parent repository (harness jail root is hardcoded); until a channel exists, in-repo gate outputs are reference evidence reproducible by the commands above. Proposed durable fix (vico decision): a repo-root override in the review harness, or a formal evidence-bundle convention.
