<!--
04-icons.md - Tutorial Chapter 4: icon strip widget.
-->

**[← Prev](03-desktop.md) · [Index](README.md) · [Next →](05-menu-stubs.md)**

# Chapter 4 — Icon Strip

## What you will add

Three icons stacked along the right edge of the screen: **Settings**,
**Files**, **Info**. They're purely visual in this chapter — no
touch, no callbacks. Adding them introduces the shared
`rlvgl-app-disco-demo` crate, which owns the widget tree and layout
state that future chapters extend.

No new feature flag is turned on here. All the code additions live
either in the shared demo crate or in the firmware's `main.rs` as
wiring.

## Before you start

- Chapter 3 works: splash → desktop with the Chapter 1 label still
  drawn over it.
- You understand that from this chapter onward the widget tree
  moves out of `main.rs` into the shared controller. The real
  firmware imports `rlvgl-app-disco-demo` for exactly this reason —
  see the shared crate at
  [`examples/apps/disco-demo/`](../../examples/apps/disco-demo/).

## Steps

### 1. Depend on `rlvgl-app-disco-demo`

In [`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml):

```toml
[dependencies]
# ...existing entries...
rlvgl-app-disco-demo = { path = "../apps/disco-demo", default-features = false }
```

The crate is `no_std` + `alloc` — it runs on the DISCO, the
simulator, and UEFI from the same source. See
[`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
for the capability matrix.

### 2. Embed the icon assets

The three main-strip icons are RLE blobs embedded by the shared
crate from the disco firmware's asset tree. The real declarations
live at
[`examples/apps/disco-demo/src/assets.rs`](../../examples/apps/disco-demo/src/assets.rs)
lines 18–23:

```rust
pub static ICON_SETTINGS: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/settings.rle");
pub static ICON_FILE: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/file.rle");
pub static ICON_INFO: &[u8] =
    include_bytes!("../../../stm32h747i-disco/assets/icons/info.rle");
```

Convert source icons with `rlvgl-creator` (see
[Chapter 2](02-splash-and-assets.md)) and drop them at
`examples/stm32h747i-disco/assets/icons/`. If you are mirroring the
real crate, the three files
[`settings.rle`](../../examples/stm32h747i-disco/assets/icons/settings.rle),
[`file.rle`](../../examples/stm32h747i-disco/assets/icons/file.rle),
and
[`info.rle`](../../examples/stm32h747i-disco/assets/icons/info.rle)
are already in place.

### 3. Build an `IconStrip`

The `IconStrip` widget lives at
[`examples/apps/disco-demo/src/icon_strip.rs`](../../examples/apps/disco-demo/src/icon_strip.rs).
It renders a fixed-count vertical strip of slots, each holding an
RLE blob and an optional `on_tap` closure. In this chapter you
populate the slots with icons and leave `on_tap` as `None`.

In the shared crate, the strip is created with the real layout
constants exposed at the top of
[`src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
(lines 190–193):

```rust
const STRIP_ICON_SIZE: i32 = 60;
const STRIP_MARGIN_TOP: i32 = 17;
const STRIP_GAP: i32 = 10;
const STRIP_X_OFFSET: i32 = 70;
```

Your Chapter 4 build-up looks like:

```rust
use rlvgl_app_disco_demo::{
    assets::{ICON_SETTINGS, ICON_FILE, ICON_INFO,
             DISPLAY_WIDTH},
    icon_strip::{IconSlot, IconStrip, SLOT_COUNT},
};

let mut strip = IconStrip::new(
    DISPLAY_WIDTH - /* STRIP_X_OFFSET */ 70,
    /* icon_size */ 60,
    /* margin_top */ 17,
    /* gap */ 10,
);

for (i, rle) in [ICON_SETTINGS, ICON_FILE, ICON_INFO].iter().enumerate() {
    strip.set_slot(i, IconSlot { rle, enabled: true, on_tap: None });
}
```

### 4. Draw the strip each frame

In the event loop, flush the strip alongside the Chapter 1 label.
The `IconStrip` widget implements `rlvgl_core::widget::Widget`, so
the flush loop is the same pattern as the label:

```rust
loop {
    display.flush(fb_addr, &label);
    display.flush(fb_addr, &strip);
    cortex_m::asm::wfi();
}
```

At this point taps do nothing — the `on_tap: None` slots don't
respond to input because input isn't wired in yet. That is
Chapter 5.

### 5. (Optional) Focus highlight

`IconStrip::set_focused_slot(Some(index))` draws a cyan border
around the chosen slot, using the constants at
[`src/assets.rs`](../../examples/apps/disco-demo/src/assets.rs)
lines 47–51
(`FOCUS_HIGHLIGHT_COLOR`, `FOCUS_BORDER_WIDTH`). Set it to `None`
for now — without input there's nothing to drive focus. Chapter 5
turns this on.

## Verify

Rebuild with the same flag set as Chapter 3 — no new features:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,pac_sdram_init
```

```bash
make flash-disco
```

Expected screen:

- Splash → desktop background (same as Chapter 3).
- Three icons stacked vertically at the right edge of the screen
  roughly 70 px in from the right margin.
- Chapter 1 label still visible in the center.

If an icon slot shows a garbled rectangle, the RLE conversion is
wrong — redo the `rlvgl-creator convert` step for that specific
icon at the correct target size (60×60 px at
`STRIP_ICON_SIZE = 60`).

## Going deeper

- [`src/icon_strip.rs`](../../examples/apps/disco-demo/src/icon_strip.rs)
  — full widget implementation, including how it rejects out-of-range
  slots and how it draws the focus highlight.
- [`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
  — capability matrix for the shared controller.

---

**[← Prev](03-desktop.md) · [Index](README.md) · [Next →](05-menu-stubs.md)**
