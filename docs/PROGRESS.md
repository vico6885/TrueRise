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
