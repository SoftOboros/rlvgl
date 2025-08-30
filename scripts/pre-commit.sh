#!/usr/bin/env bash
# scripts/pre-commit.sh - Fast, phased validation prior to commit
set -euo pipefail

echo "[phase 0] format"
cargo fmt --all

echo "[phase 1] clippy (core/workspace)"
cargo clippy --workspace -- -D warnings

echo "[phase 2] build+test: creator CLI"
# Build creator CLI and run its tests (no UI)
cargo build --bin rlvgl-creator --features creator
cargo test --tests --features creator

echo "[phase 3] build+test: creator UI"
# Layer UI feature on top of creator and run UI-focused tests
cargo test --tests --features "creator creator_ui"

echo "[phase 4] docs (nightly)"
export ARTIFACTS_INCLUDE_DIR="$(pwd)/scripts/artifacts/include"
export ARTIFACTS_LIB_DIR="$(pwd)/scripts/artifacts/lib"
export ARTIFACTS_LIB64_DIR="$ARTIFACTS_LIB_DIR"
RUSTDOCFLAGS="--cfg docsrs --cfg nightly" \
    cargo +nightly doc \
    --no-deps

echo "[phase 5] embedded example (stm32h747i-disco)"
# Ensure the STM32H747I-DISCO example builds for its target (optional toolchain)
RUSTFLAGS="" cargo build --target thumbv7em-none-eabihf --bin rlvgl-stm32h747i-disco --features stm32h747i_disco || {
  echo "warning: embedded target build skipped or failed (toolchain/target may be missing)" >&2
}

echo "pre-commit: all phases completed"
