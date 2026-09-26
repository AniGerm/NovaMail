# NovaMail Architecture

## Layers

```
apps/desktop (Tauri + React)
        │ invoke / events
        ▼
novamail-core          use-cases
   ├── novamail-db     SQLite repositories
   ├── novamail-mail   IMAP/SMTP/OAuth
   ├── novamail-crypto keyring secrets
   ├── novamail-search FTS helpers
   ├── novamail-ai     provider traits
   └── novamail-plugins WASM registry scaffold
```

## Unified Inbox

`messages_list({ unified: true })` joins all mailboxes whose `role = inbox` (or name `INBOX`) across accounts and sorts by `date DESC`. Virtualization in the React list keeps scrolling under 100 ms for large result sets.

## Sync

`SyncEngine` connects per account, lists mailboxes, syncs Inbox/Sent with a rolling UID window (100), parses RFC822 via `mail-parser`, and upserts threads by `In-Reply-To` / `References`.

## Frontend state

- Server state: TanStack Query over Tauri commands
- UI state: Zustand (selection, theme, composer)
- Keyboard: `@novamail/hooks` shortcut map (`c`, `r`, `/`, `Ctrl/Cmd+K`)

## AI + HTML safety

- Primary provider: Ollama (`novamail-ai::OllamaProvider`), default model `qwen2.5:1.5b`
- Offline fallback: `NullAiProvider` (deterministic heuristics)
- Background queue after sync writes `ai_insights` (summary + reply draft)
- Commands: `ai_summarize_message`, `ai_suggest_reply`, `ai_message_insights`
- `AppState::get_message` sanitizes HTML with ammonia before IPC (ADR 0005)

## Engineering rules

See [`ENGINEERING.md`](ENGINEERING.md) and ADR 0004.
