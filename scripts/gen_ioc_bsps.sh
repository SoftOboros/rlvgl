#!/usr/bin/env bash
# gen_ioc_bsps.sh - regenerate BSPs from all STM32_open_pin_data .ioc files.
#
# Iterates over each CubeMX .ioc under chips/stm/STM32_open_pin_data/boards and
# invokes rlvgl-creator to emit HAL and PAC BSPs.
# Requires rlvgl-creator to be built (cargo build -p rlvgl-creator).

set -euo pipefail

RLVGL_CREATOR=${RLVGL_CREATOR:-target/debug/rlvgl-creator}
AF_JSON=${AF_JSON:-stm32_af.json}
OUT_DIR=${OUT_DIR:-chips/stm/bsps/src}
OPEN_PIN_DATA=${OPEN_PIN_DATA:-chips/stm/STM32_open_pin_data}
BOARD_DIR="$OPEN_PIN_DATA/boards"

if [ ! -x "$RLVGL_CREATOR" ]; then
  echo "warning: rlvgl-creator not found at $RLVGL_CREATOR" >&2
  exit 0
fi

if [ ! -d "$BOARD_DIR" ]; then
  echo "updating STM32_open_pin_data submodule..."
  if ! git submodule update --init "$OPEN_PIN_DATA" >/dev/null 2>&1; then
    echo "warning: unable to update $OPEN_PIN_DATA; skipping BSP generation" >&2
    exit 0
  fi
fi

if [ ! -d "$BOARD_DIR" ]; then
  echo "warning: $BOARD_DIR missing; skipping BSP generation" >&2
  exit 0
fi

find "$BOARD_DIR" -name '*.ioc' | while read -r ioc; do
  board="$(basename "$ioc" .ioc)"
  for layout in one-file per-peripheral; do
    out_dir="$OUT_DIR/$board/$layout"
    layout_flag=""
    if [ "$layout" = per-peripheral ]; then
      layout_flag="--per-peripheral"
    fi
    "$RLVGL_CREATOR" bsp from-ioc "$ioc" "$AF_JSON" \
      --emit-hal --emit-pac --grouped-writes --with-deinit --allow-reserved $layout_flag \
      --out "$out_dir" || echo "failed: $board ($layout)"
  done
done
