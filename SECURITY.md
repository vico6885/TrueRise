# Security Policy

## Supported versions

The project has not released yet; security fixes target `main`.

## Reporting a vulnerability

Do NOT open a public issue for security reports. Use GitHub's **private
vulnerability reporting** on this repository (Security tab → "Report a
vulnerability") — it is enabled for this project. If the switch is
unavailable for any reason, open a
minimal issue titled "security: request private contact" without technical
details.

## Scope notes

- Secrets (API keys, tokens) must never enter the repository, logs, argv, or
  config files; the onboarding flow stores them in the OS keyring or the
  environment only.
- The audit hash chain is tamper-evident; suspected tampering is a security
  incident, not a data bug.
