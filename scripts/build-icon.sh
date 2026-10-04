#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
iconset="target/branding/Gravewake.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/gravewake-emblem.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
  doubled=$((size * 2))
  sips -z "$doubled" "$doubled" assets/gravewake-emblem.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o assets/Gravewake.icns
