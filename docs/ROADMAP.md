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

## Production phases

| Phase | Fokus |
|-------|--------|
| P1 | Threading polish, labels/tags UI, attachments DnD, Smart Search |
| P2 | Rules UI (engine crate ready), signatures/templates, POP3 |
| P3 | OpenPGP (Sequoia), Key Management UI |
| P4 | Contacts + Calendar (CalDAV) |
| P5 | Plugin Runtime WASM + erste Official Plugins |
| P6 | AI job queue + persisted insights |
| P7 | Microsoft Graph / Exchange Vertiefung |
| P8 | Flatpak, Snap, automatic signed Updates, Telemetrie opt-in |

Performance-Gates pro Release: Cold start p95 < 2s, Idle RSS < 300 MB, List-scroll < 100 ms.
