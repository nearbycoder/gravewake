#!/usr/bin/env bash
# Switch fullscreen while the game runs (F11 and the journal's switch, both
# ways) inside a private virtual KWin, with a throwaway data folder. Writes
# captures and review.log to captures/fullscreen/. Needs KDE Plasma 6's
# kwin_wayland; see scripts/nested-kwin.sh.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p captures/fullscreen
home=$(mktemp -d "$root/captures/fullscreen-home.XXXXXX")
trap 'rm -rf "$home"' EXIT
cargo build --release --locked
XDG_DATA_HOME="$home" scripts/nested-kwin.sh --size "${FULLSCREEN_OUTPUT:-1920x1080}" -- \
  target/release/gravewake --fullscreen-review 2>&1 | tee captures/fullscreen/review.log
