# schemas

Wire-schema JSON files (single authoritative source for the JSON surface;
the wire crate keeps the same contracts explicit at the Rust type level from
M1).

Current content: `audit_event.v2.schema.json` — the tightened audit-chain
record contract (7 fields, 3 chain-critical required keys, no extra
fields), asserted by `tools/lint_schemas.py` against `INDEX.json` at spec
level (fields / required / enums / additionalProperties — never mere file
references). The legacy v1 edition lands alongside in M1 (double-version
rule, explicit at the type level; legacy reads stay valid but are excluded
from governance).
