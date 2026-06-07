<!--
09-star-crawl-part-1.md - Volume II Chapter 9: star crawl pre-render + perspective + FIR.
-->

**[← Prev](08-secondary-peripherals.md) · [Index](README.md) · [Next →](10-star-crawl-part-2.md)**

# Chapter 9 — Star Crawl Part I: Pre-render, Perspective & FIR

## Volume I reference

Vol I
[Chapter 6](../disco-tutorial/06-hook-actions.md) explicitly
skipped the Star Crawl slot. Volume II's capstone — this chapter
and Chapter 10 — is the payoff.

## What this chapter covers

Two of the three conceptually interesting parts of the crawl:

1. **Pre-rendering** the full text column into a wide A8 buffer
   in SDRAM at activation.
2. **Perspective** — how each output row picks a different
   source width (narrow at top, full at bottom) to fake depth.
3. **FIR resampling** — how each row is downsampled from the
   source 600-pixel width to its perspective width without
   aliasing.

Chapter 10 covers the state machine, DMA2D pipeline, cache
cleaning, and teardown. Read in order.

## The HAL / PAC gap

None direct. This chapter is mostly pure Rust and memory
layout. The PAC-level gotchas — D-cache transparency, DMA2D
ISR races — land in Chapter 10.

## Walkthrough

### 1. Memory layout and constants

Everything hinges on four sizes, declared at
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L14–71:

```rust
const FB_W: u32        = 480;       // Portrait framebuffer width
const FB_H: u32        = 720;       // Portrait rows used by the crawl
const CRAWL_W: u32     = FB_H;      // Landscape crawl width = 720
const CRAWL_H: u32     = FB_W;      // Landscape crawl height = 480
const TEXT_W: u32      = 600;       // Pre-rendered text line width
const TOP_W: u32       = 360;       // Perspective width at the top
const BOT_W: u32       = 600;       // Perspective width at the bottom
const BPP: u32         = 4;         // ARGB8888
const STAR_ROWS: u32   = FB_H * 2;  // Double-height mirrored starfield
const STAR_STRIDE: u32 = FB_W * BPP;
const STAR_SIZE: usize = (STAR_ROWS * STAR_STRIDE) as usize;

/// SDRAM base address for crawl buffers.
const CRAWL_BASE: usize = 0xD100_0000;

/// D2 SRAM base for the portrait A8 text buffer.
///
/// 480 × 600 = 288,000 bytes. D2 SRAM is 288 KiB total; IPC mailbox at
/// 0x3004_7000 leaves 2,816 bytes headroom.
const A8_BUF:    usize = 0x3000_0000;
const A8_WIDTH:  u32   = FB_W;      // 480 portrait columns
const A8_HEIGHT: u32   = BOT_W;     // 600 rows (max text extent)
```

Three buffers exist at runtime:

| Buffer | Where | Size | Purpose |
|--------|-------|------|---------|
| **Starfield** | SDRAM `0xD100_0000` | 480 × 1440 × 4 = 2.64 MB | Double-mirrored background so row wrap is seamless. |
| **text_src** | SDRAM `CRAWL_BASE + STAR_SIZE` | 600 × `text_h` × 1 | The full landscape pre-render of every line of text as A8. |
| **A8 portrait** | D2 SRAM `0x3000_0000` | 480 × 600 × 1 = 288 KB | Per-frame staging for the perspective-projected text before DMA2D blends it yellow. |

Why each lives where:

- Starfield and `text_src` are big (multi-megabyte); they have
  to be in SDRAM.
- The A8 portrait buffer is hot — CPU writes it each frame,
  DMA2D reads it each frame. Putting it in **D2 SRAM** (fast,
  on the AXI bus but not competing with LTDC's SDRAM reads)
  keeps the FIR pass from thrashing the external memory bus.
  Chapter 10 will show why D2 SRAM's write-back cache means
  it needs explicit cleaning.
- The numbers match the D2 SRAM limit (288 KiB total; IPC
  mailbox sits at `0x3004_7000` leaving 2816 bytes of
  headroom above the A8 buffer).

### 2. Activation — wire up the buffers

At
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L235–267:

```rust
pub fn activate(&mut self, dma2d: &mut Dma2dBlitter) {
    // Enable D2 SRAM1 + SRAM2 + SRAM3 clocks for the A8 portrait buffer.
    // RCC_AHB2ENR: bit 29 = SRAM1EN, 30 = SRAM2EN, 31 = SRAM3EN.
    unsafe {
        let ahb2enr = (0x5802_44DCu32) as *mut u32;
        ahb2enr.write_volatile(ahb2enr.read_volatile() | 0xE000_0000);
    }

    let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
    let logo_h = Self::logo_height();
    self.text_h = 120
        + GRAPHIC_SIZE
        + GRAPHIC_GAP
        + self.lines.len() as u32 * line_h
        + LOGO_GAP
        + logo_h
        + CRAWL_H;
    self.starfield = CRAWL_BASE as *mut u8;
    self.text_src  = (CRAWL_BASE + STAR_SIZE) as *mut u8;

    unsafe {
        core::ptr::write_bytes(self.text_src, 0, (TEXT_W * self.text_h) as usize);
    }

    self.render_starfield(dma2d);
    self.pre_render_text(line_h);
    self.scroll_q8 = -((CRAWL_H as i32) << 8);      // start below the screen
    self.star_scroll_q8 = 0;
    self.frame_id = 0;
    self.active = true;
    self.drop_frame();
}
```

Three things happen: D2 SRAM clocks come on, SDRAM regions
get zeroed/claimed, then the two pre-render passes run
(starfield via DMA2D, text via CPU).

The initial `scroll_q8` is `-((CRAWL_H as i32) << 8)`, i.e.
`-480 << 8`. Chapter 10's scroll math in Q.8 fixed point
will advance this toward `text_h + CRAWL_H`; the crawl ends
when scroll clears the tail of the text.

### 3. Pre-rendering the text column

`pre_render_text()` runs once at activation. It walks every
line of `self.lines`, calls into
[`rlvgl_core::packed_font::PackedFont`](../../core/src/packed_font.rs)
to rasterize glyphs as A8, and writes them into the
`text_src` buffer at the correct vertical offset. The layout
reserves: 120 px top margin → the 384×384 graphic crop → a
40 px gap → every text line at `line_h = font.height * 3/2`
→ a 40 px gap → the letter logo → `CRAWL_H` of padding.

The pre-rendered buffer is **landscape-oriented** at
`TEXT_W = 600` pixels wide and `text_h` rows tall, where the
height depends on script length. Why landscape? Because when
the FIR pass runs, it walks *rows* of the text — each row
becomes one vertical scanline of the perspective-projected
output, and CPU stride-1 access is cache-friendly.

### 4. Perspective — wider-as-you-go

Inside `tick()`, one row of perspective math at
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L363–371:

```rust
if self.text_row < CRAWL_H {
    let text_row_i = self.frame_scroll_px + self.text_row as i32;
    if text_row_i >= 0 && (text_row_i as u32) < self.text_h {
        let src_row = text_row_i as u32;
        let target_w = TOP_W + (BOT_W - TOP_W) * self.text_row / (CRAWL_H - 1);
        let dst_x_off = (CRAWL_W - target_w) / 2;
```

`target_w` is a straight linear interpolation from
`TOP_W = 360` to `BOT_W = 600` across `CRAWL_H = 480` rows.
`dst_x_off` centers the narrower top inside the 720-pixel
landscape width. Linear (not trigonometric) interpolation is
intentional — the resulting shape is a trapezoid, which reads
as perspective at the low resolution the panel actually
renders. The source comment at
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L24–28 explains the other end of the equation:

```rust
/// Perspective width at the top of the crawl.
///
/// Keep this narrower than the source text width so the FIR pass has enough
/// decimation headroom to smooth distant glyph edges.
const TOP_W: u32 = 360;
```

In other words: the 360:600 ratio is chosen to keep the FIR
filter in a regime where it has enough samples to produce a
clean down-scaled output.

### 5. The FIR filter

A seven-tap FIR, weights `[8, 24, 48, 64, 48, 24, 8]`
(Gaussian-ish). Full implementation at
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L584–662:

```rust
fn fir_resample_text_row(&mut self, text_row: u32, target_w: u32) -> bool {
    // ...bounds checks + early zero-row exit...

    let step_q16 = (TEXT_W << 16) / target_w;
    let mut ox = 0usize;
    while ox < target_w as usize {
        let cx_q16 = ox as u32 * step_q16 + (step_q16 >> 1);
        let cx     = (cx_q16 >> 16) as i32;

        // ...fast-skip if center and all 6 neighbours are zero...

        let mut acc: u32  = 0;
        let mut wsum: u32 = 0;
        const TAPS: [(i32, u32); 7] = [
            (-3, 8), (-2, 24), (-1, 48), (0, 64),
            ( 1, 48), ( 2, 24), ( 3,  8),
        ];
        for &(tap, w) in &TAPS {
            let sx = cx + tap;
            if sx >= 0 && sx < TEXT_W as i32 {
                let v = unsafe { *src.add(sx as usize) } as u32;
                acc += v * w;
                wsum += w;
            }
        }
        let alpha = if wsum > 0 { (acc / wsum).min(255) as u8 } else { 0 };
        out[ox] = alpha;
        ox += 1;
    }
    any_nonzero
}
```

Mechanics:

- **`step_q16`** is the source-pixels-per-output-pixel
  advance in Q.16 fixed-point. At `target_w = 360`,
  `step_q16 = (600 << 16) / 360 ≈ 1.67`.
- **`cx_q16 = ox * step_q16 + step_q16/2`** positions the
  filter center at the *midpoint* of the source span each
  output pixel covers — this is the half-pixel offset that
  keeps the filter symmetric.
- **Early exit** on all-zero source rows (line 597–605) and
  on "center + 6 neighbours all zero" (lines 614–628) makes
  the filter cheap for the many rows between text lines.
- **`wsum`** is accumulated per-pixel rather than being a
  compile-time constant because the three leftmost and
  three rightmost output pixels have truncated taps — the
  normalizer has to shrink to match.

### 6. Writing into the A8 portrait buffer

The line right after the FIR call (in the excerpt in step 4,
continuation at L372+) copies `scanline_buf[..target_w]`
into **one column** of the A8 portrait buffer in D2 SRAM,
offset by `dst_x_off` so each row's text sits centered.
The portrait buffer is written column-major this way,
because the final DMA2D blit in Chapter 10 reads it in
portrait-framebuffer coordinates and the starfield in
landscape.

## Register diagram — D2 SRAM enables

```
RCC AHB2ENR  @ 0x5802_44DC
│
├── bit 29  D2 SRAM1 EN
├── bit 30  D2 SRAM2 EN
└── bit 31  D2 SRAM3 EN
```

Writing `0xE000_0000` sets all three. Without these, reads of
`0x3000_0000` hard-fault.

## Verify

This chapter's surface is pure RAM. Verify by reading RAM
under probe-rs with the crawl active:

- `0xD100_0000` + a few rows — should contain the dark-blue
  `BG_COLOR = 0xFF0A_0A20` punctuated by 200 starfield pixels.
- `0xD100_0000 + STAR_SIZE` (text_src start) — non-zero bytes
  wherever text glyphs were rasterized.
- `0x3000_0000` — after a few frames, non-zero bytes in the
  columns covered by the current scroll offset.

Chapter 10 is where the **visible** verification — yellow
perspective text scrolling against a star background —
happens.

## Going deeper

- [`examples/stm32h747i-disco/MEMORY.md`](../../examples/stm32h747i-disco/MEMORY.md)
  — the canonical SDRAM / D-SRAM layout this chapter carves
  buffers out of.
- [`core/README.md`](../../core/README.md) and
  `rlvgl_core::packed_font::PackedFont` — the glyph
  rasterizer `pre_render_text` calls into.
- RM0399 §8.7.30 "RCC AHB2 Clock Enable Register" — the D2
  SRAM clock bits.
- [`docs/assets/IMAGE-COMPRESSION-FORMAT.md`](../assets/IMAGE-COMPRESSION-FORMAT.md)
  — explains RLVGLRAW, which the crawl also renders (logo).

---

**[← Prev](08-secondary-peripherals.md) · [Index](README.md) · [Next →](10-star-crawl-part-2.md)**
