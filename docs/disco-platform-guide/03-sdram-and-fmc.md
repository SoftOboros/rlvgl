<!--
03-sdram-and-fmc.md - Volume II Chapter 3: FMC bring-up + SDTR raw-address fix.
-->

**[← Prev](02-clocks-and-plls.md) · [Index](README.md) · [Next →](04-gpio-pin-mux.md)**

# Chapter 3 — SDRAM & FMC

## Volume I reference

Vol I
[Chapter 1](../disco-tutorial/01-hello-world.md) required SDRAM
at `0xC000_0000` (actually `0xD000_0000` on this board — the
FMC programs SDRAM Bank 2) before LTDC could scan a framebuffer.
The `pac_sdram_init` feature was turned on and that was it.
This chapter is the "and that was it."

## What this chapter covers

The full JEDEC init sequence for the DISCO's IS42S32800J-6BLI
SDRAM (32 Mbit × 32, 2 banks, on FMC Bank 2), programmed through
the FMC peripheral register-by-register. Includes the SDTR
raw-address fix from Chapter 1.

## The HAL / PAC gap

Two separate problems:

1. `stm32h7xx-hal`'s `fmc` feature provides a driver that
   works for straightforward SDRAM parts — but the DISCO uses
   Bank 2 (SDNE1/SDCKE1), and the HAL's Bank 2 timing is
   subtly miscalibrated for the IS42S32800J at 100 MHz. The
   crate uses the PAC directly so the timing values live in
   one place it can audit.
2. The PAC itself has an off-by-one in `sdbank1().sdtr` and
   `sdbank2().sdtr` (see
   [Ch 1 §2](01-why-bare-metal.md#2-fmc-sdbank1sdtr-offset-is-wrong-in-the-pac)).
   Raw address writes for SDTR1/SDTR2 are the workaround.

## Walkthrough

All in
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1034–1132,
gated behind the `pac_sdram_init` feature.

### 1. Constants

```rust
const SDRAM_REFRESH_COUNT: u16 = 566;      // 64 ms / 8192 rows × 100 MHz ≈ 566
const SDRAM_MODE_REGISTER: u16 = 0x0230;   // CAS=3, burst length=2
```

Derived from the IS42S32800J datasheet and the 100 MHz SDRAM
clock (SYSCLK / 2, wired via PLL1_P / HCLK / SDCLK divider).

### 2. Enable the FMC and program SDCR1 / SDCR2

```rust
fn configure_fmc_sdram(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    unsafe {
        fmc.bcr1.modify(|_, w| w.fmcen().set_bit());

        // SDCR1: shared bits only (SDCLK, RBURST, RPIPE)
        fmc.sdbank1().sdcr.write(|w| w
            .sdclk().bits(0b01)   // Reserved per RM0399, but required on this silicon
            .rburst().set_bit()
            .rpipe().bits(0));

        // SDCR2: bank-specific config
        //   NC=01 (9-bit column), NR=01 (12-bit row), MWID=10 (32-bit),
        //   NB=set (4 banks), CAS=11 (3 cycles), WP=clear
        fmc.sdbank2().sdcr.write(|w| w
            .nc().bits(0b01)
            .nr().bits(0b01)
            .mwid().bits(0b10)
            .nb().set_bit()
            .cas().bits(0b11)
            .wp().clear_bit());
```

The SDCLK field value `0b01` is flagged "Reserved" in RM0399
but is what this silicon actually requires — confirmed against
the CubeMX output for the board.

### 3. Program SDTR1 / SDTR2 — raw addresses

This is the Chapter 1 §2 workaround quoted in full at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1099–1117:

```rust
        // SDTR1: shared timing (TRP, TRC must be in SDTR1)
        // PAC sdbank1().sdtr offset = 0x144 = SDCR2 (known PAC bug).
        // Use raw write to SDTR1 at 0x148.
        let sdtr1 = 0x5200_4148u32 as *mut u32;
        sdtr1.write_volatile(
            (1 << 20)   // TRP = 2 cycles
            | (6 << 12) // TRC = 7 cycles
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
    }
```

Timing values are in **number of SDRAM clock cycles − 1** (so
`(1 << 20)` in TRP field means "2 cycles"). Values come
straight from the IS42S32800J datasheet at 100 MHz.

### 4. JEDEC init command sequence

SDRAMs need a specific power-up ritual. The FMC has a command
register (SDCMR) that issues the JEDEC commands for you:

```rust
issue_sdram_command(fmc, 0b001, 0, 0);              // Clock configuration enable
cortex_m::asm::delay(100_000);                      // ≥100 µs power-up delay
issue_sdram_command(fmc, 0b010, 0, 0);              // PALL (Precharge all)
issue_sdram_command(fmc, 0b011, 7, 0);              // Auto-refresh ×8
issue_sdram_command(fmc, 0b100, 0, SDRAM_MODE_REGISTER); // Load mode register
issue_sdram_command(fmc, 0b000, 0, 0);              // Normal mode
```

Each `issue_sdram_command()` writes SDCMR and polls SDSR
until the "busy" flag clears (L1040–1044).

### 5. Enable refresh

```rust
unsafe {
    fmc.sdrtr.write(|w| w.count().bits(SDRAM_REFRESH_COUNT));
}
```

From this point, SDRAM is live at `0xD000_0000` (Bank 2 base).
The Volume I crate uses `0xC000_0000` in prose for readability
but the actual framebuffer lives in Bank 2 space — see
[`examples/stm32h747i-disco/MEMORY.md`](../../examples/stm32h747i-disco/MEMORY.md).

### 6. Prerequisites you also need

SDRAM cannot work without all its pins muxed to FMC AF12 at
VeryHigh speed. That is Chapter 4's job, in
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1181–1250
(`early_fmc_setup()`). If you're reading this chapter in order,
the FMC call site in `main()` will run **after** `early_fmc_setup()`
— but to follow the narrative you have to skip ahead briefly.

## Register diagram — FMC bank registers

```
FMC SDRAM @ 0x5200_4140  (RM0399 §22.9.5)
│
├── +0x140  SDCR1  : shared-bank config (SDCLK, RBURST, RPIPE)
├── +0x144  SDCR2  : bank-2 config (NC, NR, MWID, NB, CAS, WP)  ← PAC calls this "sdbank1().sdtr" (wrong)
├── +0x148  SDTR1  : shared timing (TRP, TRC)                    ← PAC calls this "sdbank2().sdtr" (wrong)
├── +0x14C  SDTR2  : bank-2 timing (TRCD, TWR, TRAS, TXSR, TMRD)
├── +0x150  SDCMR  : command mode + mode register data
├── +0x154  SDRTR  : refresh timer register
└── +0x158  SDSR   : status (bit 5 = busy)
```

The "PAC calls this X" comments are the off-by-one bug in the
svd2rust register map — SDCR1/SDCR2 come out with the right
offsets because they're addressed through their own bank
accessors, but SDTR1/SDTR2 appear under sibling bank accessors
one register too early. Direct address writes sidestep the
whole mess.

## Verify

- Halt under probe-rs; read `0x5200_4148` and `0x5200_414C`.
  Values should match the magic numbers above.
- Write a canary word to `0xD000_0000` and read it back; it
  should survive (SDRAM is alive).
- Volume I's splash image (Chapter 2) renders without
  corruption.

Fault modes:

- Random pixel garbage across the splash → refresh is wrong
  (SDRTR value or timing).
- Hard fault reading `0xD000_0000` → FMC not enabled, GPIO
  pins not muxed, or PLL1 not at 400 MHz (no 100 MHz SDCLK).

## Going deeper

- RM0399 §22 "Flexible memory controller (FMC)". §22.9.5 has
  the SDRAM-specific registers; §22.9.6 documents the JEDEC
  mode-register bits that `SDRAM_MODE_REGISTER` encodes.
- IS42S32800J datasheet — timing parameters and the JEDEC
  command table.
- [`examples/stm32h747i-disco/BOOT.md`](../../examples/stm32h747i-disco/BOOT.md)
  — where SDRAM init fits in the overall boot flow relative
  to the CM4 ready-flag and linker-script-owned regions.
- The `sdram_ramtest` feature
  ([`Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml)
  L39) exercises SDRAM with a walking-one pattern before
  trusting the framebuffer to it. Turn it on if the surface
  symptoms in the "Fault modes" list above match what you
  see.

---

**[← Prev](02-clocks-and-plls.md) · [Index](README.md) · [Next →](04-gpio-pin-mux.md)**
