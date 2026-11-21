# Releasing

[English](#releasing) | [简体中文](#中文发布流程)

## Pre-tag checklist (machine-checkable rows first)

| # | Item | Command / evidence |
|---|---|---|
| 1 | All gates green locally | `make verify` (fmt, clippy `-D warnings`, check, test, 12 repo gates, pytest, cross-cert) |
| 2 | Test counts match the docs | run `cargo test --workspace` and `python -m pytest python/tests -q`, then compare with README and `tests/README.md` — quoted counts drift, so re-verify before trusting them |
| 3 | Documentation contract | `python tools/check_docs.py` (bilingual pairing, links, cited commands) |
| 4 | Packaging dry-run | `cargo package --workspace --list --offline` |
| 5 | Supply chain | `cargo deny check` + `python tools/lint_deny_isolation.py` + `cargo cyclonedx --format json --spec-version 1.5 && python tools/check_sbom.py` |
| 6 | Provenance | `python tools/lint_dco.py`, `python tools/lint_git_history.py` (blob floor must not fall) |
| 7 | Leak doors | `python tools/lint_banned_words.py` over every text file |
| 8 | CHANGELOG | every wave and bench release has an entry; version single axis shared with the Python track |

## Decisions that block the first public tag

These are **not** code tasks; they need the repository/identity decision, so
they are listed instead of guessed:

| # | Blocked item | Why it cannot be filled in locally |
|---|---|---|
| A | ✅ DONE 2026-11-22 — `repository`/`homepage` landed in `[workspace.package]`, CITATION.cff repository-code filled (vico6885/TrueRise) | resolved by the org decision |
| B | CI badge URLs in `README.md` / `README.zh-CN.md` | same reason; snippet template below |
| C | `.github/ISSUE_TEMPLATE/config.yml` contact links | `about` entries require absolute URLs |
| D | `.github/CODEOWNERS` handle resolution | the handle is provisional; an unresolved file silently stops reviewer routing, so verify on the first real PR |
| E | crates.io name squatting decision (`truerise` + `truerise-*`) | one-way action: registration is not reversible in practice |
| F | PyPI `truerise` / `truerise-evals` registration | same asymmetry |
| G | Trademark database manual check | still open from M0 |
| H | the first third-party model reading through the external-solver protocol | the protocol has LANDED (`run_external_trials`, executor-signed records); the reading itself is an operational step owned by `ROADMAP.md` item 1 — no adoption claim before it |

Badge snippet to paste once the remote exists (EN and CN READMEs, under the title):

```markdown
[![ci](https://github.com/<org>/<repo>/actions/workflows/ci.yml/badge.svg)](https://github.com/<org>/<repo>/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/truerise-core)](https://crates.io/crates/truerise-core)
[![License](https://img.shields.io/badge/License-Apache_2.0_OR_MIT-blue.svg)](LICENSE-APACHE)
```

## Publish order

1. push `main`, confirm CI green (the first authoritative run of the whole
   chain happens here — until then every gate result is a local
   transcription).
2. tag `v0.2.0` → `release.yml` runs `cargo dist build` (five targets,
   shell/powershell installers, SHA256).
3. `cargo publish` leaf-first: `truerise-wire`, `truerise-platform`,
   `truerise-core`, then `truerise-governance`, `truerise-sandbox`,
   `truerise-eval`, `truerise-memory`, `truerise-rsi`, `truerise-hats`,
   `truerise-mcp`, `truerise-observe`, `truerise-cli`. Flip
   `publish = false` per crate only in the release commit that publishes it.
4. `python -m build` + publish `truerise` to PyPI.
5. Announce with the reproducible path first: the 60-second command block,
   then `readings/v0.1/`, then the report. Never announce a number that has
   no committed artifact.

---

## 中文发布流程

**先机器项，后决定项。** 上方第一张表全部是命令，跑完再谈打 tag；`make verify` 是本仓与
CI 的本地对齐点（CI 额外只多 cargo-deny、SBOM、MSRV 1.85、criterion 基线四项）。

被打断的是第二类：仓库 URL、徽章、issue 表单联系方式、CODEOWNERS 句柄、crates.io 与 PyPI
占名、商标核查。这些**不能就地猜** —— 尤其是包名与 URL：注册不可逆，把 crate 发布到一个
无人拥有的地址，比不发布更糟。所以它们列成阻塞项，等决定而非编造。

发布顺序要点：先 push 让 CI 第一次真跑（在此之前每条门禁结果都只是本地转录），再打 tag 触发 cargo-dist，然后按叶子优先逐个 crate 发布，`publish = false` 只在
真正发布它的那个 commit 里翻。最后对外宣布时，第一段永远是 60 秒命令块与 `readings/v0.1/`，
**不宣布任何没有落盘产物的数字**。

## Status after the P0-P3 construction waves (2026-11-16)

- A/B/C/D (org URL / badges / issue links / CODEOWNERS): still blocked on
  the GitHub org decision - unchanged, account-side.
- E (crates.io `truerise*`): APPROVED (occupy), execution at first publish.
- F (PyPI `truerise` / `truerise-native`): the second package name now
  exists (python-native/pyproject.toml); register BOTH at publish.
- G (trademark manual check): unchanged (account-side).
- H (external solver protocol): IMPLEMENTED (W11a: run_external_trials +
  executor-signed records); the first public-model reading is operational
  (an API key + a solver script honouring the contract) - see ROADMAP.md.
- NEW since this file was written: action SHAs are pinned (W18);
  CITATION.cff + ROADMAP.md exist; the claim gate holds the README title
  to the evidence ledger (5/5 met).
