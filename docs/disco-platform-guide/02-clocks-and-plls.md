<!--
02-clocks-and-plls.md - Volume II Chapter 2: clock tree + PLL3ON fix.
-->

**[← Prev](01-why-bare-metal.md) · [Index](README.md) · [Next →](03-sdram-and-fmc.md)**

# Chapter 2 — Clocks & PLLs

## Volume I reference

Vol I
[Chapter 1](../disco-tutorial/01-hello-world.md) listed "Clock
tree (HSE, PLL1 for CPU/AXI, PLL3 for LTDC pixel clock)" as the
first box `board::bring_up()` ticks. This chapter fills it in.

## What this chapter covers

How the CM7's clock tree is programmed out of reset: HSE →
PLL1/2/3 → SYSCLK / HCLK / APB buses + kernel clocks for LTDC,
SDMMC, and QSPI. Most of the work happens through
`stm32h7xx-hal`'s fluent builder; this chapter explains which
dials it turns, and then the one bit it leaves untouched.

## The HAL / PAC gap

`stm32h7xx-hal`'s `Rcc::freeze()` configures PLL1/2/3 dividers,
waits for `PLL1RDY` and `PLL2RDY`, and returns. It does **not**
turn PLL3 on — see [Chapter 1 §1](01-why-bare-metal.md#1-pll3_r_ck-silently-leaves-pll3-off).
The first LTDC read hangs because there is no pixel clock to
clock the LTDC AHB slave.

Fix: set PLL3ON in RCC_CR and poll PLL3RDY, immediately after
`freeze()`.

## Walkthrough

All in
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1569–1596.

### 1. Build the PLL tree

```rust
let ccdr = rcc
    .use_hse(25.MHz())
    .sys_ck(400.MHz())
    .hclk(200.MHz())
    .pll1_strategy(PllConfigStrategy::Iterative)
    // PLL1_Q needed for SDMMC kernel clock. 200 MHz = VCO/4 keeps
    // VCO at 800 MHz (same as sys_ck=400 with P_div=2), avoiding
    // any disturbance to PLL1_P or display timing.
    .pll1_q_ck(200.MHz())
    .pll2_r_ck(150.MHz())
    // Target ~33 MHz pixel clock for 800x480 panel bring-up
    .pll3_r_ck(32.MHz())
    .freeze(vos, &mut syscfg);
```

Role of each PLL:

| PLL | Output | Consumer |
|-----|--------|----------|
| PLL1_P | 400 MHz | SYSCLK (CPU); derived HCLK = 200 MHz, PCLK1/2/3/4 at /2 |
| PLL1_Q | 200 MHz | SDMMC1/2 kernel clock (Chapter 8) |
| PLL2_R | 150 MHz | QSPI kernel clock (Chapter 8, errata 2.8.5) |
| PLL3_R | 32 MHz  | LTDC pixel clock (Chapter 5) |

### 2. Gate display-domain peripherals

HAL `enable()` calls for the peripherals every later chapter
needs:

```rust
let _ = ccdr.peripheral.LTDC.enable();
let _ = ccdr.peripheral.DMA2D.enable();
let _ = ccdr.peripheral.DSI.enable();
let _ = ccdr.peripheral.FMC.enable();
```

Calling `.enable()` after `freeze()` is the HAL's one-shot gate.
You cannot flip these at runtime through the same handle —
`ccdr.peripheral.*` is consumed.

### 3. Force PLL3 on

```rust
// HAL bug: pll3_r_ck() configures PLL3 dividers but never sets PLL3ON.
// Without PLL3R running, LTDC register reads hang (no pixel clock domain).
// Force PLL3ON and wait for PLL3RDY.
unsafe {
    const RCC_CR: *mut u32 = 0x5802_4400u32 as *mut u32;
    RCC_CR.write_volatile(RCC_CR.read_volatile() | (1 << 28)); // PLL3ON
    while RCC_CR.read_volatile() & (1 << 29) == 0 {} // wait PLL3RDY
}
```

Two bits matter in `RCC_CR`: bit 28 (PLL3ON) and bit 29
(PLL3RDY). The spin is bounded (microseconds); no timeout
guard is needed because if PLL3 never locks, nothing else
brings the display up and the board is inert anyway.

### 4. Breadcrumb to D3 SRAM

Right after the clock tree is up, a debug magic is written
to shared SRAM so `probe-rs` can confirm clocks-good without
serial:

```rust
unsafe {
    (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0005u32);
}
```

These breadcrumbs appear throughout bring-up. `0xA11C_0005`
means "pre-gpio-split" — clocks are live, GPIO port handles
have not been claimed yet. See
[`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
for how to read them.

## Register diagram — RCC_CR, the bits that matter

```
RCC_CR @ 0x5802_4400 (RM0399 §8.7.2)
│
├── bit 28  PLL3ON   : 1 = enable PLL3
└── bit 29  PLL3RDY  : 1 = PLL3 locked (read-only)
```

Other RCC_CR bits (HSEON, HSERDY, CSION, etc.) are already
configured by the HAL `freeze()` call — do not touch them here.

## Verify

After reflashing a build with the PLL3 block intact:

- Halt under probe-rs and read `0x3800_0300` — it should hold
  `0xA11C_0005`.
- Read RCC_CR at `0x5802_4400` — bits 28 and 29 should both be
  set (value OR'd with `0x3000_0000`).
- The Volume I Hello World build draws its centered label
  without hanging at panel init.

Fault modes:

- Screen never lights → PLL3ON likely not set. Re-check the
  raw write.
- Screen flickers with horizontal tearing → `pll3_r_ck(32.MHz())`
  is close to the edge of the OTM8009A's tolerance. If you ran
  hot during bring-up, dropping to 30 MHz is acceptable.

## Going deeper

- RM0399 §8 "Reset and clock control (RCC)" — the full PLL
  tree, VCO ranges, and divider constraints.
- [`platform/README.md`](../../platform/README.md) — where
  `stm32h7xx-hal` types (`Ccdr`, `PllConfigStrategy`) leak
  into the platform crate.
- [`docs/EMBEDDED-TOOLING.md`](../EMBEDDED-TOOLING.md) — how
  to script D3 SRAM breadcrumb reads from probe-rs.
- [`examples/stm32h747i-disco/MEMORY.md`](../../examples/stm32h747i-disco/MEMORY.md)
  — D3 SRAM layout, including the breadcrumb region.

---

**[← Prev](01-why-bare-metal.md) · [Index](README.md) · [Next →](03-sdram-and-fmc.md)**
