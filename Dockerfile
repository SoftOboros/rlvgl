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

# Put rustup/cargo in a neutral path
ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH=$CARGO_HOME/bin:$PATH

# Install rustup without auto-default, then install & set the toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain none \
 && rustup toolchain install nightly \
 && rustup default nightly \
 && rustup component add rust-src llvm-tools-preview rustfmt clippy \
 && rustup target add thumbv7em-none-eabihf

# If you run as a non-root user at runtime, make sure they can read it
ARG RLVGL_BUILDER_USER=rlvgl
RUN useradd -m -s /bin/bash "$RLVGL_BUILDER_USER" || true \
 && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /opt/rust
RUN mkdir -p /opt/rlvgl && chown -R "$RLVGL_BUILDER_USER":"$RLVGL_BUILDER_USER" /opt/rlvgl /opt/venv

# set env vars
ENV APP_HOME=/opt/rlvgl
ENV CARGO_INCREMENTAL=1
ENV RUSTFLAGS="-Cdebuginfo=0 -Ccodegen-units=32 -Clink-self-contained=no -Clink-arg=-fuse-ld=mold"

# Default to non-root user for everything that follows
USER ${RLVGL_BUILDER_USER}
WORKDIR ${APP_HOME}

CMD ["bash"]
