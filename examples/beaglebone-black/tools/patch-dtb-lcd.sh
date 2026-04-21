#!/bin/bash
# patch-dtb-lcd.sh — Patch the active DTB for NHD-7.0CTP-CAPE-P LCD output.
#
# Run ON the BeagleBone Black after booting from the prepared SD card.
# Requires: python3, dtc (device-tree-compiler), sudo
#
# What it does:
#   1. Decompiles the active am335x-boneblack.dtb
#   2. Disables the TDA19988 HDMI bridge
#   3. Rewires LCDC endpoint from HDMI to a new panel node
#   4. Adds innolux,at070tn92 panel (800x480, timing-compatible with NHD)
#   5. Adds LCD pin mux (all 28 pins to Mode 0)
#   6. Recompiles and installs the patched DTB
#
# After running, reboot to activate. /dev/fb0 should appear at 800x480.
#
# Usage:
#   sudo bash /boot/firmware/patch-dtb-lcd.sh
#   # or from the project:
#   sudo bash tools/patch-dtb-lcd.sh

set -euo pipefail

KVER=$(uname -r)
DTB_DIR="/boot/dtbs/${KVER}"
DTB="${DTB_DIR}/am335x-boneblack.dtb"
BACKUP="${DTB}.orig"
WORK="/tmp/dtb-lcd-patch"

echo "=== BBB LCD DTB Patcher ==="
echo "Kernel: ${KVER}"
echo "DTB:    ${DTB}"
echo ""

if [ ! -f "$DTB" ]; then
    echo "ERROR: DTB not found at ${DTB}"
    exit 1
fi

mkdir -p "$WORK"

# Backup original
if [ ! -f "$BACKUP" ]; then
    cp "$DTB" "$BACKUP"
    echo "Original DTB backed up to ${BACKUP}"
else
    echo "Backup already exists at ${BACKUP}"
fi

# Decompile
echo "Decompiling DTB..."
dtc -I dtb -O dts -o "${WORK}/base.dts" "$DTB" 2>/dev/null

# Patch with Python
echo "Applying patches..."
python3 << 'PYEOF'
import re, sys

WORK = "/tmp/dtb-lcd-patch"

with open(f"{WORK}/base.dts", "r") as f:
    dts = f.read()

errors = []

# 1. Disable TDA19988 HDMI bridge
if re.search(r'tda19988@70\s*\{', dts):
    dts = re.sub(r'(tda19988@70\s*\{)', r'\1\n\t\t\t\t\t\t\tstatus = "disabled";', dts, count=1)
    print("  [OK] TDA19988 HDMI disabled")
else:
    errors.append("tda19988@70 node not found")

# 2. Find LCDC endpoint phandles
m = re.search(r'endpoint@0\s*\{\s*\n\s*remote-endpoint\s*=\s*<(0x[0-9a-f]+)>;\s*\n\s*phandle\s*=\s*<(0x[0-9a-f]+)>', dts)
if m:
    hdmi_ph = m.group(1)
    lcdc_ep_ph = m.group(2)
    print(f"  [OK] LCDC endpoint: remote={hdmi_ph}, phandle={lcdc_ep_ph}")
else:
    errors.append("LCDC endpoint@0 not found")
    sys.exit(1)

# 3. Allocate new phandles
phandles = [int(x, 16) for x in re.findall(r'phandle = <(0x[0-9a-f]+)>', dts)]
panel_ep_ph = max(phandles) + 1
pinmux_ph = panel_ep_ph + 1

# 4. Rewire LCDC endpoint to panel
dts = re.sub(
    r'(endpoint@0\s*\{\s*\n\s*remote-endpoint\s*=\s*<)0x[0-9a-f]+(>)',
    rf'\g<1>0x{panel_ep_ph:x}\2', dts, count=1
)
print(f"  [OK] LCDC endpoint rewired to panel (0x{panel_ep_ph:x})")

# 5. Add pinctrl to LCDC node
lcdc_match = re.search(r'(lcdc@0\s*\{[^}]*?)(compatible)', dts, re.DOTALL)
if lcdc_match:
    insert_at = lcdc_match.start(2)
    props = f'pinctrl-names = "default";\n\t\t\t\t\t\tpinctrl-0 = <0x{pinmux_ph:x}>;\n\t\t\t\t\t\t'
    dts = dts[:insert_at] + props + dts[insert_at:]
    print(f"  [OK] LCDC pinctrl set (0x{pinmux_ph:x})")

# 6. Add LCD pin mux group in pinmux@800
pinmux_node = f'''
\t\t\t\t\t\tbb-lcd-pins {{
\t\t\t\t\t\t\tphandle = <0x{pinmux_ph:x}>;
\t\t\t\t\t\t\tpinctrl-single,pins = <
\t\t\t\t\t\t\t\t0xa0 0x08 0xa4 0x08 0xa8 0x08 0xac 0x08
\t\t\t\t\t\t\t\t0xb0 0x08 0xb4 0x08 0xb8 0x08 0xbc 0x08
\t\t\t\t\t\t\t\t0xc0 0x08 0xc4 0x08 0xc8 0x08 0xcc 0x08
\t\t\t\t\t\t\t\t0xd0 0x08 0xd4 0x08 0xd8 0x08 0xdc 0x08
\t\t\t\t\t\t\t\t0xe0 0x08 0xe4 0x08 0xe8 0x08 0xec 0x08
\t\t\t\t\t\t\t\t0xf0 0x08 0xf4 0x08 0xf8 0x08 0xfc 0x08
\t\t\t\t\t\t\t\t0x100 0x08 0x104 0x08 0x108 0x08 0x10c 0x08
\t\t\t\t\t\t\t>;
\t\t\t\t\t\t}};
'''
pos = dts.find('nxp-hdmi-bonelt-off-pins')
if pos >= 0:
    i = dts.index('{', pos)
    depth = 0
    while i < len(dts):
        if dts[i] == '{': depth += 1
        elif dts[i] == '}': depth -= 1
        if depth == 0:
            dts = dts[:i+2] + '\n' + pinmux_node + dts[i+2:]
            print("  [OK] LCD pin mux group added")
            break
        i += 1

# 7. Add panel node before __symbols__
panel_node = f'''
\tpanel {{
\t\tcompatible = "innolux,at070tn92";
\t\tstatus = "okay";

\t\tport {{
\t\t\tendpoint {{
\t\t\t\tremote-endpoint = <{lcdc_ep_ph}>;
\t\t\t\tphandle = <0x{panel_ep_ph:x}>;
\t\t\t}};
\t\t}};
\t}};
'''
symbols_pos = dts.find('__symbols__')
if symbols_pos > 0:
    insert_pos = dts.rfind('\n', 0, symbols_pos)
    dts = dts[:insert_pos] + '\n' + panel_node + dts[insert_pos:]
    print("  [OK] Panel node added (innolux,at070tn92)")

with open(f"{WORK}/patched.dts", "w") as f:
    f.write(dts)

if errors:
    print(f"\nWARNINGS: {errors}")
else:
    print("\nAll patches applied successfully!")
PYEOF

# Compile
echo "Compiling patched DTB..."
dtc -f -I dts -O dtb -o "${WORK}/am335x-boneblack.dtb" "${WORK}/patched.dts" 2>&1 | grep -i error || true

# Install
echo "Installing patched DTB..."
cp "${WORK}/am335x-boneblack.dtb" "$DTB"
sync

echo ""
echo "=== DTB Patched Successfully ==="
echo "Reboot to activate the LCD panel."
echo "After reboot, /dev/fb0 should appear at 800x480."
echo ""
echo "To restore original: sudo cp ${BACKUP} ${DTB}"
