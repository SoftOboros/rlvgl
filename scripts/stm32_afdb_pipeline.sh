#!/usr/bin/env bash
# stm32_afdb_pipeline.sh - Package STM32 Open Pin Data into a compressed blob.
#
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

echo "Generating STM chip database"
echo "Temp Dir: $TMP_DIR"
python tools/afdb/stm32_xml_scraper.py --root "chips/stm/STM32_open_pin_data/mcu" --output "$SCRAPE_OUT"
mkdir -p "$SCRAPE_OUT/boards"
find chips/stm/STM32_open_pin_data/boards -name "*.ioc" | while read -r ioc; do
  bname="$(basename "$ioc" .ioc)"
  python tools/afdb/st_ioc_board.py --ioc "$ioc" --mcu-root "$SCRAPE_OUT/mcu" --board "$bname" --output "$SCRAPE_OUT/boards/$bname.json" || true
done
python tools/gen_pins.py --input "$SCRAPE_OUT/boards" --output chipdb/rlvgl-chips-stm/db

export RLVGL_CHIP_SRC=$PWD/chipdb/rlvgl-chips-stm/db
cargo test -p rlvgl-chips-stm

bin_path=$(find target -name chipdb.bin | head -n 1)
zstd -19 -f "$bin_path" -o chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst

du -h "$LOADER_BIN"

if [[ "$KEEP_TEMP" -eq 0 ]]; then
  rm -rf "$TMP_DIR"
else
  echo "Keeping temporary files in $TMP_DIR"
fi
