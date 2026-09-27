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
| Plugin escape | WASM plugin runtime is a **scaffold** (not shipped as a product surface yet) |
| OAuth interception | localhost callback bound to 127.0.0.1 only |
| OAuth token expiry | Automatic refresh before IMAP/SMTP when `expires_at` near |
| Open CardDAV / LDAP on LAN | **Trusted LAN only**; HTTP Basic auth (generated password in settings) — not for public internet |
| Supply-chain updates | Signed releases (packaging phase) |

## Out of scope for MVP

- Remote image proxy
- Multi-user OS profile isolation beyond keyring
- Production-hardened CardDAV/LDAP exposure beyond the local network
- Broad automated test coverage (expanding)
