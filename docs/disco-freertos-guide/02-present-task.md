<!--
02-present-task.md - Volume IV Chapter 2: ERIF-gated present with TIM7
phase-locked holdoff.
-->

**[<- Prev](01-freertos-scaffolding.md) . [Index](README.md) . [Next ->](03-touch-task.md)**

# Chapter 2 — Present Task

## Volume II reference

Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
introduced the ERIF holdoff pattern: after LTDC scan completes
(ERIF), clear LTDCEN to stop the scan, then re-enable at a fixed
DWT offset to avoid AXI contention with DMA2D. Bare-metal
implemented this as a DWT spin loop in the main thread.

## What this chapter covers

The FreeRTOS present task replaces the bare-metal DWT spin with a
TIM7 one-pulse timer and FreeRTOS semaphore — zero busy-wait,
fully preemptible, phase-locked to the panel's TE signal.

## The FreeRTOS delta

Bare-metal's DWT spin holds the CPU for 15 ms every frame. Under
FreeRTOS, that spin would starve lower-priority tasks. The TIM7
one-pulse approach:

1. DSI ERIF ISR records a DWT cycle snapshot and gives `erif_sem`.
2. Present task wakes, computes remaining microseconds until the
   holdoff deadline.
3. Arms TIM7 (OPM mode, 1 MHz tick) with ARR = remaining_us.
4. Blocks on `present_gate_sem`.
5. TIM7 UIF fires at the deadline, gives `present_gate_sem`.
6. Present task wakes, retriggers LTDC.

Between steps 3 and 6, the render task runs — it has exclusive
access to the back buffer and DMA2D while LTDC is disabled.

## Walkthrough

### 1. TIM7 configuration

```rust
unsafe fn tim7_init() {
    // Enable TIM7 clock (APB1, bit 5)
    RCC_APB1LENR.write_volatile(
        RCC_APB1LENR.read_volatile() | (1 << 5)
    );
    TIM7_PSC.write_volatile(199);  // 200 MHz / 200 = 1 MHz
    TIM7_CR1.write_volatile((1 << 3) | (1 << 2)); // OPM + URS
    TIM7_DIER.write_volatile(1);   // UIE
}
```

One-pulse mode (OPM): the counter runs once to ARR, fires UIF,
and stops. No periodic interrupts — armed per-frame.

### 2. Present task loop

```rust
loop {
    sync.wait_erif(portMAX_DELAY);  // block until ERIF

    // Compute holdoff
    let elapsed = sync.cycles_since_erif();
    if elapsed < PRESENT_HOLDOFF_CYC {
        let remaining_us = (PRESENT_HOLDOFF_CYC - elapsed) / 400;
        tim7_arm(remaining_us);
        sem_take(present_gate_sem, 50);  // block until TIM7
    }

    // Non-blocking check for new rendered frame
    if sem_take(buf_ready, 0) == pdTRUE {
        // Swap FRONT <-> BACK atomics
    }

    // DMA2D safety gate (wait if still running)
    // ...

    // Retrigger LTDC with current FRONT address
    ltdc_retrigger(fb);
}
```

The holdoff is currently 32 ms (`PRESENT_HOLDOFF_CYC = 12_800_000`
at 400 MHz). This gives the render task up to 32 ms to complete
its pristine restore + widget tree draw before LTDC scans FRONT.

### 3. LTDC retrigger

```rust
unsafe fn ltdc_retrigger(fb_addr: u32) {
    DSI_WIFCR.write_volatile(0x02);     // clear ERIF
    LTDC_L1CFBAR.write_volatile(fb_addr);
    LTDC_SRCR.write_volatile(1);         // shadow reload
    DSI_WCR.write_volatile(0x0C);        // DSIEN + LTDCEN
    DSI_WIFCR.write_volatile(0x02);      // clear spurious ERIF
}
```

The DSI ISR clears LTDCEN on every ERIF (line 267), so LTDC is
fully off between retriggers. This is the key invariant that makes
single-buffer FRONT rendering safe.

## Verify

The `?` serial command reports `tick` (present iterations) and
`erif` (ERIF wake count). Both should increment at ~18 Hz (with
32 ms holdoff + scan time).

## Going deeper

- Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
  — the bare-metal ERIF holdoff this replaces.
- RM0399 Section 54 (TIM6/TIM7) — basic timer with OPM mode.
- `freertos_sync.rs` — the `FreeRtosFrameSync` struct that wraps
  `erif_sem`, `dma2d_done_sem`, and the DWT snapshot.

---

**[<- Prev](01-freertos-scaffolding.md) . [Index](README.md) . [Next ->](03-touch-task.md)**
