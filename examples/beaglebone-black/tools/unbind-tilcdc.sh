#!/usr/bin/env bash
# unbind-tilcdc.sh — Release the AM3358 LCDC from the kernel tilcdc driver
# so rlvgl-bbb can program it directly via /dev/mem.
#
# Run as root on the BBB before launching rlvgl-bbb:
#   sudo bash unbind-tilcdc.sh
#
# This is safe to re-run — a rebind happens automatically on reboot.

set -euo pipefail

DRV=/sys/bus/platform/drivers/tilcdc
if [ ! -d "$DRV" ]; then
    echo "tilcdc driver not loaded (nothing to unbind)."
    exit 0
fi

# Find the LCDC platform device name (typically "4830e000.lcdc").
DEV=$(ls "$DRV" 2>/dev/null | grep -E '^[0-9a-f]+\.lcdc$' | head -1 || true)
if [ -z "$DEV" ]; then
    echo "tilcdc is loaded but no LCDC device is bound — nothing to do."
    exit 0
fi

echo "Unbinding $DEV from tilcdc..."
echo "$DEV" > "$DRV/unbind"
echo "Done. /dev/fb0 should be gone; LCDC registers are now owned by userspace."
