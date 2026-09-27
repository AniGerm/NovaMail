#!/usr/bin/env bash
# Launch NovaMail in the Cloud Desktop / local Linux shell.
set -uo pipefail

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
  source "${HOME}/.nvm/nvm.sh" || true
fi

export DISPLAY="${DISPLAY:-:1}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/novamail"
mkdir -p "$LOG_DIR"
LOG_FILE="${LOG_DIR}/launcher.log"

# Prefer the already-built debug binary (embeds apps/desktop/dist).
# Note: a plain `cargo build` does NOT load Vite — only `tauri dev` sets cfg(dev).
BIN="${ROOT}/target/debug/novamail-desktop"
DESKTOP_DIR="${ROOT}/apps/desktop"
DIST_JS="$(ls -1t "${DESKTOP_DIR}/dist/assets"/index-*.js 2>/dev/null | head -1 || true)"

frontend_stale() {
  [[ -n "$DIST_JS" && -f "$DIST_JS" ]] || return 0
  if find "${DESKTOP_DIR}/src" "${DESKTOP_DIR}/index.html" "${DESKTOP_DIR}/public" \
      -type f -newer "$DIST_JS" 2>/dev/null | grep -q .; then
    return 0
  fi
  return 1
}

binary_stale() {
  [[ -x "$BIN" ]] || return 0
  [[ -n "$DIST_JS" && -f "$DIST_JS" ]] || return 0
  [[ "$DIST_JS" -nt "$BIN" ]] && return 0
  return 1
}

# Avoid stacking multiple instances (looks like a hang / crash on second launch).
# Match by /proc/*/exe so we never kill/match this launcher shell.
already_running=0
for proc in /proc/[0-9]*; do
  exe="$(readlink "$proc/exe" 2>/dev/null || true)"
  if [[ "$exe" == "$BIN" || "$exe" == *"/novamail-desktop" ]]; then
    already_running=1
    break
  fi
done
if ((already_running)); then
  echo "NovaMail already running — focusing existing window." >>"$LOG_FILE"
  if command -v wmctrl >/dev/null 2>&1; then
    wmctrl -a NovaMail 2>/dev/null || true
  fi
  exit 0
fi

if frontend_stale; then
  echo "Rebuilding desktop frontend (src newer than dist)..." >>"$LOG_FILE"
  if ! (cd "$DESKTOP_DIR" && pnpm build) >>"$LOG_FILE" 2>&1; then
    echo "Frontend rebuild failed — launching existing binary if present." >>"$LOG_FILE"
  fi
  DIST_JS="$(ls -1t "${DESKTOP_DIR}/dist/assets"/index-*.js 2>/dev/null | head -1 || true)"
fi

if binary_stale; then
  echo "Rebuilding novamail-desktop (dist newer than binary)..." >>"$LOG_FILE"
  if ! cargo build -p novamail-desktop >>"$LOG_FILE" 2>&1; then
    echo "Binary rebuild failed — launching existing binary if present." >>"$LOG_FILE"
  fi
fi

if [[ -x "$BIN" ]]; then
  exec "$BIN" >>"$LOG_FILE" 2>&1
fi

# Fallback: full Tauri dev (builds if needed).
exec pnpm --filter @novamail/desktop tauri:dev >>"$LOG_FILE" 2>&1
