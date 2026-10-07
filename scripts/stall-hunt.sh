#!/bin/bash
# Launch the smoke test repeatedly to catch the rare start-up stall.
#
# Alternates plain and controller smoke runs. The watchdog aborts a run that
# makes no progress for GRAVEWAKE_WATCHDOG_SECS (default 120), and this script
# then saves the core's thread stacks from coredumpctl. Results go to
# captures/stall/ (or HUNT_OUT): one capped log per run, a stack file per stall and a
# summary.tsv. Smoke runs never touch player saves; XDG_DATA_HOME also points
# at a throwaway folder as a precaution.
#
# If a run outlives HUNT_TIMEOUT seconds (default 900) anyway, for example
# because the whole process is stopped, the script records its state from
# /proc (process and thread states, kernel wait channels) in a -proc.txt file,
# then sends SIGABRT so a core is kept, and SIGKILL 30 s later if needed.
#
# Usage: scripts/stall-hunt.sh [runs] [binary]
set -uo pipefail
cd "$(dirname "$0")/.."
runs="${1:-20}"
source_binary="${2:-target/release/gravewake}"
out="${HUNT_OUT:-captures/stall}"
mkdir -p "$out/data"
# Run a private copy so rebuilding during the hunt doesn't swap the binary.
binary="$out/gravewake"
cp "$source_binary" "$binary"
summary="$out/summary.tsv"
[ -f "$summary" ] || printf 'run\tmode\tstatus\tseconds\tlast line\n' >"$summary"
# Process and per-thread state of a run that outlived the outer timeout.
proc_state() {
    local pid=$1 task
    echo "time: $(date -Is)  load: $(cat /proc/loadavg)"
    grep -E '^(Name|State|Tgid|PPid|TracerPid|Threads|SigPnd|ShdPnd|SigBlk|SigIgn|SigCgt|voluntary_ctxt_switches|nonvoluntary_ctxt_switches):' "/proc/$pid/status"
    echo "wchan: $(cat "/proc/$pid/wchan")"
    for task in /proc/"$pid"/task/*; do
        # Fields after the command name: state is the first.
        printf '%s\t%s\tstate %s\twchan %s\n' "${task##*/}" "$(cat "$task/comm")" \
            "$(sed 's/.*) //' "$task/stat" | cut -d' ' -f1)" "$(cat "$task/wchan")"
    done
    echo "fd 1 -> $(readlink "/proc/$pid/fd/1")  fd 2 -> $(readlink "/proc/$pid/fd/2")"
}
for ((i = 1; i <= runs; i++)); do
    if ((i % 2)); then mode=plain; args=(--smoke); else mode=gamepad; args=(--smoke --gamepad); fi
    stamp="$(date +%Y%m%d-%H%M%S)-$i"
    log="$out/$stamp-$mode.log"
    start=$(date +%s)
    # The outer timeout is a backstop if the watchdog itself can't run.
    XDG_DATA_HOME="$PWD/$out/data" "$binary" "${args[@]}" > >(head -c 1000000 >"$log") 2>&1 &
    pid=$!
    deadline=$((start + ${HUNT_TIMEOUT:-900}))
    while kill -0 "$pid" 2>/dev/null && (($(date +%s) < deadline)); do
        sleep 1
    done
    if kill -0 "$pid" 2>/dev/null; then
        proc_state "$pid" >"$out/$stamp-$mode-proc.txt" 2>&1
        echo "  outer timeout: state saved to $out/$stamp-$mode-proc.txt; sending SIGABRT"
        kill -ABRT "$pid" 2>/dev/null
        kill -CONT "$pid" 2>/dev/null # a stopped process can't act on the abort
        for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
        kill -KILL "$pid" 2>/dev/null
    fi
    wait "$pid"
    status=$?
    wait # for the log's head process (bash 5.1 or later)
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
