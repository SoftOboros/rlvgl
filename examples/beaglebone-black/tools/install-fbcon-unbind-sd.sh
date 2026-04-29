#!/usr/bin/env bash
# install-fbcon-unbind-sd.sh — Install the rlvgl-unbind-fbcon.service
# systemd oneshot into a BBB SD card's ext4 rootfs via debugfs, so a
# fresh SD deploy automatically unbinds fbcon at boot and prevents the
# "flashing cursor underline" on the panel.
#
# Root cause is captured in docs/beaglebone-black/ and the memory note
# feedback_bbb_fbcon_cursor.md: the Linux framebuffer console (fbcon)
# stays bound to /dev/fb0 alongside rlvgl's userspace writes and paints
# its blinking 8x16 character cell on top of the rendered widget tree.
#
# This script stages the unit file we check in alongside it (the
# rlvgl-unbind-fbcon.service text in this same directory) into the
# card's /etc/systemd/system/ directory and creates the
# multi-user.target.wants/ symlink debugfs-style so systemd enables
# it on first boot. Serial console on ttyS0 and the USB-gadget SSH
# path are untouched.
#
# Before running:
#   1. Power off the BBB, pull the SD, insert it into a Mac reader.
#   2. Make sure e2fsprogs is installed (brew install e2fsprogs).
#
# Usage:
#   bash tools/install-fbcon-unbind-sd.sh              # auto-detect
#   bash tools/install-fbcon-unbind-sd.sh /dev/diskNs3
#
# Revert (from the Mac, SD reinserted):
#   sudo /opt/homebrew/opt/e2fsprogs/sbin/debugfs -w /dev/diskNs3 -R \
#     'rm /etc/systemd/system/multi-user.target.wants/rlvgl-unbind-fbcon.service; \
#      rm /etc/systemd/system/rlvgl-unbind-fbcon.service'

set -euo pipefail

DEBUGFS="${DEBUGFS:-/opt/homebrew/opt/e2fsprogs/sbin/debugfs}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UNIT_SRC="${SCRIPT_DIR}/rlvgl-unbind-fbcon.service"
UNIT_NAME="rlvgl-unbind-fbcon.service"
UNIT_DST="/etc/systemd/system/${UNIT_NAME}"
WANTS_DIR="/etc/systemd/system/multi-user.target.wants"
WANTS_LINK="${WANTS_DIR}/${UNIT_NAME}"

if [ ! -x "$DEBUGFS" ]; then
    echo "ERROR: $DEBUGFS not found. Install with:" >&2
    echo "  brew install e2fsprogs" >&2
    exit 1
fi

if [ ! -f "$UNIT_SRC" ]; then
    echo "ERROR: unit source missing at $UNIT_SRC" >&2
    exit 1
fi

# --- Step 1: find the BBB rootfs partition on the SD -------------------

if [ $# -ge 1 ]; then
    ROOTFS_DEV="$1"
else
    ROOTFS_DEV=""
    for n in 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do
        DEV="/dev/disk${n}s3"
        [ -b "$DEV" ] || continue
        # The BBB rootfs always has /etc/systemd on Bookworm — if that
        # path resolves, we've found the right partition.
        if sudo "$DEBUGFS" -R 'stat /etc/systemd/system' "$DEV" 2>/dev/null \
                | grep -q '^Inode:'; then
            ROOTFS_DEV="$DEV"
            break
        fi
    done
fi

if [ -z "$ROOTFS_DEV" ]; then
    echo "ERROR: could not find BBB rootfs partition. Pass explicitly:" >&2
    echo "  $0 /dev/disk12s3" >&2
    diskutil list external 2>&1 | head -20 >&2
    exit 1
fi

echo "=== installing ${UNIT_NAME} into ${ROOTFS_DEV} ==="
diskutil unmount "$ROOTFS_DEV" 2>/dev/null || true

# --- Step 2: sanity-check systemd paths on the SD ----------------------

for p in /etc/systemd/system "$WANTS_DIR"; do
    if ! sudo "$DEBUGFS" -R "stat $p" "$ROOTFS_DEV" 2>/dev/null \
            | grep -q '^Inode:'; then
        echo "ERROR: $p missing on rootfs — this SD isn't a normal Bookworm image." >&2
        exit 1
    fi
done

# --- Step 3: write the unit file + create the wants/ symlink ----------
#
# Idempotent: we `rm` the unit and wants/ entries before writing so a
# rerun of this script picks up any local edits to the .service file.
# `rm` inside debugfs silently succeeds on missing inodes, so the first
# run on a clean SD works the same as subsequent runs.

echo "[1/3] staging unit file in /etc/systemd/system/${UNIT_NAME}"
sudo "$DEBUGFS" -w -f /dev/stdin "$ROOTFS_DEV" >/dev/null <<EOF
rm ${UNIT_DST}
rm ${WANTS_LINK}
write ${UNIT_SRC} ${UNIT_DST}
quit
EOF

echo "[2/3] enabling unit via ${WANTS_DIR}/${UNIT_NAME} -> ../${UNIT_NAME}"
# debugfs `symlink <link_name> <target>` creates a short symbolic link.
# Use the relative target so the link survives rootfs relocations.
sudo "$DEBUGFS" -w -f /dev/stdin "$ROOTFS_DEV" >/dev/null <<EOF
symlink ${WANTS_LINK} ../${UNIT_NAME}
quit
EOF

echo "[3/3] verify"
echo "    unit file:"
sudo "$DEBUGFS" -R "stat ${UNIT_DST}" "$ROOTFS_DEV" 2>/dev/null \
    | grep -E '^(Inode|Size|Mode)' | sed 's/^/      /'
echo "    wants symlink:"
sudo "$DEBUGFS" -R "stat ${WANTS_LINK}" "$ROOTFS_DEV" 2>/dev/null \
    | grep -E '^(Inode|Size|Mode|Fast_link_dest)' | sed 's/^/      /'

sync
diskutil eject "$ROOTFS_DEV" 2>/dev/null || \
    diskutil eject "${ROOTFS_DEV%s*}" 2>/dev/null || true

cat <<EOT

=== done ===
Unit file:       ${UNIT_DST}
Wants-enabled:   ${WANTS_LINK} -> ../${UNIT_NAME}

Effective at next BBB boot — the service runs once at multi-user.target
and unbinds any vtcon bound to /dev/fb0. Verify on the booted board:

  ssh debian@192.168.6.2 'cat /sys/class/vtconsole/vtcon1/bind'   # 0
  ssh debian@192.168.6.2 'systemctl status rlvgl-unbind-fbcon'

Revert:
  sudo ${DEBUGFS} -w ${ROOTFS_DEV} -R \\
    'rm ${WANTS_LINK}; rm ${UNIT_DST}'
EOT
