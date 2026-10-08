#!/usr/bin/env bash
# Run one command inside a private, virtual KWin: no real outputs, no input
# devices and its own D-Bus session. Test windows, and fullscreen switches,
# never reach the desktop in use. Streams the command's output and exits with
# its status. Needs kwin_wayland (KDE Plasma 6) and dbus-run-session.
#
#   scripts/nested-kwin.sh [--size 1920x1080] [--fake-input] [--layout fr] -- command [args...]
#
# Inside, WAYLAND_DISPLAY names the private compositor, DISPLAY is unset and
# GRAVEWAKE_NESTED_KWIN=1 is set. --fake-input lets any client of this
# private KWin inject key and mouse events (KWIN_WAYLAND_NO_PERMISSION_CHECKS)
# and sets GRAVEWAKE_FAKE_INPUT=1 inside; the desktop in use is unaffected.
# --layout gives the private KWin that XKB keyboard layout, through a
# kxkbrc in its own throwaway XDG_CONFIG_HOME. Working files go under
# captures/ and are removed afterwards.
#
# The private KWin runs in a session of its own (setsid). Helpers its D-Bus
# bus starts on demand, such as ksecretd, outlive the bus; when the command
# ends, anything still in that session is listed and stopped, and nothing
# outside it is touched.
set -euo pipefail

size=1920x1080
fake_input=0
layout=
while [[ ${1:-} == --size || ${1:-} == --fake-input || ${1:-} == --layout ]]; do
  if [[ $1 == --size ]]; then
    size=$2
    shift 2
  elif [[ $1 == --layout ]]; then
    layout=$2
    shift 2
  else
    fake_input=1
    shift
  fi
done
[[ ${1:-} == -- ]] && shift
if (($# == 0)); then
  echo "usage: $0 [--size WxH] [--fake-input] [--layout XKB] -- command [args...]" >&2
  exit 2
fi
for tool in kwin_wayland dbus-run-session; do
  command -v "$tool" >/dev/null || { echo "$tool not found" >&2; exit 2; }
done

root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/captures"
work=$(mktemp -d "$root/captures/nested-kwin.XXXXXX")
sid=
stop_session() {
  # The session's id is the setsid'd process's pid; the command records it
  # too, from inside, in case setsid had to fork.
  [[ -s $work/sid ]] && sid=$(tr -d ' ' <"$work/sid")
  [[ $sid =~ ^[0-9]+$ ]] || return 0
  [[ $sid == "$(ps -o sid= -p $$ | tr -d ' ')" ]] && return 0
  local pids
  pids=$(ps -eo pid=,sid= | awk -v s="$sid" '$2 == s { print $1 }')
  [[ -n $pids ]] || return 0
  echo "nested-kwin.sh: stopping what the private session left running:" >&2
  ps -o pid=,comm= -p "$(echo $pids | tr ' ' ,)" >&2 || true
  kill -TERM $pids 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    pids=$(ps -eo pid=,sid= | awk -v s="$sid" '$2 == s { print $1 }')
    [[ -n $pids ]] || return 0
    sleep 0.2
  done
  kill -KILL $pids 2>/dev/null || true
}
trap 'stop_session; rm -rf "$work"' EXIT

printf '%q ' "$@" >"$work/cmd"
: >"$work/out"
cat >"$work/session" <<EOF
#!/usr/bin/env bash
cd $(printf %q "$PWD")
unset DISPLAY
ps -o sid= -p \$\$ >$(printf %q "$work/sid")
export GRAVEWAKE_NESTED_KWIN=1
export GRAVEWAKE_FAKE_INPUT=$fake_input
bash -c "\$(cat $(printf %q "$work/cmd"))" >>$(printf %q "$work/out") 2>&1
echo \$? >$(printf %q "$work/status")
EOF
chmod +x "$work/session"

kwin_env=()
((fake_input)) && kwin_env+=(KWIN_WAYLAND_NO_PERMISSION_CHECKS=1)
if [[ -n $layout ]]; then
  mkdir -p "$work/config"
  printf '[Layout]\nLayoutList=%s\nUse=true\n' "$layout" >"$work/config/kxkbrc"
  kwin_env+=(XDG_CONFIG_HOME="$work/config")
fi
setsid -w dbus-run-session -- env "${kwin_env[@]}" kwin_wayland --virtual --no-lockscreen --no-global-shortcuts \
  --no-kactivities --socket "gravewake-nested-$$" \
  --width "${size%x*}" --height "${size#*x}" \
  --exit-with-session "$work/session" >"$work/kwin.log" 2>&1 &
kwin=$!
sid=$kwin
tail -n +1 -f --pid="$kwin" "$work/out" &
wait "$kwin" || true
wait || true

if [[ -f $work/status ]]; then
  exit "$(cat "$work/status")"
fi
echo "nested KWin exited without finishing the command; the end of its log:" >&2
tail -n 40 "$work/kwin.log" >&2
exit 1
