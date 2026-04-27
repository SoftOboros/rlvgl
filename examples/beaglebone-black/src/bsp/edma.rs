// SPDX-License-Identifier: MIT
//! AM335x EDMA-backed copy offload for the BBB Linux renderer.
//!
//! The current BBB path still renders with CPU fills and blends, but it
//! now shares the same `Blitter` call shape as the STM32 DMA2D path. This
//! module uses that common shape to offload the "plain copy" subset:
//! same-format rectangular blits between buffers whose virtual ranges are
//! known to correspond to contiguous physical DDR.
//!
//! The implementation is intentionally narrow:
//!
//! - one manual-triggered DMA channel,
//! - AB-synchronized 2D copies only,
//! - queue 0 / TPTC0 only,
//! - CPU fallback for fills, blends, format conversion, tiny copies, and
//!   any surface not backed by a registered physical span.

use core::hint::spin_loop;

use rlvgl_platform::{BlitCaps, Blitter, CpuBlitter, PhysAddr, PixelFmt, Surface, blit::Rect};

use super::am335x::{
    EDMA3CC_CCERRCLR, EDMA3CC_DCHMAP_0, EDMA3CC_DMAQNUM_0, EDMA3CC_DRAE0, EDMA3CC_ECR,
    EDMA3CC_ECRH, EDMA3CC_EECR, EDMA3CC_EECRH, EDMA3CC_EESR, EDMA3CC_EESRH, EDMA3CC_EMCR,
    EDMA3CC_ESR, EDMA3CC_ESRH, EDMA3CC_ICR, EDMA3CC_ICRH, EDMA3CC_IESR, EDMA3CC_IESRH, EDMA3CC_IPR,
    EDMA3CC_IPRH, EDMA3CC_PARAM_BASE, EDMA3CC_PARAM_STRIDE, EDMA3CC_SECR, EDMA3CC_SECRH, reg_read,
    reg_set_bits, reg_write,
};

const DMA_CHANNEL: u32 = 0;
const EVENT_QUEUE: u32 = 0;
const MAX_PHYS_SPANS: usize = 4;
const MIN_DMA_BYTES: usize = 4096;
const MAX_POLL_ITERS: u32 = 2_000_000;

const OPT_TCINTEN: u32 = 1 << 20;
const OPT_TCC_SHIFT: u32 = 12;
const OPT_STATIC: u32 = 1 << 3;
const OPT_SYNCDIM_AB: u32 = 1 << 2;

#[derive(Clone, Copy)]
struct PhysSpan {
    va_start: usize,
    va_end: usize,
    phys_start: PhysAddr,
}

struct EdmaCopyEngine {
    channel: u32,
}

impl EdmaCopyEngine {
    fn init() -> Self {
        unsafe {
            super::prcm::enable_edma();
        }

        let engine = Self {
            channel: DMA_CHANNEL,
        };
        unsafe {
            engine.configure_channel();
        }
        engine
    }

    unsafe fn configure_channel(&self) {
        unsafe {
            let channel = self.channel;
            reg_set_bits(EDMA3CC_DRAE0, 1u32 << channel);
            reg_write(EDMA3CC_DCHMAP_0 + channel * 4, channel << 5);

            let qnum_reg = EDMA3CC_DMAQNUM_0 + (channel / 8) * 4;
            let qnum_shift = (channel % 8) * 4;
            let qnum_mask = 0x7u32 << qnum_shift;
            let qnum = reg_read(qnum_reg);
            reg_write(qnum_reg, (qnum & !qnum_mask) | (EVENT_QUEUE << qnum_shift));

            self.clear_status();
            self.enable_channel();
        }
    }

    unsafe fn clear_status(&self) {
        unsafe {
            self.write_one(EDMA3CC_ECR, EDMA3CC_ECRH);
            self.write_one(EDMA3CC_EECR, EDMA3CC_EECRH);
            self.write_one(EDMA3CC_SECR, EDMA3CC_SECRH);
            self.write_one(EDMA3CC_ICR, EDMA3CC_ICRH);
            reg_write(EDMA3CC_EMCR, 1u32 << self.channel);
            reg_write(EDMA3CC_CCERRCLR, u32::MAX);
        }
    }

    unsafe fn enable_channel(&self) {
        unsafe {
            self.write_one(EDMA3CC_EESR, EDMA3CC_EESRH);
            self.write_one(EDMA3CC_IESR, EDMA3CC_IESRH);
        }
    }

    unsafe fn trigger(&self) {
        unsafe {
            self.write_one(EDMA3CC_ESR, EDMA3CC_ESRH);
        }
    }

    unsafe fn write_one(&self, lo_reg: u32, hi_reg: u32) {
        unsafe {
            if self.channel < 32 {
                reg_write(lo_reg, 1u32 << self.channel);
            } else {
                reg_write(hi_reg, 1u32 << (self.channel - 32));
            }
        }
    }

    fn param_reg(&self, word_index: u32) -> u32 {
        EDMA3CC_PARAM_BASE + self.channel * EDMA3CC_PARAM_STRIDE + word_index * 4
    }

    fn wait_complete(&self) -> bool {
        let (ipr_reg, icr_reg, bit) = if self.channel < 32 {
            (EDMA3CC_IPR, EDMA3CC_ICR, self.channel)
        } else {
            (EDMA3CC_IPRH, EDMA3CC_ICRH, self.channel - 32)
        };
        let mask = 1u32 << bit;
        for _ in 0..MAX_POLL_ITERS {
            let ipr = unsafe { reg_read(ipr_reg) };
            if ipr & mask != 0 {
                unsafe {
                    reg_write(icr_reg, mask);
                }
                return true;
            }
            spin_loop();
        }
        false
    }

    fn copy_2d(
        &mut self,
        src: PhysAddr,
        src_stride: usize,
        dst: PhysAddr,
        dst_stride: usize,
        row_bytes: usize,
        rows: u32,
    ) -> bool {
        if row_bytes == 0 || rows == 0 {
            return true;
        }
        if row_bytes > u16::MAX as usize
            || rows > u16::MAX as u32
            || src_stride > i16::MAX as usize
            || dst_stride > i16::MAX as usize
        {
            return false;
        }

        let opt =
            OPT_TCINTEN | OPT_STATIC | OPT_SYNCDIM_AB | ((self.channel & 0x3f) << OPT_TCC_SHIFT);
        let a_b_cnt = ((rows & 0xffff) << 16) | (row_bytes as u32 & 0xffff);
        let bidx = ((dst_stride as i16 as u16 as u32) << 16) | (src_stride as i16 as u16 as u32);

        unsafe {
            self.clear_status();
            reg_write(self.param_reg(0), opt);
            reg_write(self.param_reg(1), src.raw());
            reg_write(self.param_reg(2), a_b_cnt);
            reg_write(self.param_reg(3), dst.raw());
            reg_write(self.param_reg(4), bidx);
            reg_write(self.param_reg(5), 0x0000_FFFF);
            reg_write(self.param_reg(6), 0);
            reg_write(self.param_reg(7), 1);
            self.enable_channel();
            self.trigger();
        }

        self.wait_complete()
    }
}

/// BBB blitter that offloads large same-format copies through EDMA and
/// falls back to [`CpuBlitter`] for everything else.
pub struct BbbEdmaBlitter {
    dma: EdmaCopyEngine,
    spans: [Option<PhysSpan>; MAX_PHYS_SPANS],
    dma_disabled: bool,
    warned_timeout: bool,
}

impl BbbEdmaBlitter {
    /// Initialize the EDMA-backed copy path.
    pub fn init() -> Self {
        Self {
            dma: EdmaCopyEngine::init(),
            spans: [None; MAX_PHYS_SPANS],
            dma_disabled: false,
            warned_timeout: false,
        }
    }

    /// Register a DMA-visible span so later blits can translate a
    /// surface's virtual address back to a physical DDR address.
    pub fn register_phys_span(&mut self, buf: &mut [u8], phys_start: PhysAddr) {
        if buf.is_empty() {
            return;
        }
        let span = PhysSpan {
            va_start: buf.as_mut_ptr() as usize,
            va_end: (buf.as_mut_ptr() as usize).saturating_add(buf.len()),
            phys_start,
        };

        if let Some(slot) = self
            .spans
            .iter_mut()
            .find(|slot| matches!(slot, Some(existing) if existing.va_start == span.va_start))
        {
            *slot = Some(span);
            return;
        }

        if let Some(slot) = self.spans.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(span);
        }
    }

    fn pixel_size(fmt: PixelFmt) -> Option<usize> {
        match fmt {
            PixelFmt::Argb8888 => Some(4),
            PixelFmt::Rgb565 => Some(2),
            PixelFmt::L8 | PixelFmt::A8 => Some(1),
            PixelFmt::A4 => None,
        }
    }

    fn lookup_phys_region(
        &self,
        surf: &Surface<'_>,
        x: i32,
        y: i32,
        row_bytes: usize,
        rows: u32,
    ) -> Option<PhysAddr> {
        if x < 0 || y < 0 || row_bytes == 0 || rows == 0 {
            return None;
        }
        let bpp = Self::pixel_size(surf.format)?;
        let start_off = (y as usize)
            .checked_mul(surf.stride)?
            .checked_add((x as usize).checked_mul(bpp)?)?;
        let total_span = if rows == 0 {
            0
        } else {
            (rows as usize - 1)
                .checked_mul(surf.stride)?
                .checked_add(row_bytes)?
        };
        if start_off.checked_add(total_span)? > surf.buf.len() {
            return None;
        }

        let region_start = (surf.buf.as_ptr() as usize).checked_add(start_off)?;
        let region_end = region_start.checked_add(total_span)?;
        for span in self.spans.iter().flatten() {
            if region_start >= span.va_start && region_end <= span.va_end {
                let delta = region_start - span.va_start;
                return Some(span.phys_start.offset(delta as u32));
            }
        }
        None
    }

    fn try_dma_blit(
        &mut self,
        src: &Surface<'_>,
        src_area: Rect,
        dst: &mut Surface<'_>,
        dst_pos: (i32, i32),
    ) -> bool {
        if self.dma_disabled {
            return false;
        }
        if src.format != dst.format {
            return false;
        }
        let Some(bpp) = Self::pixel_size(src.format) else {
            return false;
        };

        let clip_x = if dst_pos.0 < 0 { -dst_pos.0 } else { 0 };
        let clip_y = if dst_pos.1 < 0 { -dst_pos.1 } else { 0 };
        let dst_x = dst_pos.0.max(0);
        let dst_y = dst_pos.1.max(0);
        let w = (src_area.w as i32 - clip_x).min(dst.width as i32 - dst_x);
        let h = (src_area.h as i32 - clip_y).min(dst.height as i32 - dst_y);
        if w <= 0 || h <= 0 {
            return true;
        }

        let src_x0 = src_area.x + clip_x;
        let src_y0 = src_area.y + clip_y;
        if src_x0 < 0 || src_y0 < 0 {
            return false;
        }

        let row_bytes = w as usize * bpp;
        let total_bytes = row_bytes.saturating_mul(h as usize);
        if total_bytes < MIN_DMA_BYTES {
            return false;
        }

        let Some(src_phys) = self.lookup_phys_region(src, src_x0, src_y0, row_bytes, h as u32)
        else {
            return false;
        };
        let Some(dst_phys) = self.lookup_phys_region(dst, dst_x, dst_y, row_bytes, h as u32) else {
            return false;
        };

        if self.dma.copy_2d(
            src_phys, src.stride, dst_phys, dst.stride, row_bytes, h as u32,
        ) {
            true
        } else {
            self.dma_disabled = true;
            if !self.warned_timeout {
                self.warned_timeout = true;
                eprintln!("bbb: EDMA copy timed out, falling back to CPU blits");
            }
            false
        }
    }
}

impl Blitter for BbbEdmaBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::BLEND | BlitCaps::PFC
    }

    fn fill(&mut self, dst: &mut Surface<'_>, area: Rect, color: u32) {
        let mut cpu = CpuBlitter;
        cpu.fill(dst, area, color);
    }

    fn blit(
        &mut self,
        src: &Surface<'_>,
        src_area: Rect,
        dst: &mut Surface<'_>,
        dst_pos: (i32, i32),
    ) {
        if self.try_dma_blit(src, src_area, dst, dst_pos) {
            return;
        }
        let mut cpu = CpuBlitter;
        cpu.blit(src, src_area, dst, dst_pos);
    }

    fn blend(
        &mut self,
        src: &Surface<'_>,
        src_area: Rect,
        dst: &mut Surface<'_>,
        dst_pos: (i32, i32),
    ) {
        let mut cpu = CpuBlitter;
        cpu.blend(src, src_area, dst, dst_pos);
    }
}
