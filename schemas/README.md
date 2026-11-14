# schemas

Wire-schema JSON files (single authoritative source). Intentionally empty
at M0: schemas land with the wire crate in M1. The marker file
`EMPTY_UNTIL_M1` keeps the schema gate fail-closed-but-explicit (it emits
a SKIP line that must be recorded in `docs/PROGRESS.md`).

M1 contract: `INDEX.json` lists every schema; each entry is verified by
field-level assertions (fields / required / enum), never by file references.
