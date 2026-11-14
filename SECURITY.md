# Security Policy

## Supported versions

The project has not released yet; security fixes target `main`.

## Reporting a vulnerability

Do NOT open a public issue for security reports. The private reporting
contact will be published with the first public release; until then, open a
minimal issue titled "security: request private contact" without technical
details.

## Scope notes

- Secrets (API keys, tokens) must never enter the repository, logs, argv, or
  config files; the onboarding flow stores them in the OS keyring or the
  environment only.
- The audit hash chain is tamper-evident; suspected tampering is a security
  incident, not a data bug.
