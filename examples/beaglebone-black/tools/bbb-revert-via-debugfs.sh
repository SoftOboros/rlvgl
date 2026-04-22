#!/usr/bin/env bash
# bbb-revert-via-debugfs.sh — Revert the BBB's DTB and uEnv.txt patches
# by operating on the SD card's ext4 rootfs directly from macOS.
#
# Uses `debugfs` from e2fsprogs (brew) to read/write files on the ext4
# partition without needing to mount it (macOS can't mount ext4 natively
# without macFUSE + ext4fuse). No kernel extensions required.
#
# Usage:
#   diskutil unmountDisk /dev/diskN   # SD card's whole-disk device
#   sudo bash bbb-revert-via-debugfs.sh /dev/diskNs3
#
# Where /dev/diskNs3 is the ext4 rootfs partition (usually s3 on BBB SD
# layout: s1=FAT BOOT, s2=swap, s3=ext4 rootfs).

set -euo pipefail

DEV="${1:?Usage: $0 /dev/diskNs3}"
DEBUGFS="/opt/homebrew/opt/e2fsprogs/sbin/debugfs"
WORK="$(mktemp -d -t bbb-revert)"
trap 'rm -rf "$WORK"' EXIT

if [ ! -x "$DEBUGFS" ]; then
    echo "error: debugfs not found at $DEBUGFS. Install with: brew install e2fsprogs" >&2
    exit 1
fi

if [ "$(id -u)" -ne 0 ]; then
    echo "error: this script needs sudo to access $DEV" >&2
    exit 1
fi

echo "=== BBB ext4 DTB revert via debugfs ==="
echo "  device: $DEV"
echo "  work:   $WORK"
echo ""

# Find the kernel directory under /boot/dtbs/
KVER=$($DEBUGFS -R 'ls -l /boot/dtbs/' "$DEV" 2>/dev/null \
    | awk '/bone/ {print $NF}' | head -1)

if [ -z "$KVER" ]; then
    echo "error: could not locate a *-bone* kernel dir under /boot/dtbs/" >&2
    exit 1
fi

echo "  kernel: $KVER"
echo ""

DTBDIR="/boot/dtbs/$KVER"

# For each patched DTB, extract its .orig from the SD, then write it
# back over the patched copy.
for f in am335x-boneblack.dtb am335x-boneblack-uboot.dtb; do
    orig="$DTBDIR/$f.orig"
    target="$DTBDIR/$f"
    local_copy="$WORK/$(basename $f).orig"

    # Does .orig exist?
    if ! $DEBUGFS -R "stat $orig" "$DEV" 2>/dev/null | grep -q '^Inode:'; then
        echo "  (skipping $f — no .orig backup on SD)"
        continue
    fi

    echo "--- restoring $f ---"
    # Dump .orig to a local file
    $DEBUGFS -R "dump -p $orig $local_copy" "$DEV" 2>&1 | grep -v '^debugfs' || true

    # Replace the patched file with the .orig contents. debugfs can
    # write a host file back in place: rm + write preserves inode flags
    # better than in-place overwrite when file sizes differ.
    $DEBUGFS -w -R "rm $target" "$DEV" 2>&1 | grep -v '^debugfs' || true
    $DEBUGFS -w -R "write $local_copy $target" "$DEV" 2>&1 | grep -v '^debugfs' || true
    echo "  $f <- $f.orig"
done

# Restore uEnv.txt from the earliest rlvgl backup available.
UENV="/boot/uEnv.txt"
for bak in "$UENV.rlvgl-bak" "$UENV.rlvgl-bak2"; do
    if $DEBUGFS -R "stat $bak" "$DEV" 2>/dev/null | grep -q '^Inode:'; then
        echo "--- restoring uEnv.txt from $bak ---"
        local_uenv="$WORK/uEnv.txt.bak"
        $DEBUGFS -R "dump -p $bak $local_uenv" "$DEV" 2>&1 | grep -v '^debugfs' || true
        $DEBUGFS -w -R "rm $UENV" "$DEV" 2>&1 | grep -v '^debugfs' || true
        $DEBUGFS -w -R "write $local_uenv $UENV" "$DEV" 2>&1 | grep -v '^debugfs' || true
        echo "  uEnv.txt <- $(basename $bak)"
        break
    fi
done

echo ""
echo "=== Revert complete. Eject the SD and put it back in the BBB. ==="
echo ""
echo "  diskutil eject $(echo $DEV | sed 's/s[0-9]*$//')"
