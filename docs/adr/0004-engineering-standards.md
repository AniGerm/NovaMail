# ADR 0004 — Lead Engineering Standards

## Status

Accepted

## Context

NovaMail is built as a production open-source client, not a prototype. Incomplete surfaces and fabricated content erode trust and create architectural debt.

## Decision

1. **No placeholders** in shipping code paths. If a surface is exposed to users or IPC, it must do real work or return a typed, actionable error.
2. **Missing features get a first real version**, not a stub that only logs “coming soon”.
3. **No mock mail data** in the product UI (see ADR 0003). Tests may use fixtures; the app must not.
4. **Production-ready defaults**: typed errors, input validation, secure credential handling, deterministic migrations.
5. **Improve existing files** before creating new ones; avoid duplicate modules for the same concern.
6. **Architecture stays hexagonal**: UI → Tauri commands → `novamail-core` → adapters (`db`, `mail`, `crypto`, `ai`, `plugins`).
7. **Every non-trivial decision** is recorded under `docs/adr/`.

## Consequences

- `OllamaProvider` must call a live Ollama endpoint (or fail with `AiError`).
- HTML mail is sanitized before leaving the core.
- OAuth helpers perform real authorize-URL + token-exchange steps when credentials are configured.
