# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting

Email security issues to `security@novamail.app` (placeholder) or open a private GitHub security advisory.

Do not file public issues for credential or crypto vulnerabilities.

## Hardening checklist

- Secrets only in OS keyring
- Minimal Tauri capabilities
- CSP without remote scripts
- No telemetry by default
