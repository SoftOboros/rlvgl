//! Raw-PAC bring-up path for the TI LAUNCHXL-CC1352R1 LaunchPad.
//!
//! This binary proves the slate-9 generator pipeline end-to-end:
//!
//! 1. `chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml` +
//!    `chipdb/rlvgl-chips-ti/db/boards/launchxl_cc1352r1.yaml` describe
//!    the chip + board.
//! 2. `rlvgl-creator bsp from-yaml --vendor ti --board launchxl_cc1352r1`
//!    mechanically emits the 8 BSP files (6 .rs + memory.x + cc1352_r.x)
//!    under `src/bsp_generated/launchxl_cc1352_r1/`.
//! 3. [`bsp_generated::launchxl_cc1352_r1::init`] (re-exported from
//!    `pac.rs`) brings up SimpleLink PRCM clocks, IOC pin routing, and
//!    per-peripheral init (UART0 console real, others stubbed).
//! 4. This `main` calls [`bsp_generated::launchxl_cc1352_r1::init`] as
//!    its first action, then drives DIO_6 (LED_RED) in a busy-wait toggle
//!    loop using `cortex_m::asm::delay` for timing.
//!
//! v0 scope per CHIPS-TI-06 §9 proved that `init()` returned and the
//! binary linked. CHIPS-TI-06a (this slate) extends that with a real
//! GPIO write — DIO_6 LED_RED toggles — to validate that PRCM clock
//! gating, IOC PORT_CFG routing, and GPIO DOE setup line up against the
//! real `cc13x2_26x2_pac` PAC on hardware. UART hello-world remains
//! deferred to -06b; rlvgl widget tree deferred to -06c. See
//! [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-06-EXAMPLE.md`].
//!
//! The LED pin is `board::LED_RED == DIO_6`. The slate-9
//! `io_mux::init()` already writes `IOC.IOCFG6 = port_id(0x00 GPIO)` and
//! `GPIO.DOE31_0 |= 1 << 6`, so this main only needs to flip
//! `DOUTSET31_0` / `DOUTCLR31_0` to drive the line.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
use cc13x2_26x2_pac as pac;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[entry]
fn main() -> ! {
    // First action: bring up the BSP. Per CHIPS-TI-06 §5.5 frozen v0
    // entry-point shape, no other call may precede this one.
    #[cfg(feature = "bsp_pac")]
    {
        bsp_generated::launchxl_cc1352_r1::init();

        // SAFETY: `init()` above is the single steal point in this
        // crate's startup. Re-stealing here is sound because we run
        // single-threaded with interrupts disabled at this point and
        // never aliased PAC reference is leaked to ISRs.
        let p = unsafe { pac::Peripherals::steal() };

        // DIO_6 (LED_RED). DOE31_0 bit 6 is already set by
        // `io_mux::init()` for this pin, so the line is configured as
        // an output here.
        //
        // DOUTSET31_0 / DOUTCLR31_0 are write-1-to-set / write-1-to-clear
        // companion registers to DOUT31_0 (TI SWCU185 §15.5) — writing
        // 0 to other bits is harmless. The cc13x2_26x2_pac 0.10.3 PAC
        // exposes per-DIO bit field writers (`dio6()`), so we set just
        // that one bit each iteration.
        //
        // ~50 ms at the 48 MHz CPU clock configured by `clocks::init()`
        // (`board::CPU_HZ = 48_000_000`). Visible to the naked eye at
        // roughly 10 Hz toggle.
        const DELAY_CYCLES: u32 = 2_400_000;

        loop {
            // LED on: set DIO_6 high.
            p.gpio
                .doutset31_0()
                .write(|w| w.dio6().set_bit());
            cortex_m::asm::delay(DELAY_CYCLES);
            // LED off: clear DIO_6.
            p.gpio
                .doutclr31_0()
                .write(|w| w.dio6().set_bit());
            cortex_m::asm::delay(DELAY_CYCLES);
        }
    }

    // No-feature fallback: the binary still links and idles. The
    // `required-features = ["bsp_pac"]` stanza on the bin target keeps
    // this branch unreachable for `cargo check --target …`, but the
    // compiler still needs a `!` return type so `wfi` in a loop covers
    // the bare-no_std case.
    #[cfg(not(feature = "bsp_pac"))]
    loop {
        cortex_m::asm::wfi();
    }
}
