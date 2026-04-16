<!--
05-ltdc-dsi-and-axi-holdoff.md - Volume II Chapter 5: LTDC/DSI + ERIF-gated holdoff.
-->

**[← Prev](04-gpio-pin-mux.md) · [Index](README.md) · [Next →](06-touch-input.md)**

# Chapter 5 — LTDC, DSI & AXI Holdoff

## Volume I reference

Vol I
[Chapter 1](../disco-tutorial/01-hello-world.md) had `board::bring_up()`
return a `display` handle and
[Chapter 3](../disco-tutorial/03-desktop.md) quietly assumed
DMA2D blits would not tear the picture. Both of those rely on
the mechanism this chapter documents: the **ERIF-gated LTDCEN
holdoff** that keeps DMA2D out of the scan window.

## What this chapter covers

Three things that work together to feed the panel:

1. **LTDC** — the LCD-TFT controller that scans a framebuffer
   out of SDRAM into the MIPI-DSI host.
2. **DSI** — the MIPI-DSI host that wraps LTDC output into DSI
   packets and drives the OTM8009A panel over the D-PHY lanes.
3. **The AXI / LTDC-EN holdoff** — the orchestration pattern
   the codebase calls out (and that gives the chapter its name)
   where the DSI end-of-refresh interrupt (**ERIF**) clears
   `LTDCEN` so no scan runs while DMA2D is writing the back
   buffer, and the main loop re-enables `LTDCEN` on `present()`
   once the frame is ready.

## The HAL / PAC gap

`stm32h7xx-hal` does not ship an LTDC or DSI driver. This is
not a gap so much as scope: the LTDC configuration is board-
specific (panel timing, pin set, byte order) and the DSI side
needs a panel driver per part anyway. Both live in the platform
crate.

The real gotchas are inside DSI orchestration:

- DSI and LTDC share a clock/handshake. Writing a framebuffer
  address to `LTDC_LxCFBAR` **while a scan is in progress** is
  an aliasing hazard — the LTDC latches the new address at a
  frame boundary, and if your timing is wrong you get a half-
  torn frame. The fix is to stop scanning before swapping.
- DSI has a bank of interrupts that merge to a single NVIC line
  (IRQ 123). Only **ERIF** matters for scheduling; the others
  need to be cleared or they re-fire forever.

## Walkthrough

### 1. LTDC & DSI init (platform crate)

Vol I's `Stm32h747iDiscoDisplay::new(…)` lives in
[`platform/src/stm32h747i_disco.rs`](../../platform/src/stm32h747i_disco.rs)
and owns LTDC + DSI + panel bring-up end to end. It:

1. Programs LTDC timing for 800×480 @ ~60 Hz (see the register
   diagram below for the values).
2. Brings up the DSI host, sets video-mode burst parameters,
   waits for the D-PHY to lock.
3. Runs the OTM8009A wake sequence over DSI generic writes
   (sleep-out, display-on, pixel format, gamma load — all per
   the OTM8009A datasheet).
4. Clears the first framebuffer in SDRAM to black, enables
   LTDC layer 1, sets `LTDCEN`, and returns a `display` handle.

When Chapter 6 onwards writes into the framebuffer, LTDC is
already scanning.

### 2. The DSI ERIF interrupt

The DSI peripheral sets the **End of Refresh Interrupt Flag**
(WISR bit 1) at the end of every LTDC scan. The disco crate
hooks IRQ 123 in
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L362–405:

```rust
/// DSI interrupt — all DSI events merge into IRQ 123.
/// We only care about ERIF (end of refresh, WISR bit 1).
/// Clear ALL flags to prevent non-ERIF events from re-triggering.
#[interrupt]
unsafe fn DSI() {
    const WISR:  *const u32 = 0x5000_040C as *const u32;
    const WIFCR: *mut   u32 = 0x5000_0410 as *mut   u32;
    // ...host-level flag clear registers...
    unsafe {
        let wisr = WISR.read_volatile();
        WIFCR.write_volatile(wisr & 0x3FFF);     // clear wrapper flags
        if wisr & 0x02 != 0 {
            let cyc = (0xE000_1004u32 as *const u32).read_volatile(); // DWT_CYCCNT
            (0x5802_2418u32 as *mut u32).write_volatile(1u32 << 16);  // PJ0 low probe
            const DSI_WCR: *mut u32 = 0x5000_0404 as *mut u32;
            DSI_WCR.write_volatile(0x08);        // DSIEN only — clears LTDCEN
            super::ERIF_CYCCNT.store(cyc, Ordering::Release);
            super::ERIF_FLAG.store(true,  Ordering::Release);
        }
        // ...clear host flags so they don't re-fire...
    }
}
```

Two critical side effects in this ISR:

- **`DSI_WCR = 0x08`** clears `LTDCEN` but keeps `DSIEN` set.
  From this moment, LTDC is idle — it is **not** scanning.
  The framebuffer is safe to overwrite.
- `ERIF_CYCCNT` is snapshotted from DWT before the flag is
  raised, so anything that checks "how long since ERIF?" gets
  a consistent reading.

### 3. The present() holdoff

The main loop wakes on `ERIF_FLAG`, runs renderers, then
re-enables `LTDCEN` in `display.present()` — but only after a
deliberate **hold-off** from ERIF so the panel always gets the
next frame at the same slot relative to its Tearing Effect
output. In
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L4097–4130:

```rust
// ── Pipeline stage: PRESENT (phase-locked to ERIF) ─────────
// Hold off present until a fixed offset after ERIF, so we
// always target the same TE slot. This eliminates the
// beat-frequency drift between TE and the render loop.
//
// With LTDCEN cleared in the ERIF ISR, no scan runs until
// present() re-enables LTDCEN. ERIF_FLAG stays true until
// consumed, so take_erif() succeeds whenever we check.
//
// Holdoff = 15ms (6M cycles): safely after render (~10ms),
// before TE+1 (~19ms after ERIF). Ensures every frame
// catches the same TE slot → constant frame period.
const PRESENT_HOLDOFF: u32 = 6_000_000; // 15ms at 400MHz
if buffer_ready && cycles_since_erif() >= PRESENT_HOLDOFF && take_erif() {
    // Update ERIF-to-ERIF period estimate (EMA)...
    display.present();
    ERIF_FLAG.store(false, Ordering::Release);
    scope_probe::ltdc_active();
    buffer_ready = false;
    present_count = present_count.wrapping_add(1);
```

This is the AXI-bus "holdoff" the chapter title refers to:
the main loop voluntarily **parks** for 15 ms after each
ERIF before re-enabling `LTDCEN`, so DMA2D has exclusive
access to SDRAM during the render phase. The LTDC stops
pulling pixels across the AXI bus; DMA2D (which is also an
AXI master) doesn't compete with it.

`display.present()` is where `LTDCEN` flips back on and the
next scan begins.

### 4. DMA2D admission, briefly

The related mechanism — `dma2d_admits(cost)` at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L432–438 —
gates individual DMA2D jobs so they must finish before the
next ERIF is expected. It's covered in
[Chapter 7](07-dma2d-engine.md), but you can see it here:

```rust
pub fn dma2d_admits(cost: u32) -> bool {
    const GUARD: u32 = 400_000; // 1ms safety margin at 400MHz
    let budget = FRAME_BUDGET_CYCLES.load(Ordering::Relaxed);
    let elapsed = cycles_since_erif();
    let remaining = budget.saturating_sub(elapsed);
    remaining > cost + GUARD
}
```

The `FRAME_BUDGET_CYCLES` atomic (L355–356) is an EMA of the
observed ERIF-to-ERIF interval, so the admission gate adapts
to the real panel rate rather than a hardcoded 60 Hz.

## Register diagram — LTDC / DSI

```
LTDC timing for 800×480 @ ~60 Hz  (RM0399 §33)
│
├── LTDC_SSCR  : HSYNC/VSYNC widths    (H=1, V=1)
├── LTDC_BPCR  : back porch widths     (H=45, V=22)
├── LTDC_AWCR  : active widths         (H=845, V=502)
├── LTDC_TWCR  : total widths          (H=1055, V=510)
├── LTDC_GCR   : global (LTDCEN = bit 0)
└── LTDC_LxCFBAR : layer framebuffer start address

DSI wrapper control  (RM0399 §32)
│
├── WCR @ 0x5000_0404   bit 0 = LTDCEN, bit 3 = DSIEN
├── WISR @ 0x5000_040C  bit 1 = ERIF
└── WIFCR @ 0x5000_0410 write-1-to-clear for WISR flags
```

## Verify

- Halt; dump `0x5000_0404` (`DSI_WCR`). During the render
  phase (between ERIF and present) it reads `0x08`. After
  present, `0x09`.
- Dump `0x5000_040C` (`WISR`) — ERIF flag (bit 1) pulses
  once per frame.
- `rlvgl-playit ?` command over the VCP reports a live
  `present_count` that increments at ~60 Hz.
- Visual: the Vol I desktop background never tears even with
  Chapter 6 DMA2D blits running in the background.

Fault modes:

- Tearing diagonal lines → present() is firing while LTDCEN is
  on. Usually the holdoff got shortened by mistake.
- Screen freezes ~1 s after boot → a non-ERIF DSI flag is
  looping and pegging the ISR. Make sure the ISR clears
  WIFCR, ISR0/FIR0, ISR1/FIR1 unconditionally.

## Going deeper

- RM0399 §32 (DSI host + wrapper) and §33 (LTDC) — register
  details including the aliased LTDC register set.
- OTM8009A datasheet — the panel's DSI command set.
- [`platform/README.md`](../../platform/README.md) — the
  `Stm32h747iDiscoDisplay` type and the `Blitter` trait the
  display delegates to.
- [`docs/RENDERING-BACKEND-ARCHITECTURE.md`](../RENDERING-BACKEND-ARCHITECTURE.md)
  — how the platform crate composes LTDC + DMA2D + the
  display driver.

---

**[← Prev](04-gpio-pin-mux.md) · [Index](README.md) · [Next →](06-touch-input.md)**
