#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo_bin="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
"$cargo_bin" build --release
./target/release/gravewake --survival-review
ffmpeg -y -v error -framerate 30 -i captures/survival/frame-%04d.png -ss 0.65 -i captures/survival-review.wav -c:v libx264 -preset fast -crf 19 -pix_fmt yuv420p -c:a aac -b:a 192k -shortest -movflags +faststart captures/survival-review.mp4
