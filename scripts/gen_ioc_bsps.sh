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

mapfile -t iocs < <(find "$BOARD_DIR" -name '*.ioc' | sort)
total=${#iocs[@]}
echo "Generating BSPs for $total boards..."
count=0
for ioc in "${iocs[@]}"; do
  count=$((count + 1))
  raw="$(basename "$ioc" .ioc)"
  IFS='_' read -r -a parts <<<"$raw"
  parts=("${parts[@]:1}")
  board=""
  for part in "${parts[@]}"; do
    lpart="$(echo "$part" | tr '[:upper:]' '[:lower:]')"
    case "$lpart" in
      nucleo|discovery|evaluation|connectivity|expansion|board)
        continue
        ;;
      *)
        board="$part"
        break
        ;;
    esac
  done
  board="${board:-$raw}"
  board="$(echo "$board" | tr '[:upper:]' '[:lower:]' | tr '-' '_')"
  echo "[$count/$total] $board"
  out_dir="$OUT_DIR/$board"
  if "$RLVGL_CREATOR" bsp from-ioc "$ioc" "$AF_JSON" \
    --emit-hal --emit-pac --grouped-writes --with-deinit --allow-reserved --per-peripheral \
    --out "$out_dir"; then
    echo "    done"
  else
    echo "    failed: $board" >&2
  fi
done

echo "Generating lib.rs..."
"$RLVGL_CREATOR" gen-lib \
  --src "$OUT_DIR" \
  --out "$OUT_DIR/lib.rs" \
  --prelude hal:split \
  --features hal,pac,split,flat,summaries,pinreport \
  --family-feature-prefix stm32- || echo "warning: gen-lib failed" >&2

# Synchronize stm32-* feature flags in the BSP crate after regeneration.
echo "Updating Cargo feature flags..."
python tools/update_stm_bsp_features.py || echo "warning: feature sync failed" >&2
cargo fmt -p rlvgl-bsps-stm --all