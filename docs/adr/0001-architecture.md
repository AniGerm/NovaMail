# ADR 0001 — Core Architecture

## Status

Accepted

## Context

NovaMail must feel native on Ubuntu, start quickly, stay under 300 MB RAM, and keep mail data local. Electron was rejected because the memory budget is unrealistic for a multi-account mail client with local search.

## Decision

- Desktop shell: **Tauri 2**
- UI: **React + TypeScript + Tailwind + Nova design system**
- Domain/core: **Rust crates** with hexagonal boundaries
- Persistence: **SQLite + FTS5** via `rusqlite` (bundled)
- Secrets: **OS keyring** (`keyring` / Secret Service)
- Mail transport: **async-imap** + **lettre**
- License: **MPL-2.0**

## Consequences

- UI and native code communicate only through typed Tauri commands/events.
- Plugin and AI surfaces are traits first; WASM/Ollama land in later phases without rewriting the core.
- Packaging targets deb/AppImage first; Flatpak/Snap follow once permissions stabilize.
