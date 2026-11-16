# Documentation index

[English](README.md) | [简体中文](README.zh-CN.md)

Every claim in this tree is enforced by code and checked by a gate; nothing
here is advisory. If a document and the code disagree, the code is right and
the document is a defect — open an issue.

## Start here (10 minutes)

| Read | Audience | What you get |
|---|---|---|
| [`quickstart.md`](quickstart.md) | first-time evaluator | install, first run, first verdict, first tamper drill |
| [`cli.md`](cli.md) | everyone | the single authoritative list of subcommands and flags |
| [`anti_cheat_bench.md`](anti_cheat_bench.md) | evaluation engineers | the bench protocol: pack format, sealed taxonomy, isolation, certificate schema |

## Understand the design

| Read | Covers |
|---|---|
| [`arch.md`](arch.md) | hexagonal ports-and-adapters, crate map, the three books |
| [`governance.md`](governance.md) | audit hash chain as the single authority, three-state policy, approvals, deletion gates |
| [`timesafety.md`](timesafety.md) | the four leak vectors and the tests that enforce each |
| [`topology.md`](topology.md) | repository topology, dependency direction, license isolation rules |
| [`determinism.md`](determinism.md) | the three scoped determinism claims + the honest boundary (pinned + taped model steps, the float rule) |
| [`integrations.md`](integrations.md) | the three ecosystem surfaces: MCP, the Python SDK, the external-solver protocol |
| [`report/anti_cheat_bench_v0.1.md`](report/anti_cheat_bench_v0.1.md) | the v0.1 measurements and what they do NOT show |

## Contribute

| Read | Contribution unit |
|---|---|
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | DCO + IP-clean declarations + engineering rules |
| [`task_pack_guide.md`](task_pack_guide.md) | one task pack = one PR (the main funnel) |
| [`provider_conformance.md`](provider_conformance.md) | a sandbox provider + its conformance verdict |
| [`rfc/README.md`](rfc/README.md) | port-trait and wire-schema changes require an RFC |
| [`../RELEASING.md`](../RELEASING.md) | release checklist plus the decisions that block the first tag |
| [`../ROADMAP.md`](../ROADMAP.md) | what shipped (evidence-backed) and what is next |

RFCs are English-authoritative: a translation is a reading aid, the reviewed
text is the English file (that is what `docs/rfc/README.md` requires
maintainers to approve).

## Bilingual convention

User-facing documents ship as a pair: `name.md` (English, authoritative) and
`name.zh-CN.md` (简体中文, same structure and same numbers). Each pair cross-links
in its header. Design notes (`arch`, `governance`, `timesafety`, `topology`)
carry both languages inline in one file.

The pairing is machine-checked: `python tools/check_docs.py` fails CI when a
registered document loses its counterpart, when a header link is missing, or
when a document references a path that does not exist.

## Where the evidence lives

| Surface | Path |
|---|---|
| Committed reference readings | `readings/v0.1/*.json`, `readings/v0.2/*.json` (+ the evolve tape) |
| Certificate format under review | `rfc/0001-claim-certificate.md` |
| Gate commands (all runnable locally) | `../Makefile` `gates` target, `.github/workflows/ci.yml` |
