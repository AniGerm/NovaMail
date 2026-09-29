#!/usr/bin/env bash
# Launch NovaMail in the Cloud Desktop / local Linux shell.
set -uo pipefail

# Resolve through symlinks (e.g. ~/.local/bin/novamail -> scripts/launch-novamail.sh).
SCRIPT_PATH="${BASH_SOURCE[0]}"
if command -v readlink >/dev/null 2>&1; then
  SCRIPT_PATH="$(readlink -f "$SCRIPT_PATH" 2>/dev/null || echo "$SCRIPT_PATH")"
fi
ROOT="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
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

list_novamail_pids() {
  local proc exe
  for proc in /proc/[0-9]*; do
    exe="$(readlink "$proc/exe" 2>/dev/null || true)"
    if [[ "$exe" == "$BIN" || "$exe" == *"/novamail-desktop" ]]; then
      printf '%s\n' "${proc##*/}"
    fi
  done
}

pid_alive() {
  local pid="$1"
  [[ -n "$pid" && -d "/proc/$pid" ]] || return 1
  local exe
  exe="$(readlink "/proc/$pid/exe" 2>/dev/null || true)"
  [[ "$exe" == "$BIN" || "$exe" == *"/novamail-desktop" ]]
}

# Drop X11 windows whose owning process is already gone (looks like a hung app).
cleanup_orphan_windows() {
  command -v wmctrl >/dev/null 2>&1 || return 0
  command -v xprop >/dev/null 2>&1 || return 0
  local line wid pid
  while read -r line; do
    wid="$(awk '{print $1}' <<<"$line")"
    [[ -n "$wid" ]] || continue
    pid="$(xprop -id "$wid" _NET_WM_PID 2>/dev/null | awk -F'= ' '{print $2}' | tr -d ' ')"
    if [[ -z "$pid" ]] || ! pid_alive "$pid"; then
      echo "Closing orphan NovaMail window $wid (pid=${pid:-none})." >>"$LOG_FILE"
      wmctrl -ic "$wid" 2>/dev/null || true
      if command -v xdotool >/dev/null 2>&1; then
        xdotool windowkill "$wid" 2>/dev/null || true
      fi
    fi
  done < <(wmctrl -l 2>/dev/null | grep -i 'NovaMail' || true)
}

focus_live_novamail_window() {
  command -v wmctrl >/dev/null 2>&1 || return 1
  local line wid pid
  while read -r line; do
    wid="$(awk '{print $1}' <<<"$line")"
    pid="$(xprop -id "$wid" _NET_WM_PID 2>/dev/null | awk -F'= ' '{print $2}' | tr -d ' ')"
    if pid_alive "$pid"; then
      wmctrl -ia "$wid" 2>/dev/null && return 0
    fi
  done < <(wmctrl -l 2>/dev/null | grep -i 'NovaMail' || true)
  if command -v xdotool >/dev/null 2>&1; then
    local cand
    for cand in $(xdotool search --name NovaMail 2>/dev/null || true); do
      pid="$(xprop -id "$cand" _NET_WM_PID 2>/dev/null | awk -F'= ' '{print $2}' | tr -d ' ')"
      if pid_alive "$pid"; then
        xdotool windowactivate "$cand" 2>/dev/null && return 0
      fi
    done
  fi
  return 1
}

cleanup_orphan_windows

mapfile -t RUNNING_PIDS < <(list_novamail_pids)
if ((${#RUNNING_PIDS[@]} > 0)); then
  if focus_live_novamail_window; then
    echo "NovaMail already running — focused existing window (pids: ${RUNNING_PIDS[*]})." >>"$LOG_FILE"
    exit 0
  fi
  echo "NovaMail process(es) without usable window — restarting (${RUNNING_PIDS[*]})." >>"$LOG_FILE"
  for pid in "${RUNNING_PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  sleep 0.4
  for pid in "${RUNNING_PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  done
  sleep 0.2
  cleanup_orphan_windows
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
