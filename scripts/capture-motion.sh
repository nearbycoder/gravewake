#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
"$CARGO_BIN" run -- --motion-review
ffmpeg -y -v error -framerate 30 -i captures/motion/frame-%04d.png \
  -i captures/motion-review.wav -c:v libx264 -preset fast -crf 19 \
  -pix_fmt yuv420p -c:a aac -b:a 192k -movflags +faststart -shortest \
  captures/motion-review.mp4
