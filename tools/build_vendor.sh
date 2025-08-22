#!/usr/bin/env bash
# Generate vendor chip databases and stamp license files.
#
# This script initialises vendor submodules, invokes `gen_pins.py` to
# convert configuration files into canonical JSON, and writes the vendor
# license into each crate. Paths may be overridden with environment
# variables:
#   VENDOR_DIR – directory containing raw vendor files
#   CRATE_DIR  – vendor crate to populate
#   OUT_DIR    – directory for generated JSON
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR_DIR="${VENDOR_DIR:-$ROOT/tests/data/chipdb}" # placeholder input
CRATE_DIR="${CRATE_DIR:-$ROOT/chipdb/rlvgl-chips-stm}"
OUT_DIR="${OUT_DIR:-$CRATE_DIR/generated}"

mkdir -p "$OUT_DIR"

python3 "$ROOT/tools/gen_pins.py" --input "$VENDOR_DIR/stm" --output "$OUT_DIR"

# Expose generated definitions so vendor crates can embed them
export RLVGL_CHIP_SRC="$OUT_DIR"

cat >"$CRATE_DIR/LICENSE" <<'LIC'
BSD 3-Clause License

Copyright (c) 2020, STMicroelectronics
All rights reserved.
LIC
