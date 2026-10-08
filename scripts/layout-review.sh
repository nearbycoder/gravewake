#!/usr/bin/env bash
# Key names from the keyboard layout before any key is pressed: runs
# --layout-review in the private KWin (scripts/nested-kwin.sh) with a US and
# then a French layout, expecting W A S D and then Z Q S D on the opening
# banner. Writes captures and review.log to captures/layout/.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p captures/layout
home=$(mktemp -d "$root/captures/layout/home.XXXXXX")
trap 'rm -rf "$home"' EXIT
cargo build --release --locked
log=captures/layout/review.log
: >"$log"
for case in "us:W A S D" "fr:Z Q S D"; do
  layout=${case%%:*}
  echo "layout-review.sh: $layout layout, expecting ${case#*:}" | tee -a "$log"
  XDG_DATA_HOME="$home" GRAVEWAKE_EXPECT_MOVEMENT="${case#*:}" \
    scripts/nested-kwin.sh --size 1440x900 --layout "$layout" -- \
    target/release/gravewake --layout-review 2>&1 | tee -a "$log"
done
echo "layout-review.sh: PASS" | tee -a "$log"
