<!--
01-why-bare-metal.md - Volume II Chapter 1: motivation + the "gap gallery".
-->

**← Prev · [Index](README.md) · [Next →](02-clocks-and-plls.md)**

# Chapter 1 — Why Bare Metal?

## Volume I reference

Volume I's
[Chapter 1 (Hello World)](../disco-tutorial/01-hello-world.md)
took `board::bring_up()` on faith. This chapter explains why that
helper is not a thin wrapper over `stm32h7xx-hal` — it's ~2000
lines of hand-written PAC writes plus a dozen raw-address pokes.

## What this chapter covers

Five concrete places in the disco codebase where the `svd2rust`-
generated PAC or `stm32h7xx-hal` is wrong, incomplete, or
incompatible with the real-time path, and what the crate does
about it. Each one is a link the reader can open and read.

This is not a rant about HAL crates. It's a reference catalogue
you'll keep coming back to as later chapters cite individual
entries.

## The repo rule

From the project's operating memory:

> Prefer PAC + TRM over HAL crates. New chip bring-up goes
> against the PAC and the Technical Reference Manual directly.

That rule exists because of the five examples below. You do not
have to agree with it in the abstract — you only have to recognize
the pattern when it bites.

## The gap gallery

### 1. `pll3_r_ck()` silently leaves PLL3 off

`stm32h7xx-hal`'s `Rcc::pll3_r_ck()` configures PLL3's dividers
but never sets the **PLL3ON** bit. The freeze call returns success;
subsequent LTDC register reads hang because there is no pixel-
clock domain.

The fix, at
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1589–1596:

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

Full walk-through in [Chapter 2](02-clocks-and-plls.md).

### 2. FMC `sdbank1().sdtr` offset is wrong in the PAC

`svd2rust` computes `sdbank1().sdtr` at offset **0x144**, which
is actually SDCR2. The correct offset is 0x148. The same one-
register shift affects `sdbank2().sdtr` (0x148 vs 0x14C). Writing
through the PAC accessor corrupts SDCR2 and misconfigures
refresh, and the SDRAM never comes up.

The fix, at
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1099–1117:

```rust
// SDTR1: shared timing (TRP, TRC must be in SDTR1)
// PAC sdbank1().sdtr offset = 0x144 = SDCR2 (known PAC bug).
// Use raw write to SDTR1 at 0x148.
let sdtr1 = 0x5200_4148u32 as *mut u32;
sdtr1.write_volatile(
    (1 << 20)   // TRP = 2 cycles
    | (6 << 12), // TRC = 7 cycles
);
// SDTR2: bank-specific timing
// PAC sdbank2().sdtr offset = 0x148 = SDTR1 (same PAC bug pattern).
// Use raw write to SDTR2 at 0x14C.
let sdtr2 = 0x5200_414Cu32 as *mut u32;
sdtr2.write_volatile(
    (1 << 24)   // TRCD = 2 cycles
    | (1 << 16) // TWR = 2 cycles
    | (4 << 8)  // TRAS = 5 cycles
    | (6 << 4)  // TXSR = 7 cycles
    | (1 << 0), // TMRD = 2 cycles
);
```

Full walk-through in [Chapter 3](03-sdram-and-fmc.md).

### 3. `embedded-hal` I2C blocks per byte

The FT5336 touch controller must be polled at ≥100 Hz to catch
press/release transitions. A blocking `embedded-hal::i2c::I2c`
read inside a TIM6 ISR is a non-starter — each byte waits on
the BUSY flag, and the ISR would miss deadlines.

The disco firmware drives I2C4 **directly** from the ISR using
raw register writes to CR2, TXDR, RXDR, ISR, ICR. The ISR reads
touches in ~40 µs, pushes them to an SPSC ring buffer, and
returns. Addresses from
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) L101–106:

```rust
// I2C4 register addresses (base 0x5800_1C00, RM0399 §50.7)
const I2C4_CR2:  *mut   u32 = 0x5800_1C04 as *mut   u32;
const I2C4_ISR:  *const u32 = 0x5800_1C18 as *const u32;
const I2C4_ICR:  *mut   u32 = 0x5800_1C1C as *mut   u32;
const I2C4_RXDR: *const u32 = 0x5800_1C24 as *const u32;
const I2C4_TXDR: *mut   u32 = 0x5800_1C28 as *mut   u32;
```

Full walk-through in [Chapter 6](06-touch-input.md).

### 4. QSPI errata 2.8.5 — wrong default kernel clock

Per ES0392 §2.8.5, if the QSPI kernel clock is sourced from
HCLK (the svd2rust/HAL default) the QSPI peripheral misbehaves
on certain silicon revisions. The fix is to force QSPISEL to
PLL2R.

The crate does this by writing `D1CCIPR` directly, at
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1717–1723:

```rust
// Errata 2.8.5: Select PLL2R (150 MHz) as QSPI kernel clock
// D1CCIPR QSPISEL bits [5:4]: 00=HCLK, 01=PLL1Q, 10=PLL2R, 11=PER
unsafe {
    let d1ccipr = 0x5802_4C18u32 as *mut u32;
    let val = d1ccipr.read_volatile();
    d1ccipr.write_volatile((val & !(0b11 << 4)) | (0b10 << 4));
}
```

Full walk-through in [Chapter 8](08-secondary-peripherals.md).

### 5. D-cache transparency — the write-back trap

The Cortex-M7 D-cache is write-back. DMA2D is a bus master that
reads memory *directly*, bypassing the cache. If the CPU writes
to D2 SRAM (at `0x3000_0000`) and then kicks DMA2D to read the
same region, DMA2D sees stale data until those cache lines are
cleaned.

The disco crate cleans the relevant range manually before every
A8→ARGB blend, at
[`src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L716–736:

```rust
// Clean D-cache lines covering [addr, addr+size) so DMA2D sees CPU writes.
//
// D2 SRAM at 0x3000_0000 is Write-Back Write-Allocate under the default
// Cortex-M7 background map. Without a clean, DMA2D reads stale data.
fn dcache_clean_range(addr: usize, size: usize) {
    const DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32;
    const LINE_SIZE: usize = 32;
    let start = addr & !(LINE_SIZE - 1);
    let end = (addr + size + LINE_SIZE - 1) & !(LINE_SIZE - 1);
    let mut a = start;
    while a < end {
        unsafe { DCCMVAC.write_volatile(a as u32); }
        a += LINE_SIZE;
    }
    cortex_m::asm::dsb();
}
```

Full walk-through in [Chapter 10](10-star-crawl-part-2.md).

## Register diagram — this chapter's addresses

| Register | Address | RM0399 section |
|----------|---------|----------------|
| RCC_CR | `0x5802_4400` | §8.7.2 |
| FMC SDTR1 | `0x5200_4148` | §22.9.5 |
| FMC SDTR2 | `0x5200_414C` | §22.9.5 |
| I2C4_CR2 | `0x5800_1C04` | §50.7.2 |
| RCC D1CCIPR | `0x5802_4C18` | §8.7.13 |
| SCB DCCMVAC | `0xE000_EF68` | ARMv7-M ARM §B3.2.1 |

## Verify

No build in this chapter — it's pure prose. The quickest way
to feel the motivation: try stripping the PLL3ON block out of
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) and
reflashing. The Volume I Hello World build will hang at the
first LTDC read. Then put it back.

## Going deeper

- [`examples/stm32h747i-disco/BRINGUP.md`](../../examples/stm32h747i-disco/BRINGUP.md)
  — the hand-maintained bring-up checklist. Each item on that
  list is a potential entry in a future gap gallery.
- RM0399 §8 (RCC), §22 (FMC), §50 (I2C), §B3 Cortex-M7 system
  registers (DCCMVAC).
- ES0392 — the STM32H747XI errata sheet.
- [`stm32h7xx-hal` GitHub](https://github.com/stm32-rs/stm32h7xx-hal)
  — not a criticism; upstream welcomes patches for every item
  here. They are documented so you know why the crate bypasses
  them today.

---

**← Prev · [Index](README.md) · [Next →](02-clocks-and-plls.md)**
