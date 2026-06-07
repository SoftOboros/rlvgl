#!/bin/bash
export RUSTFLAGS="-C target-cpu=cortex-m7"
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7

cargo size --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7
