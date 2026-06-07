<!--
04-touch-and-input.md - Volume V Chapter 4: FT5336 early reset,
CTRL fix, atomic buffers, coordinate transform.
-->

**[<- Prev](03-display-modes.md) . [Index](README.md) . [Next ->](05-render-loop.md)**

# Chapter 4 — Touch & Input Pipeline

## Volume II reference

Vol II [Chapter 6](../disco-platform-guide/06-touch-input.md)
implemented touch as a TIM6 ISR polling I2C4 registers directly.
Under Zephyr, the FT5336 has a proper I2C driver and input
subsystem integration.

## What this chapter covers

The FT5336 early reset hook that makes adapted command mode work,
the CTRL register fix, Zephyr's `INPUT_MODE_SYNCHRONOUS`, and
the atomic touch/key buffer pattern between C callbacks and the
Rust render loop.

## The Zephyr delta

Bare-metal and FreeRTOS poll I2C4 directly. Zephyr has a
`ft5336` input driver that handles I2C communication, reports
events through the input subsystem, and calls registered
callbacks. The challenge: coordinating C-side callbacks with
the Rust render loop without a shared mutex.

## Walkthrough

### 1. SYS_INIT early reset

In adapted command mode, Zephyr's NT35510 driver is disabled.
Since PG3 is shared between NT35510 and FT5336, the FT5336 gets
no reset pulse. A `SYS_INIT` hook at priority 45 (before the
FT5336 driver at 50) pulses PG3 with 300 ms settle:

```c
SYS_INIT(ft5336_early_reset, POST_KERNEL, 45);
```

Without this, the FT5336 enters the "ACK but all-zero registers"
state — I2C responds but TD_STATUS is always 0.

### 2. CTRL = 0x00 (keep-active mode)

After reset, C writes CTRL:
```c
i2c_reg_write_byte(i2c_dev, 0x38, 0x86, 0x00);
```

The FT5336 defaults to monitor mode (CTRL=0x01) which waits for
INT pin activity. Zephyr's polling driver can't wake it via INT,
so CTRL must be forced to keep-active (0x00).

**Do NOT write G_MODE=0x00** — on this board variant, polling
mode kills touch detection entirely.

### 3. INPUT_MODE_SYNCHRONOUS

```
CONFIG_INPUT_MODE_SYNCHRONOUS=y
```

Without this, the input subsystem queues events in a message
buffer. When the render loop sleeps for 33 ms, the queue fills
and events are dropped. Synchronous mode invokes the callback
inline from the driver thread — the atomic buffer pattern handles
the race.

### 4. Atomic touch buffer

C callback stores touch state atomically:
```c
void rlvgl_touch_event(uint32_t packed_xy, bool pressed);
```

Rust side:
```rust
static TOUCH_XY: AtomicU32 = AtomicU32::new(0);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);
static TOUCH_DIRTY: AtomicBool = AtomicBool::new(false);
```

The render loop calls `take_touch()` which reads + clears the
dirty flag. Edge detection fires only on rising edge (finger
newly down), not on held.

### 5. Key buffer (joystick)

A 4-entry lock-free ring for joystick codes:
```rust
static KEY_BUF: [AtomicU32; 4] = [...];
static KEY_WRITE: AtomicU32 = AtomicU32::new(0);
static KEY_READ: AtomicU32 = AtomicU32::new(0);
```

`rlvgl_key_event(code, pressed)` from C packs the code +
pressed + valid bits into a single `AtomicU32` entry.

### 6. Coordinate transform

Zephyr's FT5336 driver reports landscape coordinates (after the
driver's own rotation). Rust inverts Y:

```rust
let landscape_x = raw_x;
let landscape_y = 479 - raw_y;
```

Compare with FreeRTOS (Vol IV Ch 4) where raw portrait
coordinates need the full portrait-to-landscape transform.

## Verify

Touch the screen — serial should show coordinates in the Rust
render loop's touch processing (if diagnostic enabled). The `?`
serial command should show touch events being processed.

## Going deeper

- Zephyr [FT5336 driver](https://docs.zephyrproject.org/latest/build/dts/api/bindings/input/focaltech,ft5336.html)
- Vol IV [Ch 3](../disco-freertos-guide/03-touch-task.md) — the
  FreeRTOS interrupt-driven approach for comparison.
- `zephyr_entry.rs` L25-96 — the Rust touch/key FFI declarations.

---

**[<- Prev](03-display-modes.md) . [Index](README.md) . [Next ->](05-render-loop.md)**
