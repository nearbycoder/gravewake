#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
blender_bin="${BLENDER_BIN:-$HOME/.local/share/dark-veil-tools/Blender.app/Contents/MacOS/Blender}"
if [[ ! -x "$blender_bin" ]]; then blender_bin="$(command -v blender || true)"; fi
if [[ ! -x "$blender_bin" ]]; then echo 'Set BLENDER_BIN to a Blender 4.5+ executable.' >&2; exit 1; fi
"$blender_bin" --background --python scripts/build-weathered-armory.py -- "$@"
