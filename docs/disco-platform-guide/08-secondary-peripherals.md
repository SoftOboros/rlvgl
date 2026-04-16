<!--
08-secondary-peripherals.md - Volume II Chapter 8: QSPI, USART1, audio, backlight.
-->

**[← Prev](07-dma2d-engine.md) · [Index](README.md) · [Next →](09-star-crawl-part-1.md)**

# Chapter 8 — Secondary Peripherals

## Volume I reference

Vol I
[Chapter 6](../disco-tutorial/06-hook-actions.md) turned on the
`audio` feature for the Settings → Audio and Info → AudioScope
slots, and had the `backlight_pwm` feature cycle panel brightness.
Vol I did not describe what `audio` and `backlight_pwm` actually
do at the register level. This chapter does.

## What this chapter covers

Four small, independent bring-up sequences that don't deserve a
chapter each but still need register-level explanation:

1. **QSPI flash** — MT25TL01G on Bank 1, with the errata 2.8.5
   kernel-clock override.
2. **USART1** — bare-metal console on PA9 / PA10, set up without
   any HAL because it's used for breadcrumbs before the HAL
   logger exists.
3. **Audio** — SAI1 I2S TX + SAI4 PDM mic, with the WM8994
   codec over I2C4.
4. **Backlight PWM** — TIM8_CH2 on PJ6 at 10 kHz, with an
   `embedded-hal` 1.0 adapter over the HAL's 0.2 `PwmPin` trait.

## The HAL / PAC gap

One well-documented gap (QSPI errata 2.8.5, see
[Ch 1 §4](01-why-bare-metal.md#4-qspi-errata-285--wrong-default-kernel-clock)),
plus two scope issues:

- `stm32h7xx-hal`'s USART driver works fine but has to live as
  a `static` or get threaded through every call site that
  wants to print. Bypassing it and poking USART1 directly keeps
  breadcrumbs ergonomic during bring-up.
- There is no WM8994 driver in `stm32h7xx-hal` — codec support
  lives in `rlvgl-platform`
  ([`platform/src/wm8994.rs`](../../platform/src/wm8994.rs))
  and on I2C4 shared with touch.

## Walkthrough

### 1. QSPI flash (MT25TL01G, Bank 1)

At
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1711–1770:

```rust
#[cfg(feature = "qspi_flash")]
let qspi_flash = {
    use rlvgl_platform::Mt25tlFlash;
    use stm32h7xx_hal::xspi;

    // Errata 2.8.5: Select PLL2R (150 MHz) as QSPI kernel clock
    // D1CCIPR QSPISEL bits [5:4]: 00=HCLK, 01=PLL1Q, 10=PLL2R, 11=PER
    unsafe {
        let d1ccipr = 0x5802_4C18u32 as *mut u32;
        let val = d1ccipr.read_volatile();
        d1ccipr.write_volatile((val & !(0b11 << 4)) | (0b10 << 4));
    }

    // QSPI Bank 1 GPIO pins (AF numbers verified against DS12930 Table 9)
    let qspi_clk = gpiob.pb2 .into_alternate::<9> ().speed(Speed::VeryHigh);
    let qspi_io0 = gpiod.pd11.into_alternate::<9> ().speed(Speed::VeryHigh);
    let qspi_io1 = gpiof.pf9 .into_alternate::<10>().speed(Speed::VeryHigh);
    let qspi_io2 = gpiof.pf7 .into_alternate::<9> ().speed(Speed::VeryHigh);
    let qspi_io3 = gpiof.pf6 .into_alternate::<9> ().speed(Speed::VeryHigh);

    let qspi = QUADSPI.bank1(
        (qspi_clk, qspi_io0, qspi_io1, qspi_io2, qspi_io3),
        xspi::Config::new(50.MHz()).fifo_threshold(4),
        &ccdr.clocks,
        ccdr.peripheral.QSPI,
    );

    let mut flash = Mt25tlFlash::new(qspi);
    match flash.read_id() {
        Ok(id) => unsafe {
            let bc = 0x3800_0320u32 as *mut u32;
            bc.write_volatile(0x0F00_0000 | (id[0] as u32) << 16
                                           | (id[1] as u32) << 8
                                           | id[2] as u32);
        },
        Err(_) => unsafe {
            (0x3800_0320u32 as *mut u32).write_volatile(0xDEAD_DEAD);
        },
    }
    flash
};
```

The JEDEC ID read is a smoke test — if the breadcrumb at
`0x3800_0320` is `0x0F??????` after boot, QSPI is alive. If
it's `0xDEAD_DEAD`, something upstream (pin mux, clock) is
wrong.

Bank 1 has five data/clock pins plus NCS on PG6 (managed by
the HAL internally). Note the **mix of AF9 and AF10** on the
four IO lines — this is not a typo; see DS12930 Table 9.

### 2. USART1 — raw register init for breadcrumbs

At
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1823–1849:

```rust
// ── USART1 VCP init (PA9=TX AF7, 115200 8N1) ──────────────────────
// Addresses from C HAL path (RCC C1 domain registers at 0x5802_44xx)
unsafe {
    // Enable GPIOA clock (AHB4ENR at RCC+0xE0)
    let ahb4 = 0x5802_44E0u32 as *mut u32;
    ahb4.write_volatile(ahb4.read_volatile() | (1 << 0));
    let _ = (ahb4 as *const u32).read_volatile();

    // PA9 = AF7 (TX), PA10 = AF7 (RX): AFRH bits [7:4]=7, [11:8]=7
    let gpioa = 0x5802_0000u32;
    let afrh = (gpioa + 0x24) as *mut u32;
    afrh.write_volatile(
        (afrh.read_volatile() & !(0xFFu32 << 4)) | (7u32 << 4) | (7u32 << 8));

    // MODER: PA9 = AF (10), PA10 = AF (10)
    let moder = gpioa as *mut u32;
    moder.write_volatile((moder.read_volatile() & !(0xF << 18)) | (0b1010 << 18));

    // Enable USART1 clock (C1_APB2ENR bit 4)
    let apb2 = 0x5802_44F0u32 as *mut u32;
    apb2.write_volatile(apb2.read_volatile() | (1 << 4));
    let _ = (apb2 as *const u32).read_volatile();

    // USART1 config: BRR=868 (100 MHz / 115200), TE+RE+UE+FIFOEN
    let usart1 = 0x4001_1000u32;
    ((usart1 + 0x0C) as *mut u32).write_volatile(868);              // BRR
    ((usart1 + 0x00) as *mut u32).write_volatile(
        (1 << 29) | (1 << 3) | (1 << 2) | (1 << 0));                // FIFOEN | TE | RE | UE
}
```

Once this runs, `serial_puts(…)` (the crate's one-byte-at-a-
time blocking putter) works. The interrupt-driven FIFO-backed
path in the `runtime_serial` module (L440+) upgrades this
later, but the bare setup shown here is enough for early
breadcrumbs.

### 3. Audio — SAI1 TX + SAI4 PDM mic + WM8994

At
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L2057–2119:

```rust
// ── Audio codec init (before touch claims I2C4) ──
#[cfg(feature = "audio")]
let sai = {
    let sai = Sai1Audio::new();
    sai.enable_clock(1);            // 1 = PLL2_P
    sai
};
#[cfg(feature = "audio")]
let i2c4 = {
    // SAI1 GPIO pins (AF6, VeryHigh speed)
    let _sai1_mclk = gpiog.pg7.into_alternate::<6>().speed(Speed::VeryHigh);
    let _sai1_sck  = gpioe.pe5.into_alternate::<6>().speed(Speed::VeryHigh);
    let _sai1_fs   = gpioe.pe4.into_alternate::<6>().speed(Speed::VeryHigh);
    let _sai1_sd_a = gpioe.pe6.into_alternate::<6>().speed(Speed::VeryHigh);
    let _sai1_sd_b = gpioe.pe3.into_alternate::<6>().speed(Speed::VeryHigh);

    // Configure SAI1 sub-block A as I2S master TX
    // MCKDIV=0 means /1; the WM8994 FLL handles exact audio frequency
    sai.configure_tx(0);

    // Init WM8994 codec over I2C4 (temporary ownership, then release)
    let codec_i2c = HalI2c(i2c4);
    let mut codec = Wm8994::new(codec_i2c);
    let _ = codec.init_playback(
        48_000,
        150_000_000,                 // approximate MCLK from PLL2_P
        rlvgl_platform::wm8994::OutputDevice::Headphone,
    );
    sai.enable_tx();                // codec now receiving I2S frames

    // SAI4 PDM mic GPIO (PE2=CK1, PC1=D1)
    let _sai4_ck1 = gpioe.pe2.into_alternate::<10>().speed(Speed::VeryHigh);
    let _sai4_d1  = gpioc.pc1.into_alternate::<10>();

    codec.release().0               // release I2C4 back to touch ISR
};
```

Three ordering rules the comments enforce:

- **Codec init before touch takes over I2C4.** The WM8994 needs
  a full HAL-managed I2C transaction sequence to get to a
  playable state; only after it releases I2C4 does the touch
  ISR start poking raw registers.
- **SAI clocks before codec init.** The WM8994's FLL locks to
  the MCLK the MCU puts out — it has to be there before the
  FLL config command is sent.
- **SAI TX enable *after* codec init.** The DAC needs its
  routing set up before receiving I2S frames, or the first
  few ms of audio is noise.

The DSP path (PCM playback, audio-scope visualisation) is in
[`platform/src/audio_player.rs`](../../platform/src/audio_player.rs)
and [`platform/src/sai.rs`](../../platform/src/sai.rs) —
out of scope here.

### 4. Backlight — PWM or GPIO

At
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L1777–1810:

```rust
#[cfg(feature = "backlight_pwm")]
let backlight = {
    use stm32h7xx_hal::hal::PwmPin as HalPwmPin02;

    let pj6_ch2 = gpioj.pj6.into_alternate::<3>();
    let ch = TIM8.pwm(pj6_ch2, 10.kHz(), ccdr.peripheral.TIM8, &ccdr.clocks);

    // Adapter from HAL 0.2 PwmPin to embedded-hal 1.0 SetDutyCycle
    struct TimBacklight<T: HalPwmPin02<Duty = u16>>(T);
    impl<T: HalPwmPin02<Duty = u16>> SetDutyCycle for TimBacklight<T> { /* ... */ }
    TimBacklight(ch)
};
#[cfg(not(feature = "backlight_pwm"))]
let backlight = {
    let bl_pin = gpioj.pj6.into_push_pull_output();
    HalGpioBacklight(bl_pin)
};
```

The small adapter is a live example of the **embedded-hal 0.2
vs 1.0 bridge** — `stm32h7xx-hal` still exposes its PWM
channel through `embedded_hal::PwmPin` (the 0.2 trait); rlvgl
consumes `embedded_hal::pwm::SetDutyCycle` (the 1.0 trait).
The `TimBacklight` newtype wraps one into the other.

## Register diagram — the addresses this chapter touches

```
RCC D1CCIPR  @ 0x5802_4C18 bits [5:4] QSPISEL (errata 2.8.5)
RCC AHB4ENR  @ 0x5802_44E0 bit 0      GPIOA EN
RCC APB2ENR  @ 0x5802_44F0 bit 4      USART1 EN
USART1       @ 0x4001_1000 +0x00 CR1 (FIFOEN|TE|RE|UE),  +0x0C BRR
SAI1 / SAI4  @ 0x4001_5800 / 0x5800_5400 (RM0399 §51)
TIM8         @ 0x4001_0400 (RM0399 §44)
I2C4         @ 0x5800_1C00 (shared with touch — Chapter 6)
Breadcrumb   @ 0x3800_0320 (QSPI JEDEC ID or 0xDEAD_DEAD)
```

## Verify

- Open the ST-LINK VCP; on boot the firmware prints the
  "POST-AUDIO" banner (L2113–2117).
- Read `0x3800_0320` under probe-rs. `0x0F 20 BA 19` (or
  similar) confirms QSPI is up; `0xDEAD_DEAD` means QSPI
  errored (usually a pin or D1CCIPR miss).
- With Vol I Chapter 6 flashed, tapping **Settings → Backlight**
  cycles panel brightness in four steps.
- Tapping **Info → AudioScope** starts producing I2S frames
  (verifiable on headphones if the panel has a 3.5 mm jack
  populated).

## Going deeper

- ES0392 §2.8.5 — QSPI kernel clock errata.
- RM0399 §49 "QUADSPI", §51 "SAI", §48 "USART".
- [`platform/src/wm8994.rs`](../../platform/src/wm8994.rs)
  — codec register writes and FLL lock sequence.
- [`platform/src/qspi_flash.rs`](../../platform/src/qspi_flash.rs)
  — the MT25TL01G driver that reads the JEDEC ID.
- [`examples/stm32h747i-disco/OPTIONS.md`](../../examples/stm32h747i-disco/OPTIONS.md)
  — feature flags for all four subsystems and what they
  require.

---

**[← Prev](07-dma2d-engine.md) · [Index](README.md) · [Next →](09-star-crawl-part-1.md)**
