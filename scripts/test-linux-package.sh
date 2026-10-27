#!/usr/bin/env bash
set -euo pipefail

export OPENSCL4_TEST_BINARY
OPENSCL4_TEST_BINARY="$(readlink -f "${1:?application binary required}")"

# A desktop session has a D-Bus session bus, which enforces one instance per data folder.
dbus-run-session -- xvfb-run -a bash <<'X11'
set -euo pipefail
wm_pid=""
pids=()
cleanup() {
    for pid in "${pids[@]}" "$wm_pid"; do
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

start_app() {
    OPENSCL4_CONFIG_DIR="$1" SLINT_RENDERER=software "$OPENSCL4_TEST_BINARY" </dev/null &
    started=$!
}

wait_window() {
    local pid="$1" window=""
    for _ in {1..100}; do
        window=$(wmctrl -lp | awk -v pid="$pid" '$3 == pid { print $1; exit }')
        if [ -n "$window" ]; then echo "$window"; return 0; fi
        if ! kill -0 "$pid" 2>/dev/null; then return 1; fi
        sleep 0.1
    done
    return 1
}

wait_exit() {
    local pid="$1"
    for _ in {1..100}; do
        if ! kill -0 "$pid" 2>/dev/null; then return 0; fi
        sleep 0.1
    done
    return 1
}

visible() {
    xwininfo -id "$1" 2>/dev/null | grep -q "Map State: IsViewable"
}

is_hidden() {
    ! visible "$1" || xprop -id "$1" _NET_WM_STATE 2>/dev/null | grep -q _NET_WM_STATE_HIDDEN
}

shown() {
    visible "$1" && ! xprop -id "$1" _NET_WM_STATE 2>/dev/null | grep -q _NET_WM_STATE_HIDDEN
}

wait_shown() {
    local window="$1"
    for _ in {1..100}; do
        if shown "$window"; then return 0; fi
        sleep 0.1
    done
    return 1
}

# A second start for the same data folder asks the first to show and ends; closing without a tray quits.
config="$(mktemp -d)"
start_app "$config"
first=$started
pids+=("$first")
window=$(wait_window "$first")
test -n "$window"
wmctrl -ir "$window" -b add,hidden
hidden_state=""
for _ in {1..50}; do
    if is_hidden "$window"; then hidden_state=1; break; fi
    sleep 0.1
done
test -n "$hidden_state"
start_app "$config"
second=$started
wait_exit "$second"
wait "$second"
if ! kill -0 "$first" 2>/dev/null; then echo "the first instance quit"; exit 1; fi
wait_shown "$window"
wmctrl -ic "$window"
wait_exit "$first"
wait "$first"
pids=()

# After a normal exit the same data folder starts again.
start_app "$config"
reopened=$started
pids+=("$reopened")
window=$(wait_window "$reopened")
test -n "$window"
wmctrl -ic "$window"
wait_exit "$reopened"
wait "$reopened"
pids=()

# Different data folders run beside each other.
config_a="$(mktemp -d)"
config_b="$(mktemp -d)"
start_app "$config_a"
app_a=$started
start_app "$config_b"
app_b=$started
pids+=("$app_a" "$app_b")
window_a=$(wait_window "$app_a")
window_b=$(wait_window "$app_b")
test -n "$window_a"
test -n "$window_b"
wmctrl -ic "$window_a"
wmctrl -ic "$window_b"
wait_exit "$app_a"
wait_exit "$app_b"
wait "$app_a"
wait "$app_b"
pids=()

# A start racing another for one data folder leaves exactly one app.
config="$(mktemp -d)"
start_app "$config"
racing_a=$started
start_app "$config"
racing_b=$started
pids+=("$racing_a" "$racing_b")
sleep 3
alive=0
for pid in "$racing_a" "$racing_b"; do
    if kill -0 "$pid" 2>/dev/null; then alive=$((alive + 1)); fi
done
if [ "$alive" -ne 1 ]; then echo "$alive apps run for one data folder"; exit 1; fi
for pid in "$racing_a" "$racing_b"; do
    if kill -0 "$pid" 2>/dev/null; then
        window=$(wait_window "$pid")
        test -n "$window"
        wmctrl -ic "$window"
        wait_exit "$pid"
        wait "$pid"
    fi
done
pids=()

# After a crash the app starts again; the bus released the name.
config="$(mktemp -d)"
start_app "$config"
crashed=$started
pids+=("$crashed")
test -n "$(wait_window "$crashed")"
kill -9 "$crashed"
wait_exit "$crashed"
wait "$crashed" 2>/dev/null || true
start_app "$config"
restarted=$started
pids+=("$restarted")
window=$(wait_window "$restarted")
test -n "$window"
wmctrl -ic "$window"
wait_exit "$restarted"
wait "$restarted"
pids=()

# Without a session bus the app still starts.
config="$(mktemp -d)"
env -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$(mktemp -d)" \
    OPENSCL4_CONFIG_DIR="$config" SLINT_RENDERER=software "$OPENSCL4_TEST_BINARY" &
busless=$!
pids+=("$busless")
window=$(wait_window "$busless")
test -n "$window"
wmctrl -ic "$window"
wait_exit "$busless"
wait "$busless"
pids=()
X11

dbus-run-session -- xvfb-run -a bash <<'WAYLAND'
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
