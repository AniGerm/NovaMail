#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export PATH="$(rustup which cargo 2>/dev/null | xargs dirname 2>/dev/null || true):${PATH}"

pnpm install
pnpm --filter @novamail/desktop tauri build

echo "Artifacts under apps/desktop/src-tauri/target/release/bundle/"
