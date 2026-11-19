#!/usr/bin/env bash
# Builds a portable Linux bundle with binaries, icons, and menu entries.
# Run from the repository root: bash tools/package-linux.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p twg-desktop -p twg-online-admin

OUT="dist/linux"
rm -rf "$OUT"
mkdir -p "$OUT/bin" "$OUT/share/applications"
install -m 755 target/release/twg-desktop "$OUT/bin/twg-desktop"
install -m 755 target/release/twg-online-admin "$OUT/bin/twg-online-admin"

for size in 16 24 32 48 64 128 256 512; do
  dir="$OUT/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$dir"
  install -m 644 "assets/icons/twg-desktop-${size}.png" "$dir/twg-desktop.png"
  install -m 644 "assets/icons/twg-admin-${size}.png" "$dir/twg-admin.png"
done

cat > "$OUT/share/applications/twg-desktop.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Tactical Game 3D
Comment=Editor peta dan simulasi taktis
Exec=twg-desktop
Icon=twg-desktop
Terminal=false
Categories=Game;StrategyGame;
StartupNotify=true
EOF

cat > "$OUT/share/applications/twg-online-admin.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=TWG Online Admin
Comment=Kelola sesi user yang sedang online
Exec=twg-online-admin
Icon=twg-admin
Terminal=false
Categories=Utility;Admin;
StartupNotify=true
EOF

cat > "$OUT/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
install -Dm755 "$DIR/bin/twg-desktop" "$PREFIX/bin/twg-desktop"
install -Dm755 "$DIR/bin/twg-online-admin" "$PREFIX/bin/twg-online-admin"
for icon in "$DIR"/share/icons/hicolor/*/apps/*.png; do
  rel="${icon#"$DIR/share/icons/"}"
  install -Dm644 "$icon" "$PREFIX/share/icons/$rel"
done
install -Dm644 "$DIR/share/applications/twg-desktop.desktop" \
  "$PREFIX/share/applications/twg-desktop.desktop"
install -Dm644 "$DIR/share/applications/twg-online-admin.desktop" \
  "$PREFIX/share/applications/twg-online-admin.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" || true
fi
echo "Installed under $PREFIX (bin + icons + menu entries)."
EOF
chmod +x "$OUT/install.sh"

ARCH="$(uname -m)"
tar -C dist -czf "dist/twg-linux-${ARCH}.tar.gz" linux
echo "Bundle: dist/twg-linux-${ARCH}.tar.gz"
