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
mod ipc;

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

    // Initialize mailbox; I2C4 (PD12/PD13) wiring is pending PAC-based init for CM4.
    // We’ll add FT5336 integration once we introduce a CM4-friendly HAL/PAC setup.
    ipc::init();
    let mut level: u16 = 0;
    let step: u16 = 2048; // coarse ramp
    let mut dir_up = true;
    loop {
        let _ = ipc::try_push(ipc::cmd_set_backlight(level));
        // simple triangle wave on backlight
        if dir_up {
            level = level.saturating_add(step);
            if level >= u16::MAX - step { dir_up = false; }
        } else {
            level = level.saturating_sub(step);
            if level <= step { dir_up = true; }
        }
        cortex_m::asm::delay(12_000_000);
    }
}
