#!/usr/bin/env bash
set -euo pipefail

BASE=${1:-origin/main}

changed=()

if git diff --name-only "$BASE" HEAD | grep -q '^core/'; then
  changed+=("rlvgl-core")
fi
if git diff --name-only "$BASE" HEAD | grep -q '^widgets/'; then
  changed+=("rlvgl-widgets")
fi
if git diff --name-only "$BASE" HEAD | grep -q '^ui/'; then
  changed+=("rlvgl-ui")
fi
if git diff --name-only "$BASE" HEAD | grep -q '^platform/'; then
  changed+=("rlvgl-platform")
fi
if git diff --name-only "$BASE" HEAD | grep -q -e '^src/' -e '^Cargo.toml' -e '^examples/'; then
  changed+=("rlvgl")
fi

# Detect vendor chip database crates
chipdb_crates=(
  rlvgl-chips-stm
  rlvgl-chips-nrf
  rlvgl-chips-esp
  rlvgl-chips-nxp
  rlvgl-chips-silabs
  rlvgl-chips-microchip
  rlvgl-chips-renesas
  rlvgl-chips-ti
  rlvgl-chips-rp2040
)
for crate in "${chipdb_crates[@]}"; do
  if git diff --name-only "$BASE" HEAD | grep -q "^chipdb/${crate}/"; then
    changed+=("$crate")
  fi
done

for crate in "${changed[@]}"; do
  echo "Publishing $crate"
  if [[ "$crate" == "rlvgl-chips-stm" ]]; then
    echo "Generating STM chip database"
    python tools/afdb/st_extract_af.py --input chips/stm/STM32_open_pin_data --output /tmp/stm_json
    python tools/gen_pins.py --input /tmp/stm_json --output chipdb/rlvgl-chips-stm/db
    export RLVGL_CHIP_SRC=chipdb/rlvgl-chips-stm/db
    cargo test -p rlvgl-chips-stm
    bin_path=$(find target -name chipdb.bin | head -n 1)
    zstd -19 -f "$bin_path" -o chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst
    git add chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst
    cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN" --no-verify --allow-dirty || echo "⚠️ publish $crate failed."
  else
    cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN" --no-verify || echo "⚠️ publish $crate failed."
  fi
done
