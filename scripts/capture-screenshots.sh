#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/docs/screenshots"
PORT=8766
PROFILE="$(mktemp -d /tmp/novamail-chrome.XXXXXX)"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$PROFILE"
}
trap cleanup EXIT

cd "$OUT"
python3 -m http.server "$PORT" >/tmp/novamail-screenshot-server.log 2>&1 &
SERVER_PID=$!
sleep 1

capture() {
  local view="$1"
  local file="$2"
  timeout 45 google-chrome \
    --headless=new \
    --disable-gpu \
    --no-sandbox \
    --user-data-dir="$PROFILE" \
    --window-size=1440,900 \
    --screenshot="$OUT/$file" \
    "http://127.0.0.1:${PORT}/preview.html?view=${view}" \
    >/tmp/novamail-chrome-shot.log 2>&1 || {
      echo "chrome capture failed for $file (see /tmp/novamail-chrome-shot.log)" >&2
      return 1
    }
  echo "wrote $file"
}

capture inbox inbox.png
capture triage quick-sort.png
capture contacts contacts-carddav.png
