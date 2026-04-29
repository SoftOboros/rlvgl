<!--
04-render-task.md - Volume IV Chapter 4: Desktop widget tree rendering
with pristine restore and single-buffer timing.
-->

**[<- Prev](03-touch-task.md) . [Index](README.md) . [Next ->](05-input-dispatch.md)**

# Chapter 4 — Render Task: Desktop Widget Tree

## Volume II reference

Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
described the bare-metal compositor that renders changed regions
into both ping-pong buffers with dirty-rect tracking. The
FreeRTOS render task takes a simpler approach for its first
milestone.

## What this chapter covers

Lazy-initializing the `DiscoController` widget tree, rendering
with `CpuBlitter` + `RotatedRenderer` into the FRONT framebuffer
during the back porch, and the `NEEDS_PRISTINE` / `DIRTY_FRAMES`
lifecycle that controls when frames are drawn.

## The FreeRTOS delta

Bare-metal renders into both ping-pong buffers using a
`Compositor` that tracks dirty rectangles. FreeRTOS takes a
simpler path: **single-buffer rendering directly into FRONT**
while LTDC is disabled (during the present task's TIM7 holdoff).

This eliminates double-buffer flicker entirely at the cost of a
longer holdoff (currently 32 ms, ~18 Hz effective frame rate).

## Walkthrough

### 1. DiscoController lazy-init

```rust
static mut DESKTOP_CTRL: Option<DiscoController> = None;
if DESKTOP_CTRL.is_none() {
    let screen = Screen::landscape(800, 480);
    let ctrl = DiscoController::new(
        screen,
        DiscoCapabilities::stm32h747i_disco(),
    );
    DESKTOP_CTRL = Some(ctrl);
}
```

Requires 64 KB Rust heap — the settings wing draws 5 RLE icons,
each decoding into a `Vec<Color>` (~14 KB peak).

### 2. Pristine splash restore

The splash JPEG is decoded into both framebuffers during
bare-metal init. A pristine copy lives at `0xD030_0000`.

Pristine restore is **gated by `NEEDS_PRISTINE`**: only gesture
events that change widget visibility (Enter key, PressRelease)
set this flag. Periodic refreshes skip pristine to avoid flashing
the splash background over the icon strip every second.

```rust
if NEEDS_PRISTINE {
    NEEDS_PRISTINE = false;
    core::ptr::copy_nonoverlapping(
        0xD030_0000 as *const u8,
        front as *mut u8,
        bytes,
    );
}
```

### 3. CpuBlitter + RotatedRenderer

The framebuffer is portrait (480x800). The widget tree is
landscape (800x480). `RotatedRenderer` wraps `BlitterRenderer`
with a 90-degree CCW coordinate transform:

```rust
let surface = Surface::new(buf, stride, PixelFmt::Argb8888, w, h);
let mut blit = BlitterRenderer::new(&mut cpu_blitter, surface);
let mut renderer = RotatedRenderer::new(&mut blit, w); // w=480
ctrl.root().borrow().draw(&mut renderer);
```

### 4. Portrait-to-landscape touch transform

Touch coordinates from FT5336 are portrait. The transform to
landscape widget coordinates:

```
landscape_x = portrait_y
landscape_y = 480 - 1 - portrait_x   // DW = 480, NOT 800
```

**Critical**: `DW` must be 480 (portrait width = `RotatedRenderer`
Y-axis range), not 800 (landscape width). Using 800 maps touches
to wrong icon positions.

### 5. D-cache coherency

CpuBlitter writes through the D-cache. LTDC reads SDRAM via AXI
(bypasses D-cache). Clean before retrigger:

```rust
cp.SCB.clean_dcache_by_address(front as usize, bytes as usize);
cortex_m::asm::dsb();
```

### 6. Dirty-frame lifecycle

| Source | dirty_frames | NEEDS_PRISTINE | When |
|--------|-------------|----------------|------|
| Boot | 4 | false | Initial 4 frames to draw desktop |
| Enter key | 1 | true | Panel/wing state change |
| Arrow key | 1 | false | Focus highlight move |
| PressRelease | 1 | true | Touch gesture action |
| Periodic | 1 | false | ~14s slow refresh for live stats |

When `dirty_frames > 0`, the render task does pristine (if
flagged) + draw + D-cache clean. When 0, the render task loops
without rendering — LTDC repeats the last FRONT.

## Verify

On boot, the splash desktop should be visible with icons. The
`?` serial command should show incrementing `tick` values. Arrow
keys should move focus highlights without flashing.

## Going deeper

- `rlvgl_platform::blit::RotatedRenderer` — the 90-degree CCW
  coordinate transform implementation.
- `rlvgl_platform::cpu_blitter::CpuBlitter` — software fill/blend
  implementation.
- `rlvgl_app_disco_demo::DiscoController` — the shared widget
  tree controller.

---

**[<- Prev](03-touch-task.md) . [Index](README.md) . [Next ->](05-input-dispatch.md)**
