# Changelog

All notable changes to TrueRise are documented here. Format based on
Keep a Changelog; versioning is a single axis shared with the Python-track
publications (the Rust core is the authoritative implementation).

## [0.2.0-dev.0] - unreleased

### Added (M0 — repository scaffold)

- Workspace with 12 `truerise-*` crates (skeletons, `publish = false`).
- CI: fmt / clippy (deny) / build / MSRV 1.85 / cargo-deny / SBOM / DCO /
  file-length gate / schema gate / timesafety placeholder job.
- Leak-prevention double doors: workspace banned-token gate + git
  history-object gate (token list stored hex-encoded, zero literals in-repo).
- License isolation: cargo-deny whitelist excludes the legacy Python-edition
  license (BSL-1.1) and copyleft GPL/AGPL, machine-asserted in CI.
- Governance docs: README, CONTRIBUTING (IP-clean declarations), SECURITY,
  GOVERNANCE, CODE_OF_CONDUCT, PROGRESS.
