#!/usr/bin/env bash
# deploy-overlay-sd-ext4.sh — Install BB-NHD7-CAPE.dtbo on the SD so
# u-boot applies it at boot, and rewrite /boot/uEnv.txt to enable
# cape overlay loading while suppressing the default HDMI bridge overlay.
#
# This is the Linux-prong counterpart to deploy-bare-sd-ext4.sh. It
# relies on debugfs (from e2fsprogs) to read/write the ext4 rootfs
# without mounting it — no macFUSE, no kext.
#
# Before running:
#   1. bash tools/build-overlay.sh           # produces the .dtbo
#   2. Power off BBB, pull SD, insert in Mac reader
#
# What this writes into the ext4 rootfs:
#   /boot/dtbs/<kver>/overlays/BB-NHD7-CAPE.dtbo   (our overlay)
#   /boot/uEnv.txt                                 (cape overlay config)
#
# The previous uEnv.txt is saved as /boot/uEnv.txt.pre-rlvgl (first
# backup only — subsequent runs do not overwrite the backup so the
# original distro-ship file is preserved).
#
# Usage:
#   bash tools/deploy-overlay-sd-ext4.sh             # auto-detect
#   bash tools/deploy-overlay-sd-ext4.sh /dev/disk12s3
#
# Revert:
#   sudo $DEBUGFS -w /dev/diskNs3 -R \
#       'rm /boot/uEnv.txt; mv /boot/uEnv.txt.pre-rlvgl /boot/uEnv.txt'

set -euo pipefail

DEBUGFS="${DEBUGFS:-/opt/homebrew/opt/e2fsprogs/sbin/debugfs}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

DTBO="${REPO_DIR}/target/bbb-overlays/BB-NHD7-CAPE.dtbo"
DTBO_NAME="BB-NHD7-CAPE.dtbo"

if [ ! -x "$DEBUGFS" ]; then
    echo "ERROR: $DEBUGFS not found. Install with:" >&2
    echo "  brew install e2fsprogs" >&2
    exit 1
fi

if [ ! -f "$DTBO" ]; then
    echo "ERROR: overlay not built at $DTBO" >&2
    echo "  Run tools/build-overlay.sh first." >&2
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
        # Probe with debugfs — if /boot/dtbs exists, this is a BBB rootfs.
        if sudo "$DEBUGFS" -R 'stat /boot/dtbs' "$DEV" 2>/dev/null \
                | grep -q 'Inode:'; then
            ROOTFS_DEV="$DEV"
            break
        fi
    done
fi

if [ -z "$ROOTFS_DEV" ]; then
    echo "ERROR: could not find BBB rootfs partition on any external disk." >&2
    echo "Pass explicitly, e.g. $0 /dev/disk12s3" >&2
    diskutil list external 2>&1 | head -30 >&2
    exit 1
fi

echo "=== deploying overlay to $ROOTFS_DEV ==="

# macOS auto-mounts FAT but leaves ext4 alone; ensure unmounted.
diskutil unmount "$ROOTFS_DEV" 2>/dev/null || true

# --- Step 2: discover the kernel version directory under /boot/dtbs ----

# debugfs `ls -l` lists directory entries. Grab the first directory name
# that looks like a kernel version (starts with a digit, contains "-bone"
# or "-ti" or just "-" with dots). If multiple are present, prefer the
# one that matches the uname_r line in the existing /boot/uEnv.txt.

echo "[1/4] probing /boot/dtbs for kernel version"
DTBS_LIST=$(sudo "$DEBUGFS" -R 'ls -l /boot/dtbs' "$ROOTFS_DEV" 2>/dev/null \
            | awk '{print $NF}' | grep -E '^[0-9]+\.[0-9]+' || true)

# Try to read uname_r from the existing uEnv.txt to pick the right kernel.
TMP_UENV=$(mktemp)
trap "rm -f '$TMP_UENV'" EXIT
sudo "$DEBUGFS" -R "dump /boot/uEnv.txt $TMP_UENV" "$ROOTFS_DEV" 2>/dev/null || true
EXISTING_KVER=""
if [ -s "$TMP_UENV" ]; then
    EXISTING_KVER=$(awk -F= '/^uname_r=/{print $2; exit}' "$TMP_UENV" | tr -d '\r\n ')
fi

KVER=""
if [ -n "$EXISTING_KVER" ] && echo "$DTBS_LIST" | grep -qx "$EXISTING_KVER"; then
    KVER="$EXISTING_KVER"
else
    # Fall back to first listed version.
    KVER=$(echo "$DTBS_LIST" | head -1)
fi

if [ -z "$KVER" ]; then
    echo "ERROR: no kernel version directories under /boot/dtbs on the SD." >&2
    echo "debugfs said:" >&2
    sudo "$DEBUGFS" -R 'ls -l /boot/dtbs' "$ROOTFS_DEV" 2>&1 | sed 's/^/  /' >&2
    exit 1
fi

echo "    kernel version: $KVER"

OVERLAY_DIR="/boot/dtbs/${KVER}/overlays"
OVERLAY_DST="${OVERLAY_DIR}/${DTBO_NAME}"

# --- Step 3: copy the .dtbo into place ---------------------------------

echo "[2/4] writing ${OVERLAY_DST}"
# debugfs can't mkdir -p, but overlays/ should already exist on the stock
# Bookworm image. Probe first and bail with a useful error if not.
if ! sudo "$DEBUGFS" -R "stat ${OVERLAY_DIR}" "$ROOTFS_DEV" 2>/dev/null \
        | grep -q 'Inode:'; then
    echo "ERROR: ${OVERLAY_DIR} does not exist on rootfs." >&2
    echo "The stock Bookworm image always ships it; if it's missing, the" >&2
    echo "SD was flashed from an unusual image." >&2
    exit 1
fi

sudo "$DEBUGFS" -w -f /dev/stdin "$ROOTFS_DEV" >/dev/null <<EOF
cd ${OVERLAY_DIR}
rm ${DTBO_NAME}
write ${DTBO} ${DTBO_NAME}
quit
EOF

# --- Step 4: rewrite /boot/uEnv.txt ------------------------------------

echo "[3/4] rewriting /boot/uEnv.txt"
TMP_NEW=$(mktemp)
trap "rm -f '$TMP_UENV' '$TMP_NEW'" EXIT

# Preserve the original line count / structure where sensible.
# The stock file usually starts with `uname_r=<kver>` and nothing else.
UNAME_LINE="uname_r=${KVER}"
if [ -s "$TMP_UENV" ] && grep -q '^uname_r=' "$TMP_UENV"; then
    UNAME_LINE=$(grep '^uname_r=' "$TMP_UENV" | head -1)
fi

cat > "$TMP_NEW" <<EOF
# /boot/uEnv.txt — managed by rlvgl deploy-overlay-sd-ext4.sh.
# Original file preserved as /boot/uEnv.txt.pre-rlvgl (first run only).

${UNAME_LINE}

# Enable u-boot overlay machinery.
enable_uboot_overlays=1

# Suppress the default HDMI/video bridge overlay — it fights our LCD
# pinmux by claiming the same pads with a different configuration.
# Without this flag the board boots to a black screen even when our
# overlay is otherwise correct.
disable_uboot_overlay_video=1

# Apply our NHD-7 cape overlay.
#
# On BBB u-boot (Robert C. Nelson tree), uboot_overlay_addr{0..3} are
# the USER custom-overlay slots and addr{4..7} are reserved for the
# cape-EEPROM-auto-detect path. We want a user overlay, so addr0 it is.
#
# Path is resolved by u-boot relative to the rootfs mount.
uboot_overlay_addr0=${OVERLAY_DST}

# Suppress the cape-EEPROM auto-load (slot 4..7). The stock NH7C
# overlay upstream carries a known //FIXME on the LCDC endpoint wiring
# and isn't shipped in /lib/firmware on this 6.12 image anyway, but
# setting this keeps behaviour stable if that ever changes.
disable_uboot_overlay_addr4=1
disable_uboot_overlay_addr5=1
disable_uboot_overlay_addr6=1
disable_uboot_overlay_addr7=1
EOF

# Only save the backup on the first run. debugfs exits 0 even when
# `stat` fails on a missing inode, so we have to look at the output.
if sudo "$DEBUGFS" -R 'stat /boot/uEnv.txt.pre-rlvgl' "$ROOTFS_DEV" 2>/dev/null \
        | grep -q '^Inode:'; then
    BACKUP_CMD=""
else
    if [ -s "$TMP_UENV" ]; then
        BACKUP_CMD="write $TMP_UENV /boot/uEnv.txt.pre-rlvgl"
    else
        BACKUP_CMD=""
    fi
fi

sudo "$DEBUGFS" -w -f /dev/stdin "$ROOTFS_DEV" >/dev/null <<EOF
$BACKUP_CMD
rm /boot/uEnv.txt
write $TMP_NEW /boot/uEnv.txt
quit
EOF

# --- Step 5: clear FAT-side bare-metal override artifacts --------------
#
# deploy-bare-sd.sh puts a uEnv.txt + boot.scr + rlvgl-bbb-bare.bin on the
# FAT partition. BBB u-boot's envboot scans mmc 0:1 (FAT) for uEnv.txt
# BEFORE falling through to the ext4 distro_boot path, so a leftover
# uenvcmd=... go 0x82000000 on FAT will still chainload the bare-metal
# binary and Linux never boots — the panel stays on the bare-metal
# colour bars + LED chase. We remove the FAT artifacts here so envboot
# falls through cleanly.
WHOLE_DISK="${ROOTFS_DEV%s*}"
FAT_DEV="${WHOLE_DISK}s1"
echo "[4/5] clearing FAT-side bare-metal override artifacts on ${FAT_DEV}"

# macOS auto-mounts FAT on re-attach; if not, mount it explicitly.
diskutil mount "$FAT_DEV" >/dev/null 2>&1 || true
FAT_MNT=""
for vol in "/Volumes/BOOT" "/Volumes/boot"; do
    if [ -d "$vol" ]; then FAT_MNT="$vol"; break; fi
done

if [ -n "$FAT_MNT" ]; then
    for f in uEnv.txt boot.scr rlvgl-bbb-bare.bin; do
        if [ -f "$FAT_MNT/$f" ]; then
            echo "    rm $FAT_MNT/$f"
            sudo rm -f "$FAT_MNT/$f"
        fi
    done
    sync
    diskutil eject "$FAT_MNT" 2>/dev/null || true
else
    echo "    (FAT partition not mounted — skipping; if bare-metal override"
    echo "     was deployed, re-run deploy-bare-sd.sh revert manually)"
fi

# --- Step 6: verify and eject rootfs -----------------------------------

echo "[5/5] verify and eject"
echo "    /boot/uEnv.txt on card:"
sudo "$DEBUGFS" -R "cat /boot/uEnv.txt" "$ROOTFS_DEV" 2>/dev/null | sed 's/^/      /'
echo "    overlay on card:"
sudo "$DEBUGFS" -R "stat ${OVERLAY_DST}" "$ROOTFS_DEV" 2>/dev/null \
    | grep -E '^(Inode|Size|Mode)' | sed 's/^/      /'

sync
diskutil eject "$ROOTFS_DEV" 2>/dev/null || \
    diskutil eject "$WHOLE_DISK" 2>/dev/null || true

cat <<EOT

=== done ===
Overlay installed: ${OVERLAY_DST}
uEnv.txt updated with enable_uboot_overlays=1 + disable_uboot_overlay_video=1.

Boot sequence:
  1. Insert SD in BBB (hold S2 if eMMC still boots first, otherwise not)
  2. Attach cape, apply 5V barrel power
  3. Expected after ~10 s:
       - Backlight on, panel showing the console framebuffer
       - /dev/fb0 and /sys/class/drm/card0-DPI-1/status "connected"
       - /sys/class/drm/card0-DPI-1/modes shows "800x480"
  4. SSH in and verify:
       dmesg | grep -E 'tilcdc|panel-dpi|edt_ft5x06'
       ls /sys/bus/i2c/devices/2-0038/input/

If the panel stays dark, reinsert the SD in the Mac and revert:
  sudo $DEBUGFS -w $ROOTFS_DEV <<EOF
  rm /boot/uEnv.txt
  mv /boot/uEnv.txt.pre-rlvgl /boot/uEnv.txt
  EOF
EOT
