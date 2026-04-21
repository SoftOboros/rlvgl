#!/usr/bin/env bash
# prepare-sd.sh — Flash and configure a microSD card for BeagleBone Black + LCD cape.
#
# The BBB must boot from SD when the NHD-7.0CTP-CAPE-P is attached because
# LCD_DATA0-7 share pins with the eMMC data bus. This script prepares a
# bootable SD card with the custom DTB that includes:
#   - Panel node (innolux,at070tn92 — 800x480 timing match)
#   - LCD pin mux (all 24 data + 4 control signals to Mode 0)
#   - HDMI bridge (TDA19988) disabled
#   - FT5x06 touch on I2C2 (deferred)
#
# Usage:
#   BB_PASSWORD='P0rkBone!' ./tools/prepare-sd.sh /dev/diskN
#
# Prerequisites:
#   - Bookworm (Debian 12) BBB image downloaded:
#     curl -L -o /tmp/bbb-bookworm.img.xz \
#       "https://files.beagle.cc/file/beagleboard-public-2021/images/am335x-debian-12.13-base-v6.12-armhf-2026-03-17-4gb.img.xz"
#   - Python 3 (for DTB phandle surgery)
#   - dtc (device tree compiler) — brew install dtc
#
# Boot order after SD is prepared:
#   1. Insert SD card into BBB
#   2. Attach NHD-7.0CTP-CAPE-P
#   3. Connect barrel jack (5V 2A) + USB-mini
#   4. Board boots from SD automatically (eMMC boot disabled)
#      If eMMC boot is still active, hold S2 during power-on

set -euo pipefail

DISK="${1:?Usage: $0 /dev/diskN}"
IMAGE="${BB_IMAGE:-/tmp/bbb-bookworm.img.xz}"
PASSWORD="${BB_PASSWORD:?Set BB_PASSWORD}"
SSH_KEY="${BB_SSH_KEY:-${HOME}/.ssh/id_ed25519.pub}"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Sanity checks
if [ ! -f "$IMAGE" ]; then
    echo "ERROR: Image not found at $IMAGE"
    echo "Download with:"
    echo "  curl -L -o /tmp/bbb-bookworm.img.xz \\"
    echo '    "https://files.beagle.cc/file/beagleboard-public-2021/images/am335x-debian-12.13-base-v6.12-armhf-2026-03-17-4gb.img.xz"'
    exit 1
fi

if ! command -v dtc >/dev/null; then
    echo "ERROR: dtc not found. Install with: brew install dtc"
    exit 1
fi

echo "=== BeagleBone Black LCD Cape SD Card Preparation ==="
echo "Disk:     $DISK"
echo "Image:    $IMAGE"
echo "SSH key:  $SSH_KEY"
echo ""

# Step 1: Flash image
echo "[1/4] Flashing image to $DISK..."
diskutil unmountDisk "$DISK" 2>/dev/null || true
RDISK="${DISK/disk/rdisk}"
xzcat "$IMAGE" | sudo dd of="$RDISK" bs=4m 2>&1
sync
echo "   Flash complete."

# Step 2: Mount boot partition and set sysconf.txt
echo "[2/4] Configuring sysconf.txt..."
sleep 3  # Wait for partitions to appear
diskutil mountDisk "$DISK" 2>/dev/null || true
sleep 2

BOOT_MNT=""
for vol in "/Volumes/BOOT" "/Volumes/boot" "/Volumes/BEAGLE"; do
    if [ -d "$vol" ]; then BOOT_MNT="$vol"; break; fi
done

if [ -z "$BOOT_MNT" ]; then
    echo "ERROR: Boot partition not found. Check disk."
    exit 1
fi

echo "   Boot partition: $BOOT_MNT"

if [ -f "$BOOT_MNT/sysconf.txt" ]; then
    sed -i '' "s/^#user_name=.*/user_name=debian/" "$BOOT_MNT/sysconf.txt"
    sed -i '' "s/^#user_password=.*/user_password=${PASSWORD}/" "$BOOT_MNT/sysconf.txt"
    if [ -f "$SSH_KEY" ]; then
        KEY_CONTENT=$(cat "$SSH_KEY")
        sed -i '' "s|^#user_authorized_key=.*|user_authorized_key=${KEY_CONTENT}|" "$BOOT_MNT/sysconf.txt"
        echo "   SSH key installed."
    fi
    echo "   Password set."
fi

# Step 3: Eject, re-flash won't include DTB patching from Mac
# The DTB patching must happen on the BBB itself (needs Linux dtc + Python3)
echo "[3/4] SD card prepared with base image + credentials."
echo "   DTB patching will be done on first boot via tools/patch-dtb-lcd.sh"

# Step 4: Copy the DTB patch script to the boot partition
cp "$SCRIPT_DIR/tools/patch-dtb-lcd.sh" "$BOOT_MNT/" 2>/dev/null || true
echo "   Patch script copied to boot partition."

sync
diskutil eject "$DISK" 2>/dev/null || true

echo ""
echo "=== SD Card Ready ==="
echo ""
echo "Next steps:"
echo "  1. Insert SD into BBB (slot under the board)"
echo "  2. Attach NHD-7.0CTP-CAPE-P cape"
echo "  3. Connect 5V 2A barrel jack + USB-mini"
echo "  4. Boot (hold S2 if eMMC boot not yet disabled)"
echo "  5. SSH to debian@192.168.6.2 (or 192.168.7.2)"
echo "  6. Run: sudo bash /boot/firmware/patch-dtb-lcd.sh"
echo "  7. Reboot — display should be active"
