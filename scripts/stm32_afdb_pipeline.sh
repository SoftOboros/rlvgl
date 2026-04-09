#!/usr/bin/env bash
# stm32_afdb_pipeline.sh - Package STM32 Open Pin Data into a compressed blob.
#
# Scrapes the entire STM32_open_pin_data repository into JSON, bundles it,
# compresses the archive with zstd, reports the resulting size, and optionally
# removes temporary files.

set -euov pipefail

TMP_DIR="$(mktemp -d)"
SCRAPE_OUT="$TMP_DIR/stm32_json"
ASSET_BIN="chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst"
KEEP_TEMP=${KEEP_TEMP:-0}

# Known-bad IOC files that currently fail overlay generation due to null pin
# context extraction. Keep this list narrow and temporary.
SKIP_IOC_BOARDS=(
  "N01_Discovery_STM32G0316-DISCO_STM32G031J6M_Board_AllConfig"
)

# Ensure submodules are present
#git submodule update --init --recursive

echo "Generating STM chip database"
echo "Temp Dir: $TMP_DIR"
python3 tools/afdb/stm32_xml_scraper.py --root "chips/stm/STM32_open_pin_data/mcu" --output "$SCRAPE_OUT"
mkdir -p "$SCRAPE_OUT/boards"
find chips/stm/STM32_open_pin_data/boards -name "*.ioc" | while read -r ioc; do
  bname="$(basename "$ioc" .ioc)"
  if printf '%s\n' "${SKIP_IOC_BOARDS[@]}" | grep -Fxq "$bname"; then
    echo "Skipping known-bad board overlay: $bname"
    continue
  fi
  python3 tools/afdb/st_ioc_board.py --ioc "$ioc" --mcu-root "$SCRAPE_OUT/mcu" --board "$bname" --output "$SCRAPE_OUT/boards/$bname.json" || true
done
python3 tools/gen_pins.py --input "$SCRAPE_OUT/boards" --output chipdb/rlvgl-chips-stm/db

export RLVGL_CHIP_SRC=$PWD/chipdb/rlvgl-chips-stm/db
cargo test -p rlvgl-chips-stm

mkdir -p "$(dirname "$ASSET_BIN")"
python3 tools/pack_chipdb.py --input chipdb/rlvgl-chips-stm/db --output "$ASSET_BIN"

du -h "$ASSET_BIN"

if [[ "$KEEP_TEMP" -eq 0 ]]; then
  rm -rf "$TMP_DIR"
else
  echo "Keeping temporary files in $TMP_DIR"
fi
