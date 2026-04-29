#!/usr/bin/env bash
# deploy-bare-sd.sh — Copy rlvgl-bbb-bare.bin + auto-exec U-Boot script to
# the BBB SD card's FAT partition, so U-Boot chainloads the bare-metal
# binary on the next power-on with no serial console needed.
#
# Usage (macOS):
#   1. Power off BBB, pull SD, insert into Mac reader
#   2. Wait for /Volumes/BOOT to mount
#   3. ./tools/deploy-bare-sd.sh
#   4. `diskutil eject /Volumes/BOOT` (script does this)
#   5. Insert SD into BBB, hold S2, apply 5V power
#   6. Expected: solid dark-blue panel after ~3s
#
# What this script writes to the FAT partition:
#   - rlvgl-bbb-bare.bin    (our flat binary, loaded to 0x82000000)
#   - boot.scr              (U-Boot script: fatload + go, picked up by
#                            distro_bootcmd's scan_dev_for_scripts)
#   - uEnv.txt              (stronger hook — envboot runs this first,
#                            overriding any Linux-boot uEnv.txt in ext4)
#   - uEnv.txt.rlvgl-bak    (timestamped backup if a prior uEnv.txt
#                            existed on FAT; safe to delete to restore)
#
# Revert: reinsert SD in Mac, `rm /Volumes/BOOT/uEnv.txt /Volumes/BOOT/boot.scr`.
# If the FAT partition previously had its own uEnv.txt, restore it from
# the .rlvgl-bak file.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

BIN="${REPO_DIR}/target/armv7a-none-eabihf/release/rlvgl-bbb-bare.bin"
SCR="${SCRIPT_DIR}/boot-bare.scr"
BOOT_MNT="${BOOT_MNT:-/Volumes/BOOT}"

if [ ! -f "$BIN" ]; then
    echo "ERROR: $BIN not found. Run tools/build-bare.sh first." >&2
    exit 1
fi

if [ ! -f "$SCR" ]; then
    echo "ERROR: $SCR not found. Regenerate with:" >&2
    echo "  mkimage -A arm -O linux -T script -C none -a 0 -e 0 \\" >&2
    echo "      -n rlvgl-bbb-bare -d $SCRIPT_DIR/boot-bare.cmd $SCR" >&2
    exit 1
fi

if [ ! -d "$BOOT_MNT" ]; then
    echo "ERROR: $BOOT_MNT not mounted." >&2
    echo "Insert the SD card in your Mac and wait for it to mount." >&2
    exit 1
fi

echo "=== deploy-bare-sd → $BOOT_MNT ==="

# 1. Copy flat binary
echo "[1/3] cp rlvgl-bbb-bare.bin"
cp "$BIN" "$BOOT_MNT/rlvgl-bbb-bare.bin"

# 2. Copy mkimage-blessed boot script
echo "[2/3] cp boot.scr"
cp "$SCR" "$BOOT_MNT/boot.scr"

# 3. uEnv.txt override — envboot picks this up before any scripted fallback
echo "[3/3] write uEnv.txt (with backup if needed)"
if [ -f "$BOOT_MNT/uEnv.txt" ]; then
    BACKUP="$BOOT_MNT/uEnv.txt.rlvgl-bak-$(date +%Y%m%d%H%M%S)"
    if ! ls "$BOOT_MNT"/uEnv.txt.rlvgl-bak* >/dev/null 2>&1; then
        cp "$BOOT_MNT/uEnv.txt" "$BACKUP"
        echo "    backed up existing uEnv.txt → $(basename "$BACKUP")"
    fi
fi

cat > "$BOOT_MNT/uEnv.txt" <<'EOF'
# rlvgl bare-metal chainload override.
# envboot loads this from mmc 0:1 (SD FAT partition) and runs
# uenvcmd before any Linux distro-boot logic. Remove this file
# or restore uEnv.txt.rlvgl-bak-* to re-enable normal boot.
#
# Before handing control to the flat binary we light all 4 USR
# LEDs as a u-boot-stage signature. If you never see 4 LEDs briefly
# lit, uenvcmd is NOT being picked up — u-boot is falling through
# to its normal Linux boot path.
uenvcmd=echo "rlvgl override"; led usr0 on; led usr1 on; led usr2 on; led usr3 on; fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin; go 0x82000000
EOF

sync

echo ""
echo "=== written ==="
ls -la "$BOOT_MNT/rlvgl-bbb-bare.bin" "$BOOT_MNT/boot.scr" "$BOOT_MNT/uEnv.txt"
echo ""
echo "Ejecting $BOOT_MNT ..."
diskutil eject "$BOOT_MNT" 2>/dev/null || {
    echo "  (diskutil eject failed — unmount manually before removing the SD)"
}

cat <<'EOT'

=== next ===
1. Remove SD from Mac reader
2. Insert SD into BBB (slot under the board)
3. Hold S2 (boot button near SD slot), then apply 5V barrel power
4. Release S2 after ~1s
5. Expected within ~3s:
     - Solid dark-blue 800x480 panel (ARGB 0xFF_00_40_80)
   If the panel stays dark:
     - Power off
     - Reinsert SD in Mac reader
     - rm /Volumes/BOOT/uEnv.txt /Volumes/BOOT/boot.scr
     - (if a uEnv.txt.rlvgl-bak-* exists, restore it)
     - This reverts to normal Linux boot; you can iterate from there
EOT
