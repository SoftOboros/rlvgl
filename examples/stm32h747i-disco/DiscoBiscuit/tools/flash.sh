#!/usr/bin/env bash
set -e

# Flash the built firmware to STM32H747I-DISCO using available ST tools
BUILD_DIR=${1:-build}
BIN="$BUILD_DIR/DiscoBiscuit_CM7.bin"

if [ ! -f "$BIN" ]; then
  echo "Binary $BIN not found. Run ./scripts/build.sh first." >&2
  exit 1
fi

if command -v STM32_Programmer_CLI >/dev/null 2>&1; then
  STM32_Programmer_CLI -c port=SWD -d "$BIN" 0x08000000 -rst
elif command -v st-flash >/dev/null 2>&1; then
  st-flash --reset write "$BIN" 0x08000000
else
  echo "No flashing tool found. Install STM32_Programmer_CLI or st-flash." >&2
  exit 1
fi
