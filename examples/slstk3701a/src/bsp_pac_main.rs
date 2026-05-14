//! Raw-PAC v0 bring-up for the Silicon Labs SLSTK3701A Giant Gecko Starter Kit.
//!
//! This binary proves the full chipdb → generator → BSP → cortex-m-rt
//! pipeline ratified by CHIPS-SILABS-06:
//!
//! 1. `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` +
//!    `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml` describe
//!    the board.
//! 2. `rlvgl-creator bsp from-yaml --vendor silabs --board slstk3701a`
//!    mechanically emits the 8-file BSP set under
//!    `src/bsp_generated/slstk3701_a/`.
//! 3. `bsp_generated::slstk3701_a::pac::init()` brings up CMU clock
//!    gates, GPIO MODE bits + ROUTELOC/ROUTEPEN routing, and the
//!    per-peripheral init stubs from the SILABS templates.
//! 4. CHIPS-SILABS-06a — the reset handler now toggles PH10 (LED0_R)
//!    in a busy-wait loop using `cortex_m::asm::delay`. This exercises
//!    the full BSP integration end-to-end (CMU `HFBUSCLKEN0.GPIO`
//!    from `clocks::init`, the slate-8 absolute-pin-index `mode10`
//!    field on `ph_modeh` from `io_mux::init`, and a live GPIO
//!    `ph_doutset`/`ph_doutclr` write from this entry point). UART
//!    hello-world is deferred to CHIPS-SILABS-06b; rlvgl widget tree
//!    to CHIPS-SILABS-06c.
//!
//! The example crate also exercises the linker scripts emitted by
//! CHIPS-SILABS-05a (`memory.x` + `efm32_gg11.x`), wired in via
//! `build.rs`. This is the complementary gate to
//! `tests/bsp_silabs_slstk3701a_compile.rs`: `cargo check` here pulls
//! in `cortex-m-rt`'s bundled `link.x` and actually invokes the
//! linker against the chipdb-derived `memory.x`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[entry]
fn main() -> ! {
    #[cfg(feature = "bsp_pac")]
    {
        // Bring up CMU clock gates, GPIO MODE bits + ROUTELOC routing,
        // and per-peripheral init. `io_mux::init` already drives PH10
        // as push-pull output with DOUT initialised HIGH (LEDs on this
        // board are active-LOW, so the LED starts off).
        bsp_generated::slstk3701_a::pac::init();

        // CHIPS-SILABS-06a — busy-wait toggle on PH10 (LED0_R).
        //
        // The slate-6 SKU-flatten amendment puts the `Peripherals`
        // type under the per-SKU sub-module, so the path is
        // `efm32gg11b_pac::efm32gg11b820::Peripherals::steal()`.
        // The pinned `efm32gg11b-pac 0.1.4` is the pre-method-accessor
        // svd2rust era — register fields are direct `#[repr(C)]`
        // struct members (`p.GPIO.ph_douttgl`, not `p.GPIO.ph_douttgl()`).
        // The slate-8 -02c amendment uses absolute pin-index field
        // naming on MODEH writers (`mode10` for PH10, not `mode2`).
        //
        // This PAC vintage exposes `Px_DOUT` (level) and `Px_DOUTTGL`
        // (atomic XOR) but does not expose the separate `DOUTSET` /
        // `DOUTCLR` registers per port. `ph_douttgl` is the right
        // primitive for a busy-wait blink: one write atomically XORs
        // the bit, so there is no read-modify-write race against
        // adjacent LED pins on the same port.
        let p = unsafe { efm32gg11b_pac::efm32gg11b820::Peripherals::steal() };

        loop {
            // Atomic XOR of PH10 — toggles LED0_R on/off each pass.
            // Active-LOW per the SLSTK3701A schematic; io_mux::init
            // initialised DOUT high (LED off) so the first pass turns
            // the LED on.
            p.GPIO
                .ph_douttgl
                .write(|w| unsafe { w.bits(1 << 10) });
            cortex_m::asm::delay(1_000_000);
        }
    }
    #[cfg(not(feature = "bsp_pac"))]
    loop {
        cortex_m::asm::wfi();
    }
}
