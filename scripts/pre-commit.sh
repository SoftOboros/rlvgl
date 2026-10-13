#!/usr/bin/env bash
# scripts/pre-commit.sh - Phased validation mirroring pre-publish / CI workflow.
# Stop on first failure.
set -euo pipefail

echo "[phase 0] format"
cargo fmt --all -- --check

echo "[phase 1] clippy (workspace, warnings = errors)"
RUSTFLAGS="" cargo clippy --workspace -- -D warnings

if [[ -d "$HOME/.local/rlottie/lib" ]]; then
  export DYLD_LIBRARY_PATH="$HOME/.local/rlottie/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
fi

echo "[phase 2] tests (workspace)"
RUSTFLAGS="" cargo test --workspace

echo "[phase 3] playit crate (standalone + no_std cross-compile + package)"
RUSTFLAGS="" cargo test -p rlvgl-playit
RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-playit
(cd playit && cargo package --list --allow-dirty)

echo "[phase 4] simulator + creator tests"
RUSTFLAGS="" cargo build -p rlvgl-example-sim
RUSTFLAGS="" cargo test -p rlvgl-example-sim
RUSTFLAGS="" cargo test --tests --features "creator" -p rlvgl

echo "[phase 4.5] disco demo + simulator automation tests"
RUSTFLAGS="" cargo test -p rlvgl-app-disco-demo
RUSTFLAGS="" cargo build -p rlvgl-example-disco-sim
RUSTFLAGS="" cargo test -p rlvgl-example-disco-sim
TARGET_DIR=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
HOST_TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
DISCO_SIM_BIN="$TARGET_DIR/debug/rlvgl-disco-sim"
if [[ ! -x "$DISCO_SIM_BIN" ]]; then
    DISCO_SIM_BIN="$TARGET_DIR/$HOST_TRIPLE/debug/rlvgl-disco-sim"
fi
if [[ ! -x "$DISCO_SIM_BIN" ]]; then
    echo "disco simulator binary not found under $TARGET_DIR" >&2
    exit 1
fi
(cd playit/node && RLVGL_DISCO_SIM_BIN="$DISCO_SIM_BIN" node --test)

echo "[phase 5] docs"
RUSTFLAGS="" cargo doc --workspace --no-deps

echo "[phase 5.5] documentation index"
make spec-test spec-index-check

echo "[phase 6] embedded target (CM7 full build + .hex/.bin)"
make build-disco

echo "[phase 7] publish dry run"
DRY_RUN=1 scripts/publish_changed.sh HEAD~1

echo "pre-commit: all phases passed"
