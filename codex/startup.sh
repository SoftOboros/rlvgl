# Copy of codex environment configured startup.sh
# AGENTS: modify this when modifying the environment with network enabled.
apt-get update && apt-get install -y \
    build-essential \
    curl \
    wget \
    nano \
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
    && rm -rf /var/lib/apt/lists/*
git submodule update --init --recursive
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
rustup component add rust-src llvm-tools-preview
rustup target add thumbv7em-none-eabihf
cargo install grcov
