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
//! 4. The reset handler enters a `wfi` loop — v0 scope per
//!    CHIPS-SILABS-06 §11 deliberately omits LED blink and console
//!    output. Those land in CHIPS-SILABS-06a / -06b.
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
        bsp_generated::slstk3701_a::pac::init();
    }
    loop {
        cortex_m::asm::wfi();
    }
}
