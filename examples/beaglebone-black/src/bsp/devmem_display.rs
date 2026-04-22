//! `DisplayDriver` that owns the AM335x LCDC via `/dev/mem`.
//!
//! This is the Linux counterpart to the bare-metal / FreeRTOS / Zephyr
//! entry points: they all call the same [`super::lcdc::init_raster`] and
//! write into a physically-contiguous ARGB framebuffer. Under Linux the
//! framebuffer lives at a kernel-reserved region (via the `mem=510M`
//! bootarg) mapped through `/dev/mem`.
//!
//! Preconditions:
//!
//! - `tilcdc` has been unbound (`tools/unbind-tilcdc.sh`).
//! - `uEnv.txt` contains `mem=510M` so physical [0x9FE00000, 0xA0000000)
//!   is out of the kernel RAM pool and safe to mmap through `/dev/mem`.
//! - Binary is invoked with root privileges (`sudo ./rlvgl-bbb`) for
//!   `/dev/mem` access.

use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::{DisplayDriver, Screen};

use super::{devmem, lcdc};

/// LCDC-backed display driver.
pub struct DevMemLcdcDisplay {
    fb_va: *mut u8,
    width: u32,
    height: u32,
}

unsafe impl Send for DevMemLcdcDisplay {}

impl DevMemLcdcDisplay {
    /// Map `/dev/mem`, bring up the LCDC, and return a driver ready to
    /// accept `flush` calls.
    pub fn init() -> Self {
        devmem::DevMem::init();

        let (fb_va, fb_pa) = devmem::map_framebuffer();

        unsafe {
            core::ptr::write_bytes(fb_va, 0, devmem::FB_SIZE);
            super::prcm::enable_lcdc();
            super::pinmux::configure_lcd_pins();
            lcdc::init_raster(fb_pa, devmem::FB_SIZE as u32);
        }

        eprintln!(
            "lcdc: framebuffer mapped at PA 0x{:08X}, VA {:p}, {}x{} {}Hz",
            fb_pa,
            fb_va,
            lcdc::HACTIVE,
            lcdc::VACTIVE,
            lcdc::FRAME_HZ,
        );

        Self {
            fb_va,
            width: lcdc::HACTIVE,
            height: lcdc::VACTIVE,
        }
    }
}

impl DisplayDriver for DevMemLcdcDisplay {
    fn screen(&self) -> Screen {
        Screen::landscape(self.width, self.height)
    }

    fn flush(&mut self, area: Rect, colors: &[Color]) {
        let fb = unsafe { core::slice::from_raw_parts_mut(self.fb_va, devmem::FB_SIZE) };
        let stride = self.width as usize * 4;

        for row in 0..area.height as usize {
            let py = area.y as usize + row;
            if py >= self.height as usize {
                break;
            }
            for col in 0..area.width as usize {
                let px = area.x as usize + col;
                if px >= self.width as usize {
                    break;
                }
                let color = colors[row * area.width as usize + col];
                let off = py * stride + px * 4;
                // LCDC TFT24 unpacked takes the low 24 bits of each word.
                // AM335x lcd_data_o[23:0] maps {R[7:0], G[7:0], B[7:0]}
                // from the high nibble down; the on-chip DMA reads
                // little-endian words from memory, so byte order in RAM
                // is {B, G, R, pad}.
                fb[off] = color.2; // B
                fb[off + 1] = color.1; // G
                fb[off + 2] = color.0; // R
                fb[off + 3] = 0; // pad (upper 8 bits ignored by LCDC)
            }
        }
    }
}
