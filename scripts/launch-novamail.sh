#!/usr/bin/env bash
# Launch NovaMail in the Cloud Desktop / local Linux shell.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# GUI launchers often have a minimal PATH — restore Node / Rust / local bins.
export PATH="${HOME}/.local/bin:/usr/local/cargo/bin:${HOME}/.cargo/bin:/usr/local/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:${PATH}"
if [[ -d "${HOME}/.nvm/versions/node" ]]; then
  NVM_NODE_BIN="$(ls -1d "${HOME}/.nvm/versions/node"/v*/bin 2>/dev/null | sort -V | tail -1 || true)"
  if [[ -n "${NVM_NODE_BIN}" ]]; then
    export PATH="${NVM_NODE_BIN}:${PATH}"
  fi
fi
if [[ -s "${HOME}/.nvm/nvm.sh" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.nvm/nvm.sh"
fi

export DISPLAY="${DISPLAY:-:1}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/novamail"
mkdir -p "$LOG_DIR"
LOG_FILE="${LOG_DIR}/launcher.log"

# Prefer the already-built debug binary + Vite when available (faster reopen).
BIN="${ROOT}/target/debug/novamail-desktop"
VITE_URL="http://127.0.0.1:1420"

is_up() {
  curl -fsS --max-time 1 "$VITE_URL" >/dev/null 2>&1
}

if [[ -x "$BIN" ]]; then
  if ! is_up; then
    # Start Vite in the background for the Tauri debug shell.
    (
      cd "${ROOT}/apps/desktop"
      exec pnpm exec vite --host 127.0.0.1 --port 1420
    ) >>"$LOG_FILE" 2>&1 &
    for _ in $(seq 1 40); do
      is_up && break
      sleep 0.25
    done
  fi
  if is_up; then
    exec "$BIN" >>"$LOG_FILE" 2>&1
  fi
fi

# Fallback: full Tauri dev (builds if needed).
exec pnpm --filter @novamail/desktop tauri:dev >>"$LOG_FILE" 2>&1
