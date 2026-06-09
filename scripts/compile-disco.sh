#!/bin/bash
export RUSTFLAGS="-C link-arg=-Texamples/stm32h747i-disco/memory_STM32H747XI.x"
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7

cargo size --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco --features cm7
