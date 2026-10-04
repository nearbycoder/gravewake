#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")"
if [[ ! -x "dist/Gravewake.app/Contents/MacOS/gravewake" ]]; then
  ./scripts/package-macos.sh
fi
open "dist/Gravewake.app"
