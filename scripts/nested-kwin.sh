#!/usr/bin/env bash
# Run one command inside a private, virtual KWin: no real outputs, no input
# devices and its own D-Bus session. Test windows, and fullscreen switches,
# never reach the desktop in use. Streams the command's output and exits with
# its status. Needs kwin_wayland (KDE Plasma 6) and dbus-run-session.
#
#   scripts/nested-kwin.sh [--size 1920x1080] [--fake-input] -- command [args...]
#
# Inside, WAYLAND_DISPLAY names the private compositor, DISPLAY is unset and
# GRAVEWAKE_NESTED_KWIN=1 is set. --fake-input lets any client of this
# private KWin inject key and mouse events (KWIN_WAYLAND_NO_PERMISSION_CHECKS)
# and sets GRAVEWAKE_FAKE_INPUT=1 inside; the desktop in use is unaffected. Working files go under captures/ and are
# removed afterwards.
set -euo pipefail

size=1920x1080
fake_input=0
while [[ ${1:-} == --size || ${1:-} == --fake-input ]]; do
  if [[ $1 == --size ]]; then
    size=$2
    shift 2
  else
    fake_input=1
    shift
  fi
done
[[ ${1:-} == -- ]] && shift
if (($# == 0)); then
  echo "usage: $0 [--size WxH] [--fake-input] -- command [args...]" >&2
  exit 2
fi
for tool in kwin_wayland dbus-run-session; do
  command -v "$tool" >/dev/null || { echo "$tool not found" >&2; exit 2; }
done

root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/captures"
work=$(mktemp -d "$root/captures/nested-kwin.XXXXXX")
trap 'rm -rf "$work"' EXIT

printf '%q ' "$@" >"$work/cmd"
: >"$work/out"
cat >"$work/session" <<EOF
#!/usr/bin/env bash
cd $(printf %q "$PWD")
unset DISPLAY
export GRAVEWAKE_NESTED_KWIN=1
export GRAVEWAKE_FAKE_INPUT=$fake_input
bash -c "\$(cat $(printf %q "$work/cmd"))" >>$(printf %q "$work/out") 2>&1
echo \$? >$(printf %q "$work/status")
EOF
chmod +x "$work/session"

permissions=()
((fake_input)) && permissions=(env KWIN_WAYLAND_NO_PERMISSION_CHECKS=1)
dbus-run-session -- "${permissions[@]}" kwin_wayland --virtual --no-lockscreen --no-global-shortcuts \
  --no-kactivities --socket "gravewake-nested-$$" \
  --width "${size%x*}" --height "${size#*x}" \
  --exit-with-session "$work/session" >"$work/kwin.log" 2>&1 &
kwin=$!
tail -n +1 -f --pid="$kwin" "$work/out" &
wait "$kwin" || true
wait || true

if [[ -f $work/status ]]; then
  exit "$(cat "$work/status")"
fi
echo "nested KWin exited without finishing the command; the end of its log:" >&2
tail -n 40 "$work/kwin.log" >&2
exit 1
