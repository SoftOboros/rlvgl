# Agent Name: environment-dockerfile
#
# Part of the rlvgl project.
# Developed by Softoboros Technology Inc.
# Licensed under the BSD 1-Clause License.

FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Install base languages and build tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential      \
    ca-certificates      \
    cargo                \
    clang                \
    cmake                \
    curl                 \
    git                  \
    openssh-client       \
    libclang-dev         \
    libfreetype6-dev     \
    libgtk-3-dev         \
    librlottie-dev       \
    libsdl2-dev          \
    libssl-dev           \
    libx11-dev           \
    libxext-dev          \
    libxrender1          \
    llvm-dev             \
    gcc-arm-none-eabi    \
    binutils-arm-none-eabi \
    mold                 \
    nano                 \
    ninja-build          \
    pkg-config           \
    python3              \
    python3-pip          \
    python3-venv         \
    sccache              \
    vim                  \
    wget                 \
    xvfb                 \
    && rm -rf /var/lib/apt/lists/*

    # set up python.
COPY requirements.txt .
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Put rustup/cargo in a neutral path
ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH=$CARGO_HOME/bin:$PATH

# Install rustup without auto-default, then install & set the toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain none \
 && rustup toolchain install nightly \
 && rustup default nightly \
 && rustup component add rust-src llvm-tools-preview rustfmt clippy \
 && rustup target add thumbv7em-none-eabihf \
 && cargo install cargo-binutils

# If you run as a non-root user at runtime, make sure they can read it
ARG RLVGL_BUILDER_USER=rlvgl
RUN useradd -m -s /bin/bash "$RLVGL_BUILDER_USER" || true \
 && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /opt/rust
RUN mkdir -p /opt/rlvgl && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /opt/rlvgl /opt/venv

RUN mkdir -p /home/rlvgl/.ssh && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /home/rlvgl/.ssh
RUN mkdir -p /home/ubuntu/.ssh

# S3 config comes from build args/env at build time below:
# Otherwise, inject these environment variables.
# --> See /scripts/docker-run.sh for an example.
# ARG SCCACHE_BUCKET
# ARG SCCACHE_REGION
# ARG AWS_ACCESS_KEY_ID
# ARG AWS_SECRET_ACCESS_KEY
# ARG SCCACHE_S3_KEY_PREFIX
# ENV SCCACHE_BUCKET=${SCCACHE_BUCKET}
# ENV SCCACHE_REGION=${SCCACHE_REGION}
# ENV AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}
# ENV AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}
# ENV SCCACHE_S3_KEY_PREFIX=${SCCACHE_S3_KEY_PREFIX}

# set env vars
ENV APP_HOME=/opt/rlvgl
ENV RUSTFLAGS="-Cdebuginfo=0 -Ccodegen-units=32 -Clink-self-contained=no -Clink-arg=-fuse-ld=mold"
ENV CARGO_INCREMENTAL=0
ENV SCCACHE_S3_KEY_PREFIX=/rlvgl

# Comment this out to remove sccache, or remove on run.
ENV RUSTC_WRAPPER=/opt/rust/cargo/bin/sccache

# Default to non-root user for everything that follows
USER ${RLVGL_BUILDER_USER}
WORKDIR ${APP_HOME}

CMD ["bash"]
