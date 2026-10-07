#!/usr/bin/env bash
# Real key and mouse presses, injected through KWin's fake-input protocol
# inside a private virtual KWin, with a throwaway data folder. Writes
# captures and review.log to captures/input/. Needs KDE Plasma 6's
# kwin_wayland; see scripts/nested-kwin.sh.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p captures/input
home=$(mktemp -d "$root/captures/input-home.XXXXXX")
trap 'rm -rf "$home"' EXIT
cargo build --release --locked
XDG_DATA_HOME="$home" scripts/nested-kwin.sh --size "${INPUT_OUTPUT:-1920x1080}" --fake-input -- \
  target/release/gravewake --input-review 2>&1 | tee captures/input/review.log
