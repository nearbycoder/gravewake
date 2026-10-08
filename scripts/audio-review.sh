#!/usr/bin/env bash
# The audio output through real ALSA, without speakers: runs --audio-review
# with HOME set to a throwaway folder whose .asoundrc points ALSA's default
# device at files, and at a private PipeWire the review starts and stops
# (see src/audio_review.rs). PIPEWIRE_RUNTIME_DIR points inside that folder,
# so nothing reaches the machine's own sound server. Needs python3,
# alsa-lib's file and null plugins, pipewire, wireplumber, pipewire-alsa and
# pw-link. Writes review.log and the two captured streams (out-1.raw,
# out-2.raw: stereo 32-bit floats at 48 kHz) to captures/audio-review/, and
# fails if the errors printed more than 20 lines.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p captures/audio-review
home=$(mktemp -d "$root/captures/audio-review/home.XXXXXX")
cleanup() {
  # Stop any private server the review left behind: only processes whose
  # environment names this run's throwaway folder.
  for pid in $(pgrep -u "$(id -u)" -x 'pipewire|wireplumber' || true); do
    if tr '\0' '\n' <"/proc/$pid/environ" 2>/dev/null | grep -qxF "XDG_CONFIG_HOME=$home/config"; then
      echo "audio-review.sh: stopping leftover server $pid"
      kill "$pid" || true
    fi
  done
  rm -rf "$home"
}
trap cleanup EXIT
cargo build --release --locked
log=captures/audio-review/review.log
status=0
# Cap the log at 1 MB in case anything floods it.
HOME="$home" XDG_DATA_HOME="$home/data" PIPEWIRE_RUNTIME_DIR="$home/pw-run" \
  timeout -s KILL 180 target/release/gravewake --audio-review 2>&1 \
  | head -c 1000000 | tee "$log" || status=$?
cp "$home"/*.wav "$home"/*.log captures/audio-review/ 2>/dev/null || true
lines=$(wc -l <"$log")
error_lines=$(grep -c '^Audio output' "$log" || true)
echo "audio-review.sh: $lines lines in the log, $error_lines from the audio output" | tee -a "$log"
grep -q '^AUDIO REVIEW PASS$' "$log" || { echo "audio-review.sh: FAIL (no pass line)" | tee -a "$log"; exit 1; }
((status == 0)) || { echo "audio-review.sh: FAIL (status $status)" | tee -a "$log"; exit 1; }
if ((lines > 40 || error_lines > 20)); then
  echo "audio-review.sh: FAIL (too many lines)" | tee -a "$log"
  exit 1
fi
echo "audio-review.sh: PASS" | tee -a "$log"
