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
- [lvgl](https://github.com/lvgl/lvgl/blob/master/README.md)/ – C submodule (reference only)
- [rlvgl-sim](https://github.com/SoftOboros/rlvgl/tree/main/examples/sim/README.md)/ – Desktop simulator example
## Status

As-built. See [TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO.md) for component-by-component progress.
- [TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO.md)
- [TEST-TODO](https://github.com/SoftOboros/rlvgl/blob/main/docs/TEST-TODO.md)
- [TODO-PLUGINS](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO-PLUGINS.md)
- [TODO-UI](https://github.com/SoftOboros/rlvgl/blob/main/docs/TODO-UI.md)

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
    label.style.bg_color = rlvgl_core::widget::Color(0, 0, 255, 255);
    // Rendering would use a DisplayDriver implementation.
}
```

## Coverage

LLVM coverage instrumentation is configured via `.cargo/config.toml` and the
`coverage` target in the `Makefile`. Run `make coverage` to execute the tests
with instrumentation and generate an HTML report under `./coverage/`.

## [rlvgl crate](https://crates.io/crates/rlvgl)
- The link above is for the top crate which bundles the others and include the simulator.
- [rlvgl-core crate](https://crates.io/crates/rlvgl-core)
- [rlvgl-widgets crate](https://crates.io/crates/rlvgl-widgets)
- [rlvgl-platform crate](https://crates.io/crates/rlvgl-platform)

Run the following Cargo command in your project directory:
```bash
cargo add rlvgl
```
Or add the following line to your Cargo.toml:
```toml
rlvgl = "0.1.5"
```

## Dockerhub
The build image used by the Github worflow for this repo is publiclly available on [Dockerhub](https://hub.docker.com/r/iraa/rlvgl).
```bash
docker pull iraa/rlvgl:latest
```

Consult the [Dockerfile](https://github.com/SoftOboros/rlvgl/blob/main/Dockerfile) for details on the build environment.

Other useful helper scripts may be found in [`/scripts`](https://github.com/SoftOboros/rlvgl/blob/main/scripts).

## License
rlvgl is licensed under the MIT license.  See [LICENSE](https://github.com/SoftOboros/rlvgl/blob/main/LICENSE) for more details.