#!/usr/bin/env bash
# Builds the Linux AppImage used by twg-desktop auto-update.
# Requires `appimagetool` on PATH (or set APPIMAGETOOL).
# Run from the repository root: bash tools/package-linux-appimage.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p twg-desktop

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
ARCH="$(uname -m)"
TOOL="${APPIMAGETOOL:-appimagetool}"
APPDIR="dist/TacticalGame3D.AppDir"
OUT="dist/twg-desktop-${VERSION}-linux-${ARCH}.AppImage"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/256x256/apps"
install -m 755 target/release/twg-desktop "$APPDIR/usr/bin/twg-desktop"
install -m 644 assets/icons/twg-desktop-256.png \
  "$APPDIR/usr/share/icons/hicolor/256x256/apps/twg-desktop.png"

cat > "$APPDIR/twg-desktop.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Tactical Game 3D
Comment=Editor peta dan simulasi taktis
Exec=twg-desktop
Icon=twg-desktop
Terminal=false
Categories=Game;StrategyGame;
EOF

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/twg-desktop" "$@"
EOF
chmod +x "$APPDIR/AppRun"

if ! command -v "$TOOL" >/dev/null 2>&1; then
  echo "appimagetool tidak ditemukan. Pasang dari https://github.com/AppImage/appimagetool" >&2
  echo "atau set APPIMAGETOOL=/path/ke/appimagetool" >&2
  exit 1
fi

"$TOOL" "$APPDIR" "$OUT"
echo "Bundle: $OUT"
