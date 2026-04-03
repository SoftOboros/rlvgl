//! NT35510 MIPI-DSI LCD panel initialization.
//!
//! For MB1166 Rev A-09 (FRD400B25025-A-CTK panel) on STM32H747I-DISCO.
//! Ref: STM32CubeH7 stm32-nt35510/nt35510.c, Linux panel-novatek-nt35510.c

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::DSIHOST;

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// NT35510 panel driver.
pub struct Nt35510;

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
impl Nt35510 {
    /// Wait for command FIFO empty (CMDFE).
    fn wait_fifo(dsi: &DSIHOST) -> bool {
        let mut tries = 1_000_000u32;
        while !dsi.gpsr.read().cmdfe().bit_is_set() {
            tries -= 1;
            if tries == 0 {
                return false;
            }
            cortex_m::asm::nop();
        }
        true
    }

    /// DCS short write, no parameter (data type 0x05).
    fn dcs_short_write(dsi: &mut DSIHOST, cmd: u8) -> bool {
        if !Self::wait_fifo(dsi) {
            return false;
        }
        dsi.ghcr.write(|w| unsafe {
            w.dt().bits(0x05).vcid().bits(0).wclsb().bits(cmd).wcmsb().bits(0)
        });
        true
    }

    /// DCS short write, one parameter (data type 0x15).
    /// GHCR layout: DT[5:0] | VCID[7:6] | WCLSB[15:8]=cmd | WCMSB[23:16]=param
    fn dcs_short_write1(dsi: &mut DSIHOST, cmd: u8, param: u8) -> bool {
        if !Self::wait_fifo(dsi) {
            return false;
        }
        dsi.ghcr.write(|w| unsafe {
            w.dt().bits(0x15).vcid().bits(0).wclsb().bits(cmd).wcmsb().bits(param)
        });
        true
    }

    /// DCS long write (data type 0x39). `data` includes the DCS command byte.
    fn dcs_long_write(dsi: &mut DSIHOST, data: &[u8]) -> bool {
        if data.is_empty() {
            return true;
        }
        if !Self::wait_fifo(dsi) {
            return false;
        }
        // Pack payload into GPDR (generic payload data register), 4 bytes at a time
        let mut i = 0;
        while i < data.len() {
            let mut word: u32 = 0;
            for j in 0..4 {
                if i + j < data.len() {
                    word |= (data[i + j] as u32) << (8 * j);
                }
            }
            dsi.gpdr.write(|w| unsafe { w.bits(word) });
            i += 4;
        }
        // Write header: DT=0x39 (DCS long write), VCID=0, WC=data.len()
        let len = data.len() as u16;
        dsi.ghcr.write(|w| unsafe {
            w.dt()
                .bits(0x39)
                .vcid()
                .bits(0)
                .wclsb()
                .bits(len as u8)
                .wcmsb()
                .bits((len >> 8) as u8)
        });
        true
    }

    /// Initialize the NT35510 panel. Returns false if DSI FIFO is unresponsive.
    pub fn init(dsi: &mut DSIHOST) -> bool {
        // ── Phase 1: Page 1 — Power supply configuration ────────────────
        // Enable MTP page 1 (vendor commands)
        if !Self::dcs_long_write(dsi, &[0xF0, 0x55, 0xAA, 0x52, 0x08, 0x01]) {
            return false;
        }
        // AVDD: 5.2V
        Self::dcs_long_write(dsi, &[0xB0, 0x03, 0x03, 0x03]);
        // AVDD ratio
        Self::dcs_long_write(dsi, &[0xB6, 0x46, 0x46, 0x46]);
        // AVEE: -5.2V
        Self::dcs_long_write(dsi, &[0xB1, 0x03, 0x03, 0x03]);
        // AVEE ratio
        Self::dcs_long_write(dsi, &[0xB7, 0x36, 0x36, 0x36]);
        // VCL: -2.5V
        Self::dcs_long_write(dsi, &[0xB2, 0x00, 0x00, 0x02]);
        // VCL ratio
        Self::dcs_long_write(dsi, &[0xB8, 0x26, 0x26, 0x26]);
        // VGH setting
        Self::dcs_short_write1(dsi, 0xBF, 0x01);
        // VGH: 16V
        Self::dcs_long_write(dsi, &[0xB3, 0x09, 0x09, 0x09]);
        // VGH ratio
        Self::dcs_long_write(dsi, &[0xB9, 0x36, 0x36, 0x36]);
        // VGL: -10V
        Self::dcs_long_write(dsi, &[0xB5, 0x08, 0x08, 0x08]);
        // VGLX ratio
        Self::dcs_long_write(dsi, &[0xBA, 0x26, 0x26, 0x26]);
        // VGMP/VGSP: 4.5V / 0V
        Self::dcs_long_write(dsi, &[0xBC, 0x00, 0x80, 0x00]);
        // VGMN/VGSN: -4.5V / 0V
        Self::dcs_long_write(dsi, &[0xBD, 0x00, 0x80, 0x00]);
        // VCOM: -1.325V
        Self::dcs_long_write(dsi, &[0xBE, 0x00, 0x50]);

        // ── Phase 2: Page 0 — Display control ──────────────────────────
        Self::dcs_long_write(dsi, &[0xF0, 0x55, 0xAA, 0x52, 0x08, 0x00]);
        // Display optional control
        Self::dcs_long_write(dsi, &[0xB1, 0xFC, 0x00]);
        // Source output hold time
        Self::dcs_short_write1(dsi, 0xB6, 0x03);
        // Display resolution control (480×800)
        Self::dcs_short_write1(dsi, 0xB5, 0x50);
        // Gate EQ control
        Self::dcs_long_write(dsi, &[0xB7, 0x00, 0x00]);
        // Source EQ control
        Self::dcs_long_write(dsi, &[0xB8, 0x01, 0x02, 0x02, 0x02]);
        // Inversion control
        Self::dcs_long_write(dsi, &[0xBC, 0x00, 0x00, 0x00]);
        // Display timing control
        Self::dcs_long_write(dsi, &[0xCC, 0x03, 0x00, 0x00]);
        // Source driving capability
        Self::dcs_short_write1(dsi, 0xBA, 0x01);

        // ── Phase 3: Standard DCS commands ──────────────────────────────
        // Memory access control: portrait (0x00) — landscape would be 0x60
        Self::dcs_short_write1(dsi, 0x36, 0x00);
        // Column address: 0..479
        Self::dcs_long_write(dsi, &[0x2A, 0x00, 0x00, 0x01, 0xDF]);
        // Page address: 0..799
        Self::dcs_long_write(dsi, &[0x2B, 0x00, 0x00, 0x03, 0x1F]);

        // Sleep out
        if !Self::dcs_short_write(dsi, 0x11) {
            return false;
        }
        // 200ms delay for sleep-out (at 400 MHz ≈ 80M cycles)
        cortex_m::asm::delay(80_000_000);

        // Pixel format: RGB888 (0x77)
        Self::dcs_short_write1(dsi, 0x3A, 0x77);

        // Set Tear On — required for adapted command mode TE handshake
        // 0x35 param=0x00: TE output V-blanking only
        Self::dcs_short_write1(dsi, 0x35, 0x00);

        // Backlight control
        Self::dcs_short_write1(dsi, 0x51, 0x7F); // WRDISBV: brightness 50%
        Self::dcs_short_write1(dsi, 0x53, 0x2C); // WRCTRLD: BL+DD+BL_EN
        Self::dcs_short_write1(dsi, 0x55, 0x02); // WRCABC: still picture
        Self::dcs_short_write1(dsi, 0x5E, 0xFF); // WRCABCMB: min brightness

        // Display ON
        if !Self::dcs_short_write(dsi, 0x29) {
            return false;
        }
        cortex_m::asm::delay(4_000_000);

        // Manual pixel test moved to dedicated function called after
        // LTDC/DSI init with auto-refresh disabled.

        true
    }

    /// Manual pixel test: write a 10×10 green block at top-left via DCS.
    /// Call with LTDC stopped and auto-refresh disabled so nothing overwrites.
    pub fn pixel_test(dsi: &mut DSIHOST) {
        // Enable LP mode for DCS writes
        dsi.cmcr.write(|w| unsafe {
            w.bits(
                (1 << 0)   // TEARE
                | (1 << 16) // DSW0TX (short write 0p in LP)
                | (1 << 17) // DSW1TX (short write 1p in LP)
                | (1 << 19) // DLWTX  (long write in LP)
            )
        });
        cortex_m::asm::delay(400_000);

        // Set column 0..9
        Self::dcs_long_write(dsi, &[0x2A, 0x00, 0x00, 0x00, 0x09]);
        // Set row 0..9
        Self::dcs_long_write(dsi, &[0x2B, 0x00, 0x00, 0x00, 0x09]);
        // Write Memory Start: 10 green pixels (row 0)
        Self::dcs_long_write(dsi, &[
            0x2C,
            0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
            0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
            0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
            0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
            0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
        ]);
        // Rows 1..9 via Write Memory Continue
        for _ in 1..10 {
            Self::dcs_long_write(dsi, &[
                0x3C,
                0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
                0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
                0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
                0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
                0x00, 0xFF, 0x00,  0x00, 0xFF, 0x00,
            ]);
        }

        // Restore column/page to full panel
        Self::dcs_long_write(dsi, &[0x2A, 0x00, 0x00, 0x01, 0xDF]);
        Self::dcs_long_write(dsi, &[0x2B, 0x00, 0x00, 0x03, 0x1F]);

        // Clear LP overrides, keep TEARE
        dsi.cmcr.write(|w| unsafe { w.bits(1) });
    }
}
