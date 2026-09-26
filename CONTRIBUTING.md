# Contributing to NovaMail

## Principles

- Prefer small, reviewable PRs
- Keep secrets out of the repo
- Document architectural decisions in `docs/adr/`
- Add tests for db/mail parsing and pure domain logic

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
