//! STM32H7 DMA2D-based blitter.
//!
//! Provides hardware-accelerated fills, pixel format conversions, and blending
//! using the DMA2D engine.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};
#[cfg(feature = "dma2d")]
use stm32h7::stm32h747cm7::DMA2D;

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

    /// Consume the blitter and return the raw DMA2D peripheral.
    pub fn into_inner(self) -> DMA2D {
        self.regs
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
    const CR_MODE_M2M_BLEND: u32 = 0x0002_0000;
    const CR_MODE_R2M: u32 = 0x0003_0000;
    const CR_TCIE: u32 = 1 << 9;
    const ISR_TC: u32 = 1;

    /// Enable the transfer-complete interrupt.
    pub fn enable_tc_interrupt(&mut self) {
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_TCIE) });
    }

    /// Disable the transfer-complete interrupt.
    pub fn disable_tc_interrupt(&mut self) {
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() & !Self::CR_TCIE) });
    }

    /// Returns `true` if the engine is currently processing a command.
    pub fn is_busy(&self) -> bool {
        self.regs.cr.read().bits() & Self::CR_START != 0
    }

    /// Returns `true` if the last command has completed.
    pub fn is_complete(&self) -> bool {
        self.regs.isr.read().bits() & Self::ISR_TC != 0
    }

    /// Clear the transfer-complete flag.
    pub fn clear_complete(&mut self) {
        self.regs.ifcr.write(|w| unsafe { w.bits(Self::ISR_TC) });
    }

    fn wait(&mut self) {
        while !self.is_complete() {}
        self.clear_complete();
    }

    fn start_fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
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
        }
        self.regs.cr.write(|w| unsafe { w.bits(Self::CR_MODE_R2M) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    fn start_blit(
        &mut self,
        src: &Surface,
        src_area: Rect,
        dst: &mut Surface,
        dst_pos: (i32, i32),
    ) {
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
        }
        self.regs
            .cr
            .write(|w| unsafe { w.bits(Self::CR_MODE_M2M_PFC) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    fn start_blend(
        &mut self,
        src: &Surface,
        src_area: Rect,
        dst: &mut Surface,
        dst_pos: (i32, i32),
    ) {
        let src_bpp = Self::pixel_size(src.format);
        let dst_bpp = Self::pixel_size(dst.format);

        let fg_start = unsafe {
            src.buf
                .as_ptr()
                .add((src_area.y as usize * src.stride) + (src_area.x as usize * src_bpp))
        } as u32;
        let bg_start = unsafe {
            dst.buf
                .as_mut_ptr()
                .add((dst_pos.1 as usize * dst.stride) + (dst_pos.0 as usize * dst_bpp))
        } as u32;

        let fg_offset = src.stride - (src_area.w as usize * src_bpp);
        let bg_offset = dst.stride - (src_area.w as usize * dst_bpp);

        unsafe {
            self.regs.fgmar.write(|w| w.bits(fg_start));
            self.regs.fgor.write(|w| w.bits(fg_offset as u32));
            self.regs
                .fgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(src.format)));
            self.regs.bgmar.write(|w| w.bits(bg_start));
            self.regs.bgor.write(|w| w.bits(bg_offset as u32));
            self.regs
                .bgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(dst.format)));
            self.regs.omar.write(|w| w.bits(bg_start));
            self.regs.oor.write(|w| w.bits(bg_offset as u32));
            self.regs
                .nlr
                .write(|w| w.bits(((src_area.h as u32) << 16) | src_area.w as u32));
        }
        self.regs
            .cr
            .write(|w| unsafe { w.bits(Self::CR_MODE_M2M_BLEND) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }
}

// ── Raw-pointer methods for direct SDRAM buffer operations ───────────────
//
// These bypass the Surface abstraction for use by overlay renderers
// (e.g. star crawl) that manage their own SDRAM buffers.

#[cfg(feature = "dma2d")]
impl Dma2dBlitter {
    /// R2M fill by raw pointer. Fills `width × height` pixels at `dst` with
    /// a solid color. `dst_stride` is in bytes.
    pub fn fill_raw(
        &mut self,
        dst: *mut u8,
        dst_stride: u32,
        width: u32,
        height: u32,
        color: u32,
        fmt: PixelFmt,
    ) {
        let bpp = Self::pixel_size(fmt) as u32;
        let line_offset = dst_stride - width * bpp;
        unsafe {
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.ocolr.write(|w| w.bits(color));
            self.regs
                .opfccr
                .write(|w| w.bits(Self::dma2d_fmt(fmt)));
            self.regs.oor.write(|w| w.bits(line_offset));
            self.regs
                .nlr
                .write(|w| w.bits((height << 16) | width));
        }
        self.regs
            .cr
            .write(|w| unsafe { w.bits(Self::CR_MODE_R2M) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
        self.wait();
    }

    /// M2M copy by raw pointer. Copies `width × height` pixels from `src` to
    /// `dst`. Both must be the same pixel format. Strides are in bytes.
    pub fn blit_raw(
        &mut self,
        src: *const u8,
        src_stride: u32,
        dst: *mut u8,
        dst_stride: u32,
        width: u32,
        height: u32,
        fmt: PixelFmt,
    ) {
        let bpp = Self::pixel_size(fmt) as u32;
        let src_offset = src_stride - width * bpp;
        let dst_offset = dst_stride - width * bpp;
        let cm = Self::dma2d_fmt(fmt);
        unsafe {
            self.regs.fgmar.write(|w| w.bits(src as u32));
            self.regs.fgor.write(|w| w.bits(src_offset));
            self.regs.fgpfccr.write(|w| w.bits(cm));
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.oor.write(|w| w.bits(dst_offset));
            self.regs.opfccr.write(|w| w.bits(cm));
            self.regs
                .nlr
                .write(|w| w.bits((height << 16) | width));
        }
        self.regs
            .cr
            .write(|w| unsafe { w.bits(Self::CR_MODE_M2M_PFC) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
        self.wait();
    }

    /// Blend an A8 alpha source with a fixed foreground color onto an
    /// ARGB8888 background. Each source byte is treated as the alpha channel
    /// and multiplied by `fg_color` (0x00RRGGBB). The result is blended
    /// onto the destination which is read-modify-written in place.
    ///
    /// This is the key DMA2D operation for anti-aliased colored text
    /// rendering: set fg_color to yellow (0x00FFFF00) and feed glyph
    /// bitmaps as A8 source data.
    ///
    /// `a8_src` must be contiguous (stride == width).
    /// `dst` points to the first ARGB8888 pixel to blend onto.
    /// `dst_stride` is in bytes.
    pub fn blend_a8_color(
        &mut self,
        a8_src: *const u8,
        width: u32,
        height: u32,
        fg_color: u32,
        dst: *mut u8,
        dst_stride: u32,
    ) {
        let dst_bpp = 4u32; // ARGB8888
        let dst_offset = dst_stride - width * dst_bpp;
        unsafe {
            // Foreground: A8 source with fixed color
            self.regs.fgmar.write(|w| w.bits(a8_src as u32));
            self.regs.fgor.write(|w| w.bits(0)); // contiguous A8
            // CM=0x9 (A8), AM=00 (no alpha modify), ALPHA=0xFF
            self.regs
                .fgpfccr
                .write(|w| w.bits(0xFF00_0000 | 9));
            self.regs.fgcolr.write(|w| w.bits(fg_color));

            // Background: ARGB8888 destination (read side)
            self.regs.bgmar.write(|w| w.bits(dst as u32));
            self.regs.bgor.write(|w| w.bits(dst_offset));
            self.regs.bgpfccr.write(|w| w.bits(0)); // ARGB8888

            // Output: same as background (in-place blend)
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.oor.write(|w| w.bits(dst_offset));
            self.regs.opfccr.write(|w| w.bits(0)); // ARGB8888

            self.regs
                .nlr
                .write(|w| w.bits((height << 16) | width));
        }
        self.regs
            .cr
            .write(|w| unsafe { w.bits(Self::CR_MODE_M2M_BLEND) });
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
        self.wait();
    }
}

#[cfg(feature = "dma2d")]
impl Blitter for Dma2dBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::BLEND | BlitCaps::PFC
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        self.start_fill(dst, area, color);
        self.wait();
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        self.start_blit(src, src_area, dst, dst_pos);
        self.wait();
    }

    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        self.start_blend(src, src_area, dst, dst_pos);
        self.wait();
    }
}
