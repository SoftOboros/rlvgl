//! STM32H7 DMA2D-based blitter.
//!
//! Provides hardware-accelerated fills and pixel format conversions using the
//! DMA2D engine. Future revisions will add blending operations.

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

    /// Translate [`PixelFmt`] to the DMA2D color mode value.
    fn dma2d_fmt(fmt: PixelFmt) -> u32 {
        match fmt {
            PixelFmt::Argb8888 => 0,
            PixelFmt::Rgb565 => 2,
            PixelFmt::L8 => 5,
            PixelFmt::A8 => 9,
            PixelFmt::A4 => 10,
        }
    }

    const CR_START: u32 = 1 << 0;
    const CR_MODE_M2M_PFC: u32 = 0x0001_0000;
    const CR_MODE_R2M: u32 = 0x0003_0000;
    const ISR_TC: u32 = 1;
}

#[cfg(feature = "dma2d")]
impl Blitter for Dma2dBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::PFC
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        let bpp = Self::pixel_size(dst.format);
        let start = unsafe {
            dst.buf
                .as_mut_ptr()
                .add((area.y as usize * dst.stride) + (area.x as usize * bpp))
        } as u32;
        let line_offset = dst.stride - (area.w as usize * bpp);

        unsafe {
            self.regs.omar.write(|w| w.bits(start));
            self.regs.ocolr.write(|w| w.bits(color));
            self.regs.oor.write(|w| w.bits(line_offset as u32));
            self.regs
                .nlr
                .write(|w| w.bits(((area.h as u32) << 16) | area.w as u32));
            self.regs.cr.write(|w| unsafe { w.bits(Self::CR_MODE_R2M) });
            self.regs
                .cr
                .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
            while self.regs.isr.read().bits() & Self::ISR_TC == 0 {}
            self.regs.ifcr.write(|w| w.bits(Self::ISR_TC));
        }
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        let src_bpp = Self::pixel_size(src.format);
        let dst_bpp = Self::pixel_size(dst.format);

        let src_start = unsafe {
            src.buf
                .as_ptr()
                .add((src_area.y as usize * src.stride) + (src_area.x as usize * src_bpp))
        } as u32;
        let dst_start = unsafe {
            dst.buf
                .as_mut_ptr()
                .add((dst_pos.1 as usize * dst.stride) + (dst_pos.0 as usize * dst_bpp))
        } as u32;

        let src_offset = src.stride - (src_area.w as usize * src_bpp);
        let dst_offset = dst.stride - (src_area.w as usize * dst_bpp);

        unsafe {
            self.regs.fgmar.write(|w| w.bits(src_start));
            self.regs.fgor.write(|w| w.bits(src_offset as u32));
            self.regs
                .fgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(src.format)));
            self.regs.omar.write(|w| w.bits(dst_start));
            self.regs.oor.write(|w| w.bits(dst_offset as u32));
            self.regs
                .opfccr
                .write(|w| w.bits(Self::dma2d_fmt(dst.format)));
            self.regs
                .nlr
                .write(|w| w.bits(((src_area.h as u32) << 16) | src_area.w as u32));
            self.regs
                .cr
                .write(|w| unsafe { w.bits(Self::CR_MODE_M2M_PFC) });
            self.regs
                .cr
                .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
            while self.regs.isr.read().bits() & Self::ISR_TC == 0 {}
            self.regs.ifcr.write(|w| w.bits(Self::ISR_TC));
        }
    }

    fn blend(&mut self, _src: &Surface, _src_area: Rect, _dst: &mut Surface, _dst_pos: (i32, i32)) {
        // Blend support will be added in a later iteration.
    }
}
