#!/usr/bin/env bash
# stm32_afdb_pipeline.sh - Package STM32 Open Pin Data into a compressed blob.
# Scrapes the entire STM32_open_pin_data repository into JSON, bundles it,
# compresses the archive with zstd, reports the resulting size, and optionally
# removes temporary files.

set -euov pipefail

TMP_DIR="$(mktemp -d)"
SCRAPE_OUT="$TMP_DIR/stm32_json"
LOADER_BIN="chipdb/rlvgl-chips-stm/db/loader.bin.zst"
KEEP_TEMP=${KEEP_TEMP:-0}

# Ensure submodules are present
#git submodule update --init --recursive

echo $TMP_DIR
python tools/afdb/stm32_xml_scraper.py --root "chips/stm/STM32_open_pin_data/mcu" --output "$SCRAPE_OUT"
python tools/afdb/st_extract_af.py --input "chips/stm/STM32_open_pin_data/boards" --output "$SCRAPE_OUT/boards" --mcu-root "$SCRAPE_OUT/mcu"

tar -cf - -C "$SCRAPE_OUT" . | zstd -19 -f -o "$LOADER_BIN"

du -h "$LOADER_BIN"

if [[ "$KEEP_TEMP" -eq 0 ]]; then
  rm -rf "$TMP_DIR" "$LOADER_BIN"
else
  echo "Keeping temporary files in $TMP_DIR and $LOADER_BIN"
fi
