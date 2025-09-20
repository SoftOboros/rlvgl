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
    /// Issue a DCS short write command without parameters.
    fn dcs_short_write(dsi: &mut DSIHOST, cmd: u8) {
        // Wait for command FIFO space
        while dsi.gpsr.read().cmdff().bit_is_set() {}
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
    }

    /// Issue a DCS short write command with one parameter.
    fn dcs_short_write1(dsi: &mut DSIHOST, cmd: u8, param: u8) {
        // Wait for command FIFO space
        while dsi.gpsr.read().cmdff().bit_is_set() {}
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
    }

    /// Initialize the panel for basic operation.
    ///
    /// This sequence exits sleep mode and turns the display on. A complete
    /// bring-up would configure power, pixel format, and gamma tables.
    pub fn init(dsi: &mut DSIHOST) {
        // Exit sleep
        Self::dcs_short_write(dsi, 0x11);
        // Small delay
        cortex_m::asm::delay(10_000_00);
        // Set pixel format to 16-bit (RGB565)
        Self::dcs_short_write1(dsi, 0x3A, 0x55);
        // Memory access control: landscape orientation (value may be panel-specific)
        Self::dcs_short_write1(dsi, 0x36, 0x60);
        // Display on
        Self::dcs_short_write(dsi, 0x29);
    }
}
