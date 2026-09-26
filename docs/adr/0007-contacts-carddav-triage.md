# ADR 0007 — Contacts CardDAV, LDAP, Attachments, Quick Triage

## Status

Accepted

## Context

NovaMail needed a usable address book for MFP/fax appliances, attachment
round-trips, iCloud setup, and a Yahoo-style keep/delete triage flow.

## Decision

1. **Embedded CardDAV** (`novamail-contacts`) listens on `0.0.0.0:8765` and
   serves `/addressbooks/novamail/` as vCard 3.0 resources backed by SQLite so
   phones, printers, fax MFPs, and other LAN devices share one address book on
   the main NovaMail PC. The UI advertises the machine’s LAN IP (not `0.0.0.0`).
   Access requires **HTTP Basic auth** (username `novamail` + generated password
   stored in settings). Encrypted `.nmbak` backups are for config migration
   between NovaMail installs, not day-to-day contact sync.
2. **LDAP/LDAPS sync** pulls `inetOrgPerson` entries (mail, phone, fax, address,
   photo, org/title) and upserts them by `ldap_dn`; CardDAV then redistributes
   those contacts. The contacts UI also supports fuzzy search, photos, fax,
   postal addresses, notes, and custom fields.
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
