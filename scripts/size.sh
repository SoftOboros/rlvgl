#!/usr/bin/env bash
# Simple size reporting helper for embedded targets
set -euo pipefail

TARGET=${1:-thumbv7em-none-eabihf}
BINARY=${2:-rlvgl-demo}

arm-none-eabi-size target/$TARGET/release/$BINARY || true
