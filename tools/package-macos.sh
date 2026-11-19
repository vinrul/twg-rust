#!/usr/bin/env bash
# Builds macOS .app bundles for the desktop and online-admin apps.
# Run from the repository root: bash tools/package-macos.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p twg-desktop -p twg-online-admin

OUT="dist/macos"
rm -rf "$OUT"
mkdir -p "$OUT"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"

make_bundle() {
  local app_name="$1" exe="$2" bundle_id="$3" icon="$4"
  local app="$OUT/$app_name.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  install -m 755 "target/release/$exe" "$app/Contents/MacOS/$exe"
  install -m 644 "$icon" "$app/Contents/Resources/AppIcon.icns"
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>$app_name</string>
	<key>CFBundleDisplayName</key>
	<string>$app_name</string>
	<key>CFBundleIdentifier</key>
	<string>$bundle_id</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleExecutable</key>
	<string>$exe</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
EOF
}

make_bundle "Tactical Game 3D" "twg-desktop" "com.twg.desktop" "assets/icons/twg-desktop.icns"
make_bundle "TWG Online Admin" "twg-online-admin" "com.twg.onlineadmin" "assets/icons/twg-admin.icns"

if command -v ditto >/dev/null 2>&1; then
  ditto -c -k --sequesterRsrc --keepParent "$OUT/Tactical Game 3D.app" "dist/TacticalGame3D-macos.zip"
  ditto -c -k --sequesterRsrc --keepParent "$OUT/TWG Online Admin.app" "dist/TWGOnlineAdmin-macos.zip"
  echo "Bundles: dist/TacticalGame3D-macos.zip, dist/TWGOnlineAdmin-macos.zip"
else
  echo "ditto not found; .app bundles are in $OUT"
fi
