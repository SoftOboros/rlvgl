<!--
07-adapted-cmd-deep-dive.md - Volume V Chapter 7: Full Rust DSI + LTDC
init, ERIF gating, PLL3, CSleep/LPENR fix.
-->

**[<- Prev](06-star-crawl-and-dma2d.md) . [Index](README.md)**

# Chapter 7 — Adapted Command Mode Deep Dive

## Volume II reference

Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
covered DSI + LTDC init and the ERIF holdoff. Under Zephyr ACM,
the same init runs — but with additional complications from
Zephyr's clock tree, CSleep power gating, and the dual-core
H747 RCC register layout.

## What this chapter covers

The full Rust DSI + LTDC init sequence for adapted command mode,
the PLL3 frequency issue, the C1_LPENR CSleep fix, and how the
ERIF-gated rendering integrates with Zephyr's `k_sleep`.

## Why adapted command mode exists

Zephyr's DSI driver only supports **video mode** — continuous
LTDC scan at 60 Hz. This monopolizes the AXI bus and prevents
DMA2D M2M transfers. Adapted command mode was implemented to
enable DMA2D-accelerated rendering (star crawl, future compositor).

The DTS overlay disables Zephyr's DSI, LTDC, and panel drivers.
Rust performs the full init from scratch, reusing the same
`display_init.rs` code as bare-metal.

## Walkthrough

### 1. Display init sequence

`display_init::init_full_adapted_cmd()` executes the RM0399
Section 34.14 init sequence:

1. Enable peripheral clocks (RCC AHB3ENR, APB3ENR, APB4ENR)
2. Ensure PLL3 running (PLLSAI for DSI byte clock)
3. GPIO pin mux: PG3 (reset), PJ2 (TE), DSI lane pins
4. DSI PHY init: PLL, timing, lane config
5. DSI host + wrapper: adapted command mode registers
6. LTDC init: sync timing, layer config, pixel format
7. Panel init: NT35510 DCS command sequence via DSI LP
8. First LTDCEN pulse: starts the scan cycle

### 2. PLL3 frequency

Bare-metal configures PLL3 for 32 MHz pixel clock. Under Zephyr,
the shield DTS sets PLL3 for 27.5 MHz. The Rust init calls
`ensure_pll3_running()` which checks PLL3ON and starts it if
needed, using whatever frequency the DTS configured.

### 3. ERIF gating

Same as bare-metal (Vol II Ch 5):

```rust
// DSI ISR on ERIF:
DSI_WCR.write_volatile(0x08);  // Clear LTDCEN, keep DSIEN
sync.isr_record_erif(cyc);
k_sem_give(&erif_sem);
```

LTDC scans once per LTDCEN pulse. Between scans, AXI is free
for DMA2D. The render loop re-pulses LTDCEN via `do_present()`
after composing the next frame.

### 4. CSleep and C1_LPENR

**Critical Zephyr-specific fix**: when the render loop calls
`k_sleep(K_MSEC(33))`, Zephyr enters **CSleep** (WFI). In
CSleep, peripheral clocks governed by `*LPENR` bits are gated.

On the dual-core H747, CM7 CSleep uses `RCC_C1_*LPENR`
registers — separate from the domain-level `RCC_*LPENR` that
bare-metal writes. The `RCC_C1_AHB3LPENR`, `RCC_C1_APB3LPENR`,
etc. bits for LTDC, DSI, DMA2D, and FMC must be set, or their
clocks gate off mid-scan when `k_sleep` executes WFI.

```rust
// Set LPEN bits for display peripherals
// RCC_C1_AHB3LPENR: DMA2DLPEN, FMCLPEN
// RCC_C1_APB3LPENR: LTDCLPEN, DSILPEN
```

Without this fix, the display corrupts randomly during `k_sleep`
— LTDC scan freezes mid-line, DMA2D transfers hang.

### 5. Dual-core register aliasing

On single-core H743, `RCC_AHB3LPENR` and `RCC_C1_AHB3LPENR`
are the same register (aliased). On dual-core H747, they are
separate. Code that writes `RCC_AHB3LPENR` works on H743 but
silently fails on H747 — the C1 variant must be used explicitly.

### 6. WCFGR.AR (auto-refresh)

Bare-metal uses AR=1 (automatic TE-triggered refresh). Under
Zephyr ACM, AR=1 causes icon flicker — the ERIF ISR's clear of
LTDCEN races the auto-refresh TE pulse. Setting AR=0 (manual
refresh) and having the render loop pulse LTDCEN explicitly
eliminates the race.

## Verify

```bash
make zephyr-disco-acm-flash
```

- Splash visible without corruption.
- Star crawl smooth (DMA2D M2M working).
- Icons stable during navigation (no CSleep corruption).
- `k_sleep` works without display artifacts.

## Going deeper

- Vol II [Ch 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
  — the bare-metal ERIF holdoff.
- RM0399 Section 8.7.46 — `RCC_C1_AHB3LPENR` vs `RCC_AHB3LPENR`.
- `display_init.rs` — shared DSI + LTDC init.
- `dsi_cmd_mode.rs` — adapted command mode register helpers.

---

**[<- Prev](06-star-crawl-and-dma2d.md) . [Index](README.md)**
