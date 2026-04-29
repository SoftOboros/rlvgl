// SPDX-License-Identifier: MIT
//
// rust_inline_v1 layout fragment for the Beetle ESP32-C3 bsp_pac intent.
// Cited by ../app-bsp-pac.yaml as `screens[].layout`. Per
// docs/app-schema/02-generator-pipeline.md §7.7, the orchestrator copies
// this file verbatim into src/screens/led-blink.rs.
//
// Status: candidate fragment. **Stretched abstraction** — bsp_pac has no
// display and no rlvgl runtime; this fragment is the "what runs each tick"
// body of a headless bring-up demo, not a UI screen in the conventional
// sense. See docs/app-schema/03-round-trip.md §6.13 for the schema-fit
// discussion.
//
// The existing src/bsp_pac_main.rs holds an equivalent loop body inline
// — it calls `bsp_generated::init()` once at boot, then loops with raw
// `out_w1ts` / `out_w1tc` writes against `GPIO8` (the LED) bracketed by
// `busy_wait`. APP-02c per-prong templates do not currently account for
// headless intents; orchestrator emission of this manifest is explicitly
// deferred until a real headless-prong template ships.

use esp32c3 as pac;

/// LED is on GPIO8 per the chipdb board entry for `beetle_esp32c3`.
/// The chipdb-generated BSP (src/bsp_generated/io_mux.rs) configures
/// GPIO8 as a software-driven output during `bsp_generated::init()`.
const LED: u8 = 8;

/// Toggle the LED once per tick.
///
/// `frame` is a wrapping counter incremented by the prong glue. Even
/// frames drive the LED on, odd frames drive it off — the actual blink
/// rate depends on the tick period the glue uses (~0.5 s in the
/// existing hand-written `src/bsp_pac_main.rs` via `busy_wait`).
pub fn tick(p: &pac::Peripherals, frame: u32) {
    let mask: u32 = 1u32 << LED;
    if frame & 1 == 0 {
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
    } else {
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
    }
}
