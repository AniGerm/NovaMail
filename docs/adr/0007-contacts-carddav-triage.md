# ADR 0007 — Contacts CardDAV, LDAP, Attachments, Quick Triage

## Status

Accepted

## Context

NovaMail needed a usable address book for MFP/fax appliances, attachment
round-trips, iCloud setup, and a Yahoo-style keep/delete triage flow.

## Decision

1. **Embedded CardDAV** (`novamail-contacts`) listens on `127.0.0.1:8765` and
   serves `/addressbooks/novamail/` as vCard 3.0 resources backed by SQLite.
2. **LDAP/LDAPS** lookup imports `inetOrgPerson` entries into the local book.
3. **Attachments** are stored under the app blobs directory during IMAP sync,
   exposed on `MessageDetailDto`, openable via the shell plugin, and sendable
   as base64 MIME parts through SMTP.
4. **Quick Sort** is a modal triage queue with Keep / Delete / Preview and
   hotkeys `K`, `D`, `Space`, `Esc`.
5. **iCloud** is a first-class IMAP/SMTP preset that requires an Apple
   app-specific password.
6. **Signatures, labels, rules, POP3 test** land in settings / IPC so the
   product is usable beyond the MVP reading pane.

## Consequences

- Printers can sync contacts without a separate CardDAV host.
- Local blobs grow with attachment sync; deletes remove blob files.
- POP3 is connectivity + list/retr foundation; full POP3 sync into mailboxes
  can deepen later without changing the IPC surface.
