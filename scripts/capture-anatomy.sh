#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
"$CARGO_BIN" run --release -- --anatomy-review
ffmpeg -y -v error -framerate 30 -i captures/anatomy/frame-%04d.png \
  -i captures/anatomy-review.wav -t 24 -frames:v 720 -c:v libx264 -preset fast -crf 19 \
  -pix_fmt yuv420p -c:a aac -b:a 192k -movflags +faststart \
  captures/anatomy-review.mp4
