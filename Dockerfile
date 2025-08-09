# Agent Name: environment-dockerfile
#
# Part of the rlvgl project.
# Developed by Softoboros Technology Inc.
# Licensed under the BSD 1-Clause License.

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Install base languages and build tools
RUN apt-get update && apt-get install -y --no-install-recommends\
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
    mold \
    librlottie0-1 \
    libsdl2-dev \
    xvfb \
    libxrender1 \
    libfreetype6-dev \
    libx11-dev \
    libxext-dev \
    libgtk-3-dev \
    librlottie-dev \
    sccache \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# set up python.
COPY requirements.txt .
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Install Rust from rustup (more control, avoids apt rustc issues)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
ENV PATH="/root/.cargo/bin:$PATH"
RUN rustup component add rust-src llvm-tools-preview rustfmt clippy
RUN rustup target add thumbv7em-none-eabihf

# --- user config (build-time) ---
# username is a build-arg (not secret, but fine)
ARG RLVGL_BUILDER_USER=rlvgl-builder

# Add user and give it access to the working dir
RUN useradd -m -s /bin/bash "$RLVGL_BUILDER_USER"
RUN mkdir -p /opt/rlvgl && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /opt/rlvgl /opt/venv

# set 
ENV APP_HOME=/opt/rlvgl
ENV CARGO_INCREMENTAL=1
ENV RUSTFLAGS="-Cdebuginfo=0 -Ccodegen-units=32 -Clink-self-contained=no -Clink-arg=-fuse-ld=mold"

# Default to non-root user for everything that follows
USER ${RLVGL_BUILDER_USER}
WORKDIR ${APP_HOME}

CMD ["bash"]
