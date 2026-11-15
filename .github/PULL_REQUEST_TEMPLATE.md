## What / why

<!-- One paragraph. What changes, and which behaviour it makes true. -->

## Which spec row does this serve?

- [ ] This change maps to a row in `spec_map.json` (23 rows / 67 identifiers).
      If it adds behaviour, the row **and** a named test are in this PR.
- [ ] Pure refactor / docs — no behaviour row affected.

## Verification (paste real output, not intent)

```
# commands you ran and their tail output
cargo test --workspace
python -m pytest python/tests -q
python tools/check_dual_impl.py
```

| Gate | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | |
| `cargo fmt --all -- --check` | |
| `python tools/lint_banned_words.py` (workspace door) | |
| `python tools/lint_git_history.py` (history door) | |
| `python tools/lint_spec_map.py` | |
| `python tools/lint_file_length.py` | |
| `python tools/check_docs.py` | |
| `python tools/lint_dco.py` | |

## If this touches ports, wire schemas, licences or history

- Port trait or wire-schema change: **RFC first** (`docs/rfc/README.md`), two
  maintainer approvals. No RFC, no change.
- New dependency: licence must be inside the allow list
  (`deny.toml`); BSL/GPL/AGPL cannot enter, and CI asserts it.
- Anything that would delete history: stop and follow the deletion gates in
  `docs/governance.md`. `tools/history_floor.json` makes a blob-count decrease
  a hard failure.

## Contributor declarations

- [ ] Every commit is signed off (`git commit -s`).
- [ ] IP-clean declarations in `CONTRIBUTING.md` re-affirmed for this change:
      no code carried in from a previous employer or third-party codebase;
      design recorded against this repository's own specifications; anything
      re-implementing a known mechanism was written from the behaviour spec.

## Notes for the reviewer

<!-- What you would check first if you were reviewing this. Be explicit about
     the weakest part of your own change. -->
