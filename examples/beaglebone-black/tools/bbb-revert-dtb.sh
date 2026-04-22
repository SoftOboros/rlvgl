#!/usr/bin/env bash
# bbb-revert-dtb.sh — Revert the BBB's DTB and uEnv.txt patches.
#
# Use this from a RESCUE boot when rlvgl's DTB patch has left the BBB
# unbootable. Typical sequence:
#
#   1. Insert a pristine Bookworm microSD into the BBB.
#   2. Hold S2 (boot button) while applying power. BBB boots from the
#      rescue SD.
#   3. SSH to the rescue SD (192.168.6.2 or 192.168.7.2 depending on
#      image). The stuck SD card appears as a second block device.
#   4. Mount the stuck rootfs: `sudo mount /dev/mmcblk1p1 /mnt`
#      (adjust device — /dev/mmcblk1p1 is eMMC's first partition when
#      the rescue SD is mmcblk0; for SD-boot use `ls /dev/mmcblk*`).
#   5. Run: `sudo bash bbb-revert-dtb.sh /mnt`
#   6. `sync; umount /mnt; reboot`
#
# Or, if you just want to SSH to the stuck BBB in rescue mode and
# revert from the running system, pass `/` as the mountpoint.

set -euo pipefail

MNT="${1:-/}"
KVER=$(basename "$(ls -d "$MNT"/boot/dtbs/*-bone* 2>/dev/null | head -1)")

if [ -z "$KVER" ]; then
    echo "error: could not find /boot/dtbs/<kver>-bone* under $MNT" >&2
    exit 1
fi

DIR="$MNT/boot/dtbs/$KVER"
UENV="$MNT/boot/uEnv.txt"

echo "=== BBB DTB revert ==="
echo "  mountpoint: $MNT"
echo "  kernel:     $KVER"
echo ""

restored=0

for f in am335x-boneblack.dtb am335x-boneblack-uboot.dtb; do
    if [ -f "$DIR/$f.orig" ]; then
        cp -v "$DIR/$f.orig" "$DIR/$f"
        restored=$((restored + 1))
    else
        echo "  (no $f.orig — leaving $f as-is)"
    fi
done

for bak in "$UENV.rlvgl-bak2" "$UENV.rlvgl-bak"; do
    if [ -f "$bak" ]; then
        cp -v "$bak" "$UENV"
        restored=$((restored + 1))
        break
    fi
done

echo ""
if [ "$restored" -gt 0 ]; then
    echo "=== Restored $restored file(s). Sync, unmount, and reboot. ==="
else
    echo "=== Nothing restored — no backups found. ==="
    exit 1
fi
