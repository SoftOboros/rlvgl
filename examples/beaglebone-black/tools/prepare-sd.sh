#!/usr/bin/env bash
# prepare-sd.sh — Prepare a microSD card for BeagleBone Black + NHD LCD cape.
#
# This is the main setup entry point. It flashes the Bookworm image, sets
# credentials via sysconf.txt, and copies the DTB patch script.
#
# IMPORTANT: The BBB must boot from SD when the LCD cape is attached because
# LCD_DATA[0:7] share pins with the eMMC data bus on the P8 header.
#
# Usage:
#   BB_PASSWORD='YourPassword' ./tools/prepare-sd.sh /dev/diskN
#
# After this script completes:
#   1. Insert SD into BBB, attach cape, connect 5V barrel + USB
#   2. Hold S2 during power-on (first time only, until eMMC boot is disabled)
#   3. SSH to debian@192.168.6.2
#   4. Disable eMMC boot:
#        sudo dd if=/dev/zero of=/dev/mmcblk1 bs=512 seek=256 count=64
#   5. Patch the DTB for LCD:
#        sudo bash /boot/firmware/patch-dtb-lcd.sh
#   6. Reboot — display should be active
#
# Prerequisites:
#   - macOS with diskutil
#   - Bookworm image at /tmp/bbb-bookworm.img.xz (see docs/BEAGLEBONE-BLACK.md)
#
# Download the image:
#   curl -L -o /tmp/bbb-bookworm.img.xz \
#     "https://files.beagle.cc/file/beagleboard-public-2021/images/am335x-debian-12.13-base-v6.12-armhf-2026-03-17-4gb.img.xz"

set -euo pipefail

DISK="${1:?Usage: $0 /dev/diskN}"
IMAGE="${BB_IMAGE:-/tmp/bbb-bookworm.img.xz}"
PASSWORD="${BB_PASSWORD:?Set BB_PASSWORD to the desired user password}"
# Prefer BB_SSH_KEY if set; else probe common key names in order.
if [ -n "${BB_SSH_KEY:-}" ]; then
    SSH_KEY="$BB_SSH_KEY"
else
    for cand in id_ed25519.pub id_rsa.pub id_ecdsa.pub; do
        if [ -f "${HOME}/.ssh/$cand" ]; then
            SSH_KEY="${HOME}/.ssh/$cand"
            break
        fi
    done
    SSH_KEY="${SSH_KEY:-${HOME}/.ssh/id_ed25519.pub}"
fi
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Sanity checks
if [ ! -f "$IMAGE" ]; then
    echo "ERROR: Image not found at $IMAGE"
    echo ""
    echo "Download with:"
    echo "  curl -L -o /tmp/bbb-bookworm.img.xz \\"
    echo '    "https://files.beagle.cc/file/beagleboard-public-2021/images/am335x-debian-12.13-base-v6.12-armhf-2026-03-17-4gb.img.xz"'
    exit 1
fi

echo "=== BeagleBone Black LCD Cape — SD Card Setup ==="
echo ""
echo "  Disk:    $DISK"
echo "  Image:   $IMAGE ($(du -h "$IMAGE" | awk '{print $1}'))"
echo "  SSH key: ${SSH_KEY}"
echo ""
echo "This will ERASE ${DISK}. Press Ctrl-C to abort, Enter to continue."
read -r

# Step 1: Flash
echo "[1/3] Flashing Bookworm image..."
diskutil unmountDisk "$DISK" 2>/dev/null || true
RDISK="${DISK/disk/rdisk}"
echo "  Writing to ${RDISK} (this takes ~2 minutes)..."
echo "  You may be prompted for your Mac password by sudo."
xzcat "$IMAGE" | sudo dd of="$RDISK" bs=4m 2>&1
sync
echo "  Flash complete."

# Step 2: Mount boot partition and configure
echo "[2/3] Configuring credentials..."
sleep 3  # Wait for partitions to appear
diskutil mountDisk "$DISK" 2>/dev/null || true
sleep 2

BOOT_MNT=""
for vol in "/Volumes/BOOT" "/Volumes/boot"; do
    if [ -f "$vol/sysconf.txt" ]; then BOOT_MNT="$vol"; break; fi
done

if [ -z "$BOOT_MNT" ]; then
    echo "  WARNING: Boot partition not found. Credentials not set."
    echo "  You will need to use the serial console for first login."
else
    echo "  Boot partition: $BOOT_MNT"

    # Set username and password
    sed -i '' "s/^#user_name=.*/user_name=debian/" "$BOOT_MNT/sysconf.txt"
    sed -i '' "s/^#user_password=.*/user_password=${PASSWORD}/" "$BOOT_MNT/sysconf.txt"
    echo "  Username: debian, password: set"

    # Install SSH key
    if [ -f "$SSH_KEY" ]; then
        KEY_CONTENT=$(cat "$SSH_KEY")
        sed -i '' "s|^#user_authorized_key=.*|user_authorized_key=${KEY_CONTENT}|" "$BOOT_MNT/sysconf.txt"
        echo "  SSH key: installed from ${SSH_KEY}"
    else
        echo "  SSH key: skipped (${SSH_KEY} not found)"
    fi
fi

# Step 3: Copy the DTB patch script to the boot partition
echo "[3/3] Copying DTB patch script..."
if [ -n "$BOOT_MNT" ] && [ -f "${SCRIPT_DIR}/patch-dtb-lcd.sh" ]; then
    cp "${SCRIPT_DIR}/patch-dtb-lcd.sh" "$BOOT_MNT/"
    echo "  patch-dtb-lcd.sh copied to boot partition"
else
    echo "  WARNING: Could not copy patch script"
fi

# Eject
sync
diskutil eject "$DISK" 2>/dev/null || true

echo ""
echo "=== SD Card Ready ==="
echo ""
echo "Next steps (see docs/BEAGLEBONE-BLACK.md for details):"
echo ""
echo "  1. Insert SD into BBB (slot under the board)"
echo "  2. Attach NHD-7.0CTP-CAPE-P cape"
echo "  3. Connect 5V 2A barrel jack + USB-mini"
echo "  4. Hold S2 during power-on (until eMMC boot is disabled)"
echo "  5. SSH to debian@192.168.6.2 (use your configured password)"
echo "  6. Disable eMMC boot (one-time):"
echo "       sudo dd if=/dev/zero of=/dev/mmcblk1 bs=512 seek=256 count=64"
echo "  7. Patch DTB for LCD output:"
echo "       sudo bash /boot/firmware/patch-dtb-lcd.sh"
echo "  8. Reboot"
echo ""
echo "  NOTE: Step 7 must be run AFTER first boot completes"
echo "  (bbbio-set-sysconf overwrites DTBs on first boot)."
