#!/usr/bin/env bash
# build-bare.sh — Build the bare-metal BBB binary and emit a flat .bin
# ready for U-Boot `go 0x82000000`.
#
# Requires:
#   rustup target add armv7a-none-eabihf
#   arm-none-eabi-objcopy  (brew install --cask gcc-arm-embedded, or
#                           brew tap osx-cross/arm && brew install arm-gcc-bin)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${PROJECT_DIR}/../.." && pwd)"

TARGET=armv7a-none-eabihf
PROFILE=release
ELF="${REPO_DIR}/target/${TARGET}/${PROFILE}/rlvgl-bbb-bare"
BIN="${ELF}.bin"

echo "[1/2] cargo build --target ${TARGET} --bin rlvgl-bbb-bare --features bare_metal --release"
RUSTFLAGS="" cargo build \
    --manifest-path "${REPO_DIR}/Cargo.toml" \
    --target "${TARGET}" \
    -p rlvgl-example-bbb \
    --bin rlvgl-bbb-bare \
    --no-default-features \
    --features bare_metal \
    --release

echo "[2/2] objcopy -O binary ${ELF} ${BIN}"
arm-none-eabi-objcopy -O binary "${ELF}" "${BIN}"

echo ""
echo "=== bare-metal build ready ==="
echo "  ELF: ${ELF}  ($(wc -c < "${ELF}") bytes)"
echo "  BIN: ${BIN}  ($(wc -c < "${BIN}") bytes)"
echo ""
echo "Copy .bin to an SD FAT partition, then in U-Boot:"
echo "  => fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin"
echo "  => go 0x82000000"
