#!/usr/bin/env bash
set -euo pipefail
# Regenerate the BSP using rlvgl-creator.
# Run from the example directory: examples/stm32h747i-disco/

cargo run --bin rlvgl-creator --features creator -- \
  bsp from-ioc DiscoBiscuit.ioc ../../stm32_af.json \
  --out bsp --emit-hal --emit-pac
