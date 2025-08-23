#!/usr/bin/env bash
# stm32_afdb_pipeline.sh - Package STM32 Open Pin Data into a compressed blob.
# Scrapes the entire STM32_open_pin_data repository into JSON, bundles it,
# compresses the archive with zstd, reports the resulting size, and optionally
# removes temporary files.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
SCRAPE_OUT="$TMP_DIR/stm32_json"
LOADER_BIN="$ROOT_DIR/loader.bin.zst"
KEEP_TEMP=${KEEP_TEMP:-0}

# Ensure submodules are present
git -C "$ROOT_DIR" submodule update --init --recursive

python tools/afdb/stm32_xml_scraper.py --root "$ROOT_DIR/chips/stm/STM32_open_pin_data" --output "$SCRAPE_OUT"
python tools/afdb/st_extract_af.py --input "$ROOT_DIR/chips/stm/STM32_open_pin_data/boards" --output "$SCRAPE_OUT/boards" --mcu-root "$SCRAPE_OUT/mcu"

tar -cf - -C "$SCRAPE_OUT" . | zstd -19 -f -o "$LOADER_BIN"

du -h "$LOADER_BIN"

if [[ "$KEEP_TEMP" -eq 0 ]]; then
  rm -rf "$TMP_DIR" "$LOADER_BIN"
else
  echo "Keeping temporary files in $TMP_DIR and $LOADER_BIN"
fi
