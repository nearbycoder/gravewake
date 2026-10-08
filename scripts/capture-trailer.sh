#!/usr/bin/env bash
# Record everything the trailer and the README gallery are edited from, on
# Linux, inside a private virtual KWin (scripts/nested-kwin.sh) so no window
# reaches the desktop in use. Each review runs in its own working directory
# under captures/trailer/, with a throwaway data folder, at 1920×1080 and the
# fidelity step in $FIDELITY (default ultra). The reviews step a fixed 1/60 s
# and save every second frame, so the footage is a steady 30 fps whatever
# each frame costs to draw. Then run scripts/build-trailer.py.
#
#   scripts/capture-trailer.sh            # build, then capture everything
#   FIDELITY=high scripts/capture-trailer.sh
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
fidelity=${FIDELITY:-ultra}
work=$root/captures/trailer
game=$root/target/release/gravewake
mkdir -p "$work/data"
export XDG_DATA_HOME=$work/data

nice -n 10 cargo build --release --locked -j "${JOBS:-8}"

# One review in its own directory; the game writes captures/... relative to it.
review() {
  local name=$1
  shift
  rm -rf "$work/run-$name"
  mkdir -p "$work/run-$name"
  echo "capture-trailer.sh: $name ($*)"
  (cd "$work/run-$name" && nice -n 10 "$root/scripts/nested-kwin.sh" --size 2560x1440 -- \
    "$game" "$@" --capture-size 1920x1080 --capture-fidelity "$fidelity") \
    | grep -v '^Captured ' | tail -n 40 >"$work/$name.log"
  tail -n 1 "$work/$name.log"
}
# Frames at 30 fps with the review's own sound track. Survival frames start
# at game frame 40, so its sound is offset to match.
encode() {
  local name=$1 frames=$2 offset=$3
  ffmpeg -y -v error -framerate 30 -i "$work/run-$name/captures/$frames/frame-%04d.png" \
    -ss "$offset" -i "$work/run-$name/captures/$name-review.wav" \
    -c:v libx264 -preset medium -crf 14 -pix_fmt yuv420p \
    -c:a aac -b:a 256k -shortest -movflags +faststart "$work/$name-review.mp4"
}

review motion --motion-review --capture-full-mix
review survival --survival-review --capture-full-mix
review anatomy --anatomy-review --capture-full-mix
review text --text-review
encode motion motion 0
encode survival survival 0.65
encode anatomy anatomy 0

# The score's calm layer, for the gallery stills.
rm -rf "$work/run-audio"
mkdir -p "$work/run-audio"
(cd "$work/run-audio" && "$game" --export-audio >/dev/null)

grep -h 'PASS' "$work"/{motion,survival,anatomy,text}.log
echo "capture-trailer.sh: captures in $work; now run scripts/build-trailer.py"
