#!/usr/bin/env bash
# Install packages and tools needed for CI builds.
set -euo pipefail

git submodule update --init --recursive

# Install Rust using rustup
#curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
#source "$HOME/.cargo/env"
#rustup component add rust-src llvm-tools-preview
#rustup target add thumbv7em-none-eabihf
