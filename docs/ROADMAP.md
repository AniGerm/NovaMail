# Roadmap

## MVP (current)

- [x] Tauri-Shell + Nova Design System (Light/Dark/HC + density)
- [x] Account-Setup: IMAP/SMTP manuell + OAuth browser callback
- [x] Sync Inbox/Sent, Message List + Reading Pane
- [x] Compose/Send, Reply/Forward
- [x] Multi-Account + Unified Inbox
- [x] SQLite Persistenz + Offline Lesen
- [x] Globale Suche (FTS5)
- [x] Keyboard Shortcuts + Basis-A11y (Command Palette, live regions)
- [x] Secret Service Integration
- [x] Background sync scheduler (5 min)
- [x] HTML sanitization + AI summarize/suggest
- [x] Deb/AppImage packaging config (`scripts/package-linux.sh`, Tauri bundle)
- [x] CI workflow + issue templates + Code of Conduct
- [x] Attachments: IMAP sync, open, compose attach
- [x] iCloud preset + app-password hint
- [x] Address book + embedded CardDAV server + LDAP import
- [x] Quick Sort (Keep/Delete/Preview + hotkeys)
- [x] Signatures, Labels UI, Rules UI, POP3 test

## Production phases

| Phase | Fokus |
|-------|--------|
| P1 | Threading polish, attachment DnD polish, Smart Search |
| P2 | POP3 full mailbox sync, signature HTML templates |
| P3 | OpenPGP (Sequoia), Key Management UI |
| P4 | Calendar (CalDAV) |
| P5 | Plugin Runtime WASM + erste Official Plugins |
| P6 | ~~AI job queue + persisted insights~~ (done: qwen2.5:1.5b, ai_insights, 2 reply variants) |
| P7 | Microsoft Graph / Exchange Vertiefung |
| P8 | Flatpak, Snap, automatic signed Updates, Telemetrie opt-in |

Performance-Gates pro Release: Cold start p95 < 2s, Idle RSS < 300 MB, List-scroll < 100 ms.
