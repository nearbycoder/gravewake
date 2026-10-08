#!/usr/bin/env bash
# The window's size at launch, in a private virtual KWin with a throwaway
# data folder: three launches of --window-review (see src/window_review.rs).
#   1. A first launch on a 1366x768 output opens at 1440x900 fitted to 90%
#      of it (1105x691). It then asks for 1440x900 unfitted, to show what the
#      compositor does with a window bigger than the screen, and resizes to
#      1100x650 before quitting through the normal save.
#   2. The next launch on the same output reopens at 1100x650.
#   3. With 3000x1800 saved, a launch on a 1920x1080 output opens at
#      1620x972.
# Writes review.log to captures/window/. Needs KDE Plasma 6's kwin_wayland;
# see scripts/nested-kwin.sh.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
if [[ ${1:-} == --inside ]]; then
  # Inside the private KWin: run the review, and when it waits for a resize,
  # have KWin resize the game's window, as dragging its edge would.
  out=$2
  : >"$out"
  target/release/gravewake --window-review >"$out" 2>&1 &
  game=$!
  if [[ -n ${GRAVEWAKE_WINDOW_RESIZE:-} ]]; then
    until grep -q 'waiting for the compositor' "$out"; do
      kill -0 "$game" 2>/dev/null || break
      sleep 0.2
    done
    if kill -0 "$game" 2>/dev/null; then
      script="$(dirname "$out")/resize.js"
      cat >"$script" <<JS
for (const w of workspace.windowList()) {
  if (w.caption.indexOf("Gravewake") === 0) {
    const frame = w.frameGeometry;
    const client = w.clientGeometry;
    w.frameGeometry = {
      x: frame.x,
      y: frame.y,
      width: ${GRAVEWAKE_WINDOW_RESIZE%x*} + frame.width - client.width,
      height: ${GRAVEWAKE_WINDOW_RESIZE#*x} + frame.height - client.height,
    };
    print("resize.js: " + w.caption + " client " + client.width + "x" + client.height
      + " -> ${GRAVEWAKE_WINDOW_RESIZE}");
  }
}
JS
      id=$(gdbus call --session --dest org.kde.KWin --object-path /Scripting \
        --method org.kde.kwin.Scripting.loadScript "$script" gravewake-resize | tr -dc '0-9')
      gdbus call --session --dest org.kde.KWin --object-path "/Scripting/Script$id" \
        --method org.kde.kwin.Script.run >/dev/null
      echo "window-review.sh: asked KWin (script $id) to resize the window to $GRAVEWAKE_WINDOW_RESIZE"
    fi
  fi
  status=0
  wait "$game" || status=$?
  cat "$out"
  exit "$status"
fi
mkdir -p captures/window
log=captures/window/review.log
: >"$log"
home=$(mktemp -d "$root/captures/window-home.XXXXXX")
trap 'rm -rf "$home"' EXIT
cargo build --release --locked
settings="$home/gravewake/settings.json"
launch() {
  local output=$1
  shift
  echo "window-review.sh: launch on a $output output: $*" | tee -a "$log"
  env "$@" XDG_DATA_HOME="$home" scripts/nested-kwin.sh --size "$output" -- \
    scripts/window-review.sh --inside "$home/game.log" 2>&1 | head -c 200000 | tee -a "$log"
  local status=${PIPESTATUS[0]}
  if ((status != 0)); then
    echo "window-review.sh: FAIL (status $status)" | tee -a "$log"
    exit 1
  fi
}
saved_size() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("window_size"))' "$settings"
}
launch 1366x768 GRAVEWAKE_WINDOW_EXPECT=1105x691 GRAVEWAKE_WINDOW_PROBE=1440x900 \
  GRAVEWAKE_WINDOW_RESIZE=1100x650
echo "window-review.sh: settings.json holds window_size $(saved_size)" | tee -a "$log"
[[ $(saved_size) == "[1100.0, 650.0]" ]] || { echo "window-review.sh: FAIL (saved size)" | tee -a "$log"; exit 1; }
launch 1366x768 GRAVEWAKE_WINDOW_EXPECT=1100x650
python3 - "$settings" <<'PY'
import json, sys
path = sys.argv[1]
data = json.load(open(path))
data["window_size"] = [3000, 1800]
json.dump(data, open(path, "w"))
PY
echo "window-review.sh: settings.json now holds window_size $(saved_size)" | tee -a "$log"
launch 1920x1080 GRAVEWAKE_WINDOW_EXPECT=1620x972
echo "window-review.sh: PASS" | tee -a "$log"
