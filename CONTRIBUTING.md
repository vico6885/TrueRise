# Contributing to TrueRise

## Development status

The project is in active construction (M0-M5). Open an issue before large
changes; port-trait additions go through RFCs (see `GOVERNANCE.md`).

## Commit sign-off (DCO)

Every commit must carry a `Signed-off-by:` trailer (`git commit -s`). DCO
certifies that you have the right to submit the work under the repository
license (Apache-2.0 / MIT dual license).

## IP-clean declarations (mandatory, in addition to DCO)

DCO certifies license granting only; it does NOT certify source provenance.
Together with your first contribution (and re-affirmed on substantive
contributions), you assert all three:

1. **Clean origin**: this contribution contains no code, translation, or
   adaptation carried in from any previous employer or third-party codebase.
2. **Spec-based design**: design decisions reference the repository's own
   specifications and are recorded with timestamps (issue / PR / docs).
3. **No source access**: where a feature re-implements a known mechanism,
   you wrote it from the behaviour specification, not from the original
   source text.

## Engineering rules

- safe Rust by default; `unsafe` is allowed ONLY in `crates/truerise-platform` (file locking; every block carries a SAFETY comment and is audited) — see `docs/arch.md`.
- no `unwrap()` / `expect()` in production code (tests may).
- every public item is documented (`missing_docs` is denied).
- single file <= 600 lines; workspace lints deny violations.
- all gates in `.github/workflows/ci.yml` must pass; none may be skipped.

## The leak-prevention gates (read before your first PR)

Two CI gates scan for a small set of banned tokens that must never enter the
repository. The token list exists ONLY hex-encoded (`tools/banned_words.hex`);
the literals are not published anywhere in the repository.

- **Workspace door** (`tools/lint_banned_words.py`): scans every repository
  text file. ASCII tokens match on word boundaries, CJK tokens match as
  substrings, matching is case-insensitive. A hit lists `file:line` and
  fails the gate.
  - If your PR trips it: rephrase the flagged line (pick a different word,
    including in comments and commit messages). Do NOT obfuscate or encode
    a banned token to slip it through — that is treated as a leak attempt
    and fails closed. Durable path exemptions live in the auditable
    plaintext `EXEMPT_PATHS` list in the script and change only via
    reviewed PRs.
- **History door** (`tools/lint_git_history.py`): scans every git object
  (full history + reflog) for the same tokens, and enforces a blob-count
  floor (`tools/history_floor.json`) — any history rewrite that destroys
  objects fails the gate. Never amend or rewrite pushed commits outside
  the deletion three-gate protocol (see `GOVERNANCE.md`): an explicit,
  recorded maintainer approval, a single amend commit replacing the
  offending content, and a reflog/gc expulsion with the pre-prune snapshot
  retained OUTSIDE the repository (in-repo retention would trip this very
  gate). The blob-count floor in `tools/history_floor.json` is the
  technical control that keeps the no-prune commitment verifiable.

Both gates are fail-closed: an empty or corrupt token list is a red gate,
not a skipped check.

## Data contributions (task packs)

Task packs are DATA and are licensed CC-BY-4.0, separate from the code
license (see `LICENSE-DATA.md`). Generator/judge CODE accompanying a pack
follows the code license above. A pack contribution must:

- pass the pack validation gates (id alignment, line ranges, taxonomy);
- keep the sealed surface sealed (isolation canary intact; no family field
  on public rows);
- be generator-driven and deterministic (seed + byte-reproducible surfaces).
