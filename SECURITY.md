# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting

Report vulnerabilities via GitHub Security Advisories on the NovaMail repository.
Do not file public issues for credential or crypto vulnerabilities.

## Hardening checklist

- Secrets only in OS keyring
- Minimal Tauri capabilities
- CSP without remote scripts
- No telemetry by default
