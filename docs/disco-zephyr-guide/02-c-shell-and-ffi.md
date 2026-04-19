<!--
02-c-shell-and-ffi.md - Volume V Chapter 2: The C main, SYS_INIT hooks,
input callbacks, ISR registration, and the Rust FFI entry.
-->

**[<- Prev](01-build-and-link.md) . [Index](README.md) . [Next ->](03-display-modes.md)**

# Chapter 2 — C Shell & FFI Boundary

## Volume II reference

Vol II's `main.rs` owned everything — startup, hardware init, the
cooperative loop. Under Zephyr, C owns the boot and kernel. Rust
enters via a single `extern "C"` entry point.

## What this chapter covers

The ~440-line C shell in `main.c`: the `SYS_INIT` FT5336 reset
hook, the `INPUT_CALLBACK_DEFINE` touch/joystick dispatcher, ISR
registration via `irq_connect_dynamic`, and the `rlvgl_init()`
call that hands off to Rust.

## The Zephyr delta

FreeRTOS has no C shell — everything is in Rust. Zephyr requires
C for `SYS_INIT` hooks, Zephyr input API callbacks, and kernel
API calls that don't have Rust bindings.

## Walkthrough

### 1. SYS_INIT: FT5336 early reset

The FT5336 shares the PG3 reset line with the NT35510 panel. In
adapted command mode, Zephyr's NT35510 driver is disabled, so PG3
never pulses. The FT5336 needs a reset before its I2C driver probes.

```c
static int ft5336_early_reset(void) {
    gpio_pin_configure(gpiog, 3, GPIO_OUTPUT_ACTIVE);
    gpio_pin_set(gpiog, 3, 0);  // low
    k_busy_wait(10000);          // 10 ms
    gpio_pin_set(gpiog, 3, 1);  // high
    k_busy_wait(300000);         // 300 ms settle
    return 0;
}
SYS_INIT(ft5336_early_reset, POST_KERNEL, 45);
```

Priority 45 runs before the FT5336 driver (priority 50). The
300 ms settle gives the chip firmware time to boot.

After reset, C forces CTRL=0x00 ("keep active" mode):
```c
i2c_reg_write_byte(i2c_dev, 0x38, 0x86, 0x00);
```

### 2. Input callback

```c
INPUT_CALLBACK_DEFINE(NULL, input_cb, NULL);
```

Global sink — receives all input events. Accumulates touch
`ABS_X`/`ABS_Y`/`BTN_TOUCH` across samples. On `evt->sync`
(frame boundary), calls Rust:

```c
rlvgl_touch_event(
    (uint32_t)accum_x | ((uint32_t)accum_y << 16),
    accum_pressed
);
```

Joystick keys forwarded immediately:
```c
rlvgl_key_event(evt->code, evt->value);
```

### 3. ISR registration

DMA2D and DSI interrupts are registered dynamically:
```c
irq_connect_dynamic(DMA2D_IRQn, 3, rlvgl_dma2d_isr, NULL, 0);
irq_enable(DMA2D_IRQn);
irq_connect_dynamic(DSI_IRQn, 2, rlvgl_dsi_isr, NULL, 0);
irq_enable(DSI_IRQn);
```

The ISR bodies are in Rust (`zephyr_entry.rs`), declared
`extern "C"`.

### 4. rlvgl_init entry point

C main builds a `RlvglDisplayInfo` struct with framebuffer
addresses, dimensions, and semaphore pointers, then calls:

```c
rlvgl_init(&display_info);
```

This enters Rust and **never returns to C main** during normal
operation. C main falls through to a heartbeat `k_sleep` loop.

### 5. Filesystem FFI

```c
int rlvgl_readdir(const char *path, readdir_cb cb, void *ctx);
```

Wraps Zephyr `fs_readdir()`. Rust calls this from the
`ZephyrStorageBrowser` trait implementation. Returns file entries
for the SD card file browser.

## Verify

Serial output from C shell:
```
RLVGL main() begin
FT5336: chip_id=0x64 (ok)
FT5336: REG_CTRL=0x00 (set keep-active)
```

Then Rust takes over with splash decode and widget init.

## Going deeper

- Zephyr [Input Subsystem](https://docs.zephyrproject.org/latest/services/input/index.html)
  — `INPUT_CALLBACK_DEFINE`, synchronous mode.
- Zephyr [SYS_INIT](https://docs.zephyrproject.org/latest/kernel/services/other/init.html)
  — boot-time hook priorities.
- `zephyr/src/main.c` — the complete C shell source.

---

**[<- Prev](01-build-and-link.md) . [Index](README.md) . [Next ->](03-display-modes.md)**
