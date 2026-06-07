//! STM32H7 DMA2D-based blitter.
//!
//! Provides hardware-accelerated fills, pixel format conversions, and blending
//! using the DMA2D engine.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};
#[cfg(feature = "dma2d")]
use crate::hwcore::surface::{BackBuffer, BorrowedForDma, InFlight};
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
        // Preserve any in-flight transfer when the raw peripheral is moved
        // in and out of the display wrapper between round-robin tasks.
        if regs.cr.read().bits() & Self::CR_START == 0 {
            regs.ifcr.write(|w| unsafe { w.bits(Self::IFCR_ALL) });
        }
        // Enable AXI dead time to avoid starving LTDC scanout.
        // DT=8 cycles between DMA2D AXI bursts gives LTDC room to read.
        regs.amtcr.write(|w| unsafe { w.bits((8 << 8) | 1) });
        Self { regs }
    }

    /// Create a DMA2D blitter by stealing the peripheral singleton.
    ///
    /// Uses `Peripherals::steal()` to get the DMA2D register block.
    ///
    /// # Safety
    ///
    /// The caller must ensure no other code holds the DMA2D peripheral.
    /// The DMA2D clock must already be enabled (RCC AHB3ENR bit 4).
    pub unsafe fn steal() -> Self {
        // SAFETY: caller asserts no other code holds DMA2D and the
        // DMA2D clock is enabled, per this function's contract above.
        let p = unsafe { stm32h7::stm32h747cm7::Peripherals::steal() };
        Self::new(p.DMA2D)
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
    const CR_TEIE: u32 = 1 << 8;
    const CR_TCIE: u32 = 1 << 9;
    const CR_IRQ_MASK: u32 = Self::CR_TEIE | Self::CR_TCIE;
    /// TCIF = bit 1 of DMA2D_ISR (transfer complete).
    /// Note: bit 0 is TEIF (transfer error), NOT transfer complete.
    const ISR_TC: u32 = 1 << 1;
    const ISR_CEIF: u32 = 1 << 5; // Configuration error
    const ISR_TEIF: u32 = 1 << 0; // Transfer error
    const ISR_ERROR_MASK: u32 = Self::ISR_CEIF | Self::ISR_TEIF;
    const IFCR_ALL: u32 = 0x3F;

    /// Enable the transfer-complete interrupt.
    pub fn enable_tc_interrupt(&mut self) {
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_IRQ_MASK) });
    }

    /// Disable the transfer-complete interrupt.
    pub fn disable_tc_interrupt(&mut self) {
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() & !Self::CR_IRQ_MASK) });
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

    /// Returns `true` when a transfer is still running.
    pub fn is_in_flight(&self) -> bool {
        self.is_busy()
    }

    /// Poll the DMA2D transfer-complete flag without blocking.
    pub fn poll_complete(&self) -> bool {
        self.is_complete()
    }

    /// Acknowledge the most recent transfer-complete interrupt source.
    pub fn ack_complete(&mut self) {
        self.clear_complete();
    }

    /// Return the currently asserted DMA2D error flags.
    pub fn read_error(&self) -> u32 {
        self.regs.isr.read().bits() & Self::ISR_ERROR_MASK
    }

    fn prepare_start(&mut self) {
        self.regs.ifcr.write(|w| unsafe { w.bits(Self::IFCR_ALL) });
    }

    fn write_cr_mode(&mut self, mode: u32) {
        let irq_bits = self.regs.cr.read().bits() & Self::CR_IRQ_MASK;
        self.regs.cr.write(|w| unsafe { w.bits(mode | irq_bits) });
    }

    fn wait(&mut self) {
        while self.is_in_flight() {
            cortex_m::asm::nop();
        }
        self.regs.ifcr.write(|w| unsafe { w.bits(Self::IFCR_ALL) });
    }

    fn start_fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        let bpp = Self::pixel_size(dst.format);
        let start = unsafe {
            dst.buf
                .as_mut_ptr()
                .add((area.y as usize * dst.stride) + (area.x as usize * bpp))
        } as u32;
        let line_offset_px = (dst.stride - (area.w as usize * bpp)) / bpp;

        unsafe {
            self.regs.omar.write(|w| w.bits(start));
            self.regs.ocolr.write(|w| w.bits(color));
            self.regs.oor.write(|w| w.bits(line_offset_px as u32));
            self.regs
                .nlr
                .write(|w| w.bits((area.w as u32) << 16 | area.h as u32));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_R2M);
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

        let src_offset_px = (src.stride - (src_area.w as usize * src_bpp)) / src_bpp;
        let dst_offset_px = (dst.stride - (src_area.w as usize * dst_bpp)) / dst_bpp;

        unsafe {
            self.regs.fgmar.write(|w| w.bits(src_start));
            self.regs.fgor.write(|w| w.bits(src_offset_px as u32));
            self.regs
                .fgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(src.format)));
            self.regs.omar.write(|w| w.bits(dst_start));
            self.regs.oor.write(|w| w.bits(dst_offset_px as u32));
            self.regs
                .opfccr
                .write(|w| w.bits(Self::dma2d_fmt(dst.format)));
            self.regs
                .nlr
                .write(|w| w.bits((src_area.w as u32) << 16 | src_area.h as u32));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_M2M_PFC);
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

        let fg_offset_px = (src.stride - (src_area.w as usize * src_bpp)) / src_bpp;
        let bg_offset_px = (dst.stride - (src_area.w as usize * dst_bpp)) / dst_bpp;

        unsafe {
            self.regs.fgmar.write(|w| w.bits(fg_start));
            self.regs.fgor.write(|w| w.bits(fg_offset_px as u32));
            self.regs
                .fgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(src.format)));
            self.regs.bgmar.write(|w| w.bits(bg_start));
            self.regs.bgor.write(|w| w.bits(bg_offset_px as u32));
            self.regs
                .bgpfccr
                .write(|w| w.bits(Self::dma2d_fmt(dst.format)));
            self.regs.omar.write(|w| w.bits(bg_start));
            self.regs.oor.write(|w| w.bits(bg_offset_px as u32));
            self.regs
                .nlr
                .write(|w| w.bits((src_area.w as u32) << 16 | src_area.h as u32));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_M2M_BLEND);
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
    /// R2M fill by raw pointer (crate-internal engine).
    ///
    /// Step 9 of the Register-Mashing Discipline migration: visibility
    /// downgraded to `pub(crate)` and renamed to remove the literal
    /// `start_fill_raw` token from the public surface. Callers use the
    /// typed [`Self::start_fill_typed`] which delegates here.
    pub(crate) fn fill_engine_raw(
        &mut self,
        dst: *mut u8,
        dst_stride: u32,
        width: u32,
        height: u32,
        color: u32,
        fmt: PixelFmt,
    ) {
        let bpp = Self::pixel_size(fmt) as u32;
        let line_offset_px = (dst_stride / bpp) - width;
        unsafe {
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.ocolr.write(|w| w.bits(color));
            // OPFCCR reset default is 0 = ARGB8888 — skip write for R2M
            self.regs.oor.write(|w| w.bits(line_offset_px));
            // NLR: PL[29:16] = pixels per line (width), NL[15:0] = number of lines (height)
            self.regs.nlr.write(|w| w.bits((width << 16) | height));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_R2M);
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    /// Blocking compatibility wrapper for [`Self::fill_engine_raw`].
    pub fn fill_raw(
        &mut self,
        dst: *mut u8,
        dst_stride: u32,
        width: u32,
        height: u32,
        color: u32,
        fmt: PixelFmt,
    ) {
        self.fill_engine_raw(dst, dst_stride, width, height, color, fmt);
        self.wait();
    }

    /// M2M copy by raw pointer (crate-internal engine).
    ///
    /// Step 9 of the Register-Mashing Discipline migration: visibility
    /// downgraded to `pub(crate)` and renamed. Callers use the typed
    /// [`Self::start_blit_typed`] which delegates here.
    pub(crate) fn blit_engine_raw(
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
        let src_offset_px = (src_stride - width * bpp) / bpp;
        let dst_offset_px = (dst_stride - width * bpp) / bpp;
        let cm = Self::dma2d_fmt(fmt);
        unsafe {
            self.regs.fgmar.write(|w| w.bits(src as u32));
            self.regs.fgor.write(|w| w.bits(src_offset_px));
            self.regs.fgpfccr.write(|w| w.bits(cm));
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.oor.write(|w| w.bits(dst_offset_px));
            self.regs.opfccr.write(|w| w.bits(cm));
            self.regs.nlr.write(|w| w.bits((width << 16) | height));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_M2M_PFC);
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    /// Blocking compatibility wrapper for [`Self::blit_engine_raw`].
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
        self.blit_engine_raw(src, src_stride, dst, dst_stride, width, height, fmt);
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
    pub fn start_blend_a8_color(
        &mut self,
        a8_src: *const u8,
        width: u32,
        height: u32,
        fg_color: u32,
        dst: *mut u8,
        dst_stride: u32,
    ) {
        let dst_bpp = 4u32; // ARGB8888
        let dst_offset_px = (dst_stride - width * dst_bpp) / dst_bpp;
        unsafe {
            // Foreground: A8 source with fixed color
            self.regs.fgmar.write(|w| w.bits(a8_src as u32));
            self.regs.fgor.write(|w| w.bits(0)); // contiguous A8
            // CM=0x9 (A8), AM=00 (no alpha modify), ALPHA=0xFF
            self.regs.fgpfccr.write(|w| w.bits(0xFF00_0000 | 9));
            self.regs.fgcolr.write(|w| w.bits(fg_color));

            // Background: ARGB8888 destination (read side)
            self.regs.bgmar.write(|w| w.bits(dst as u32));
            self.regs.bgor.write(|w| w.bits(dst_offset_px));
            self.regs.bgpfccr.write(|w| w.bits(0)); // ARGB8888

            // Output: same as background (in-place blend)
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.oor.write(|w| w.bits(dst_offset_px));
            self.regs.opfccr.write(|w| w.bits(0)); // ARGB8888

            self.regs.nlr.write(|w| w.bits((width << 16) | height));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_M2M_BLEND);
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    /// Start a non-blocking A8 alpha blend where the A8 source is
    /// contiguous but the ARGB destination has a custom line offset.
    ///
    /// This handles the portrait-column write pattern: NLR width=1,
    /// height=`pixel_count`, with `dst_line_offset_px` pixels between
    /// consecutive output rows.  DMA2D handles the stride-separated
    /// writes internally with AXI burst optimization.
    ///
    /// `a8_src`: contiguous A8 alpha values (`pixel_count` bytes).
    /// `pixel_count`: number of pixels to blend.
    /// `fg_color`: 0x00RRGGBB foreground colour.
    /// `dst`: first ARGB8888 pixel in the destination column.
    /// `dst_line_offset_px`: pixels to skip between consecutive output
    ///   rows (fb_width − 1 for a single-pixel-wide vertical column).
    pub fn start_blend_a8_column(
        &mut self,
        a8_src: *const u8,
        pixel_count: u32,
        fg_color: u32,
        dst: *mut u8,
        dst_line_offset_px: u32,
    ) {
        unsafe {
            // Foreground: A8 source, 1 pixel per line, contiguous
            self.regs.fgmar.write(|w| w.bits(a8_src as u32));
            self.regs.fgor.write(|w| w.bits(0)); // 1 byte/line, no gap
            self.regs.fgpfccr.write(|w| w.bits(0xFF00_0000 | 9)); // A8, full alpha
            self.regs.fgcolr.write(|w| w.bits(fg_color));

            // Background: ARGB8888 column with large line offset
            self.regs.bgmar.write(|w| w.bits(dst as u32));
            self.regs.bgor.write(|w| w.bits(dst_line_offset_px));
            self.regs.bgpfccr.write(|w| w.bits(0)); // ARGB8888

            // Output: same layout as background (in-place blend)
            self.regs.omar.write(|w| w.bits(dst as u32));
            self.regs.oor.write(|w| w.bits(dst_line_offset_px));
            self.regs.opfccr.write(|w| w.bits(0)); // ARGB8888

            // NLR: 1 pixel wide, pixel_count lines tall
            self.regs.nlr.write(|w| w.bits((1 << 16) | pixel_count));
        }
        self.prepare_start();
        self.write_cr_mode(Self::CR_MODE_M2M_BLEND);
        self.regs
            .cr
            .modify(|r, w| unsafe { w.bits(r.bits() | Self::CR_START) });
    }

    /// Blocking compatibility wrapper for [`Self::start_blend_a8_color`].
    pub fn blend_a8_color(
        &mut self,
        a8_src: *const u8,
        width: u32,
        height: u32,
        fg_color: u32,
        dst: *mut u8,
        dst_stride: u32,
    ) {
        self.start_blend_a8_color(a8_src, width, height, fg_color, dst, dst_stride);
        self.wait();
    }
}

// ── Typed DMA2D submission API ───────────────────────────────────────────
//
// Step 3 of the Register-Mashing Discipline migration. The methods below
// consume a `BorrowedForDma<'_, BackBuffer<'_>>` reborrow and return an
// `InFlight<'_, BackBuffer<'_>>` token, so the borrow checker prevents:
//   * a second DMA submission while one is outstanding (the engine is
//     `&mut`-borrowed for the InFlight lifetime), and
//   * CPU access to the back buffer during the transfer (`back.cpu_slice()`
//     would re-borrow `back` mutably while `BorrowedForDma` already holds it).
//
// They forward to the existing `start_*_raw` methods after extracting the
// destination address and stride from the typed handle. The raw methods
// remain available (now `#[doc(hidden)]`) until Step 9 — overlay paths
// migrate one at a time in Steps 4–8.

#[cfg(feature = "dma2d")]
impl Dma2dBlitter {
    /// Non-blocking R2M fill of `area` (in back-buffer pixels) with
    /// `color`, using the typed framebuffer-ownership handles.
    ///
    /// Returns an [`InFlight`] holding the `BorrowedForDma` reborrow for
    /// the duration of the transfer. Completion is observed via the
    /// usual [`Self::is_in_flight`] / [`Self::poll_complete`] /
    /// [`Self::ack_complete`] sequence; consume the token via
    /// [`InFlight::into_borrow`] and drop the borrow once the transfer
    /// has acked.
    pub fn start_fill_typed<'b, 'fb>(
        &'b mut self,
        mut dst: BorrowedForDma<'b, BackBuffer<'fb>>,
        area: Rect,
        color: u32,
    ) -> InFlight<'b, BackBuffer<'fb>> {
        let dst_stride = dst.stride_bytes();
        let fmt = dst.format();
        let bpp = Self::pixel_size(fmt);
        let dst_base = dst.dma_addr().raw() as *mut u8;
        // SAFETY: `BorrowedForDma` carries a `&mut BackBuffer` reborrow
        // so the destination region is not aliased by any CPU-visible
        // handle for the lifetime of the returned `InFlight`. The
        // pointer arithmetic stays within the back buffer the borrow
        // refers to (caller-supplied `area.x` / `area.y` are assumed
        // non-negative and within bounds; out-of-bounds origins are a
        // caller bug, not unsafety in this layer). Alignment was
        // validated by `BackBuffer::dma_addr()` via `DmaAddr::from_phys`.
        let dst_ptr =
            unsafe { dst_base.add(area.y as usize * dst_stride as usize + area.x as usize * bpp) };
        let _ = &mut dst; // bind the reborrow into this scope explicitly
        self.fill_engine_raw(dst_ptr, dst_stride, area.w, area.h, color, fmt);
        InFlight::new(dst)
    }

    /// Non-blocking M2M copy of `width × height` pixels from `src` (raw
    /// pointer) to the typed back buffer at offset `dst_pos`.
    ///
    /// `src` is intentionally still raw because Step 3 only types the
    /// destination side; sources backed by SDRAM textures and font glyphs
    /// migrate to typed `Texture<'a>` / `GlyphAtlas<'a>` handles in a
    /// follow-up step. The destination's borrow contract is enforced as
    /// for [`Self::start_fill_typed`].
    pub fn start_blit_typed<'b, 'fb>(
        &'b mut self,
        src: *const u8,
        src_stride: u32,
        mut dst: BorrowedForDma<'b, BackBuffer<'fb>>,
        dst_pos: (i32, i32),
        width: u32,
        height: u32,
    ) -> InFlight<'b, BackBuffer<'fb>> {
        let dst_stride = dst.stride_bytes();
        let fmt = dst.format();
        let bpp = Self::pixel_size(fmt);
        let dst_base = dst.dma_addr().raw() as *mut u8;
        // SAFETY: dst_base + offset stays within the back buffer the
        // BorrowedForDma reborrow refers to (caller-supplied dst_pos is
        // assumed to be inside the buffer; out-of-bounds offsets are a
        // caller bug, not unsafety in this layer).
        let dst_ptr = unsafe {
            dst_base.add(dst_pos.1 as usize * dst_stride as usize + dst_pos.0 as usize * bpp)
        };
        let _ = &mut dst;
        self.blit_engine_raw(src, src_stride, dst_ptr, dst_stride, width, height, fmt);
        InFlight::new(dst)
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
