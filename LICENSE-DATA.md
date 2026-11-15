# Data license — task packs

The **code** in this repository is dual-licensed Apache-2.0 / MIT
(see `LICENSE-APACHE`, `LICENSE-MIT`). The **task packs** (the JSONL surfaces
under `readings/` and anything produced by `truerise.evals.generate_pack`)
are DATA and are licensed differently:

## Task packs: CC-BY-4.0

Task packs are licensed under **Creative Commons Attribution 4.0
International (CC-BY-4.0)**. The authoritative legal text is
<https://creativecommons.org/licenses/by/4.0/legalcode.txt>.

In short: you may share and adapt the packs for any purpose (including
commercial) provided you give attribution to the TrueRise Authors and link
the license.

Why data and code are licensed separately (RFC 0001 §2 rationale): an
evaluation pack must support a two-layer protocol — a PUBLIC surface anyone
may study and redistribute, and SEALED answer surfaces whose integrity the
attribution requirement protects. Bundling both under a software license
would give the sealed surface no independent protection.

## Sealed surfaces

`answers.jsonl` rows carry the isolation canary; a pack whose sealed surface
has leaked (canary found outside the judge side) is considered compromised
and MUST NOT be used for adjudication — regenerate with a new seed instead.

## Contributing packs

Pack contributions follow `CONTRIBUTING.md` (DCO + IP-clean declarations)
and are accepted under CC-BY-4.0 for data plus Apache-2.0 / MIT for any
accompanying generator or judge code.
