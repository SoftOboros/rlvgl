<!--
03-touch-task.md - Volume IV Chapter 3: Interrupt-driven I2C4 touch
with FreeRTOS semaphore.
-->

**[<- Prev](02-present-task.md) . [Index](README.md) . [Next ->](04-render-task.md)**

# Chapter 3 — Touch Task: Interrupt-Driven I2C4

## Volume II reference

Vol II [Chapter 6](../disco-platform-guide/06-touch-input.md)
implemented touch as a TIM6 ISR that polls FT5336 via blocking
I2C4 register reads. The tight polling loop completes a 31-byte
read in ~40 us — fast enough for ISR context in bare-metal.

## What this chapter covers

Why that polling approach breaks under FreeRTOS, and the
interrupt-driven replacement that uses the I2C4_EV ISR + a
FreeRTOS binary semaphore.

## The FreeRTOS delta

FreeRTOS preempts tasks at any point — including in the middle of
a tight I2C4 register-polling loop. When the scheduler context-
switches away during a multi-byte I2C read, the FT5336 sees an
incomplete transaction. The chip's I2C slave state machine gets
stuck, and all subsequent reads return zeros.

The fix: move the I2C4 transaction into the I2C4_EV ISR, which
runs at NVIC priority 7 (above the FreeRTOS syscall ceiling) and
cannot be preempted by the scheduler.

## Walkthrough

### 1. ISR state machine

The `I2c4Phase` enum drives a two-phase read (write register
address, then read N bytes) or a single-phase write (2 bytes
with AUTOEND):

```rust
enum I2c4Phase {
    Idle,       // No active transaction
    WaitTxis,   // START sent, waiting for TXIS
    WaitTC,     // Register addr written, waiting for TC
    Reading,    // Repeated START read, collecting RXNE bytes
    Writing,    // AUTOEND write, sending value byte
}
```

### 2. Starting a read

```rust
pub unsafe fn i2c4_irq_start(reg: u8, len: usize) {
    ISR_REG = reg;
    ISR_LEN = len;
    ISR_IDX = 0;
    // Clear stale flags, enable TXIE/RXIE/NACKIE/STOPIE/TCIE
    // Write phase: 1 byte (register addr), no AUTOEND -> TC
    ISR_PHASE = I2c4Phase::WaitTxis;
    I2C4_CR2.write_volatile(FT5336_SADD | (1 << 16) | (1 << 13));
}
```

The task calls `i2c4_irq_start`, then blocks on the I2C4 done
semaphore via `i2c4_irq_wait`. The ISR advances the state machine:

- **TXIS**: write register address, transition to WaitTC
- **TC**: start read phase (repeated START + AUTOEND), transition
  to Reading
- **RXNE**: collect bytes into `ISR_BUF`
- **STOPF**: disable interrupts, give semaphore, go Idle

### 3. The `read_sample_irq` wrapper

```rust
pub unsafe fn read_sample_irq() -> RawTouchSample {
    i2c4_irq_start(0x02, 31);  // TD_STATUS + 5 touch points
    let Some(buf) = i2c4_irq_wait() else {
        return RawTouchSample::EMPTY;
    };
    // Parse count + (id, event_flag, x, y) per point
}
```

Drop-in replacement for the blocking `read_sample()`.

### 4. FT5336 CTRL stability

The FT5336 has two modes controlled by register 0x86 (CTRL):

- `0x00` — keep-active: continuously scans the sensor.
- `0x01` — monitor: enters low-power, wakes on INT activity.

`init_ctrl()` writes CTRL=0x00 at boot. However:

**G_MODE=0x00 (polling) kills touch** on this board variant.
Leave G_MODE at its default 0x01 (trigger). CTRL may auto-revert
to monitor mode after seconds without INT activity.

**Blocking `read_reg` kills the ISR path.** The blocking I2C4
polling path leaves stale state that prevents subsequent
interrupt-driven transactions from completing. Semaphore drain
and NVIC masking were tested but proved insufficient.

Current mitigation: CTRL=0x00 written at boot via blocking I2C4
(before scheduler starts). Periodic re-write is deferred pending
an interrupt-driven `i2c4_irq_write` implementation.

### 5. SPSC ring buffer

Touch events flow from the touch task to the render task via a
lock-free single-producer / single-consumer ring buffer:

```rust
const TOUCH_EVT_CAP: usize = 16;
static TOUCH_EVT_HEAD: AtomicU32 = AtomicU32::new(0);
static TOUCH_EVT_TAIL: AtomicU32 = AtomicU32::new(0);
```

Release ordering on store, Acquire on load. The touch task is the
sole producer; the render task is the sole consumer.

## Verify

The `F` serial command pauses the touch task and does a blocking
I2C4 probe:

```
FT: cnt=01 id=64 ct=00 gm=01 th=1C I=L
```

- `cnt=01` with finger on screen = touch working
- `ct=00` = CTRL keep-active
- `gm=01` = G_MODE trigger (correct — do NOT set to 0x00)

## Going deeper

- RM0399 Section 50 (I2C) — TXIS, TC, RXNE, STOPF flag semantics.
- Vol II [Chapter 6](../disco-platform-guide/06-touch-input.md)
  — the bare-metal blocking approach this replaces.
- `touch_i2c.rs` — the complete ISR state machine source.

---

**[<- Prev](02-present-task.md) . [Index](README.md) . [Next ->](04-render-task.md)**
