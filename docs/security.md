# Threat Model (MVP)

## Assets

- IMAP/SMTP credentials and OAuth tokens
- Local mail bodies and attachments
- User address book metadata

## Trust boundaries

1. Untrusted network (IMAP/SMTP/OAuth/HTML content)
2. Local OS keyring
3. Tauri WebView (UI)
4. Optional localhost Ollama

## Mitigations

| Threat | Mitigation |
|--------|------------|
| Credential theft from DB | Secrets only in OS keyring |
| HTML XSS from mail | ammonia sanitization before IPC |
| Plugin escape | WASM + capability manifests (scaffold) |
| OAuth interception | localhost callback bound to 127.0.0.1 only |
| OAuth token expiry | Automatic refresh before IMAP/SMTP when `expires_at` near |
| Open CardDAV on LAN | HTTP Basic auth (generated password in settings) |
| Supply-chain updates | Signed releases (packaging phase) |

## Out of scope for MVP

- Full OpenPGP key lifecycle UI
- Remote image proxy
- Multi-user OS profile isolation beyond keyring
