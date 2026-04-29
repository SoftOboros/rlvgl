# FreeRTOS for BeagleBone Black

This directory holds the FreeRTOS kernel configuration and C glue layer
for the BBB bare-metal + FreeRTOS runtime.

## Setup

The FreeRTOS kernel source files are **not checked in**. You must obtain
them from the FreeRTOS-Kernel repository and place them here:

```bash
# Clone FreeRTOS-Kernel (or use a release zip)
git clone https://github.com/FreeRTOS/FreeRTOS-Kernel.git /tmp/freertos

# Copy kernel sources
cp /tmp/freertos/tasks.c Source/
cp /tmp/freertos/queue.c Source/
cp /tmp/freertos/list.c Source/
cp /tmp/freertos/timers.c Source/
cp /tmp/freertos/event_groups.c Source/
cp /tmp/freertos/stream_buffer.c Source/
cp /tmp/freertos/include/*.h Source/include/

# Copy Cortex-A port (ARM_CA8 or ARM_CA9 — both work for AM335x)
cp -r /tmp/freertos/portable/GCC/ARM_CA8/* Source/portable/GCC/ARM_CA8/
# Fallback: ARM_CA9 port is nearly identical for single-core A8 use
# cp -r /tmp/freertos/portable/GCC/ARM_CA9/* Source/portable/GCC/ARM_CA8/

# Copy heap allocator
cp /tmp/freertos/portable/MemMang/heap_4.c Source/portable/MemMang/
```

## Port Notes

The Cortex-A8 FreeRTOS port differs from Cortex-M:
- **No SysTick** — uses a DMTIMER for the tick interrupt
- **No NVIC** — AM335x has its own INTC at 0x4820_0000
- **SVC/IRQ mode switching** instead of SVC/PendSV
- **GIC or INTC configuration** required by the port layer

The `FreeRTOSConfig.h` is pre-configured for AM3358 @ 1 GHz.
The `ffi_shims.c` provides the Rust FFI wrappers for semaphore
operations (same pattern as the STM32H747I-DISCO FreeRTOS build).
