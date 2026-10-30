#!/usr/bin/env bash
# Starts the AppImage with --appimage-extract-and-run, as a system without FUSE must, under X11 and nested Wayland.
# The application inside is the one scripts/test-linux-package.sh checks from the .deb; this checks only that the
# AppImage starts it, that its window opens, and that closing the window without a tray ends the AppImage.
# Each session writes its programs' output to its own folder in the log folder; a failed session's logs are printed.
set -euo pipefail

export OPENSCL4_TEST_APPIMAGE
OPENSCL4_TEST_APPIMAGE="$(readlink -f "${1:?AppImage required}")"
mkdir -p "${2:?log folder required}"
log_root="$(readlink -f "$2")"

# Runs the script on stdin in its own session bus and X server, with the folder for its logs as $1. Both sessions
# run even when one fails, so the result shows whether only one of them broke.
failed=()
session() {
    local logs="$log_root/$1"
    mkdir -p "$logs"
    if ! dbus-run-session -- xvfb-run -a bash -s "$logs"; then
        failed+=("$1")
        for log in "$logs"/*.log; do
            if [ -f "$log" ]; then
                echo "--- $log"
                cat "$log"
            fi
        done
    fi
}

# A session bus without a tray host: closing the window ends the application.
session x11 <<'X11'
set -euo pipefail
logs="$1"
runtime=""
openbox >"$logs/openbox.log" 2>&1 &
wm=$!
trap 'kill ${runtime:+"$runtime"} "$wm" 2>/dev/null || true' EXIT
for _ in {1..50}; do
    if wmctrl -m >/dev/null 2>&1; then break; fi
    sleep 0.1
done

OPENSCL4_CONFIG_DIR="$(mktemp -d)" SLINT_RENDERER=software \
    "$OPENSCL4_TEST_APPIMAGE" --appimage-extract-and-run </dev/null >"$logs/app.log" 2>&1 &
runtime=$!

# The runtime unpacks the AppImage, then forks and waits for AppRun, which becomes the application: the window
# belongs to a child of the started process, not to the started process itself.
window=""
for _ in {1..300}; do
    while read -r id _ pid _; do
        if [ "$(awk '/^PPid:/ { print $2 }' "/proc/$pid/status" 2>/dev/null)" = "$runtime" ]; then window=$id; fi
    done < <(wmctrl -lp 2>/dev/null)
    if [ -n "$window" ]; then break; fi
    if ! kill -0 "$runtime" 2>/dev/null; then echo "the AppImage ended without a window"; exit 1; fi
    sleep 0.1
done
if [ -z "$window" ]; then echo "no window of the AppImage's application appeared"; exit 1; fi

wmctrl -ic "$window"
for _ in {1..100}; do
    if ! kill -0 "$runtime" 2>/dev/null; then break; fi
    sleep 0.1
done
if kill -0 "$runtime" 2>/dev/null; then echo "the AppImage still runs after its window closed"; exit 1; fi
# The runtime ends with the application's exit status.
status=0
wait "$runtime" || status=$?
runtime=""
if [ "$status" -ne 0 ]; then echo "the AppImage ended with status $status"; exit 1; fi
X11

session wayland <<'WAYLAND'
set -euo pipefail
logs="$1"
export XDG_RUNTIME_DIR
XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
weston --backend=x11-backend.so --renderer=pixman --no-config --socket=openscl4-test --idle-time=0 \
    >"$logs/weston.log" 2>&1 &
weston_pid=$!
trap 'kill "$weston_pid" 2>/dev/null || true' EXIT
for _ in {1..100}; do
    if [ -S "$XDG_RUNTIME_DIR/openscl4-test" ]; then break; fi
    if ! kill -0 "$weston_pid" 2>/dev/null; then echo "weston ended before it opened its socket"; exit 1; fi
    sleep 0.1
done
if [ ! -S "$XDG_RUNTIME_DIR/openscl4-test" ]; then echo "weston opened no socket"; exit 1; fi

# timeout ends the runtime and the application it forked together; still running at the deadline is the pass.
# The protocol log shows whether the application opened a window rather than only staying alive.
set +e
env -u DISPLAY WAYLAND_DISPLAY=openscl4-test WAYLAND_DEBUG=client OPENSCL4_CONFIG_DIR="$(mktemp -d)" \
    SLINT_RENDERER=software timeout 8s "$OPENSCL4_TEST_APPIMAGE" --appimage-extract-and-run \
    </dev/null >"$logs/app.log" 2>&1
result=$?
set -e
if [ "$result" -ne 124 ]; then echo "the AppImage ended with status $result before the deadline"; exit 1; fi
if ! grep -qF 'set_app_id("io.github.sckrat.openscl4")' "$logs/app.log"; then
    echo "the application opened no window"
    exit 1
fi
WAYLAND

if [ "${#failed[@]}" -ne 0 ]; then
    echo "the AppImage check failed under ${failed[*]}"
    exit 1
fi
