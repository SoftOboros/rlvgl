<!--
03-desktop.md - Tutorial Chapter 3: desktop background + DMA2D.
-->

**[← Prev](02-splash-and-assets.md) · [Index](README.md) · [Next →](04-icons.md)**

# Chapter 3 — Desktop Background

## What you will add

A persistent desktop background that lives behind every subsequent
widget. You'll also turn on DMA2D, the STM32H7 2D raster engine, so
the blits that paint the background run in the background while the
CPU does other work.

After this chapter the screen no longer shows "just a splash" — the
splash hands off to an always-on desktop image that future chapters
draw widgets on top of.

Features turned on:

- `desktop` — gates the `DESKTOP_RLE` blob in `main.rs` and the
  refresh-behind-widget path.
- `dma2d` — enables the `rlvgl-platform` DMA2D backend and the
  accelerated blit used by the desktop refresh.

## Before you start

- Chapter 2 works: splash paints, then label draws over it.
- You have a desktop-sized image ready. The real crate reuses the
  splash for the desktop background — see
  [`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) line
  61, where `DESKTOP_RLE` is literally another `include_bytes!` of
  `splash.rle`. Do the same while you get your bearings, or run
  `rlvgl-creator convert` on a separate source image.

## Steps

### 1. Turn on `desktop` and `dma2d`

In [`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml):

```toml
[features]
# ...existing entries...
desktop = []
dma2d   = ["rlvgl-platform/dma2d"]
```

Note `desktop` has no `rlvgl-platform` dependency of its own — it is
purely an in-crate flag that enables the desktop blob and refresh
path. `dma2d` on the other hand flips `rlvgl-platform`'s DMA2D
backend on, which is the acceleration engine the desktop refresh
uses.

### 2. Declare the desktop blob

In `src/main.rs`, add the blob next to `SPLASH_RLE`. This mirrors
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) lines
60–61:

```rust
/// Desktop background image — decoded into the framebuffer and
/// restored behind widgets when they hide. Independent of the
/// splash boot screen.
#[cfg(feature = "desktop")]
static DESKTOP_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");
```

Comment is not cosmetic — it documents the invariant that
subsequent chapters rely on (the desktop is the *persistent*
background; the splash is a boot-only flash).

### 3. Paint the desktop once and save a pristine copy

After the splash hold from Chapter 2, decode the desktop image
into **both** framebuffers (the LTDC flips between two) and keep
a pristine copy in RAM so later chapters can restore areas that
widgets temporarily occlude.

This matches the real crate's logic at
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) lines
2782–2830:

```rust
#[cfg(feature = "desktop")]
{
    let desktop = rlvgl_decomp::parse_rle_blob(DESKTOP_RLE)
        .expect("desktop RLE parse");

    // Paint into both framebuffers so both scanout targets agree.
    rlvgl_platform::stm32h747i_disco::paint_rle(fb_front, &desktop);
    rlvgl_platform::stm32h747i_disco::paint_rle(fb_back,  &desktop);

    // Keep a pristine save-under copy for later restore.
    rlvgl_platform::stm32h747i_disco::snapshot_framebuffer(fb_back);
}
```

If you are re-using `splash.rle` for the desktop, the panel content
does not change visibly yet — the splash stays on the screen, it is
just now "the desktop" instead of a one-shot boot image.

### 4. Wire in the DMA2D blit path

`rlvgl-platform`'s `dma2d` feature replaces the memcpy-style blit
in `paint_rle` (and in `snapshot_framebuffer`) with one that
programs the DMA2D peripheral. You don't call DMA2D directly — the
platform crate does — but you do need to hand the blit routine a
buffer aligned for DMA2D.

The practical effect: with `dma2d` on, the desktop redraw that
Chapter 5 kicks off when a menu closes runs in the background
while the CPU keeps servicing the event loop.

No new code in `main.rs` is required for this — the feature flag
is enough. See
[`docs/RENDERING-BACKEND-ARCHITECTURE.md`](../RENDERING-BACKEND-ARCHITECTURE.md)
for how the platform crate picks the DMA2D backend when the flag
is on.

### 5. Skip the splash delay

With a real desktop in place, the splash hold you added in
Chapter 2 becomes redundant — the desktop *is* the post-boot
image. The real crate notes this explicitly at
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) line
1879: "No splash delay — splash is the desktop background."

Gate the Chapter 2 hold loop on `desktop` being off:

```rust
#[cfg(all(feature = "splash", not(feature = "desktop")))]
{
    for _ in 0..120 {
        cortex_m::asm::delay(16_000_000 / 60);
    }
}
```

## Verify

Build with all three flags:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,pac_sdram_init
```

Flash:

```bash
make flash-disco
```

Expected behaviour:

- The splash image appears immediately on boot.
- The Chapter 1 label appears centered on the desktop background
  (the background no longer vanishes).
- The image behind the label stays visible until power-off.

If the background flashes or blanks when the label appears, the
most common cause is forgetting to paint the desktop into *both*
framebuffers — double-check step 3.

## Going deeper

- [`docs/RENDERING-BACKEND-ARCHITECTURE.md`](../RENDERING-BACKEND-ARCHITECTURE.md)
  — how `rlvgl-platform` picks a blit backend based on features.
- [`docs/RENDERING-ALPHA-BLENDING.md`](../RENDERING-ALPHA-BLENDING.md)
  — the alpha math DMA2D accelerates.
- [STM32H747XIHx RM0399 §DMA2D] — the register reference for the
  2D engine itself. You will not need to touch it directly; the
  platform crate owns that surface.

---

**[← Prev](02-splash-and-assets.md) · [Index](README.md) · [Next →](04-icons.md)**
