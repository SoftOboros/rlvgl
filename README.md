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

support/ – Fonts, geometry, style, color utils

lvgl/ – C submodule (reference only)

## Status

Initial development. See `docs/TODO.md` for component-by-component progress.
