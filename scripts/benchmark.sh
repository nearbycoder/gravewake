#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
"${CARGO_BIN:-$HOME/.cargo/bin/cargo}" build --release
GRAVEWAKE_BENCH_LABEL="${GRAVEWAKE_BENCH_LABEL:-latest}" ./target/release/gravewake --benchmark
