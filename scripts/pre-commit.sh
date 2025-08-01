#!/usr/bin/env bash
# Git pre-commit hook to enforce formatting and linting
set -e

cargo fmt --all
cargo clippy --workspace \
    --features "canvas,fatfs,fontdue,gif,jpeg,nes,png,pinyin,qrcode" \
    --target x86_64-unknown-linux-gnu -- -D warnings

# Build with all features enabled to catch lints like `missing_docs`
cargo check --workspace --all-targets \
    --features "canvas,fatfs,fontdue,gif,jpeg,nes,png,pinyin,qrcode" \
    --target x86_64-unknown-linux-gnu

# check document generation
export ARTIFACTS_INCLUDE_DIR="$(pwd)/scripts/artifacts/include"
export ARTIFACTS_LIB_DIR="$(pwd)/scripts/artifacts/lib"
export ARTIFACTS_LIB64_DIR="$ARTIFACTS_LIB_DIR"
RUSTDOCFLAGS="--cfg docsrs --cfg nightly" \
    cargo +nightly doc \
    --all-features \
    --no-deps --target x86_64-unknown-linux-gnu
