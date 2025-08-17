//! CPU-based fallback blitter.
//!
//! Provides a pure software implementation of the [`Blitter`] trait used for
//! testing and as a baseline on platforms lacking acceleration.

use crate::blit::{BlitCaps, Blitter, PixelFmt, Rect, Surface};

/// Blitter that performs all operations on the CPU using scalar loops.
pub struct CpuBlitter;

impl CpuBlitter {
    fn pixel_size(fmt: PixelFmt) -> usize {
        match fmt {
            PixelFmt::Argb8888 => 4,
            PixelFmt::Rgb565 => 2,
            PixelFmt::L8 | PixelFmt::A8 => 1,
            PixelFmt::A4 => 1,
        }
    }

    fn argb8888_to_rgb565(c: u32) -> u16 {
        let r = ((c >> 16) & 0xff) as u16;
        let g = ((c >> 8) & 0xff) as u16;
        let b = (c & 0xff) as u16;
        ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
    }

    fn rgb565_to_argb8888(c: u16) -> u32 {
        let r = ((c >> 11) & 0x1f) as u32;
        let g = ((c >> 5) & 0x3f) as u32;
        let b = (c & 0x1f) as u32;
        0xff00_0000 | ((r << 3) << 16) | ((g << 2) << 8) | (b << 3)
    }

    fn read_pixel(surf: &Surface, x: i32, y: i32) -> u32 {
        let bpp = Self::pixel_size(surf.format);
        let offset = match surf.format {
            PixelFmt::A4 => (y as usize * surf.stride) + (x as usize / 2),
            _ => (y as usize * surf.stride) + (x as usize * bpp),
        };
        match surf.format {
            PixelFmt::Argb8888 => {
                let bytes: [u8; 4] = surf.buf[offset..offset + 4].try_into().unwrap();
                u32::from_le_bytes(bytes)
            }
            PixelFmt::Rgb565 => {
                let bytes: [u8; 2] = surf.buf[offset..offset + 2].try_into().unwrap();
                Self::rgb565_to_argb8888(u16::from_le_bytes(bytes))
            }
            PixelFmt::L8 => {
                let v = surf.buf[offset] as u32;
                0xff00_0000 | (v << 16) | (v << 8) | v
            }
            PixelFmt::A8 => {
                let a = surf.buf[offset] as u32;
                a << 24
            }
            PixelFmt::A4 => {
                let byte = surf.buf[offset];
                let nib = if x & 1 == 0 { byte >> 4 } else { byte & 0x0f } as u32;
                let a = (nib << 4) | nib;
                a << 24
            }
        }
    }

    fn write_pixel(surf: &mut Surface, x: i32, y: i32, color: u32) {
        let bpp = Self::pixel_size(surf.format);
        let offset = match surf.format {
            PixelFmt::A4 => (y as usize * surf.stride) + (x as usize / 2),
            _ => (y as usize * surf.stride) + (x as usize * bpp),
        };
        match surf.format {
            PixelFmt::Argb8888 => {
                surf.buf[offset..offset + 4].copy_from_slice(&color.to_le_bytes());
            }
            PixelFmt::Rgb565 => {
                let c = Self::argb8888_to_rgb565(color);
                surf.buf[offset..offset + 2].copy_from_slice(&c.to_le_bytes());
            }
            PixelFmt::L8 => {
                let r = (color >> 16) & 0xff;
                let g = (color >> 8) & 0xff;
                let b = color & 0xff;
                surf.buf[offset] = ((r + g + b) / 3) as u8;
            }
            PixelFmt::A8 => {
                surf.buf[offset] = (color >> 24) as u8;
            }
            PixelFmt::A4 => {
                let a = (color >> 24) as u8 >> 4;
                let byte = &mut surf.buf[offset];
                if x & 1 == 0 {
                    *byte = (*byte & 0x0f) | (a << 4);
                } else {
                    *byte = (*byte & 0xf0) | a;
                }
            }
        }
    }

    fn blend_pixel(src: u32, dst: u32) -> u32 {
        let sa = (src >> 24) & 0xff;
        let inv = 255 - sa;
        let sr = (src >> 16) & 0xff;
        let sg = (src >> 8) & 0xff;
        let sb = src & 0xff;
        let dr = (dst >> 16) & 0xff;
        let dg = (dst >> 8) & 0xff;
        let db = dst & 0xff;
        let r = (sr * sa + dr * inv) / 255;
        let g = (sg * sa + dg * inv) / 255;
        let b = (sb * sa + db * inv) / 255;
        0xff00_0000 | (r << 16) | (g << 8) | b
    }
}

impl Blitter for CpuBlitter {
    fn caps(&self) -> BlitCaps {
        BlitCaps::FILL | BlitCaps::BLIT | BlitCaps::BLEND | BlitCaps::PFC
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        match dst.format {
            PixelFmt::Argb8888 => {
                for row in 0..area.h as i32 {
                    let start = ((area.y + row) as usize * dst.stride) + (area.x as usize * 4);
                    let line = &mut dst.buf[start..start + area.w as usize * 4];
                    for px in line.chunks_exact_mut(4) {
                        px.copy_from_slice(&color.to_le_bytes());
                    }
                }
            }
            PixelFmt::Rgb565 => {
                let c = Self::argb8888_to_rgb565(color);
                for row in 0..area.h as i32 {
                    let start = ((area.y + row) as usize * dst.stride) + (area.x as usize * 2);
                    let line = &mut dst.buf[start..start + area.w as usize * 2];
                    for px in line.chunks_exact_mut(2) {
                        px.copy_from_slice(&c.to_le_bytes());
                    }
                }
            }
            _ => {
                for y in area.y..area.y + area.h as i32 {
                    for x in area.x..area.x + area.w as i32 {
                        Self::write_pixel(dst, x, y, color);
                    }
                }
            }
        }
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        if src.format == dst.format {
            let bpp = Self::pixel_size(src.format);
            for row in 0..src_area.h as i32 {
                let src_start =
                    ((src_area.y + row) as usize * src.stride) + (src_area.x as usize * bpp);
                let dst_start =
                    ((dst_pos.1 + row) as usize * dst.stride) + (dst_pos.0 as usize * bpp);
                let len = src_area.w as usize * bpp;
                dst.buf[dst_start..dst_start + len]
                    .copy_from_slice(&src.buf[src_start..src_start + len]);
            }
            return;
        }

        if src.format == PixelFmt::Argb8888 && dst.format == PixelFmt::Rgb565 {
            for row in 0..src_area.h as i32 {
                for col in 0..src_area.w as i32 {
                    let px = Self::read_pixel(src, src_area.x + col, src_area.y + row);
                    Self::write_pixel(dst, dst_pos.0 + col, dst_pos.1 + row, px);
                }
            }
            return;
        }

        for row in 0..src_area.h as i32 {
            for col in 0..src_area.w as i32 {
                let px = Self::read_pixel(src, src_area.x + col, src_area.y + row);
                Self::write_pixel(dst, dst_pos.0 + col, dst_pos.1 + row, px);
            }
        }
    }

    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        for row in 0..src_area.h as i32 {
            for col in 0..src_area.w as i32 {
                let s = Self::read_pixel(src, src_area.x + col, src_area.y + row);
                let d = Self::read_pixel(dst, dst_pos.0 + col, dst_pos.1 + row);
                let out = Self::blend_pixel(s, d);
                Self::write_pixel(dst, dst_pos.0 + col, dst_pos.1 + row, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heapless::Vec;

    #[test]
    fn fill_argb8888() {
        let mut buf = [0u8; 64];
        let mut surf = Surface::new(&mut buf, 16, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        blit.fill(
            &mut surf,
            Rect {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
            0x11223344,
        );
        for px in buf.chunks_exact(4) {
            assert_eq!(u32::from_le_bytes(px.try_into().unwrap()), 0x11223344);
        }
    }

    #[test]
    fn blit_argb8888_to_rgb565() {
        let mut src_buf = [0u8; 16];
        let mut dst_buf = [0u8; 32];
        let src_colors = [0xff0000ffu32, 0xff00ff00, 0xffff0000, 0xffffffff];
        for (i, chunk) in src_buf.chunks_exact_mut(4).enumerate() {
            chunk.copy_from_slice(&src_colors[i].to_le_bytes());
        }
        let src = Surface::new(&mut src_buf, 8, PixelFmt::Argb8888, 2, 2);
        let mut dst = Surface::new(&mut dst_buf, 8, PixelFmt::Rgb565, 4, 4);
        let mut blit = CpuBlitter;
        blit.blit(
            &src,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
            &mut dst,
            (1, 1),
        );
        let expected: Vec<u16, 4> = Vec::from_slice(&[
            CpuBlitter::argb8888_to_rgb565(0xff0000ff),
            CpuBlitter::argb8888_to_rgb565(0xff00ff00),
            CpuBlitter::argb8888_to_rgb565(0xffff0000),
            CpuBlitter::argb8888_to_rgb565(0xffffffff),
        ])
        .unwrap();
        for (i, row) in (1..3).enumerate() {
            for (j, col) in (1..3).enumerate() {
                let idx = row * 8 + col * 2;
                let val = u16::from_le_bytes([dst_buf[idx], dst_buf[idx + 1]]);
                assert_eq!(val, expected[i * 2 + j]);
            }
        }
    }

    #[test]
    fn blend_argb8888() {
        let mut src_buf = [0u8; 16];
        let mut dst_buf = [0u8; 16];
        for chunk in src_buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&0x80ff0000u32.to_le_bytes());
        }
        for chunk in dst_buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&0xff000000u32.to_le_bytes());
        }
        let src = Surface::new(&mut src_buf, 8, PixelFmt::Argb8888, 2, 2);
        let mut dst = Surface::new(&mut dst_buf, 8, PixelFmt::Argb8888, 2, 2);
        let mut blit = CpuBlitter;
        blit.blend(
            &src,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            },
            &mut dst,
            (0, 0),
        );
        for chunk in dst_buf.chunks_exact(4) {
            assert_eq!(u32::from_le_bytes(chunk.try_into().unwrap()), 0xff800000);
        }
    }
}
