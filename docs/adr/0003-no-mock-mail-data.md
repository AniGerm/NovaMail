# ADR 0003 — No Mock Mail Data in the Product UI

## Status

Accepted

## Context

Browser-only Vite previews previously injected fabricated messages so designers could see the Unified Inbox without IMAP. That conflicts with the product rule that NovaMail never shows placeholder or mock mail content.

## Decision

- The UI never fabricates accounts, messages, or bodies.
- Outside the Tauri shell, NovaMail shows an empty state instructing the user to run `pnpm dev`.
- Inside the shell with zero accounts, the welcome empty state prompts real account setup.
- Automated tests may use in-memory SQLite fixtures; the shipping UI must not.

## Consequences

- Design iteration for the inbox requires either the desktop shell or Storybook-style isolated component stories (future), not fake inbox rows in `AppShell`.
