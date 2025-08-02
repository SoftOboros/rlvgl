#!/usr/bin/env bash
# Install packages and tools needed for CI builds.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    curl \
    wget \
    git \
    python3 \
    python3-venv \
    python3-pip \
    cmake \
    ninja-build \
    llvm-dev \
    libclang-dev \
    clang \
    libsdl2-dev \
    xvfb \
    libxrender1 \
    libfreetype6-dev \
    libx11-dev \
    libxext-dev \
    pkg-config \
    && sudo rm -rf /var/lib/apt/lists/*

git submodule update --init --recursive

# Install Rust using rustup
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
source "$HOME/.cargo/env"
rustup component add rust-src llvm-tools-preview
rustup target add thumbv7em-none-eabihf

# Create Python virtual environment
sudo python3 -m venv /opt/venv

# Set install prefix path (GitHub Actions uses this convention)
INSTALL_PREFIX="${GITHUB_WORKSPACE:-$PWD}/install"

# Build and install rlottie locally
git clone https://github.com/Samsung/rlottie
cd rlottie && mkdir build && cd build
cmake .. \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DLIB_INSTALL_DIR=lib \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
make -j"$(nproc)"
make install
cd ../..

# Export environment variables to future steps
echo "PATH=/opt/venv/bin:$HOME/.cargo/bin:$PATH" >> "$GITHUB_ENV"
echo "PKG_CONFIG_PATH=$INSTALL_PREFIX/lib/pkgconfig" >> "$GITHUB_ENV"
echo "BINDGEN_EXTRA_CLANG_ARGS=-I${GITHUB_WORKSPACE}/install/include" >> "$GITHUB_ENV"