# Linux Killer Features — Plan & Checkliste

Ziel: Funktionen, die NovaMail auf Linux klar von Thunderbird/Evolution/Geary abheben — lokal-first, keyboard-first, ohne Cloud-Lock-in.

## Übersicht

| Phase | Feature | Status |
|-------|---------|--------|
| 1 | Snooze + Send Later + Sidebar „Geplant“ | erledigt |
| 2 | Command Palette Expansion (Aktionen auf Auswahl) | erledigt |
| 3 | OpenPGP (Sequoia) + Key-UI | erledigt |
| 4 | CalDAV Kalender + Tasks | erledigt |

---

## Phase 1 — Snooze & Send Later

- [x] Migration v6, Jobs, IPC, UI, i18n, README

## Phase 2 — Command Palette Expansion

- [x] Aktionen auf aktuelle Nachricht: Archive, Delete, Star, Snooze, Spam, Label
- [x] Schnellfilter: Ungelesen / Favoriten / Konto wechseln
- [x] Fuzzy-Suche über Kontakte + kürzliche Empfänger
- [x] „Geplant“ / „Offline“ / „Drafts“ / „Kalender“ als Palette-Ziele

## Phase 3 — OpenPGP (Sequoia)

- [x] Sequoia-Integration (sign / encrypt / decrypt / verify)
- [x] Keyring-Import (Datei/Clipboard-Text) + Key-Management-UI
- [x] Compose: Signieren / Verschlüsseln Toggle
- [x] ReadingPane: Signaturstatus + Entschlüsselung
- [x] Autocrypt-light optional (später) — bewusst zurückgestellt

## Phase 4 — CalDAV + Tasks

- [x] CalDAV-Account + Sync (Kalender + VTODO)
- [x] Tages-/Wochenansicht im Shell
- [x] „Als Termin“ aus Mail; Deadline aus Mail → Task
- [x] Sidebar-Eintrag Kalender / Aufgaben

## Docs

- [x] README mit allen Killer-Features
- [x] ROADMAP aktualisiert

## Follow-ups (erledigt)

- [x] AI-Terminvorschläge aus Ollama als klickbare Links
- [x] CalDAV Discovery + PUT/DELETE Write-back (Nextcloud/generic; Google OAuth später)
