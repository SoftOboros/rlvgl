#!/usr/bin/env bash
set -euo pipefail
# Regenerate the BSP using rlvgl-creator.
# Run from the repo root directory.

# Default measured power values for STM32H747I-DISCO
# Allow overriding via environment if needed.
export STM32_PWR_SUPPLY=${STM32_PWR_SUPPLY:-SMPS}
export STM32_PWR_SDLEVEL=${STM32_PWR_SDLEVEL:-VOS1}

OUT_DIR=./examples/stm32h747i-disco/src/bsp
PAC_RS="$OUT_DIR/pac.rs"

need_regen=false
if [[ "${FORCE_BSP:-}" == 1 ]]; then
  need_regen=true
fi
if [[ ! -f "$PAC_RS" ]]; then
  need_regen=true
else
  # Check that the generated file reflects our desired defaults
  if ! rg -n "Supply::${STM32_PWR_SUPPLY}" "$PAC_RS" >/dev/null 2>&1; then
    need_regen=true
  fi
  if ! rg -n "Vos::${STM32_PWR_SDLEVEL}" "$PAC_RS" >/dev/null 2>&1; then
    need_regen=true
  fi
fi

if [[ "$need_regen" == true ]]; then
  echo "[gen-bsp] Generating BSP with STM32_PWR_SUPPLY=$STM32_PWR_SUPPLY STM32_PWR_SDLEVEL=$STM32_PWR_SDLEVEL"
  cargo run --bin rlvgl-creator --features creator -- \
    bsp from-ioc ./examples/stm32h747i-disco/DiscoBiscuit.ioc \
    --out "$OUT_DIR" \
    --emit-hal --emit-pac \
    --grouped-writes --with-deinit \
    --use-label-names \
    --emit-label-consts \
    --label-prefix pin_
else
  echo "[gen-bsp] BSP already reflects STM32_PWR_SUPPLY=$STM32_PWR_SUPPLY STM32_PWR_SDLEVEL=$STM32_PWR_SDLEVEL; skipping regeneration."
fi
