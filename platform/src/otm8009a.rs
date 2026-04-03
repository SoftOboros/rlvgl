//! OTM8009A MIPI-DSI LCD panel initialization helpers.
//!
//! Provides a minimal command sequence to bring the panel out of sleep and
//! enable display output using the STM32H7 DSI host peripheral. The full
//! datasheet sequence is considerably longer; this module only implements the
//! essential steps for getting a visible test pattern.

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::DSIHOST;

/// Driver for the OTM8009A LCD controller.
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
pub struct Otm8009a;

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl Otm8009a {
    /// Wait for command FIFO space with timeout.  Returns false if timed out.
    fn wait_fifo(dsi: &DSIHOST) -> bool {
        let mut tries = 100_000u32;
        while dsi.gpsr.read().cmdff().bit_is_set() {
            tries -= 1;
            if tries == 0 {
                return false;
            }
            cortex_m::asm::nop();
        }
        true
    }

    /// Issue a DCS short write command without parameters.
    fn dcs_short_write(dsi: &mut DSIHOST, cmd: u8) -> bool {
        if !Self::wait_fifo(dsi) {
            return false;
        }
        dsi.ghcr.write(|w| unsafe {
            w.dt()
                .bits(0x05)
                .vcid()
                .bits(0)
                .wclsb()
                .bits(cmd)
                .wcmsb()
                .bits(0)
        });
        true
    }

    /// Issue a DCS short write command with one parameter.
    fn dcs_short_write1(dsi: &mut DSIHOST, cmd: u8, param: u8) -> bool {
        if !Self::wait_fifo(dsi) {
            return false;
        }
        dsi.ghcr.write(|w| unsafe {
            w.dt()
                .bits(0x15) // DCS short write, 1 param
                .vcid()
                .bits(0)
                .wclsb()
                .bits(param)
                .wcmsb()
                .bits(cmd)
        });
        true
    }

    /// Initialize the panel for basic operation.  Returns false if DSI
    /// command FIFO is unresponsive (DSI host not initialized).
    pub fn init(dsi: &mut DSIHOST) -> bool {
        // Exit sleep
        if !Self::dcs_short_write(dsi, 0x11) {
            return false;
        }
        // 120 ms delay for sleep-out (at 400 MHz ≈ 48M cycles)
        cortex_m::asm::delay(48_000_000);
        // Set pixel format to 24-bit (RGB888) — matches LCOLCR.COLC = 5
        if !Self::dcs_short_write1(dsi, 0x3A, 0x77) {
            return false;
        }
        // Memory access control: landscape orientation
        if !Self::dcs_short_write1(dsi, 0x36, 0x60) {
            return false;
        }
        // Set column address (0..799)
        // Set page address (0..479)
        // (These use long writes which we skip for now — defaults should work)

        // Display on
        if !Self::dcs_short_write(dsi, 0x29) {
            return false;
        }
        // Small delay after display-on
        cortex_m::asm::delay(4_000_000);
        true
    }
}
