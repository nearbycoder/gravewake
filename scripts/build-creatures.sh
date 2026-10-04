#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
blender_bin="${BLENDER_BIN:-$HOME/.local/share/dark-veil-tools/Blender.app/Contents/MacOS/Blender}"
"$blender_bin" --background --python scripts/build-enemies.py
