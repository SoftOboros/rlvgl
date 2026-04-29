<!--
01-freertos-scaffolding.md - Volume IV Chapter 1: Linking FreeRTOS,
exception routing, static allocation, scheduler start.
-->

**[<- Prev](README.md) . [Index](README.md) . [Next ->](02-present-task.md)**

# Chapter 1 — FreeRTOS Scaffolding

## Volume II reference

Vol II treated the entire application as a single cooperative loop
in `main()`. The SysTick drove the sample clock, DSI ERIF gated
presents, and every peripheral was polled from the main thread.
This chapter replaces that loop with the FreeRTOS preemptive
scheduler.

## What this chapter covers

1. Building and linking the FreeRTOS C archive (`libfreertos.a`)
   alongside the Rust binary.
2. Routing SVCall, PendSV, and SysTick exceptions to FreeRTOS
   without corrupting the naked handler stack frames.
3. Static allocation of tasks and semaphores (no `pvPortMalloc`).
4. The `start()` entry point that creates tasks and launches the
   scheduler.

## The FreeRTOS delta

Bare-metal owns the entire execution context — one stack, one
thread, no preemption. FreeRTOS adds:

- **Multiple stacks**: each task gets its own `StackType_t` array.
- **Priority-based preemption**: higher-priority tasks run first.
- **Semaphores**: ISRs signal tasks via `xSemaphoreGiveFromISR`.
- **Exception ownership**: FreeRTOS takes SVCall, PendSV, SysTick.

The challenge: cortex-m-rt expects to own the vector table.
FreeRTOS's SVCall/PendSV handlers are `__attribute__((naked))` —
they hand-roll the Cortex-M context save and must sit at the
vector slot unmodified. Any Rust wrapper would emit a prologue
that corrupts the exception frame.

## Walkthrough

### 1. Feature gate and Cargo setup

The `freertos` feature in `examples/stm32h747i-disco/Cargo.toml`
enables the FreeRTOS path and forwards to the platform crate:

```toml
[features]
freertos = ["rlvgl-platform/freertos"]
```

The FreeRTOS C archive is built by a `build.rs` script that
compiles the FreeRTOS kernel sources and the `ffi_shims.c`
bridge, then emits `cargo:rustc-link-lib=static=freertos`.

### 2. SVCall / PendSV trampolines (`ffi_shims.c`)

cortex-m-rt declares `SVCall` and `PendSV` as weak defaults.
Providing strong globals with the same names makes the linker
pick ours. But `__attribute__((alias))` requires the target in
the same translation unit — FreeRTOS's handlers are in
`port.c`, not here. So we use naked tail-branch trampolines:

```c
// ffi_shims.c
__attribute__((naked)) void SVCall(void) {
    __asm__ volatile(
        "ldr  r0, =0x38000600   \n"  // D3 SRAM breadcrumb
        "ldr  r1, [r0]          \n"
        "adds r1, #1            \n"
        "str  r1, [r0]          \n"
        "b    vPortSVCHandler   \n"
    );
}

__attribute__((naked)) void PendSV(void) {
    __asm__ volatile(
        "ldr  r0, =0x38000604   \n"
        "ldr  r1, [r0]          \n"
        "adds r1, #1            \n"
        "str  r1, [r0]          \n"
        "b    xPortPendSVHandler\n"
    );
}
```

The `r0`/`r1` clobber is safe: these registers are
hardware-saved on the exception stack frame. The `b` (branch)
does not touch SP/LR, so the hardware-stacked frame reaches the
real handler untouched.

### 3. SysTick gate

FreeRTOS's `xPortSysTickHandler` must not run before
`vTaskStartScheduler`. But the HAL's `rcc.freeze()` enables
SysTick as a side effect. The solution: an atomic gate.

```rust
// freertos_entry.rs
static SYSTICK_READY: AtomicBool = AtomicBool::new(false);

#[cortex_m_rt::exception]
fn SysTick() {
    if SYSTICK_READY.load(Ordering::Relaxed) {
        unsafe { xPortSysTickHandler() }
    }
}
```

`SYSTICK_READY` is set `true` in `start()` right before
`vTaskStartScheduler()`.

### 4. Static task and semaphore allocation

FreeRTOS can allocate TCBs and stacks from its heap
(`pvPortMalloc`), but static allocation avoids fragmentation
and keeps memory deterministic:

```rust
static mut PRESENT_TCB:   StaticTask = StaticTask::new();
static mut PRESENT_STACK: [StackType_t; 512] = [0; 512]; // 2 KB

static mut ERIF_SEM_BUF:  StaticSemaphore = StaticSemaphore::new();
```

Binary semaphores are created via `rlvgl_sem_create_binary_static`
(a C shim around `xSemaphoreCreateBinaryStatic`).

### 5. Task model

| Task     | Priority | Stack  | Blocks on                |
|----------|----------|--------|--------------------------|
| present  | 3 (high) | 2 KB   | `erif_sem` (DSI ERIF)    |
| render   | 1 (low)  | 8 KB   | `render_start_sem` (ERIF)|
| touch    | 2 (mid)  | 1 KB   | `vTaskDelay` (120 Hz)    |
| playit   | 2 (mid)  | 2 KB   | `vTaskDelay` (50 Hz)     |

Present runs at the highest priority — it must retrigger LTDC
without delay when the holdoff expires. Render is lowest — it
runs in the back-porch window while present is blocked on TIM7.

### 6. The `start()` entry point

```rust
pub unsafe fn start() -> ! {
    // 0. Stop TIM6 (bare-metal touch ISR)
    // 0b. FT5336 init: CTRL=0x00
    // 1. Create binary semaphores
    // 1b. Init TIM7 (present holdoff timer)
    // 2. Init sync object (FreeRtosFrameSync)
    // 3. Create tasks (xTaskCreateStatic)
    // 4. Enable DSI, DMA2D, TIM7, I2C4_EV IRQs at NVIC
    // 5. SYSTICK_READY = true; vTaskStartScheduler()
}
```

Called from `main()` after all bare-metal hardware init is
complete. Never returns.

## Verify

```bash
make flash-disco-freertos
```

Serial output should show:
```
FT5336: id=0x64 ctrl=0x01->0x00
RND:ctrl_new
RND:ctrl_ok
```

The splash screen should be visible. The `?` serial command
should report incrementing `tick` and `erif` counts.

## Going deeper

- FreeRTOS
  [Static Allocation](https://www.freertos.org/a00110.html#configSUPPORT_STATIC_ALLOCATION)
  — why static TCBs avoid heap fragmentation.
- cortex-m-rt
  [exception handling](https://docs.rs/cortex-m-rt/latest/cortex_m_rt/attr.exception.html)
  — how weak defaults enable override.
- `FreeRTOSConfig.h` at
  `examples/stm32h747i-disco/freertos/FreeRTOSConfig.h` —
  tick rate, priority ceiling, idle hook settings.

---

**[<- Prev](README.md) . [Index](README.md) . [Next ->](02-present-task.md)**
