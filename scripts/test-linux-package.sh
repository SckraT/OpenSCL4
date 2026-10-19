#!/usr/bin/env bash
set -euo pipefail

export OPENSCL4_TEST_BINARY
OPENSCL4_TEST_BINARY="$(readlink -f "${1:?application binary required}")"

xvfb-run -a bash <<'X11'
set -euo pipefail
wm_pid=""
app_pid=""
cleanup() {
    for pid in "$app_pid" "$wm_pid"; do
        if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
    done
}
trap cleanup EXIT
openbox >/dev/null 2>&1 &
wm_pid=$!
for _ in {1..50}; do
    if wmctrl -m >/dev/null 2>&1; then break; fi
    sleep 0.1
done
config="$(mktemp -d)"
OPENSCL4_CONFIG_DIR="$config" SLINT_RENDERER=software "$OPENSCL4_TEST_BINARY" &
app_pid=$!
window=""
for _ in {1..100}; do
    window=$(wmctrl -lp | awk -v pid="$app_pid" '$3 == pid { print $1; exit }')
    if [ -n "$window" ]; then break; fi
    kill -0 "$app_pid"
    sleep 0.1
done
test -n "$window"
wmctrl -ic "$window"
for _ in {1..50}; do
    if ! kill -0 "$app_pid" 2>/dev/null; then break; fi
    sleep 0.1
done
if kill -0 "$app_pid" 2>/dev/null; then
    echo "Application did not quit after closing its window without a tray"
    exit 1
fi
wait "$app_pid"
app_pid=""
X11

xvfb-run -a bash <<'WAYLAND'
set -euo pipefail
export XDG_RUNTIME_DIR
XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
weston --backend=x11-backend.so --renderer=pixman --no-config --socket=openscl4-test --idle-time=0 >/dev/null 2>&1 &
weston_pid=$!
trap 'kill "$weston_pid" 2>/dev/null || true' EXIT
for _ in {1..100}; do
    if [ -S "$XDG_RUNTIME_DIR/openscl4-test" ]; then break; fi
    kill -0 "$weston_pid"
    sleep 0.1
done
test -S "$XDG_RUNTIME_DIR/openscl4-test"
config="$(mktemp -d)"
set +e
env -u DISPLAY WAYLAND_DISPLAY=openscl4-test OPENSCL4_CONFIG_DIR="$config" SLINT_RENDERER=software timeout 8s "$OPENSCL4_TEST_BINARY"
result=$?
set -e
test "$result" = 124
WAYLAND
