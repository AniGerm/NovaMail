#!/usr/bin/env bash
# Install NovaMail into the Linux app menu and Plank dock (Cloud Desktop).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCHER="${ROOT}/scripts/launch-novamail.sh"
ICON_SRC="${ROOT}/apps/desktop/src-tauri/icons/icon.png"
MARK_SRC="${ROOT}/apps/desktop/public/novamail-mark.png"

ICON_NAME="novamail"
DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
BIN_DIR="${HOME}/.local/bin"
DESKTOP_FILE="${DESKTOP_DIR}/novamail.desktop"
DOCK_DIR="${HOME}/.config/plank/dock1/launchers"
DOCK_SETTINGS="${HOME}/.config/plank/dock1/settings"
DESKTOP_HOME="${HOME}/Desktop"

# German spellcheck dictionary for the composer (best-effort).
if command -v apt-get >/dev/null 2>&1; then
  if [[ ! -f /usr/share/hunspell/de_DE.dic ]]; then
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq hunspell-de-de >/dev/null 2>&1 || true
  fi
fi

chmod +x "$LAUNCHER"
mkdir -p "$DESKTOP_DIR" "$BIN_DIR" "$DESKTOP_HOME" "$DOCK_DIR" \
  "${ICON_DIR}/256x256/apps" \
  "${ICON_DIR}/128x128/apps" \
  "${ICON_DIR}/64x64/apps" \
  "${ICON_DIR}/48x48/apps" \
  "${ICON_DIR}/32x32/apps"

SRC="$ICON_SRC"
if [[ -f "$MARK_SRC" ]]; then
  SRC="$MARK_SRC"
fi

install_icons() {
  if command -v convert >/dev/null 2>&1; then
    convert "$SRC" -resize 256x256 "${ICON_DIR}/256x256/apps/${ICON_NAME}.png"
    convert "$SRC" -resize 128x128 "${ICON_DIR}/128x128/apps/${ICON_NAME}.png"
    convert "$SRC" -resize 64x64 "${ICON_DIR}/64x64/apps/${ICON_NAME}.png"
    convert "$SRC" -resize 48x48 "${ICON_DIR}/48x48/apps/${ICON_NAME}.png"
    convert "$SRC" -resize 32x32 "${ICON_DIR}/32x32/apps/${ICON_NAME}.png"
    return 0
  fi

  if python3 - "$SRC" "$ICON_DIR" "$ICON_NAME" <<'PY'
import sys
from pathlib import Path
try:
    from PIL import Image
except ImportError:
    sys.exit(2)
src, icon_dir, name = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
img = Image.open(src).convert("RGBA")
try:
    resample = Image.Resampling.LANCZOS
except AttributeError:
    resample = Image.LANCZOS
for size in (256, 128, 64, 48, 32):
    out = icon_dir / f"{size}x{size}" / "apps" / f"{name}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    img.resize((size, size), resample).save(out)
PY
  then
    return 0
  fi

  cp -f "${ROOT}/apps/desktop/src-tauri/icons/henry.w@example.net" \
    "${ICON_DIR}/256x256/apps/${ICON_NAME}.png" 2>/dev/null \
    || cp -f "$SRC" "${ICON_DIR}/256x256/apps/${ICON_NAME}.png"
  cp -f "${ROOT}/apps/desktop/src-tauri/icons/128x128.png" \
    "${ICON_DIR}/128x128/apps/${ICON_NAME}.png"
  cp -f "${ROOT}/apps/desktop/src-tauri/icons/128x128.png" \
    "${ICON_DIR}/64x64/apps/${ICON_NAME}.png"
  cp -f "${ROOT}/apps/desktop/src-tauri/icons/32x32.png" \
    "${ICON_DIR}/48x48/apps/${ICON_NAME}.png"
  cp -f "${ROOT}/apps/desktop/src-tauri/icons/32x32.png" \
    "${ICON_DIR}/32x32/apps/${ICON_NAME}.png"
}

install_icons

ln -sfn "$LAUNCHER" "${BIN_DIR}/novamail"

cat >"$DESKTOP_FILE" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=NovaMail
GenericName=E-Mail
Comment=NovaMail – lokaler E-Mail-Client
Exec=${LAUNCHER}
Icon=${ICON_NAME}
Terminal=false
Categories=Network;Email;Office;
Keywords=mail;email;nova;inbox;
StartupNotify=true
StartupWMClass=NovaMail
EOF
chmod +x "$DESKTOP_FILE"

cp -f "$DESKTOP_FILE" "${DESKTOP_HOME}/NovaMail.desktop"
chmod +x "${DESKTOP_HOME}/NovaMail.desktop"
if command -v gio >/dev/null 2>&1; then
  gio set "${DESKTOP_HOME}/NovaMail.desktop" metadata::trusted true 2>/dev/null || true
fi

cat >"${DOCK_DIR}/novamail.dockitem" <<EOF
[PlankDockItemPreferences]
Launcher=file://${DESKTOP_FILE}
EOF

if [[ -f "$DOCK_SETTINGS" ]]; then
  current="$(grep -E '^DockItems=' "$DOCK_SETTINGS" || true)"
  if [[ -n "$current" && "$current" != *novamail.dockitem* ]]; then
    if [[ "$current" == *google-chrome.dockitem* ]]; then
      sed -i 's/DockItems=google-chrome\.dockitem/DockItems=google-chrome.dockitem;;novamail.dockitem/' "$DOCK_SETTINGS"
    else
      sed -i 's/^DockItems=/DockItems=novamail.dockitem;;/' "$DOCK_SETTINGS"
    fi
  fi
fi

# Remove broken double-suffix entries from earlier installer revisions.
rm -f \
  "${DESKTOP_DIR}/app.novamail.desktop.desktop" \
  "${ICON_DIR}/256x256/apps/app.novamail.desktop.png" \
  "${ICON_DIR}/128x128/apps/app.novamail.desktop.png" \
  "${ICON_DIR}/64x64/apps/app.novamail.desktop.png" \
  "${ICON_DIR}/48x48/apps/app.novamail.desktop.png" \
  "${ICON_DIR}/32x32/apps/app.novamail.desktop.png" \
  2>/dev/null || true

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f "${ICON_DIR}" 2>/dev/null || true
fi

if command -v xfce4-panel >/dev/null 2>&1; then
  xfce4-panel -r 2>/dev/null || true
fi
if pgrep -x plank >/dev/null 2>&1; then
  pkill plank 2>/dev/null || true
  sleep 0.3
  nohup plank >/dev/null 2>&1 &
fi

echo "NovaMail launcher installed:"
echo "  menu:     ${DESKTOP_FILE}"
echo "  bin:      ${BIN_DIR}/novamail"
echo "  dock:     ${DOCK_DIR}/novamail.dockitem"
echo "  desktop:  ${DESKTOP_HOME}/NovaMail.desktop"
echo "  icon:     ${ICON_DIR}/128x128/apps/${ICON_NAME}.png"
