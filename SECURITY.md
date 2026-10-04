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

## LAN directory hub

When address-book share mode is **Server**, NovaMail binds LDAP (`0.0.0.0:1389`, optionally `:389`) and CardDAV (`0.0.0.0:8765`) for phones, MFPs, and sibling NovaMail PCs.

These listeners are **cleartext** (`ldap://` / `http://` with HTTP Basic). Traffic on a shared Wi‑Fi can be sniffed; there is no TLS on the hub today. Use the hub only on a **trusted LAN** (office or home). Do not port-forward or expose 1389/389/8765 to the public internet. For remote access, put clients on a VPN into that LAN.

Credentials shown in Settings are shared secrets for LAN devices — treat them like a local Wi‑Fi password, not like an internet-facing account.
