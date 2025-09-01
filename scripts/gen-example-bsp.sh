#!/usr/bin/env bash
# Regenerate STM32H747I-DISCO BSP from its CubeMX .ioc using rlvgl-creator.
# Requires canonical STM32 DB embedded (RLVGL_CHIP_SRC or assets in vendor crate).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
IOC="$ROOT/examples/stm32h747i-disco/DiscoBiscuit.ioc"
OUT_DIR="$ROOT/examples/stm32h747i-disco/bsp"

if [[ ! -f "$IOC" ]]; then
  echo "missing .ioc: $IOC" >&2
  exit 1
fi

cargo build --bin rlvgl-creator --features creator >/dev/null

target/debug/rlvgl-creator bsp from-ioc "$IOC" \
  --emit-hal --emit-pac --grouped-writes --with-deinit --one-file \
  --out "$OUT_DIR"

echo "BSP regenerated at $OUT_DIR"
