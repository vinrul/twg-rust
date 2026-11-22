#!/usr/bin/env bash
# Checks the native libraries needed to build twg-desktop on Linux.
# Run from the repository root: bash tools/check-linux-deps.sh
set -euo pipefail

APT_PACKAGES="pkg-config libasound2-dev libudev-dev libwayland-dev libx11-dev \
libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev"

if ! command -v pkg-config >/dev/null 2>&1; then
  cat >&2 <<EOF
pkg-config tidak ditemukan. Dependensi build Linux belum lengkap.
Pasang dengan:
  sudo apt install -y $APT_PACKAGES
EOF
  exit 1
fi

missing=()
for module in alsa libudev wayland-client x11 xcb xkbcommon; do
  if ! pkg-config --exists "$module"; then
    missing+=("$module")
  fi
done

if [ "${#missing[@]}" -gt 0 ]; then
  cat >&2 <<EOF
Dependensi build Linux belum lengkap (pkg-config tidak menemukan: ${missing[*]}).
Bevy membutuhkan header ALSA, udev, Wayland/X11, dan xkbcommon.
Pasang dengan:
  sudo apt install -y $APT_PACKAGES
Di WSL jalankan perintah yang sama di distro Ubuntu-nya.
EOF
  exit 1
fi
