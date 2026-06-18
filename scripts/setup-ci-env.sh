#!/usr/bin/env bash
# Install packages and tools needed for CI builds.
set -euo pipefail

git submodule update --init --recursive

# Refresh the nightly toolchain at container runtime.
#
# The docker.io/iraa/rlvgl:latest image bakes a *floating* `nightly`
# (see Dockerfile), so its rustc freezes to whenever the image was last
# built and drifts behind the committed trybuild goldens over time — e.g.
# platform/tests/ui/mmio_dsi_offset.stderr pins the current-nightly
# `offset_of!` const-eval span, which a stale baked nightly renders at a
# different column, failing `discipline_compile`. Updating here keeps CI on
# current nightly without rebuilding the image. `rustup update` advances to
# the newest nightly that still carries the installed components (rustfmt,
# clippy, rust-src, llvm-tools-preview), so the toolchain stays complete.
rustup update nightly
rustc +nightly --version

# Install Rust using rustup
#curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
#source "$HOME/.cargo/env"
#rustup component add rust-src llvm-tools-preview
#rustup target add thumbv7em-none-eabihf
