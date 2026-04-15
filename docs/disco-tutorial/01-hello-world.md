<!--
01-hello-world.md - Tutorial Chapter 1: project skeleton + centered label.
-->

**← Prev · [Index](README.md) · [Next →](02-splash-and-assets.md)**

# Chapter 1 — Hello World on the DISCO

## What you will add

A new firmware crate that boots the Cortex-M7 core, brings up the
DSI/LTDC display path against SDRAM, and draws a single `rlvgl-widgets`
label centered on the screen. No assets, no splash, no touch — just
text on a framebuffer.

Features turned on:

- `cm7` — the required-feature for the main binary. Pulls in the
  STM32H7 PAC, HAL, and `rlvgl-platform`'s `stm32h747i_disco` support.
- `pac_sdram_init` — bring up SDRAM directly through the FMC PAC. The
  DSI framebuffer lives in SDRAM at `0xC000_0000`, so it has to be
  working before LTDC scans.

## Before you start

- You have the Rust toolchain, the `thumbv7em-none-eabihf` target,
  `probe-rs`, and `make` installed per
  [`docs/EMBEDDED-TOOLING.md`](../EMBEDDED-TOOLING.md).
- The DISCO board enumerates as an ST-LINK over USB when plugged in.

## Steps

### 1. Create the crate

Create `examples/stm32h747i-disco/` with `Cargo.toml` declaring both
the CM7 binary and the feature flags. Mirror the real crate's shape
at [`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml).
Start with only the features Chapter 1 needs:

```toml
[package]
name = "rlvgl-example-disco"
version = "0.1.0"
edition = "2024"
publish = false
build = "build.rs"

[[bin]]
name = "rlvgl-stm32h747i-disco"
path = "src/main.rs"
required-features = ["cm7"]

[features]
default = []
cm7 = [
    "rlvgl-platform/stm32h747i_disco",
    "stm32h7/stm32h747cm7",
    "dep:stm32h7xx-hal",
    "dep:embedded-hal",
    "dep:embedded-hal-02",
]
pac_sdram_init = []

[dependencies]
rlvgl-core      = { path = "../../core",      default-features = false }
rlvgl-platform  = { path = "../../platform",  default-features = false }
rlvgl-widgets   = { path = "../../widgets",   default-features = false }
cortex-m        = { version = "0.7", features = ["critical-section-single-core"] }
cortex-m-rt     = "0.7"
embedded-alloc  = "=0.5.1"
panic-halt      = "1"
stm32h7         = { version = "0.15.1", features = ["rt"] }
critical-section = "1.1.2"

[target.'cfg(any(target_arch = "arm", target_os = "none"))'.dependencies]
stm32h7xx-hal    = { version = "0.16", optional = true, features = ["stm32h747cm7", "fmc"] }
embedded-hal     = { version = "1",     optional = true }
embedded-hal-02  = { package = "embedded-hal", version = "0.2.7", optional = true, features = ["unproven"] }
```

You also need `memory.x`, `memory_STM32H747XI.x`, and `build.rs` — copy
these unchanged from the real crate. They describe the linker layout
and run the DISCO BSP generator. The tutorial does not modify them.

### 2. Write the entry point

Create `src/main.rs`. The shape below matches the top of the real
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs), trimmed
to just Chapter 1:

```rust
#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

extern crate alloc;

use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
use panic_halt as _;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry]
fn main() -> ! {
    // --- heap ---
    const HEAP_SIZE: usize = 32 * 1024;
    static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe {
        HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
    }

    // --- board bring-up ---
    // Clock tree (HSE, PLL1 for CPU/AXI, PLL3 for LTDC pixel clock),
    // SDRAM via FMC PAC, DSI/LTDC bring-up, then hand the framebuffer
    // at 0xC000_0000 to rlvgl-platform's display driver.
    //
    // This block is substantial (~200 lines of PAC pokes). Rather than
    // reproduce it here, copy the bring-up helpers used by the real
    // crate from `examples/stm32h747i-disco/src/main.rs` (search for
    // the `pac_sdram_init` cfg block and the LTDC/DSI sequence).
    //
    // What the helpers must leave true at this point:
    //   * CPU at 400 MHz, AXI at 200 MHz
    //   * FMC programmed, SDRAM responsive at 0xC000_0000
    //   * LTDC scanning an 800x480 RGB565 framebuffer from SDRAM
    //   * DSI panel woken and displaying the framebuffer
    let (mut display, fb_addr) = board::bring_up();

    // --- rlvgl widget tree: single centered label ---
    use rlvgl_core::{
        bitmap_font::FONT_6X10,
        widget::{Color, Rect},
    };
    use rlvgl_widgets::label::Label;

    const DISPLAY_W: i32 = 800;
    const DISPLAY_H: i32 = 480;
    const TEXT: &str = "Hello, rlvgl";
    let text_w = (TEXT.len() as i32) * FONT_6X10.advance_x as i32;
    let text_h = FONT_6X10.line_height as i32;

    let mut label = Label::new(
        TEXT,
        Rect {
            x: (DISPLAY_W - text_w) / 2,
            y: (DISPLAY_H - text_h) / 2,
            width:  text_w,
            height: text_h,
        },
    );
    label.style.text_color = Color(0xFF, 0xFF, 0xFF, 0xFF);
    label.style.bg_color   = Color(0, 0, 0, 0xFF);

    // --- event loop ---
    loop {
        display.flush(fb_addr, &label);
        cortex_m::asm::wfi();
    }
}
```

Treat `board::bring_up()` and `display.flush()` as placeholders for
the real platform calls. The real crate does this inline in `main()`
and through `rlvgl-platform/stm32h747i_disco`; at this stage it is
enough to have *something* that clears the framebuffer to black and
renders the label.

### 3. Reference points in the real crate

When your skeleton diverges from what works, diff against these
anchors in [`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs):

- The module declarations and heap setup at the top.
- The `#[cfg(feature = "pac_sdram_init")]` FMC bring-up block.
- The LTDC/DSI sequence that programs the panel and kicks scanout.
- The centered-label draw path uses `rlvgl_core::bitmap_font::FONT_6X10`
  exactly as above — the real crate references it at
  [`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) around
  the label-init site.

## Verify

Build the CM7 binary. This is the Chapter 1 subset of the canonical
build command from [`CLAUDE.md`](../../CLAUDE.md) §Build Profiles:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,pac_sdram_init
```

Flash via the `make` target documented in
[`CLAUDE.md`](../../CLAUDE.md) §Flashing and Debug:

```bash
make flash-disco
```

What you should see on the DISCO: a black screen with the white text
**`Hello, rlvgl`** centered on the 800×480 panel.

If the screen stays white or flickers, walk the
[`STM32H747I-DISCO-BRINGUP.md`](../STM32H747I-DISCO-BRINGUP.md)
checklist before continuing — every subsequent chapter assumes a
stable framebuffer.

## Going deeper

- [`rlvgl-platform` README](../../platform/README.md) — what
  `stm32h747i_disco` pulls in and what it exposes.
- [`docs/RENDERING-BACKEND-ARCHITECTURE.md`](../RENDERING-BACKEND-ARCHITECTURE.md)
  — how display flush is structured.
- [`docs/STM32H747I-DISCO.md`](../STM32H747I-DISCO.md) — one-page
  overview of what the finished demo does, for context.

---

**← Prev · [Index](README.md) · [Next →](02-splash-and-assets.md)**
