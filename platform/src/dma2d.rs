//! STM32H7 DMA2D-based blitter.
//!
//! Provides hardware-accelerated fills using the DMA2D engine. Future
//! revisions will add copy and blend operations.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};
#[cfg(feature = "dma2d")]
use stm32h7::stm32h747::DMA2D;

/// Blitter backed by the STM32H7 DMA2D peripheral.
#[cfg(feature = "dma2d")]
pub struct Dma2dBlitter {
    /// DMA2D register block.
    regs: DMA2D,
}

#[cfg(feature = "dma2d")]
impl Dma2dBlitter {
    /// Create a new DMA2D blitter from the peripheral registers.
    ///
    /// The caller must enable the DMA2D clock before invoking this
    /// constructor.
    pub fn new(regs: DMA2D) -> Self {
        // Ensure the engine is stopped.
        regs.cr.write(|w| unsafe { w.bits(0) });
        Self { regs }
    }

    fn pixel_size(fmt: PixelFmt) -> usize {
        match fmt {
            PixelFmt::Argb8888 => 4,
            PixelFmt::Rgb565 => 2,
            PixelFmt::L8 | PixelFmt::A8 => 1,
            PixelFmt::A4 => 1,
        }
    }
}

#[cfg(feature = "dma2d")]
impl Blitter for Dma2dBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        let bpp = Self::pixel_size(dst.format);
        let start = unsafe {
            dst.buf
                .as_mut_ptr()
                .add((area.y as usize * dst.stride) + (area.x as usize * bpp))
        } as u32;
        let line_offset = dst.stride - (area.w as usize * bpp);

        // Configure output address and color.
        unsafe {
            self.regs.omar.write(|w| w.bits(start));
            self.regs.ocolr.write(|w| w.bits(color));
            self.regs.oor.write(|w| w.bits(line_offset as u32));
            self.regs
                .nlr
                .write(|w| w.bits(((area.h as u32) << 16) | area.w as u32));
            // Mode 3 = register to memory (R2M).
            self.regs.cr.write(|w| w.bits(3));
            self.regs.cr.modify(|_, w| w.bits(1));
            while self.regs.isr.read().bits() & 1 == 0 {}
            self.regs.ifcr.write(|w| w.bits(1));
        }
    }

    fn blit(&mut self, _src: &Surface, _src_area: Rect, _dst: &mut Surface, _dst_pos: (i32, i32)) {
        // Blit support will be added in a later iteration.
    }

    fn blend(&mut self, _src: &Surface, _src_area: Rect, _dst: &mut Surface, _dst_pos: (i32, i32)) {
        // Blend support will be added in a later iteration.
    }
}
