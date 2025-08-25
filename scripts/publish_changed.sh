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

# Detect vendor chip database crates - all listed here
#chipdb_crates=(
#  rlvgl-chips-stm
#  rlvgl-chips-nrf
#  rlvgl-chips-esp
#  rlvgl-chips-nxp
#  rlvgl-chips-silabs
#  rlvgl-chips-microchip
#  rlvgl-chips-renesas
#  rlvgl-chips-ti
#  rlvgl-chips-rp2040
#)

# Detect vendor chip database crates - active
chipdb_crates=(
  rlvgl-chips-stm
)
for crate in "${chipdb_crates[@]}"; do
  if git diff --name-only "$BASE" HEAD | grep -q "^chipdb/${crate}/"; then
    changed+=("$crate")
  fi
done

for crate in "${changed[@]}"; do
  echo "Publishing $crate"
  if [[ "$crate" == "rlvgl-chips-stm" ]]; then
    scripts/stm32_afdb_pipeline.sh
    git add chipdb/rlvgl-chips-stm/assets/chipdb.bin.zst
    cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN" --no-verify --allow-dirty || echo "⚠️ publish $crate failed."
  else
    cargo publish -p "$crate" --token "$CARGO_REGISTRY_TOKEN" --no-verify || echo "⚠️ publish $crate failed."
  fi
done
