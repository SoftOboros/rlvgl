<!--
03-display-modes.md - Volume V Chapter 3: Video mode vs adapted
command mode.
-->

**[<- Prev](02-c-shell-and-ffi.md) . [Index](README.md) . [Next ->](04-touch-and-input.md)**

# Chapter 3 — Display Modes

## Volume II reference

Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
described LTDC + DSI in adapted command mode with the ERIF holdoff.
Zephyr's upstream DSI driver uses **video mode** instead.

## What this chapter covers

The two display modes available on the STM32H747I-DISCO under
Zephyr, their tradeoffs, and how the build selects between them.

## Video mode (default)

Zephyr's `dsi_stm32` driver brings up DSI in continuous video
mode. LTDC scans the framebuffer at 60 Hz in landscape (800x480).
The display is always live.

**Advantages**:
- No manual DSI/LTDC init — Zephyr drivers handle everything.
- Low display latency — always scanning.
- Simpler code path in `zephyr_entry.rs`.

**Disadvantages**:
- **DMA2D M2M deadlocks**: LTDC continuously reads SDRAM via AXI,
  monopolizing the bus. DMA2D M2M transfers stall indefinitely.
  DMA2D R2M (register-to-memory fills) work because they don't
  read SDRAM.
- Star crawl broken: the crawl pipeline requires DMA2D M2M for
  starfield row shifts.
- CPU-only rendering works fine.

### Framebuffer layout (video mode)

Zephyr allocates landscape framebuffers:
- FRONT: `0xD000_0000` (800x480x4 = 1.5 MB)
- BACK: `0xD017_7000` (offset by Zephyr's stride calculation)

### Present (video mode)

In video mode, LTDC scans continuously. "Present" is just swapping
the LTDC L1CFBAR register to point at the newly rendered buffer:

```rust
#[cfg(not(feature = "adapted_cmd"))]
{
    // Write new FB address, trigger shadow reload
    LTDC_L1CFBAR.write_volatile(fb_addr);
    LTDC_SRCR.write_volatile(1); // IMR
}
```

## Adapted command mode

Rust takes over DSI + LTDC initialization from scratch, disabling
Zephyr's display drivers via `adapted_cmd.overlay`. LTDC scans
only when explicitly triggered by pulsing LTDCEN in DSI_WCR.

**Advantages**:
- **DMA2D M2M works**: LTDC is off between scans, freeing the
  AXI bus for DMA2D.
- Star crawl pipeline functional.
- Same ERIF holdoff pattern as bare-metal (Vol II Ch 5).

**Disadvantages**:
- Manual DSI + LTDC init (~500 lines of raw register writes).
- Portrait framebuffer (480x800) — needs RotatedRenderer.
- Pulsed scans add latency.

### Framebuffer layout (adapted command mode)

Portrait framebuffers at hardcoded SDRAM addresses:
- FRONT: `0xD000_0000` (480x800x4 = 1.5 MB)
- BACK: `0xD080_0000` (SDRAM bank 1, different row)

### Present (adapted command mode)

Same as bare-metal: clear ERIF, write CFBAR, shadow reload,
pulse LTDCEN + DSIEN:

```rust
#[cfg(feature = "adapted_cmd")]
fn do_present(fb: *mut u8, w: u32, h: u32) {
    DSI_WIFCR.write_volatile(0x02);     // clear ERIF
    LTDC_L1CFBAR.write_volatile(fb as u32);
    LTDC_SRCR.write_volatile(1);
    DSI_WCR.write_volatile(0x0C);       // DSIEN + LTDCEN
}
```

## Choosing a mode

| Need | Use |
|------|-----|
| Simplest build, no DMA2D | Video mode |
| DMA2D acceleration, star crawl | Adapted command mode |
| File browser (SD card) | Either (filesystem is independent) |
| Zephyr display API compliance | Video mode |

## Verify

- **Video mode**: `make zephyr-disco` — splash visible, touch
  responsive, star crawl will NOT work (DMA2D deadlock).
- **ACM**: `make zephyr-disco-acm` — splash visible, star crawl
  functional, DMA2D pipeline active.

## Going deeper

- Vol II [Ch 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
  — the ERIF holdoff this reuses.
- [Ch 7](07-adapted-cmd-deep-dive.md) — full ACM init walkthrough.
- `display_init.rs` — the shared bare-metal + Zephyr ACM init.

---

**[<- Prev](02-c-shell-and-ffi.md) . [Index](README.md) . [Next ->](04-touch-and-input.md)**
