#!/usr/bin/env bash
set -euo pipefail
# Regenerate the BSP using rlvgl-creator.
# Run from the repo root directory.

cargo run --bin rlvgl-creator --features creator -- \
  bsp from-ioc ./examples/stm32h747i-disco/DiscoBiscuit.ioc \
  --out ./examples/stm32h747i-disco/src/bsp \
  --emit-hal --emit-pac \
  --grouped-writes --with-deinit
