# Linux Killer Features — Plan & Checkliste

Ziel: Funktionen, die NovaMail auf Linux klar von Thunderbird/Evolution/Geary abheben — lokal-first, keyboard-first, ohne Cloud-Lock-in.

## Übersicht

| Phase | Feature | Status |
|-------|---------|--------|
| 1 | Snooze + Send Later + Sidebar „Geplant“ | erledigt |
| 2 | Command Palette Expansion (Aktionen auf Auswahl) | offen |
| 3 | OpenPGP (Sequoia) + Key-UI | offen |
| 4 | CalDAV Kalender + Tasks | offen |

---

## Phase 1 — Snooze & Send Later

Lokale SQLite-Steuerung (kein IMAP-Label nötig). Job-Tick alle ~60s weckt Snoozes und sendet fällige Outbound-Mails.

### Backend

- [x] Migration v6: `snoozed_messages`, `outbound_queue`
- [x] Repository: snooze / unsnooze / list / due; enqueue / cancel / list outbound
- [x] Core: `resolve_snooze_wake`, `enqueue_send_later`, `run_jobs_tick`
- [x] IPC DTOs (`jobs.rs`) + Tauri commands
- [x] Background jobs loop (~60s) + `jobs_tick` event
- [x] List filter: `snoozed_only`; aktive Snoozes aus Normal-Inbox ausblenden

### Frontend

- [x] Sidebar-Eintrag **Geplant** (Badge = snoozed + pending outbound)
- [x] `PlannedPanel`: Snoozed + Send-Later-Liste, Unsnooze / Abbrechen
- [x] ReadingPane: Snooze-Menü (Später heute / Morgen früh / Nächsten Montag)
- [x] Composer: **Später senden** mit Presets
- [x] Command Palette: Geplant öffnen, Snooze-Presets
- [x] Shortcut `h` → Snooze „Später heute“
- [x] i18n DE/EN für alle neuen Strings
- [x] MessageList-Badge „Zurückgestellt“

### Docs / Qualität

- [x] README: Snooze / Send Later / Geplant
- [x] ROADMAP aktualisieren
- [x] `cargo test` + Frontend-Typecheck
- [x] Commit + Push Branch `cursor/snooze-send-later-0350`

---

## Phase 2 — Command Palette Expansion

- [ ] Aktionen auf aktuelle Nachricht: Archive, Delete, Star, Snooze, Spam, Label
- [ ] Schnellfilter: Ungelesen / Favoriten / Konto wechseln
- [ ] Fuzzy-Suche über Kontakte + kürzliche Empfänger
- [ ] „Geplant“ / „Offline“ / „Drafts“ als Palette-Ziele

---

## Phase 3 — OpenPGP (Sequoia)

- [ ] Sequoia-Integration (sign / encrypt / decrypt / verify)
- [ ] Keyring-Import (Datei / Clipboard) + Key-Management-UI
- [ ] Compose: Signieren / Verschlüsseln Toggle
- [ ] ReadingPane: Signaturstatus + Entschlüsselung
- [ ] Autocrypt-light optional (später)

---

## Phase 4 — CalDAV + Tasks

- [ ] CalDAV-Account + Sync (Kalender + VTODO)
- [ ] Tages-/Wochenansicht im Shell
- [ ] „Als Termin“ aus Mail; Deadline aus Mail → Task
- [ ] Sidebar-Eintrag Kalender / Aufgaben

---

## Akzeptanzkriterien Phase 1

1. Snooze entfernt Mail aus dem normalen Posteingang bis `wake_at`.
2. Fällige Snoozes erscheinen nach Job-Tick wieder.
3. Send Later speichert Outbound; Versand zum `send_at` (App muss laufen).
4. Sidebar **Geplant** zeigt beide Listen; Abbrechen / Unsnooze funktioniert.
5. UI auf Deutsch und Englisch vollständig.
