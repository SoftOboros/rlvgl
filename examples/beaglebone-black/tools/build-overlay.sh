#!/usr/bin/env bash
# build-overlay.sh — Compile the BB-NHD7-CAPE display overlay to .dtbo.
#
# The `-@` flag is critical: it emits a __symbols__ node in the compiled
# blob so the u-boot overlay applier can resolve &lcdc / &am33xx_pinmux
# / &i2c2 / &gpio3 / &vmmcsd_fixed against the live DTB at boot. Without
# it, fixup references stay unresolved and the overlay silently fails to
# apply. This was the invisible failure mode on the older fdtoverlay
# path (kernel DTB compiled without -@).
#
# We use cpp(1) first so #include <dt-bindings/...> resolves against the
# host system's Linux kernel headers (linux-libc-dev on macOS via brew,
# or the kernel-source dir). Fallback: strip the #includes and inline the
# handful of bindings we actually reference.
#
# Usage:
#   bash examples/beaglebone-black/tools/build-overlay.sh
#   -> writes target/bbb-overlays/BB-NHD7-CAPE.dtbo

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

SRC="${PROJECT_DIR}/linux/BB-NHD7-CAPE.dts"
OUT_DIR="${REPO_DIR}/target/bbb-overlays"
OUT="${OUT_DIR}/BB-NHD7-CAPE.dtbo"

if [ ! -f "$SRC" ]; then
    echo "ERROR: source overlay not found at $SRC" >&2
    exit 1
fi

if ! command -v dtc >/dev/null 2>&1; then
    echo "ERROR: dtc not in PATH. Install with:" >&2
    echo "  brew install dtc" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"

# Locate dt-bindings headers. Check the common places a macOS dev
# machine might have them before falling back to an inline shim.
DTBINDINGS_CANDIDATES=(
    "/opt/homebrew/opt/linux-headers/include"
    "/usr/local/opt/linux-headers/include"
    "${DTBINDINGS:-}"
)
INCLUDE_DIR=""
for d in "${DTBINDINGS_CANDIDATES[@]}"; do
    [ -z "$d" ] && continue
    if [ -d "$d/dt-bindings" ]; then
        INCLUDE_DIR="$d"
        break
    fi
done

STAGE="${OUT_DIR}/BB-NHD7-CAPE.cpp.dts"

SHIM="${OUT_DIR}/BB-NHD7-CAPE.shim.dts"

if [ -n "$INCLUDE_DIR" ]; then
    echo "[1/2] cpp -nostdinc -I${INCLUDE_DIR} -undef -x assembler-with-cpp"
    cc -E -P -nostdinc -I"$INCLUDE_DIR" -undef -x assembler-with-cpp \
        -o "$STAGE" "$SRC"
else
    echo "[1/2] no dt-bindings headers found — running cpp with inline shim"
    # Inline the few symbols we reference, then run cpp to expand them.
    # cpp is happy to leave the DTS directives alone — it only acts on the
    # tokens it recognises as C macros.
    {
        echo '#define IRQ_TYPE_NONE          0'
        echo '#define IRQ_TYPE_EDGE_RISING   1'
        echo '#define IRQ_TYPE_EDGE_FALLING  2'
        echo '#define IRQ_TYPE_EDGE_BOTH     3'
        echo '#define IRQ_TYPE_LEVEL_HIGH    4'
        echo '#define IRQ_TYPE_LEVEL_LOW     8'
        # Drop the #include line so cpp doesn't try to find the header.
        grep -v '^#include ' "$SRC"
    } > "$SHIM"
    cc -E -P -nostdinc -undef -x assembler-with-cpp -o "$STAGE" "$SHIM"
fi

echo "[2/2] dtc -@ -I dts -O dtb -o $OUT $STAGE"
dtc -@ -I dts -O dtb -o "$OUT" "$STAGE"

# Quick sanity check — overlay should carry __symbols__ (that's what -@
# emits) and a /fragment@N node referencing each phandle target.
if command -v fdtdump >/dev/null 2>&1; then
    if ! fdtdump "$OUT" | grep -q '__symbols__\|__fixups__'; then
        echo "WARNING: compiled .dtbo does not contain __symbols__ / __fixups__." >&2
        echo "u-boot overlay fixups may fail to resolve. Check that dtc is" >&2
        echo "recent enough to honour -@ (v1.4.7+ required)." >&2
    fi
fi

echo
echo "=== built $OUT ($(du -h "$OUT" | awk '{print $1}')) ==="
echo "Next: bash tools/deploy-overlay-sd-ext4.sh [/dev/diskNs3]"
