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

As-built. See  for component-by-component progress.
- [TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO.md)
- [TEST-TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TEST-TODO.md)
- [TODO-PLUGINS](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO-PLUGINS.md)

As of 0.1.0 many features are implemented and an 87% unit test coverage
is achived, but functional testing has and bare metal testing have not
occured.

## Quick Example

```rust
use rlvgl_core::widget::Rect;
use rlvgl_widgets::label::Label;

fn main() {
    let mut label = Label::new(
        "hello",
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 20,
        },
    );
    label.style.bg_color = rlvgl_core::widget::Color(0, 0, 255);
    // Rendering would use a DisplayDriver implementation.
}
```

## Coverage

LLVM coverage instrumentation is configured via `.cargo/config.toml` and the
`coverage` target in the `Makefile`. Run `make coverage` to execute the tests
with instrumentation and generate an HTML report under `./coverage/`.

## License
rlvgl is licensed under the MIT license.  See [LICENSE](https://github.com/SoftOboros/rlvgl/blob/main/LICENSE) for more details.