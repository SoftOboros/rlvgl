#!/usr/bin/env bash
# fdtput-lcd.sh — Patch -uboot.dtb on the BBB to enable tilcdc + NHD-7 panel,
# using fdtput only (no fdtoverlay, which wedges u-boot on this Bookworm image).
#
# Restores from .orig, then adds incrementally: bb-lcd-pins state, lcdc
# pinctrl-0 ref, /panel node with panel-timing + port/endpoint, and rewires
# the LCDC endpoint's remote-endpoint to the panel.
#
# Run on BBB: sudo bash ~/fdtput-lcd.sh

set -euo pipefail
PATH=/usr/bin:/usr/sbin:/bin:/sbin

KVER=$(uname -r)
DIR=/boot/dtbs/$KVER
DTB=$DIR/am335x-boneblack-uboot.dtb
LCDC=/ocp/interconnect@48000000/segment@300000/target-module@e000/lcdc@0
PINMUX=/ocp/interconnect@44c00000/segment@200000/target-module@10000/scm@0/pinmux@800

# Phandles not used in the base DTB (verified: max phandle in -uboot.dtb.orig
# is < 0x100).
BB_LCD_PHANDLE=0x200
PANEL_EP_PHANDLE=0x201

[ -f "$DTB.orig" ] || { echo "no $DTB.orig — aborting" >&2; exit 1; }
cp "$DTB.orig" "$DTB"

LCDC_EP_PHANDLE=0x202

echo "=== create lcdc@0/port/endpoint@0 (missing in -uboot.dtb) ==="
fdtput -c "$DTB" "$LCDC/port"
fdtput -c "$DTB" "$LCDC/port/endpoint@0"
fdtput -t i "$DTB" "$LCDC/port/endpoint@0" phandle "$LCDC_EP_PHANDLE"
# remote-endpoint written later after panel phandle is assigned

echo "=== bb-lcd-pins under $PINMUX ==="
fdtput -c "$DTB" "$PINMUX/bb-lcd-pins"
fdtput -t i "$DTB" "$PINMUX/bb-lcd-pins" pinctrl-single,pins \
    0xa0 0x8 0xa4 0x8 0xa8 0x8 0xac 0x8 \
    0xb0 0x8 0xb4 0x8 0xb8 0x8 0xbc 0x8 \
    0xc0 0x8 0xc4 0x8 0xc8 0x8 0xcc 0x8 \
    0xd0 0x8 0xd4 0x8 0xd8 0x8 0xdc 0x8 \
    0xe0 0x8 0xe4 0x8 0xe8 0x8 0xec 0x8 \
    0xf0 0x8 0xf4 0x8 0xf8 0x8 0xfc 0x8 \
    0x100 0x8 0x104 0x8 0x108 0x8 0x10c 0x8
fdtput -t i "$DTB" "$PINMUX/bb-lcd-pins" phandle "$BB_LCD_PHANDLE"

echo "=== lcdc pinctrl + enable ==="
fdtput -t s "$DTB" "$LCDC" status okay
fdtput -t s "$DTB" "$LCDC" pinctrl-names default
fdtput -t i "$DTB" "$LCDC" pinctrl-0 "$BB_LCD_PHANDLE"

echo "=== /panel ==="
fdtput -c "$DTB" /panel
fdtput -t s "$DTB" /panel compatible panel-dpi
fdtput -t s "$DTB" /panel status okay
fdtput -t i "$DTB" /panel connector-type 14
fdtput -t i "$DTB" /panel bus-format 0x1017
fdtput -t i "$DTB" /panel bpc 8
fdtput -t s "$DTB" /panel data-mapping rgb565

echo "=== /panel/panel-timing ==="
fdtput -c "$DTB" /panel/panel-timing
fdtput -t i "$DTB" /panel/panel-timing clock-frequency 33300000
fdtput -t i "$DTB" /panel/panel-timing hactive 800
fdtput -t i "$DTB" /panel/panel-timing vactive 480
fdtput -t i "$DTB" /panel/panel-timing hfront-porch 210
fdtput -t i "$DTB" /panel/panel-timing hback-porch 46
fdtput -t i "$DTB" /panel/panel-timing hsync-len 20
fdtput -t i "$DTB" /panel/panel-timing vfront-porch 22
fdtput -t i "$DTB" /panel/panel-timing vback-porch 23
fdtput -t i "$DTB" /panel/panel-timing vsync-len 10
fdtput -t i "$DTB" /panel/panel-timing hsync-active 0
fdtput -t i "$DTB" /panel/panel-timing vsync-active 0
fdtput -t i "$DTB" /panel/panel-timing de-active 1
fdtput -t i "$DTB" /panel/panel-timing pixelclk-active 0

echo "=== /panel/port/endpoint ==="
fdtput -c "$DTB" /panel/port
fdtput -c "$DTB" /panel/port/endpoint
fdtput -t i "$DTB" /panel/port/endpoint phandle "$PANEL_EP_PHANDLE"
fdtput -t i "$DTB" /panel/port/endpoint remote-endpoint "$LCDC_EP_PHANDLE"

echo "=== rewire lcdc endpoint -> panel ==="
fdtput -t i "$DTB" "$LCDC/port/endpoint@0" remote-endpoint "$PANEL_EP_PHANDLE"

echo "=== verify ==="
md5sum "$DTB" "$DTB.orig"
echo "panel:        $(fdtget -t s "$DTB" /panel compatible) / bus-format=$(fdtget -t i "$DTB" /panel bus-format)"
echo "bb-lcd-pins:  phandle=$(fdtget -t i "$DTB" "$PINMUX/bb-lcd-pins" phandle)"
echo "lcdc:         pinctrl-0=$(fdtget -t i "$DTB" "$LCDC" pinctrl-0)"
echo "endpoints:    lcdc->$(fdtget -t i "$DTB" "$LCDC/port/endpoint@0" remote-endpoint) panel->$(fdtget -t i "$DTB" /panel/port/endpoint remote-endpoint)"
