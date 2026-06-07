<!--
04-gpio-pin-mux.md - Volume II Chapter 4: GPIO AF mux for FMC/LTDC/DSI.
-->

**[← Prev](03-sdram-and-fmc.md) · [Index](README.md) · [Next →](05-ltdc-dsi-and-axi-holdoff.md)**

# Chapter 4 — GPIO Pin Mux

## Volume I reference

Vol I
[Chapter 1](../disco-tutorial/01-hello-world.md) glossed over
"GPIO bring-up" as one step inside `board::bring_up()`. This
chapter is the reference page for the dozens of pins that get
multiplexed to peripheral signals.

## What this chapter covers

How every pin that matters gets put into **alternate function
mode** for the right peripheral, at the right **output slew
rate**. The chapter is short: GPIO mux is mechanical. But every
later chapter (LTDC, DSI, touch, audio, QSPI, USART1) assumes
the pins it needs are already muxed, and this is where that
happens.

## The HAL / PAC gap

None worth a chapter. `stm32h7xx-hal`'s `gpio` module exposes
`pin.into_alternate::<AF>()` and `pin.set_speed(Speed::VeryHigh)`
and they work. The disco crate uses both.

The honest friction here is **volume**: the FMC alone needs
~50 pins muxed. The crate uses a tiny macro to keep the block
readable.

## Walkthrough

### 1. Split the port handles

Right after the RCC freeze, claim every GPIO port the board
uses. Pattern in
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1606–1608:

```rust
let gpioj = GPIOJ.split(ccdr.peripheral.GPIOJ);
let gpiog = GPIOG.split(ccdr.peripheral.GPIOG);
let gpiok = GPIOK.split(ccdr.peripheral.GPIOK);
// ...through GPIOI
```

The `.split()` call is what enables the port clock and returns
individual pin handles. Each pin handle is single-use.

### 2. The `af12_high!` macro

FMC needs nearly every pin muxed to AF12 at VeryHigh slew.
Rather than repeat the boilerplate, the crate introduces a
local macro at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1645–1650:

```rust
macro_rules! af12_high {
    ($pin:expr) => {{
        let mut pin = $pin.into_alternate::<12>();
        pin.set_speed(Speed::VeryHigh);
    }};
}
```

The macro `let`-binds the pin into a mutable variable solely
so `set_speed()` can be called — the HAL returns an owned
typed pin whose state must be fixed before it's dropped.

### 3. The FMC pin set

Applied in bulk at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1651–1706.
The pin set breaks down as:

| Bus | Pins | Port coverage |
|-----|------|---------------|
| Address A0–A11 | 24 pins (address + NBL + NWE + NCAS + NRAS + SDCKE1 + SDNE1) | PF0–PF5, PF11–PF15, PG0–PG2, PG4, PG8, PG15, PH5–PH7 |
| Data D0–D31    | 32 pins | PD0/1/8/9/10/14/15, PE7–PE15, PH8–PH15, PI0–PI7/9/10 |

`af12_high!(gpioe.pe7); af12_high!(gpioe.pe8); …` sixty times.
That is fine. Long is not wrong.

**Why VeryHigh speed** is non-negotiable: the FMC clocks at 100
MHz, which means 10 ns edges. Slower drive strengths cause
setup/hold violations on the SDRAM's data lines. The macro
exists so no pin gets missed.

### 4. Other peripherals' pin sets

Not all in one block — each chapter configures the pins it owns
right before using the peripheral:

- **LTDC** data pins (RGB565 on PI14/15, PJ0–PJ11, PK0–PK2)
  and control (HSYNC/VSYNC/DE/CLK) — configured inside
  `Stm32h747iDiscoDisplay::new()` in the platform crate.
- **DSI** — lane pins are dedicated silicon, no mux; enable
  gated by the DSI peripheral clock from Chapter 2.
- **QSPI Bank 1** — PB2, PD11, PF6/7/9 at various AF9/AF10,
  configured in
  [`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1726–1731
  (shown in Chapter 8).
- **USART1** — PA9/PA10 at AF7, raw-register configured
  (no HAL) in
  [`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1830–1838
  (Chapter 8).
- **I2C4** — SCL/SDA shared with touch + codec; configured
  in the platform crate.
- **Touch INT** — PK7 as floating input,
  [`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L2126.

### 5. Breadcrumb

Immediately after the FMC pin block:

```rust
unsafe {
    (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0007u32);
}
```

`0xA11C_0007` = "post-FMC-pins". If SDRAM comes up, this
landmark was passed.

## Register diagram — what each call actually writes

```
GPIOx @ 0x5802_0000 + 0x400 × port   (RM0399 §14.4)
│
├── +0x00  MODER     : 2 bits/pin — 10 = alternate function
├── +0x08  OSPEEDR   : 2 bits/pin — 11 = very high speed
├── +0x20  AFRL      : 4 bits/pin (pins 0..7)  — AF number
└── +0x24  AFRH      : 4 bits/pin (pins 8..15) — AF number
```

`into_alternate::<12>()` writes MODER and AFRL/AFRH. `set_speed(VeryHigh)`
writes OSPEEDR. That's the whole mechanism.

## Verify

- `probe-rs run` to this point; halt.
- Dump GPIOF MODER at `0x5802_1400` — pins 0–5 and 11–15 should
  read `0b10` (alternate function) in their bit pairs.
- Dump GPIOF AFRL at `0x5802_1420` — pins 0–5 should each have
  nibble value `0xC` (AF12).
- Dump GPIOF OSPEEDR at `0x5802_1408` — pins 0–5 should read
  `0b11` (VeryHigh).

If any FMC pin stays at `0b00` MODER, SDRAM access at
`0xD000_0000` bus-faults.

## Going deeper

- RM0399 §14 "General-purpose I/Os (GPIO)" — the register
  layout and all AF mappings.
- DS12930 (STM32H747XI datasheet) Table 9 — per-pin alternate
  function tables. Cross-reference when adding new peripherals.
- `stm32h7xx-hal`'s
  [`gpio`](https://docs.rs/stm32h7xx-hal/latest/stm32h7xx_hal/gpio/index.html)
  module — the typed-pin API the macro wraps.

---

**[← Prev](03-sdram-and-fmc.md) · [Index](README.md) · [Next →](05-ltdc-dsi-and-axi-holdoff.md)**
