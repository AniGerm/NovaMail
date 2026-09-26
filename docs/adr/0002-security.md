# ADR 0002 — Security Baseline

## Status

Accepted

## Context

Mail clients handle credentials, OAuth tokens, and untrusted HTML. NovaMail must avoid plaintext credential storage and remote-content surprises.

## Decision

1. Passwords and OAuth tokens live only in the OS keyring (memory fallback for headless CI).
2. SQLite stores metadata and message bodies, never secrets.
3. Tauri CSP denies remote script execution; remote images are opt-in (phase 2).
4. HTML bodies are sanitized before any future rich rendering (`ammonia` planned in reading pipeline).
5. Capability allowlists for plugins are mandatory before WASM execution is enabled.
6. Updates will be signed; interim distribution is via trusted GitHub Releases.

## Consequences

- Account add requires a live IMAP login before credentials are persisted.
- OAuth client IDs are injected via environment variables, never committed.
