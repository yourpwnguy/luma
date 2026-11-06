#!/usr/bin/env bash
# Visible hardware validation for all three devices.
#
# Run as the desktop user after installing the udev rules:
#   cargo build --release
#   ./scripts/live-test.sh
#
# If the ASUS sysfs rule is not active, run it once with sudo instead:
#   sudo ./scripts/live-test.sh
#
# Output is mirrored to luma-live-test.log in the repository root.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd -- "${script_dir}/.." && pwd)"
binary="${root}/target/release/luma"
log="${root}/luma-live-test.log"

if [[ ! -x "${binary}" ]]; then
    echo "error: ${binary} not found; run 'cargo build --release' first" >&2
    exit 1
fi

cd "${root}"
exec > >(tee "${log}") 2>&1

echo "luma live test"
echo "date: $(date -Is)"
echo "binary: ${binary}"
echo "user: $(id -un) groups=$(id -Gn)"
echo

echo "=== config ==="
"${binary}" validate-config

echo
echo "=== discovery ==="
"${binary}" list-devices

echo
echo "=== read-only probe ==="
"${binary}" probe

echo
echo "=== static red (all devices) ==="
"${binary}" run --effect static --color '#ff0000' --duration 2

echo
echo "=== static green (all devices) ==="
"${binary}" run --effect static --color '#00ff00' --duration 2

echo
echo "=== wave, speed 2 (watch coherence) ==="
"${binary}" run --effect wave --speed 2 --duration 4

echo
echo "=== global cycle (watch synchronization) ==="
"${binary}" run --effect cycle --speed 2 --duration 3

echo
echo "=== orbit: one comet around the triangle ==="
"${binary}" run --effect orbit --speed 1 --duration 8

echo
echo "=== flow: rainbow around the device loop ==="
"${binary}" run --effect flow --duration 6

echo
echo "=== full-field atmospheric effects (all keys stay lit) ==="
for effect in aurora constellation circuit ribbon bloom; do
    echo "--- ${effect} ---"
    "${binary}" run --effect "${effect}" --duration 4
done

echo
echo "=== self-test colors ==="
"${binary}" self-test

echo
echo "=== K617 focused keys (dim blue background, one bright key) ==="
"${binary}" self-test --keys Escape,Menu,RCtrl

echo
echo "=== K617 keymap sweep (watch one key at a time) ==="
"${binary}" self-test --keymap

echo
echo "=== restore check ==="
"${binary}" run --effect static --color '#202020' --duration 1

echo
echo "live test complete; devices were restored by each run"
