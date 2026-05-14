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
//!    its first action, then idles in `wfi` indefinitely.
//!
//! v0 scope per CHIPS-TI-06 §9: prove `init()` returns and the binary
//! links. LED blink deferred to CHIPS-TI-06a; UART hello-world deferred
//! to -06b; rlvgl widget tree deferred to -06c. See
//! [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-06-EXAMPLE.md`].

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[entry]
fn main() -> ! {
    // First action: bring up the BSP. Per CHIPS-TI-06 §5.5 frozen v0
    // entry-point shape, no other call may precede this one.
    #[cfg(feature = "bsp_pac")]
    {
        bsp_generated::launchxl_cc1352_r1::init();
    }

    // Idle. LED toggling / UART output / rlvgl widget-tree pumping all
    // land in follow-up slates (-06a / -06b / -06c) per CHIPS-TI-06 §14.
    loop {
        cortex_m::asm::wfi();
    }
}
