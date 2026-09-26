# ADR 0006 — MVP Completion Scope

## Status

Accepted

## Context

The architecture plan defines an MVP that is shippable for Ubuntu: real mail sync,
unified inbox, compose, search, keyring auth, OAuth, packaging hooks, and a11y basics.

## Decision

MVP includes:

- Schema v2 tables for future phases (attachments/labels/rules/…) even before full UI
- `novamail-rules` engine crate with evaluate API
- Background `SyncScheduler` every 5 minutes
- OAuth localhost callback listener on `127.0.0.1:17832`
- Forward drafts, archive flag, command palette, settings density
- GitHub CI + Code of Conduct + packaging script

Deferred to production phases: POP3 UI, OpenPGP, CalDAV, WASM plugins, attachment DnD UI.

## Consequences

- Database is forward-compatible with P1–P5 features
- Users can run a complete local mail loop today with password or OAuth accounts
