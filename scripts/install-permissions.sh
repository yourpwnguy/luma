#!/usr/bin/env bash
# Install the luma udev rules. Run once with sudo, then replug both USB
# devices. The ASUS attributes are handled by the same rule file.
set -euo pipefail

if [[ ${EUID} -ne 0 ]]; then
    echo "error: run this script with sudo" >&2
    exit 1
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
rule_source="${script_dir}/../udev/70-luma.rules"
rule_target="/etc/udev/rules.d/70-luma.rules"

if [[ ! -f "${rule_source}" ]]; then
    echo "error: cannot find ${rule_source}" >&2
    exit 1
fi

# Remove the rule from the pre-rename version if it is still installed.
rm -f /etc/udev/rules.d/70-rgbsync.rules

install -m 0644 "${rule_source}" "${rule_target}"
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw
udevadm trigger --subsystem-match=leds

echo "installed ${rule_target}"
echo "replug the K617 and G102 once, then run: luma list-devices"
echo "the ASUS sysfs attributes are re-applied by udevadm trigger immediately"
