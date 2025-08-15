# rlvgl

A modular, idiomatic Rust reimplementation of LVGL (Light and Versatile Graphics Library).

rlvgl preserves the widget-based UI paradigm of LVGL while eliminating unsafe C-style memory management and global state. This library is structured to support no_std environments, embedded targets (e.g., STM32H7), and simulator backends for rapid prototyping.

The C version of LVGL is included as a git submodule for reference and test vector extraction, but not linked or compiled into this library.

## Goals

Preserve LVGL architecture and layout system

Replace C memory handling with idiomatic Rust ownership

Support embedded display flush/input via embedded-hal

Enable widget hierarchy, styles, and events using Rust traits

Use existing Rust crates where possible (e.g., embedded-graphics, heapless, tinybmp)

## Features

no_std + allocator support

Component-based module layout (core, widgets, platform)

Simulatable via std-enabled feature flag

Pluggable display and input backends

## Project Structure

core/ – Widget base trait, layout, event dispatch

widgets/ – Rust-native reimplementations of LVGL widgets

platform/ – Display/input traits and HAL adapters

lvgl/ – C submodule (reference only)

## Status

As-built. See ./docs/TODO.md for component-by-component progress.

## Coverage Notes

LLVM coverage is available using `grcov`. Run `make coverage` to build the tests
with instrumentation and generate an HTML report in `./coverage`. When
collecting coverage, ensure the following environment variables are set (they're
also present in `.cargo/config.toml`):

```
CARGO_INCREMENTAL=0
RUSTFLAGS="-Zinstrument-coverage"
LLVM_PROFILE_FILE="coverage-%p-%m.profraw"
```

Future Codex runs should focus on measurable coverage and use these variables
when generating tests.

Always run `cargo fmt --all` and fix formatting errors before preparing a
pull request. Verify formatting with `cargo fmt --all -- --check`.

Public APIs must be documented. The `#![deny(missing_docs)]` lint is enabled in
all crates, so compilation will fail if any public item lacks a meaningful
docstring. These crates are published to crates.io and require clear
documentation for users.
All files must include a descriptive file header summarizing their purpose.

Run ./scripts/pre-commit.sh and ensure it succeeds before opening a pull request. This script enforces formatting, runs clippy, builds with all features, and verifies documentation generation using nightly.

## Example linker scripts

Each example project that provides a `memory.x` linker script must include a
`build.rs` which:

- copies the local `memory.x` into the Cargo build output directory,
- emits `cargo:rustc-link-search` for that directory, and
- emits `cargo:rustc-link-arg=-Tmemory.x`.

This avoids relying on a global `.cargo/config.toml` for linker script
configuration.
