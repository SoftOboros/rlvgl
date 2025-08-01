#!/usr/bin/env bash
# Install packages and tools needed for CI builds.
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    curl \
    wget \
    git \
    nano \
    vim \
    python3 \
    python3-venv \
    python3-pip \
    cargo \
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
    && sudo rm -rf /var/lib/apt/lists/*

git submodule update --init --recursive

# Install Rust using rustup
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
source "$HOME/.cargo/env"
rustup component add rust-src llvm-tools-preview
rustup target add thumbv7em-none-eabihf

# Create Python virtual environment
sudo python3 -m venv /opt/venv

# install lottie to system
git clone https://github.com/Samsung/rlottie
cd rlottie && mkdir build && cd build
cmake .. \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_INSTALL_LIBDIR=lib \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
make -j$(sysctl -n hw.ncpu)
make install && cd ../..

# Propagate environment updates to subsequent workflow steps
echo "PATH=/opt/venv/bin:$HOME/.cargo/bin:$PATH" >> "$GITHUB_ENV"
