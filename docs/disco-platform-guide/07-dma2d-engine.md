<!--
07-dma2d-engine.md - Volume II Chapter 7: DMA2D blitter + ISR latch + admission.
-->

**[← Prev](06-touch-input.md) · [Index](README.md) · [Next →](08-secondary-peripherals.md)**

# Chapter 7 — DMA2D Engine

## Volume I reference

Vol I
[Chapter 3](../disco-tutorial/03-desktop.md) turned the `dma2d`
feature on with "no new code in main.rs is required for this —
the feature flag is enough." That's true as far as the desktop
blit goes. What the feature flag pulls in is this chapter.

## What this chapter covers

Three mechanisms that make DMA2D usable under the ERIF-gated
render/present pipeline from Chapter 5:

1. **DMA2D modes** (R2M, M2M, M2M+PFC, M2M+blend) and the
   `Blitter` trait that hides them behind a uniform interface.
2. **ISR completion latch** — why polling DMA2D's `TCIF`
   races, and how an atomic latch set from the ISR fixes it.
3. **Admission control** — `dma2d_admits(cost)` keeps the
   render phase from overrunning the next ERIF deadline.

## The HAL / PAC gap

`stm32h7xx-hal` does not provide a DMA2D driver. The svd2rust
PAC exposes the register block through
`stm32h7::stm32h747cm7::DMA2D`; rlvgl-platform wraps that into
a blitter type, and `main.rs` owns the ISR + admission glue.

No off-by-one here — the gap is scope, not correctness.

## Walkthrough

### 1. DMA2D modes and the `Blitter` trait

Four DMA2D transfer modes matter for this crate:

| Mode | What it does | Used for |
|------|--------------|----------|
| **R2M** (Register to Memory) | Fill a rectangle with a solid color. | Clearing regions; starfield background. |
| **M2M** (Memory to Memory) | Straight copy, no format conversion. | Same-format blits (RGB565→RGB565). |
| **M2M+PFC** (Pixel Format Conversion) | Copy with per-pixel format conversion. | Decoded RLE assets at different depths. |
| **M2M+Blend** | Alpha-blend source over destination. | The A8 yellow text blend in the star crawl. |

`rlvgl-platform` exposes a `Blitter` trait (see
[`platform/src/blit.rs`](../../platform/src/blit.rs)) whose
`Dma2dBlitter` impl in
[`platform/src/dma2d.rs`](../../platform/src/dma2d.rs) drives
the DMA2D register block directly.
[`docs/rendering/BACKEND-ARCHITECTURE.md`](../rendering/BACKEND-ARCHITECTURE.md)
documents how a given display picks a blitter.

### 2. Admission control — `dma2d_admits`

Full implementation at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L426–438:

```rust
/// True if `cost` cycles of DMA2D work can finish before the guard window.
/// The guard starts 1ms (400K cycles) before the expected next TE/ERIF.
pub fn dma2d_admits(cost: u32) -> bool {
    const GUARD: u32 = 400_000; // 1ms safety margin at 400MHz
    let budget = FRAME_BUDGET_CYCLES.load(Ordering::Relaxed);
    let elapsed = cycles_since_erif();
    let remaining = budget.saturating_sub(elapsed);
    remaining > cost + GUARD
}
```

`FRAME_BUDGET_CYCLES` is an EMA of the ERIF-to-ERIF interval
maintained in Chapter 5's `present()` path — so if the panel
is actually running at 60 Hz, the budget converges to ≈6.7 M
cycles. Every render task (desktop refresh, icon draw, star
crawl row blit) calls `dma2d_admits(expected_cost)` before
kicking DMA2D, and yields back to the main loop if admission
fails.

The Chapter 10 star crawl uses this aggressively — it walks
the crawl one row at a time, asking for admission before each
row and stopping early if the budget runs out.

### 3. The DMA2D ISR + completion latch

Interrupt wiring at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L319–333:

```rust
mod _dma2d_isr {
    use stm32h7::stm32h747cm7::interrupt;
    #[interrupt]
    unsafe fn DMA2D() {
        unsafe { super::dma2d_irq::irq_handler(); }
    }
}
```

The `dma2d_irq` module at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L720–800+
owns the ISR, the static atomics, and the complete-count /
error-count / cycle telemetry:

```rust
static START_CYCLES:    AtomicU32  = AtomicU32::new(0);
static LAST_CYCLES:     AtomicU32  = AtomicU32::new(0);
static MAX_CYCLES:      AtomicU32  = AtomicU32::new(0);
static COMPLETE_COUNT:  AtomicU16  = AtomicU16::new(0);
static ERROR_COUNT:     AtomicU16  = AtomicU16::new(0);
static COMPLETE_LATCH:  AtomicBool = AtomicBool::new(false);
static ERROR_LATCH:     AtomicU32  = AtomicU32::new(0);

/// Consume the completion latch (set by ISR, races poll_complete).
pub fn take_complete() -> bool {
    COMPLETE_LATCH.swap(false, Ordering::AcqRel)
}

pub unsafe fn irq_handler() {
    let regs = unsafe { &*stm32h7::stm32h747cm7::DMA2D::ptr() };
    let isr = regs.isr.read().bits();
    let clear = isr & 0x3F;
    if clear != 0 {
        unsafe { regs.ifcr.write(|w| w.bits(clear)); }
    }
    // ...TCIF → COMPLETE_LATCH, TEIF → ERROR_LATCH...
}
```

Two things to notice:

- **TCIF is cleared by the ISR writing `IFCR`.** If the CPU
  instead tried to clear TCIF from the main loop after a
  poll-based wait, it could clear TCIF *after* the next DMA2D
  job already set it — losing a completion. The latch is an
  atomic `swap(false)` so a single transfer can only be
  "consumed" once.
- **`take_complete()` returns a `bool`** — not a count. Multiple
  jobs completing between calls still collapse to "something
  completed." The render loop always pairs a latch take with
  a `poll_complete()` check on the actual DMA2D BUSY bit to
  cover that edge.

NVIC priority is **3** (`init()` at L751), one notch below
TIM6's priority 2 — touch sampling wins a race against
DMA2D completion.

### 4. How a render looks end-to-end

```
ERIF ISR fires                        LTDCEN cleared; ERIF_FLAG set
  │
  ├── main loop wakes
  │   ├── render phase:
  │   │   ├── dma2d_admits(cost)   → true
  │   │   ├── dma2d_irq::note_start()
  │   │   ├── kick DMA2D (R2M/M2M/blend)
  │   │   ├── wait on COMPLETE_LATCH / poll BUSY
  │   │   └── repeat for next tile / row / job
  │   │
  │   └── cycles_since_erif() >= PRESENT_HOLDOFF?
  │
  └── present() → LTDCEN back on, next scan starts
```

## Register diagram — DMA2D surfaces

```
DMA2D @ 0x5200_1000  (RM0399 §18)
│
├── +0x00  CR     : MODE | START | CEIE | CTCIE | CAEIE | CTEIE
├── +0x04  ISR    : CEIF | CTCIF | CAEIF | TCIF | TWIF | TEIF
├── +0x08  IFCR   : write-1-to-clear for each ISR flag
├── +0x0C  FGMAR  : FG memory address
├── +0x10  FGOR   : FG line offset
├── +0x14  BGMAR  : BG memory address
├── +0x18  BGOR   : BG line offset
├── +0x1C  FGPFCCR: FG pixel format / CLUT / alpha
├── +0x20  FGCOLR : FG constant color (A8 + R2M mode)
├── +0x2C  BGPFCCR: BG pixel format
├── +0x3C  OPFCCR : output pixel format
├── +0x3C  OMAR   : output memory address
├── +0x40  OOR    : output line offset
└── +0x44  NLR    : pixels-per-line | number-of-lines
```

## Verify

- `rlvgl-playit ?` reports DMA2D last/max cycles incrementing
  under normal UI load.
- Force a fault: comment out the `dma2d_admits()` check in one
  renderer. You'll see `display.check_fifo_underrun()` start
  returning true in the frame-timing report at L4081 —
  LTDC's input FIFO ran dry because the AXI bus was still
  servicing DMA2D when the scan restarted.
- `rlvgl-playit RS / RD` (record start / record dump) captures
  the DMA2D completion events interleaved with ERIF.

## Going deeper

- RM0399 §18 "Chrom-Art Accelerator (DMA2D)" — the full
  register table and blend-mode equations.
- [`docs/rendering/BACKEND-ARCHITECTURE.md`](../rendering/BACKEND-ARCHITECTURE.md)
  — how the `Blitter` trait is picked per target.
- [`docs/rendering/ALPHA-BLENDING.md`](../rendering/ALPHA-BLENDING.md)
  — the alpha math DMA2D is accelerating.
- [`docs/assets/IMAGE-COMPRESSION-FORMAT.md`](../assets/IMAGE-COMPRESSION-FORMAT.md)
  — the RLE decoder that feeds DMA2D's `FGMAR`.

---

**[← Prev](06-touch-input.md) · [Index](README.md) · [Next →](08-secondary-peripherals.md)**
