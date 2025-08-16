//! Software blitter used by the desktop simulator.
//!
//! `WgpuBlitter` operates on RGBA pixel buffers and implements the
//! [`Blitter`](crate::blit::Blitter) trait so widgets can render through the
//! generic [`BlitterRenderer`](crate::blit::BlitterRenderer) before a GPU path
//! is available.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};

/// Simple blitter that writes pixels directly into a buffer.
pub struct WgpuBlitter;

impl WgpuBlitter {
    /// Create a new blitter instance.
    pub fn new() -> Self {
        Self
    }
}

impl Blitter for WgpuBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::BLEND
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        let r = ((color >> 16) & 0xff) as u8;
        let g = ((color >> 8) & 0xff) as u8;
        let b = (color & 0xff) as u8;
        let a = ((color >> 24) & 0xff) as u8;
        let x0 = area.x.max(0);
        let y0 = area.y.max(0);
        let x1 = (area.x + area.w as i32).min(dst.width as i32);
        let y1 = (area.y + area.h as i32).min(dst.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = y as usize * dst.stride + x as usize * 4;
                dst.buf[idx] = r;
                dst.buf[idx + 1] = g;
                dst.buf[idx + 2] = b;
                dst.buf[idx + 3] = a;
            }
        }
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        if src.format != PixelFmt::Argb8888 || dst.format != PixelFmt::Argb8888 {
            return;
        }
        let bpp = 4usize;
        for row in 0..src_area.h as i32 {
            let sy = src_area.y + row;
            let dy = dst_pos.1 + row;
            if sy < 0 || dy < 0 || sy as u32 >= src.height || dy as u32 >= dst.height {
                continue;
            }
            let src_offset = sy as usize * src.stride + src_area.x.max(0) as usize * bpp;
            let dst_offset = dy as usize * dst.stride + dst_pos.0.max(0) as usize * bpp;
            let width = src_area.w as usize * bpp;
            dst.buf[dst_offset..dst_offset + width]
                .copy_from_slice(&src.buf[src_offset..src_offset + width]);
        }
    }

    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        if src.format != PixelFmt::Argb8888 || dst.format != PixelFmt::Argb8888 {
            return;
        }
        let bpp = 4usize;
        for row in 0..src_area.h as i32 {
            let sy = src_area.y + row;
            let dy = dst_pos.1 + row;
            if sy < 0 || dy < 0 || sy as u32 >= src.height || dy as u32 >= dst.height {
                continue;
            }
            for col in 0..src_area.w as i32 {
                let sx = src_area.x + col;
                let dx = dst_pos.0 + col;
                if sx < 0 || dx < 0 || sx as u32 >= src.width || dx as u32 >= dst.width {
                    continue;
                }
                let s_idx = sy as usize * src.stride + sx as usize * bpp;
                let d_idx = dy as usize * dst.stride + dx as usize * bpp;
                let sr = src.buf[s_idx] as u16;
                let sg = src.buf[s_idx + 1] as u16;
                let sb = src.buf[s_idx + 2] as u16;
                let sa = src.buf[s_idx + 3] as u16;
                let dr = dst.buf[d_idx] as u16;
                let dg = dst.buf[d_idx + 1] as u16;
                let db = dst.buf[d_idx + 2] as u16;
                let da = dst.buf[d_idx + 3] as u16;
                let inv_a = 255 - sa;
                dst.buf[d_idx] = ((sr * sa + dr * inv_a) / 255) as u8;
                dst.buf[d_idx + 1] = ((sg * sa + dg * inv_a) / 255) as u8;
                dst.buf[d_idx + 2] = ((sb * sa + db * inv_a) / 255) as u8;
                dst.buf[d_idx + 3] = da.max(sa) as u8;
            }
        }
    }
}
