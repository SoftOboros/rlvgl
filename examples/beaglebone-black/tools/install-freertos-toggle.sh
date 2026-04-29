#!/usr/bin/env bash
# install-freertos-toggle.sh — Install the Linux↔FreeRTOS swap toggle
# on a running BBB over SSH.
#
# Stages:
#   1. Build rlvgl-bbb-bare.bin (no_std, ~1.8 KB) on the host.
#   2. scp the .bin and the FAT-side /uEnv.txt onto /tmp on the BBB.
#   3. sudo-install both into /boot/firmware/, install the
#      /usr/local/bin/swap-to-freertos helper, and sync.
#
# After this:
#   ssh <bbb> sudo swap-to-freertos
# triggers the cycle: BBB reboots, u-boot chainloads the bare-metal
# binary (which fires a warm reset back to Linux ~10s later).
#
# Env:
#   BBB_HOST       ssh target          (default: debian@192.168.6.2)
#   BBB_SUDO_PASS  sudo password on BBB (required; piped via stdin)

set -euo pipefail

BBB_HOST="${BBB_HOST:-debian@192.168.6.2}"

if [[ -z "${BBB_SUDO_PASS:-}" ]]; then
  echo "BBB_SUDO_PASS is unset. Export it before running this script." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

ALT_BIN_HOST="${REPO_DIR}/target/armv7a-none-eabihf/release/rlvgl-bbb-bare.bin"
UENV_HOST="${PROJECT_DIR}/linux/uEnv.txt.fat-toggle"
HELPER_HOST="${SCRIPT_DIR}/swap-to-freertos"

echo "=== build rlvgl-bbb-bare.bin ==="
bash "${SCRIPT_DIR}/build-bare.sh"

echo "=== stage to /tmp on ${BBB_HOST} ==="
scp "${ALT_BIN_HOST}" "${UENV_HOST}" "${HELPER_HOST}" "${BBB_HOST}:/tmp/"

echo "=== install on BBB (FAT uEnv.txt + alt bin + helper) ==="
printf '%s\n%s\n' "$BBB_SUDO_PASS" '
set -e
cp /tmp/rlvgl-bbb-bare.bin /boot/firmware/rlvgl-bbb-bare.bin
cp /tmp/uEnv.txt.fat-toggle /boot/firmware/uEnv.txt
install -m 755 /tmp/swap-to-freertos /usr/local/bin/swap-to-freertos
sync
ls -la /boot/firmware/rlvgl-bbb-bare.bin /boot/firmware/uEnv.txt /usr/local/bin/swap-to-freertos
' | ssh -T "${BBB_HOST}" 'sudo -S -p "" bash -s'

echo ""
echo "=== installed ==="
echo "Trigger a cycle from the host:"
echo "  ssh ${BBB_HOST} sudo swap-to-freertos"
echo ""
echo "Or directly: write a marker on the FAT partition then reboot"
echo "  ssh ${BBB_HOST} sudo bash -c 'echo F > /boot/firmware/freertos-next && reboot'"
