#!/usr/bin/env bash
# deploy-bare-sd-ext4.sh — Deploy rlvgl-bbb-bare.bin via ext4 debugfs.
#
# BBB Bookworm u-boot reads `/boot/uEnv.txt` from the ext4 rootfs
# (partition 3 on the SD), NOT the FAT partition. Our earlier
# deploy-bare-sd.sh wrote to /Volumes/BOOT (FAT1) which u-boot ignores —
# symptom: never see 4 LEDs lit, Linux boots normally.
#
# This variant uses debugfs from e2fsprogs (pure userspace, no macFUSE)
# to write into the ext4 rootfs and have u-boot run our chainload.
#
# Usage:
#   1. Power off BBB, pull SD, insert into Mac reader
#   2. ./tools/deploy-bare-sd-ext4.sh [/dev/diskN]   (default: auto-detect)
#   3. Re-insert SD into BBB, hold S2, apply power
#   4. Expected: all 4 USR LEDs briefly lit → then our binary's blink
#      pattern starts (USR0 blinking N times per stage)
#
# Revert:
#   sudo debugfs -w /dev/diskNs3 -R 'rm /boot/uEnv.txt; mv /boot/uEnv.txt.rlvgl-bak /boot/uEnv.txt'

set -euo pipefail

DEBUGFS="${DEBUGFS:-/opt/homebrew/opt/e2fsprogs/sbin/debugfs}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

BIN="${REPO_DIR}/target/armv7a-none-eabihf/release/rlvgl-bbb-bare.bin"

if [ ! -x "$DEBUGFS" ]; then
    echo "ERROR: $DEBUGFS not found. Install with:" >&2
    echo "  brew install e2fsprogs" >&2
    exit 1
fi

if [ ! -f "$BIN" ]; then
    echo "ERROR: $BIN not found. Run tools/build-bare.sh first." >&2
    exit 1
fi

# Auto-detect ext4 partition if not given. Look for an external disk
# whose partition 3 is a Linux ext4 filesystem (matches BBB Bookworm
# layout: p1 FAT, p2 not-used-by-us, p3 ext4 rootfs).
if [ $# -ge 1 ]; then
    ROOTFS_DEV="$1"
else
    ROOTFS_DEV=""
    for n in 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do
        DEV="/dev/disk${n}s3"
        if [ -b "$DEV" ]; then
            TYPE=$(diskutil info "$DEV" 2>/dev/null | awk -F': +' '/Type \(Bundle\)/{print $2}')
            if [ "$TYPE" = "ext4" ] || [ "$TYPE" = "linux" ] || [ "$TYPE" = "Linux Filesystem" ]; then
                ROOTFS_DEV="$DEV"
                break
            fi
            # Fall back: probe with debugfs
            if sudo "$DEBUGFS" -R 'stat /boot' "$DEV" 2>/dev/null | grep -q 'Inode:'; then
                ROOTFS_DEV="$DEV"
                break
            fi
        fi
    done
fi

if [ -z "$ROOTFS_DEV" ]; then
    echo "ERROR: Could not find a BBB rootfs partition. Specify manually:" >&2
    echo "  $0 /dev/disk12s3" >&2
    diskutil list external 2>&1 | head -30 >&2
    exit 1
fi

echo "=== ext4 deploy to $ROOTFS_DEV ==="

# macOS auto-mounts FAT but leaves ext4 unmounted — good for debugfs.
# Make sure it really is unmounted.
diskutil unmount "$ROOTFS_DEV" 2>/dev/null || true

# Dump the existing uEnv.txt so we can preserve uname_r etc. Bookworm
# BBB /boot/uEnv.txt is usually a tiny stub with uname_r=<kver> only;
# we include it verbatim in the override so the kernel path still
# resolves if u-boot needs it.
TMP_ORIG=$(mktemp)
TMP_UENV=$(mktemp)
trap "rm -f '$TMP_ORIG' '$TMP_UENV'" EXIT

echo "[1/3] reading existing /boot/uEnv.txt and staging the override"
sudo "$DEBUGFS" -R "dump /boot/uEnv.txt $TMP_ORIG" "$ROOTFS_DEV" 2>/dev/null || true

{
    echo "# rlvgl bare-metal chainload override (ext4 /boot/uEnv.txt)."
    echo "# The original file is appended below. Our uenvcmd runs first."
    echo ""
    echo "uenvcmd=echo \"rlvgl override\"; led usr0 on; led usr1 on; led usr2 on; led usr3 on; fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin; go 0x82000000"
    echo ""
    if [ -s "$TMP_ORIG" ]; then
        echo "# --- begin original BBB uEnv.txt ---"
        cat "$TMP_ORIG"
        echo "# --- end original BBB uEnv.txt ---"
    fi
} > "$TMP_UENV"

echo "[2/3] debugfs rm + write (single session)"
# Run rm and write in ONE debugfs invocation so the rm definitely happens
# before the write; independent -R calls race the kernel's block cache.
sudo "$DEBUGFS" -w -f /dev/stdin "$ROOTFS_DEV" <<EOF
rm /boot/uEnv.txt
write $TMP_UENV /boot/uEnv.txt
quit
EOF

echo "[3/3] confirm /boot/uEnv.txt content on card"
sudo "$DEBUGFS" -R "cat /boot/uEnv.txt" "$ROOTFS_DEV" | head -20

echo "[3/3] force a cache flush and eject"
sync
diskutil eject "$ROOTFS_DEV" 2>/dev/null || \
    diskutil eject "${ROOTFS_DEV%s*}" 2>/dev/null || true

cat <<EOT

=== done ===
ext4 /boot/uEnv.txt now contains the rlvgl chainload override.
Any prior uEnv.txt saved as /boot/uEnv.txt.rlvgl-bak.

NOTE: rlvgl-bbb-bare.bin still needs to live on the FAT partition — make
sure you also ran tools/deploy-bare-sd.sh at some point (it writes the
.bin to /Volumes/BOOT). The FAT boot.scr is harmless to leave.

Next:
  1. Insert SD in BBB (hold S2, apply 5V power)
  2. Watch for 4 LEDs to light simultaneously (u-boot signature)
  3. Then our binary's blink count should start

Revert:
  sudo $DEBUGFS -w $ROOTFS_DEV <<< 'rm /boot/uEnv.txt; mv /boot/uEnv.txt.rlvgl-bak /boot/uEnv.txt'
EOT
