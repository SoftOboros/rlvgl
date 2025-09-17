#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! Minimal CM4 entry for STM32H747I-DISCO.
//! Uses the generated dual-core PAC BSP to apply power config, then idles.

use cortex_m_rt::entry;
#[cfg(target_os = "none")]
#[cfg(not(doc))]
use panic_halt as _;

// Use the split-core generated PAC BSP for CM4
#[path = "bsp/cm4/pac.rs"]
mod bsp_pac;

#[cfg(not(doc))]
#[entry]
fn main() -> ! {
    // Take CM4 PAC and run minimal power init via generated BSP
    let dp = stm32h7::stm32h747cm4::Peripherals::take().unwrap();
    bsp_pac::init_power(&dp);
    // Wait until CM7 signals clocks ready via mailbox
    #[allow(clippy::let_unit_value)]
    {
        let _ = bsp_pac::wait_for_clocks();
    }

    // TODO: Optionally wait for CM7 clocks via HSEM/flag handshake
    // bsp_pac::wait_for_clocks();

    // For now, idle
    loop {
        cortex_m::asm::wfi();
    }
}
