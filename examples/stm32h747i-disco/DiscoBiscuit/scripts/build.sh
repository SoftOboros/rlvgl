#!/usr/bin/env bash
set -e

# Ensure the GNU Arm toolchain is in PATH
TOOLCHAIN_BIN="${CMAKE_TOOLCHAIN_BIN:-/opt/arm-toolchain-14.2.rel1/bin}"
export PATH="${TOOLCHAIN_BIN}:$PATH"
export CMAKE_TOOLCHAIN_BIN="$TOOLCHAIN_BIN"

mkdir -p build
cd build
LTO_OPT=""
if [ "${ENABLE_LTO:-}" = "ON" ]; then
  LTO_OPT="-DENABLE_LTO=ON"
fi

cmake -G Ninja ${LTO_OPT} ..
ninja
arm-none-eabi-objcopy -O binary DiscoBiscuit_CM7.elf DiscoBiscuit_CM7.bin
