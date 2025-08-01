<p align="center">
  <img src="./rlvgl-logo.png" alt="rlvgl" />
</p>

<span style="font-size:26px"><b>rlvgl</b></span> is a modular, idiomatic Rust reimplementation of LVGL (Light and Versatile Graphics Library).

rlvgl preserves the widget-based UI paradigm of LVGL while eliminating unsafe C-style memory management and global state. This library is structured to support no_std environments, embedded targets (e.g., STM32H7), and simulator backends for rapid prototyping.

The C version of LVGL is included as a git submodule for reference and test vector extraction, but not linked or compiled into this library.

## Goals
- Preserve LVGL architecture and layout system
- Replace C memory handling with idiomatic Rust ownership
- Support embedded display flush/input via embedded-hal
- Enable widget hierarchy, styles, and events using Rust traits
- Use existing Rust crates where possible (e.g., embedded-graphics, heapless, tinybmp)

## Features
- no_std + allocator support
- Component-based module layout (core, widgets, platform)
- Simulatable via std-enabled feature flag
- Pluggable display and input backends
- Optional Lottie support via the `rlottie` crate for dynamic playback.
  Embedded targets should pre-render animations to APNG for minimal size.

## Project Structure
- [core](https://github.com/SoftOboros/rlvgl/blob/main/core/README.md)/ – Widget base trait, layout, event dispatch
- [widgets](https://github.com/SoftOboros/rlvgl/blob/main/widgets/widgets/README.md)/ – Rust-native reimplementations of LVGL widgets
- [platform](https://github.com/SoftOboros/rlvgl/blob/main/platform/platform/README.md)/ – Display/input traits and HAL adapters
- [support](https://github.com/SoftOboros/rlvgl/blob/main/support/README.md)/ – Fonts, geometry, style, color utils
- [lvgl](https://github.com/lvgl/lvgl/blob/master/README.md)/ – C submodule (reference only)
## Status

As-built. See [TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO.md) for component-by-component progress.
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