# Agent Name: environment-dockerfile
#
# Part of the rlvgl project.
# Developed by Softoboros Technology Inc.
# Licensed under the BSD 1-Clause License.

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Install base languages and build tools
RUN apt-get update && apt-get install -y \
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
    && rm -rf /var/lib/apt/lists/*

# Install Rust from rustup (more control, avoids apt rustc issues)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
ENV PATH="/root/.cargo/bin:$PATH"
RUN rustup component add rust-src llvm-tools-preview
RUN rustup target add thumbv7em-none-eabihf

# Create and activate Python venv
WORKDIR /opt/rlvgl
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Cache dependencies
COPY Cargo.toml .
#RUN cargo fetch

# Copy everything and build
COPY . .
RUN git submodule update --init --recursive
#RUN pip install -r requirements.txt && cd ..
#RUN cargo build --release

# build compiled items.
#RUN cargo clean && cargo fetch && cargo build -Znext-lockfile-bump --locked && cd ..

CMD ["bash"]
