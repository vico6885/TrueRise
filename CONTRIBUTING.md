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

- safe Rust only; `unsafe` is forbidden workspace-wide.
- no `unwrap()` / `expect()` in production code (tests may).
- every public item is documented (`missing_docs` is denied).
- single file <= 600 lines; workspace lints deny violations.
- all gates in `.github/workflows/ci.yml` must pass; none may be skipped.
