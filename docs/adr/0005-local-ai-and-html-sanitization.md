# ADR 0005 — Local AI + HTML Sanitization

## Status

Accepted

## Context

NovaMail needs AI assist (summaries, reply drafts) without sending mail to third-party clouds by default. HTML bodies from IMAP are untrusted.

## Decision

1. Primary AI provider is **Ollama** over `http://127.0.0.1:11434` (`/api/generate`).
2. If Ollama is unreachable, fall back to **NullAiProvider** (deterministic extractive/heuristic helpers) and report the provider name to the UI.
3. `get_message` always runs HTML through **ammonia** before IPC.
4. Frontend may render sanitized HTML via `dangerouslySetInnerHTML` only for that core-sanitized field.

## Consequences

- Users can run fully offline AI when Ollama + a model are installed.
- Reply suggestions and summaries never leave the machine unless a future cloud provider is explicitly opted in.
