#!/usr/bin/env bash
# stm32_afdb_pipeline.sh - Convert STM32 XML and CubeMX IOC to compressed IR.
# Runs the AFDB pipeline: import MCU XML, build catalog, process IOC,
# encode and compress catalog to a zstd blob, report size, and clean up.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
LOADER_BIN="$ROOT_DIR/loader.bin.zst"

# Ensure submodules are present
 git -C "$ROOT_DIR" submodule update --init --recursive

IOC_FILE="$ROOT_DIR/tests/fixtures/simple.ioc"
MCU_NAME="$(grep -E '^Mcu.Name=' "$IOC_FILE" | cut -d'=' -f2)"
MCU_XML="$ROOT_DIR/chips/stm/STM32_open_pin_data/mcu/${MCU_NAME}.xml"

python -m tools.afdb.cli import-mcu --in "$MCU_XML" --out "$TMP_DIR/${MCU_NAME}.json"
python -m tools.afdb.cli build-catalog --mcu "$TMP_DIR/${MCU_NAME}.json" --out "$TMP_DIR/catalog.json"
python tools/afdb/st_ioc_board.py --ioc "$IOC_FILE" --mcu-root "$TMP_DIR" --board SampleBoard --output "$TMP_DIR/board.json"
python -m tools.afdb.cli encode-ir --catalog "$TMP_DIR/catalog.json" --out "$LOADER_BIN"

du -h "$LOADER_BIN"

rm -rf "$TMP_DIR" "$LOADER_BIN"
