<!--
06-touch-input.md - Volume II Chapter 6: I2C4 raw touch ISR + TIM6 + ring buffer.
-->

**[← Prev](05-ltdc-dsi-and-axi-holdoff.md) · [Index](README.md) · [Next →](07-dma2d-engine.md)**

# Chapter 6 — Touch Input

## Volume I reference

Vol I
[Chapter 5](../disco-tutorial/05-menu-stubs.md) introduced the
touch ISR as "copy these pieces unchanged from the real crate":
the I2C4 init, the TIM6 setup, and the ring buffer. This
chapter walks through each of those pieces line-by-line.

## What this chapter covers

Three things that together turn FT5336 capacitive-touch events
into `rlvgl_core::event::Event::PointerDown/Move/Up` values:

1. A TIM6 interrupt firing at 120 Hz as the sample clock.
2. A raw-I2C4 read routine that pulls 31 bytes from the FT5336
   in ~40 µs, bypassing the HAL entirely.
3. A single-producer / single-consumer **ring buffer** between
   the ISR and the main loop.

## The HAL / PAC gap

`embedded-hal`'s blocking `i2c::I2c` trait — and `stm32h7xx-hal`'s
implementation of it — reads bytes one at a time and busy-waits
on each one. A 31-byte FT5336 read takes >300 µs that way, and
the ISR missed press/release edges. The crate pokes I2C4
registers directly from the ISR, implementing a tight state
machine over CR2 / TXDR / RXDR / ISR / ICR.

## Walkthrough

Everything in this chapter lives inside the `touch_isr` module
at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L86–281.

### 1. I2C4 and GPIOK register addresses

Peripheral addresses captured once as constants at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L101–115:

```rust
// I2C4 register addresses (base 0x5800_1C00, RM0399 §50.7)
const I2C4_CR2:  *mut   u32 = 0x5800_1C04 as *mut   u32;
const I2C4_ISR:  *const u32 = 0x5800_1C18 as *const u32;
const I2C4_ICR:  *mut   u32 = 0x5800_1C1C as *mut   u32;
const I2C4_RXDR: *const u32 = 0x5800_1C24 as *const u32;
const I2C4_TXDR: *mut   u32 = 0x5800_1C28 as *mut   u32;

// GPIOK IDR for PK7 touch INT pin (active-low)
const GPIOK_IDR: *const u32 = 0x5802_2810 as *const u32;

// TIM6 SR (status register, clear UIF on entry)
const TIM6_SR:   *mut   u32 = 0x4000_1010 as *mut   u32;

// FT5336 7-bit address, shifted left into SADD[7:1]
const FT5336_SADD: u32 = 0x38 << 1; // 0x70
```

I2C4 itself still needs its GPIO pins (SCL/SDA) muxed and the
peripheral clocked + enabled. That setup happens earlier,
through the platform crate's WM8994 codec init (Chapter 8) —
I2C4 is shared with the audio codec. By the time touch sampling
starts, I2C4 is already live.

### 2. SPSC ring buffer

A lock-free single-writer (ISR) / single-reader (main loop)
ring buffer, implemented with volatile head/tail pointers and
compiler fences. In
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L134–182:

```rust
pub const TOUCH_RING_CAP: usize = 16;

pub struct TouchRing {
    pub head:  u32,
    pub tail:  u32,
    pub slots: [RawTouchSample; TOUCH_RING_CAP],
}

pub static mut TOUCH_RING: TouchRing = TouchRing {
    head:  0,
    tail:  0,
    slots: [RawTouchSample::EMPTY; TOUCH_RING_CAP],
};

pub unsafe fn touch_ring_push(sample: RawTouchSample) {
    unsafe {
        let ring = addr_of_mut!(TOUCH_RING);
        let head = core::ptr::read_volatile(addr_of!((*ring).head));
        let tail = core::ptr::read_volatile(addr_of!((*ring).tail));
        if head.wrapping_sub(tail) >= TOUCH_RING_CAP as u32 {
            return; // full — drop newest
        }
        (*ring).slots[(head % TOUCH_RING_CAP as u32) as usize] = sample;
        compiler_fence(Ordering::Release);
        core::ptr::write_volatile(addr_of_mut!((*ring).head), head.wrapping_add(1));
    }
}
```

Key properties:

- **Drop-newest on full** (rather than blocking or wrapping)
  so the ISR never stalls.
- **One release fence** on push, **one acquire fence** on pop —
  makes this safe across cores/interrupts without requiring a
  critical section.
- **`wrapping_sub`** on the head/tail subtraction means this
  works correctly when the counters wrap at `u32::MAX`.

### 3. Raw I2C4 read

The full 31-byte FT5336 read at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L207–257:

```rust
unsafe fn i2c4_read_touches_raw() -> RawTouchSample {
    unsafe {
        // Clear stale status flags from any prior aborted transaction.
        I2C4_ICR.write_volatile((1 << 5) | (1 << 4) | (1 << 8) | (1 << 9) | (1 << 10));

        // ── Write phase: send register address 0x02 ──
        // CR2: SADD, NBYTES=1, RD_WRN=0, START=1, AUTOEND=0
        I2C4_CR2.write_volatile(FT5336_SADD | (1 << 16) | (1 << 13));
        if !i2c4_wait(1) { return RawTouchSample::EMPTY; }  // TXIS
        I2C4_TXDR.write_volatile(0x02);
        if !i2c4_wait(6) { return RawTouchSample::EMPTY; }  // TC

        // ── Read phase: read 31 bytes ──
        // CR2: SADD, NBYTES=31, RD_WRN=1, START=1, AUTOEND=1
        I2C4_CR2.write_volatile(FT5336_SADD | (1 << 10) | (31 << 16) | (1 << 13) | (1 << 25));
        let mut buf = [0u8; 31];
        for b in buf.iter_mut() {
            if !i2c4_wait(2) { return RawTouchSample::EMPTY; }  // RXNE
            *b = (I2C4_RXDR.read_volatile() & 0xFF) as u8;
        }
        if i2c4_wait(5) { I2C4_ICR.write_volatile(1 << 5); }    // STOPF / STOPCF
        // ...parse into RawTouchSample...
    }
}
```

Four things to notice:

- **Pre-clearing the status flags** at the top is mandatory.
  A previous transaction that timed out leaves `STOPF` set,
  and the next `START=1` write hangs.
- **`i2c4_wait(bit)`** is a bounded busy-loop (50 000
  iterations ≈ 125 µs at 400 MHz). On timeout it returns
  `false` and the read gives back `EMPTY` — the ISR doesn't
  get stuck.
- **`AUTOEND=1`** in the read phase makes the controller
  generate the STOP condition after the 31st byte.
- **`NACKF` handling** inside `i2c4_wait` (L190–193): if the
  FT5336 NACKs (device unplugged, glitch, etc.), the wait
  returns `false` instead of hanging.

### 4. TIM6 at 120 Hz

Timer init at
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L2125–2157:

```rust
unsafe {
    // Enable TIM6 clock (RCC APB1LENR bit 4)
    let apb1lenr = 0x5802_44E8u32 as *mut u32;
    apb1lenr.write_volatile(apb1lenr.read_volatile() | (1 << 4));
    let _ = (apb1lenr as *const u32).read_volatile(); // readback fence

    let tim6 = 0x4000_1000u32;
    // Timer clock = 2 × APB1 = 200 MHz
    // 200 MHz / (199+1) / (8332+1) = 120.0 Hz
    (tim6 as *mut u32).write_volatile(0);                   // CR1 stop
    ((tim6 + 0x0C) as *mut u32).write_volatile(1);          // DIER: UIE
    ((tim6 + 0x14) as *mut u32).write_volatile(1);          // EGR: UG
    ((tim6 + 0x28) as *mut u32).write_volatile(199);        // PSC
    ((tim6 + 0x2C) as *mut u32).write_volatile(8332);       // ARR
    ((tim6 + 0x10) as *mut u32).write_volatile(0);          // SR clear
    (tim6 as *mut u32).write_volatile(1);                   // CR1: CEN

    cortex_m::peripheral::NVIC::unmask(Interrupt::TIM6_DAC);
    cp.NVIC.set_priority(Interrupt::TIM6_DAC, 2);
}
```

Two choices worth calling out:

- **120 Hz**, not 60 Hz. The panel refreshes at 60 Hz but
  sampling at the same rate aliases press/release events
  against the frame. 120 Hz catches every transition.
- **NVIC priority 2**, below SysTick. The touch ISR is
  allowed to pre-empt the main loop but not SysTick.

### 5. The ISR itself

At
[`main.rs`](../../examples/stm32h747i-disco/src/main.rs) L261–280:

```rust
pub unsafe fn tim6_dac_handler() {
    unsafe {
        TIM6_SR.write_volatile(TIM6_SR.read_volatile() & !1);   // clear UIF

        // Read PK7: low = touch data available
        let int_low = GPIOK_IDR.read_volatile() & (1 << 7) == 0;

        // Read when INT active OR on the LOW→HIGH edge (catches release)
        let prev = core::ptr::read_volatile(addr_of!(PREV_INT_LOW));
        let should_read = int_low || prev;

        if should_read {
            let sample = i2c4_read_touches_raw();
            touch_ring_push(sample);
        }

        core::ptr::write_volatile(addr_of_mut!(PREV_INT_LOW), int_low);
    }
}
```

The `int_low || prev` condition is the release trick: when PK7
goes from low back to high, the ISR reads *once* more so the
"all-touches-up" frame makes it through to the main loop.

### 6. Draining the ring

The main loop drains by calling `touch_ring_pop()` each tick
and dispatches the samples into the rlvgl event tree. That
code lives further down in `main.rs` and is the Vol I layer —
no new register access.

## Register diagram — the ISR's surfaces

```
I2C4 @ 0x5800_1C00  (RM0399 §50.7)
│
├── +0x04 CR2  : SADD | NBYTES | RD_WRN | START | STOP | AUTOEND
├── +0x18 ISR  : BUSY | TXIS | RXNE | TC | STOPF | NACKF | BERR | ARLO | OVR
├── +0x1C ICR  : write-1-to-clear for each ISR flag
├── +0x24 RXDR : byte just received
└── +0x28 TXDR : byte to transmit

TIM6 @ 0x4000_1000  (RM0399 §40)
│
├── +0x00 CR1  : bit 0 = CEN (counter enable)
├── +0x0C DIER : bit 0 = UIE (update interrupt enable)
├── +0x10 SR   : bit 0 = UIF (update interrupt flag)
├── +0x14 EGR  : bit 0 = UG  (force update)
├── +0x28 PSC  : prescaler
└── +0x2C ARR  : auto-reload
```

## Verify

- `rlvgl-playit ?` reports `serial_queue=0, drops=0` with no
  finger on the panel.
- Tap the panel; it reports a non-zero tick/present counter
  with `T<x>,<y>` events echoing back.
- Read `TOUCH_RING.head` and `TOUCH_RING.tail` via probe-rs
  while idle — both grow when touch activity occurs but the
  difference stays < 16.

Fault modes:

- `serial_queue` drops climb fast → I2C4 pins not muxed or
  WM8994 init (which sets up I2C4) never ran. Check Chapter 8.
- First tap works, second tap hangs → the pre-clear of ICR at
  the top of `i2c4_read_touches_raw()` was skipped.

## Going deeper

- RM0399 §50 "I2C" — the ISR flag table and the state machine
  diagrams for the CR2 phases.
- FT5336 datasheet — register 0x02 returns one byte of touch
  count plus six bytes per touch point × 5 points = 31 bytes.
- [`playit/README.md`](../../playit/README.md) — the `T<x>,<y>`
  command is the scripted-tap equivalent of the ISR path and
  pushes into the same ring.
- [`examples/stm32h747i-disco/README.md`](../../examples/stm32h747i-disco/README.md)
  — project-level notes including touch-specific quirks.

---

**[← Prev](05-ltdc-dsi-and-axi-holdoff.md) · [Index](README.md) · [Next →](07-dma2d-engine.md)**
