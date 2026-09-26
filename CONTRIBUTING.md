# Contributing to NovaMail

## Principles

Follow [`docs/ENGINEERING.md`](docs/ENGINEERING.md) and [ADR 0004](docs/adr/0004-engineering-standards.md):

- No placeholders in product/IPC paths
- Missing features get a first real implementation
- No mock mail data in the UI
- Prefer improving existing modules over creating duplicates
- Document architectural decisions in `docs/adr/`
- Add tests for db/mail parsing and pure domain logic
- Keep secrets out of the repo

## Development

```bash
pnpm install
cargo test --workspace
pnpm typecheck
pnpm dev
```

## Code style

- Rust: `cargo fmt`, idiomatic error types via `thiserror`
- TypeScript: strict mode, no unused locals
- Comments only where intent is non-obvious

## License

By contributing you agree your changes are licensed under MPL-2.0.
