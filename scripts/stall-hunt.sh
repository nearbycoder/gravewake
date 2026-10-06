#!/bin/bash
# Launch the smoke test repeatedly to catch the rare start-up stall.
#
# Alternates plain and controller smoke runs. The watchdog aborts a run that
# makes no progress for GRAVEWAKE_WATCHDOG_SECS (default 120), and this script
# then saves the core's thread stacks from coredumpctl. Results go to
# captures/stall/: one capped log per run, a stack file per stall and a
# summary.tsv. Smoke runs never touch player saves; XDG_DATA_HOME also points
# at a throwaway folder as a precaution.
#
# Usage: scripts/stall-hunt.sh [runs] [binary]
set -uo pipefail
cd "$(dirname "$0")/.."
runs="${1:-20}"
source_binary="${2:-target/release/gravewake}"
out=captures/stall
mkdir -p "$out/data"
# Run a private copy so rebuilding during the hunt doesn't swap the binary.
binary="$out/gravewake"
cp "$source_binary" "$binary"
summary="$out/summary.tsv"
[ -f "$summary" ] || printf 'run\tmode\tstatus\tseconds\tlast line\n' >"$summary"
for ((i = 1; i <= runs; i++)); do
    if ((i % 2)); then mode=plain; args=(--smoke); else mode=gamepad; args=(--smoke --gamepad); fi
    stamp="$(date +%Y%m%d-%H%M%S)-$i"
    log="$out/$stamp-$mode.log"
    start=$(date +%s)
    # The outer timeout is a backstop if the watchdog itself can't run.
    XDG_DATA_HOME="$PWD/$out/data" timeout -s TERM 900 "$binary" "${args[@]}" 2>&1 | head -c 1000000 >"$log"
    status=${PIPESTATUS[0]}
    seconds=$(($(date +%s) - start))
    last="$(tail -n 1 "$log" | tr '\t' ' ')"
    printf '%s\t%s\t%s\t%s\t%s\n' "$stamp" "$mode" "$status" "$seconds" "$last" >>"$summary"
    echo "$stamp $mode status=$status ${seconds}s"
    if ((status != 0)); then
        sleep 5 # let systemd-coredump finish writing
        coredumpctl info --no-pager -1 "$(realpath "$binary")" 2>&1 | head -c 2000000 >"$out/$stamp-$mode-stack.txt"
        coredumpctl debug --no-pager -1 "$(realpath "$binary")" \
            --debugger-arguments="-batch -ex 'thread apply all bt'" 2>&1 |
            head -c 4000000 >"$out/$stamp-$mode-gdb.txt"
        echo "  stacks saved to $out/$stamp-$mode-{stack,gdb}.txt"
    fi
done
