#!/usr/bin/env bash
# reserve-fb.sh — Reserve the top 2 MB of DDR3L for the rlvgl-bbb framebuffer.
#
# The AM3358 has 512 MB of DDR3L at [0x8000_0000, 0xA000_0000). We want
# a physically-contiguous 1.5 MB (800*480*4) region at a known physical
# address so the LCDC DMA can be pointed at it while Linux is running.
#
# `mem=510M` limits the kernel to the first 510 MB (up to 0x9FE0_0000),
# leaving 2 MB at [0x9FE0_0000, 0xA000_0000) untouched. The region can
# then be mmap'd via /dev/mem without STRICT_DEVMEM objecting (it's
# outside the kernel-managed RAM pool).
#
# Run as root on the BBB:
#   sudo bash reserve-fb.sh
# Then reboot. Verify with:
#   cat /proc/cmdline | tr ' ' '\n' | grep '^mem='

set -euo pipefail

UENV=/boot/firmware/uEnv.txt
if [ ! -f "$UENV" ]; then
    UENV=/boot/uEnv.txt
fi
if [ ! -f "$UENV" ]; then
    echo "ERROR: cannot find uEnv.txt (tried /boot/firmware/uEnv.txt and /boot/uEnv.txt)" >&2
    exit 1
fi

BACKUP="${UENV}.rlvgl-bak"
if [ ! -f "$BACKUP" ]; then
    cp "$UENV" "$BACKUP"
    echo "Saved backup to $BACKUP"
fi

if grep -q "mem=510M" "$UENV"; then
    echo "$UENV already contains mem=510M, nothing to do."
    exit 0
fi

# Locate (or create) the cmdline= line and append mem=510M.
if grep -q '^cmdline=' "$UENV"; then
    sed -i 's|^cmdline=\(.*\)$|cmdline=\1 mem=510M|' "$UENV"
else
    echo 'cmdline=mem=510M' >> "$UENV"
fi

echo "Appended mem=510M to cmdline in $UENV."
echo "Reboot the BBB for the change to take effect."
echo "After reboot, verify with:  cat /proc/cmdline"
