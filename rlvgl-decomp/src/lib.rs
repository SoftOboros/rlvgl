//! rlvgl-decomp: Compact RLE codec for RGBA frames.
//!
//! This module defines a palette + run-length encoding with short and long
//! repeats and inline-pixel escapes. Pixels are encoded as RGB565 in the
//! palette or inline; the external API converts to/from RGBA8888.
//!
//! Format (byte stream):
//! - Control keys:
//!   - 0xFF: Single inline pixel. Next two bytes are RGB565; emit once.
//!   - 0xFE: Double inline pixel. Next two bytes are RGB565; emit twice.
//!   - 0xFD: Long repeat. Repeats the most recent palette index color for
//!     (SHORT_REPEAT_MAX + 1 + next_byte) pixels.
//! - Data bytes:
//!   - 0..(palette_len-1): palette index; emit once and update recent index.
//!   - palette_len..(palette_len + SHORT_REPEAT_MAX): short repeat; emit the
//!     recent palette index color (data - palette_len + 1) times.
//!
//! Encoder builds a palette (up to MAX_PALETTE) from the image's RGB565
//! histogram and emits the above byte stream.

#![no_std]

extern crate alloc;
use alloc::vec::Vec;

/// Encoding constants
pub mod consts {
    pub const ENCODE_KEY_SINGLE_INLINE_PIXEL: u8 = 0xFF;
    pub const ENCODE_KEY_DOUBLE_INLINE_PIXEL: u8 = 0xFE;
    pub const ENCODE_KEY_LONG_REPEAT: u8 = 0xFD;
    pub const SHORT_REPEAT_MAX: u8 = 60; // 1..=60
    pub const LONG_REPEAT_MIN: u16 = (SHORT_REPEAT_MAX as u16) + 1; // 61
    pub const LONG_REPEAT_MAX: u16 = 316;
    pub const MAX_PALETTE: usize = 192; // ensure short-repeat range stays < 0xFD
    /// Magic bytes for the RLEC binary blob format.
    pub const BLOB_MAGIC: [u8; 4] = *b"RLEC";
    /// Header size: magic(4) + width(2) + height(2) + palette_len(2) = 10 bytes.
    pub const BLOB_HEADER_SIZE: usize = 10;
}

/// Error type for (de)compression issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Output buffer size mismatch vs. `width*height`
    SizeMismatch,
    /// Input stream terminated prematurely
    Truncated,
    /// Palette too large for encoding constraints
    PaletteTooLarge,
    /// Invalid or missing blob magic
    BadMagic,
}

/// Memory orientation for decoding a compressed asset into a destination
/// framebuffer.
///
/// The decoder walks the encoded stream in source raster order
/// (left→right, top→bottom). `Orientation` selects the mapping from
/// source pixel coordinates `(sx, sy)` to destination pixel coordinates
/// `(dx, dy)`, so the decoded image lands in the destination buffer in
/// the orientation the platform's scan-out reads.
///
/// This exists so each board's BSP code can choose the splash/asset
/// memory layout that matches its render pipeline without relying on
/// a separate post-decode rotation pass:
///
/// * STM32H747I-DISCO video mode scans a landscape 800×480 FB; the 480×800
///   portrait splash decodes with `Rot90Ccw`.
/// * STM32H747I-DISCO adapted-cmd mode scans a portrait 480×800 FB; the
///   same splash decodes with `Identity`.
/// * BeagleBone Black + NHD-7 cape is a native landscape 800×480 panel;
///   the portrait splash decodes with `Rot90Ccw` so its orientation
///   agrees with the landscape widget tree that paints on top.
///
/// The destination buffer's dimensions must match the chosen orientation:
/// for 90° rotations `(dst_w, dst_h) == (src_h, src_w)`; for identity /
/// 180° they match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Source pixels written in raster order, dst dims = src dims.
    Identity,
    /// 90° clockwise. Source `(sx, sy)` → dst `(sh-1-sy, sx)`. Dst dims = `(sh, sw)`.
    Rot90Cw,
    /// 90° counter-clockwise. Source `(sx, sy)` → dst `(sy, sw-1-sx)`. Dst dims = `(sh, sw)`.
    Rot90Ccw,
    /// 180°. Source `(sx, sy)` → dst `(sw-1-sx, sh-1-sy)`. Dst dims = src dims.
    Rot180,
}

fn rgb565_to_rgba(c: u16) -> [u8; 4] {
    let r5 = ((c >> 11) & 0x1F) as u8;
    let g6 = ((c >> 5) & 0x3F) as u8;
    let b5 = (c & 0x1F) as u8;
    let r = (r5 << 3) | (r5 >> 2);
    let g = (g6 << 2) | (g6 >> 4);
    let b = (b5 << 3) | (b5 >> 2);
    [r, g, b, 0xFF]
}

/// Convert RGB565 to a native-endian ARGB8888 u32 for LTDC.
fn rgb565_to_argb_u32(c: u16) -> u32 {
    let r5 = ((c >> 11) & 0x1F) as u32;
    let g6 = ((c >> 5) & 0x3F) as u32;
    let b5 = (c & 0x1F) as u32;
    let r = (r5 << 3) | (r5 >> 2);
    let g = (g6 << 2) | (g6 >> 4);
    let b = (b5 << 3) | (b5 >> 2);
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

fn rgba_to_rgb565(px: &[u8]) -> u16 {
    let r = px[0] as u16;
    let g = px[1] as u16;
    let b = px[2] as u16;
    ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
}

/// Decode a frame from palette + byte stream into RGBA8888.
pub fn decode_rgba(
    width: usize,
    height: usize,
    palette_rgb565: &[u16],
    stream: &[u8],
) -> Result<Vec<u8>, Error> {
    use consts::*;
    let mut out = Vec::with_capacity(width * height * 4);
    let mut recent_idx: u8 = 0;
    let mut i = 0;
    while i < stream.len() && out.len() < width * height * 4 {
        let b = stream[i];
        i += 1;
        match b {
            ENCODE_KEY_SINGLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let hi = stream[i] as u16;
                let lo = stream[i + 1] as u16;
                i += 2;
                let c = (hi << 8) | lo;
                out.extend_from_slice(&rgb565_to_rgba(c));
            }
            ENCODE_KEY_DOUBLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let hi = stream[i] as u16;
                let lo = stream[i + 1] as u16;
                i += 2;
                let c = (hi << 8) | lo;
                let px = rgb565_to_rgba(c);
                out.extend_from_slice(&px);
                out.extend_from_slice(&px);
            }
            ENCODE_KEY_LONG_REPEAT => {
                if i >= stream.len() {
                    return Err(Error::Truncated);
                }
                let add = stream[i] as u16;
                i += 1;
                let mut count = (SHORT_REPEAT_MAX as u16 + 1) + add; // 61..316
                let idx = recent_idx as usize;
                if idx >= palette_rgb565.len() {
                    return Err(Error::Truncated);
                }
                let px = rgb565_to_rgba(palette_rgb565[idx]);
                while count > 0 {
                    out.extend_from_slice(&px);
                    count -= 1;
                }
            }
            data => {
                if (data as usize) < palette_rgb565.len() {
                    recent_idx = data;
                    let c = palette_rgb565[data as usize];
                    out.extend_from_slice(&rgb565_to_rgba(c));
                } else {
                    // short repeat: repeat recent_idx for (data - palette_len + 1)
                    let base = palette_rgb565.len() as u8;
                    let mut count = (data.saturating_sub(base)).saturating_add(1);
                    let idx = recent_idx as usize;
                    if idx >= palette_rgb565.len() {
                        return Err(Error::Truncated);
                    }
                    let px = rgb565_to_rgba(palette_rgb565[idx]);
                    while count > 0 {
                        out.extend_from_slice(&px);
                        count -= 1;
                    }
                }
            }
        }
    }
    if out.len() != width * height * 4 {
        return Err(Error::SizeMismatch);
    }
    Ok(out)
}

/// Encode an RGBA frame into (palette RGB565, byte stream) using short and long repeats.
pub fn encode_rgba(width: usize, height: usize, rgba: &[u8]) -> Result<(Vec<u16>, Vec<u8>), Error> {
    use consts::*;
    // Build RGB565 histogram
    use alloc::collections::BTreeMap;
    let mut hist: BTreeMap<u16, u32> = BTreeMap::new();
    for y in 0..height {
        let row = y * width * 4;
        for x in 0..width {
            let px = &rgba[row + x * 4..row + x * 4 + 4];
            let c = rgba_to_rgb565(px);
            *hist.entry(c).or_insert(0) += 1;
        }
    }
    // Pick top MAX_PALETTE colors
    let mut pairs: Vec<(u16, u32)> = hist.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let take = core::cmp::min(pairs.len(), MAX_PALETTE);
    let palette: Vec<u16> = pairs.iter().take(take).map(|p| p.0).collect();
    if palette.len() > MAX_PALETTE {
        return Err(Error::PaletteTooLarge);
    }
    // Map color to palette index
    use alloc::collections::BTreeMap as Map;
    let mut lut: Map<u16, u8> = Map::new();
    for (i, &c) in palette.iter().enumerate() {
        lut.insert(c, i as u8);
    }
    let base = palette.len() as u8;
    let mut out: Vec<u8> = Vec::new();

    // Walk image and emit runs
    let mut cur_idx: Option<u8> = None;
    let mut run_color: Option<u16> = None;
    let mut run_len: usize = 0;
    let flush_run = |_palette: &Vec<u16>,
                     out: &mut Vec<u8>,
                     base: u8,
                     cur_idx: Option<u8>,
                     run_color: Option<u16>,
                     run_len: usize| {
        let mut emitted = 0usize;
        if run_len == 0 {
            return 0usize;
        }
        if let Some(idx) = cur_idx {
            // Emit palette index first (one pixel)
            out.push(idx);
            emitted += 1;
            let mut remain = run_len.saturating_sub(1);
            // Use long repeats for large runs (61..316)
            while remain >= (LONG_REPEAT_MIN as usize) {
                let chunk = core::cmp::min(remain, consts::LONG_REPEAT_MAX as usize);
                out.push(ENCODE_KEY_LONG_REPEAT);
                out.push((chunk as u16 - LONG_REPEAT_MIN) as u8);
                remain -= chunk;
                emitted += chunk;
            }
            // Use short repeats for up to 60
            while remain > 0 {
                let chunk = core::cmp::min(remain, consts::SHORT_REPEAT_MAX as usize);
                out.push(base + (chunk as u8 - 1));
                remain -= chunk;
                emitted += chunk;
            }
        } else if let Some(raw) = run_color {
            // Use inline pixels for colors not in palette
            let mut remain = run_len;
            while remain >= 2 {
                out.push(ENCODE_KEY_DOUBLE_INLINE_PIXEL);
                out.push((raw >> 8) as u8);
                out.push((raw & 0xFF) as u8);
                remain -= 2;
                emitted += 2;
            }
            if remain == 1 {
                out.push(ENCODE_KEY_SINGLE_INLINE_PIXEL);
                out.push((raw >> 8) as u8);
                out.push((raw & 0xFF) as u8);
                emitted += 1;
            }
        }
        emitted
    };

    for y in 0..height {
        let row = y * width * 4;
        for x in 0..width {
            let c = rgba_to_rgb565(&rgba[row + x * 4..row + x * 4 + 4]);
            let this_idx = lut.get(&c).copied();
            match (run_color, run_len, this_idx) {
                (None, 0, _) => {
                    run_color = Some(c);
                    cur_idx = this_idx;
                    run_len = 1;
                }
                (Some(rc), n, _) if rc == c => {
                    run_len = n + 1;
                }
                _ => {
                    // flush previous run
                    let _ = flush_run(&palette, &mut out, base, cur_idx, run_color, run_len);
                    run_color = Some(c);
                    cur_idx = this_idx;
                    run_len = 1;
                }
            }
        }
    }
    // Flush tail
    let _ = flush_run(&palette, &mut out, base, cur_idx, run_color, run_len);

    Ok((palette, out))
}

/// Write an RLEC binary blob: magic + header + palette (LE u16s) + stream.
///
/// This is the on-disk format consumed by `parse_rle_blob`.
pub fn write_rle_blob(width: u16, height: u16, palette: &[u16], stream: &[u8], out: &mut Vec<u8>) {
    use consts::BLOB_MAGIC;
    out.extend_from_slice(&BLOB_MAGIC);
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&(palette.len() as u16).to_le_bytes());
    for &c in palette {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend_from_slice(stream);
}

/// Zero-copy view of a parsed RLEC blob.
///
/// The tuple layout is `(width, height, palette_bytes, stream)`.
pub type ParsedRleBlob<'a> = (u16, u16, &'a [u8], &'a [u8]);

/// Parse an RLEC binary blob into (width, height, palette_bytes, stream).
///
/// Returns raw palette bytes (pairs of LE u16) and the RLE stream slice.
/// Both are zero-copy references into `data`. The caller must read palette
/// entries as `u16::from_le_bytes` since alignment is not guaranteed.
pub fn parse_rle_blob(data: &[u8]) -> Result<ParsedRleBlob<'_>, Error> {
    use consts::{BLOB_HEADER_SIZE, BLOB_MAGIC};
    if data.len() < BLOB_HEADER_SIZE {
        return Err(Error::Truncated);
    }
    if data[0..4] != BLOB_MAGIC {
        return Err(Error::BadMagic);
    }
    let width = u16::from_le_bytes([data[4], data[5]]);
    let height = u16::from_le_bytes([data[6], data[7]]);
    let palette_len = u16::from_le_bytes([data[8], data[9]]) as usize;
    let pal_bytes = palette_len * 2;
    let pal_end = BLOB_HEADER_SIZE + pal_bytes;
    if data.len() < pal_end + 4 {
        return Err(Error::Truncated);
    }
    let stream_len = u32::from_le_bytes([
        data[pal_end],
        data[pal_end + 1],
        data[pal_end + 2],
        data[pal_end + 3],
    ]) as usize;
    let stream_start = pal_end + 4;
    if data.len() < stream_start + stream_len {
        return Err(Error::Truncated);
    }
    Ok((
        width,
        height,
        &data[BLOB_HEADER_SIZE..pal_end],
        &data[stream_start..stream_start + stream_len],
    ))
}

/// Decode an RLE stream directly into a pre-allocated ARGB8888 buffer.
///
/// Writes pixels as native-endian u32 values (`0xFF_RR_GG_BB`) matching
/// STM32 LTDC ARGB8888 format. The output buffer must be exactly
/// `width * height * 4` bytes and 4-byte aligned (SDRAM framebuffer).
///
/// No heap allocation is performed — suitable for `no_std` without `alloc`.
pub fn decode_argb_into(
    width: usize,
    height: usize,
    palette_rgb565: &[u16],
    stream: &[u8],
    out: &mut [u8],
) -> Result<(), Error> {
    use consts::*;
    let total = width * height;
    if out.len() < total * 4 {
        return Err(Error::SizeMismatch);
    }
    // Precompute ARGB u32 palette for fast lookup
    let mut pal_argb = [0u32; MAX_PALETTE];
    for (i, &c) in palette_rgb565.iter().enumerate() {
        pal_argb[i] = rgb565_to_argb_u32(c);
    }

    let mut pos: usize = 0; // pixel position
    let mut recent_idx: u8 = 0;
    let mut i = 0;

    // Capture the raw base pointer once, outside the loop.  All writes
    // go through this pointer so the compiler cannot assume the SDRAM
    // region is unreachable.
    let base_ptr = out.as_mut_ptr() as *mut u32;

    while i < stream.len() && pos < total {
        let b = stream[i];
        i += 1;
        match b {
            ENCODE_KEY_SINGLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let c = ((stream[i] as u16) << 8) | (stream[i + 1] as u16);
                i += 2;
                unsafe { base_ptr.add(pos).write_volatile(rgb565_to_argb_u32(c)) };
                pos += 1;
            }
            ENCODE_KEY_DOUBLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let c = ((stream[i] as u16) << 8) | (stream[i + 1] as u16);
                i += 2;
                let argb = rgb565_to_argb_u32(c);
                unsafe { base_ptr.add(pos).write_volatile(argb) };
                pos += 1;
                if pos < total {
                    unsafe { base_ptr.add(pos).write_volatile(argb) };
                    pos += 1;
                }
            }
            ENCODE_KEY_LONG_REPEAT => {
                if i >= stream.len() {
                    return Err(Error::Truncated);
                }
                let add = stream[i] as usize;
                i += 1;
                let count = (SHORT_REPEAT_MAX as usize + 1) + add;
                let idx = recent_idx as usize;
                if idx >= palette_rgb565.len() {
                    return Err(Error::Truncated);
                }
                let argb = pal_argb[idx];
                for _ in 0..count {
                    if pos >= total {
                        break;
                    }
                    unsafe { base_ptr.add(pos).write_volatile(argb) };
                    pos += 1;
                }
            }
            data => {
                if (data as usize) < palette_rgb565.len() {
                    recent_idx = data;
                    unsafe { base_ptr.add(pos).write_volatile(pal_argb[data as usize]) };
                    pos += 1;
                } else {
                    let base = palette_rgb565.len() as u8;
                    let count = (data.saturating_sub(base)).saturating_add(1) as usize;
                    let idx = recent_idx as usize;
                    if idx >= palette_rgb565.len() {
                        return Err(Error::Truncated);
                    }
                    let argb = pal_argb[idx];
                    for _ in 0..count {
                        if pos >= total {
                            break;
                        }
                        unsafe { base_ptr.add(pos).write_volatile(argb) };
                        pos += 1;
                    }
                }
            }
        }
    }
    if pos != total {
        return Err(Error::SizeMismatch);
    }
    Ok(())
}

/// Decode RLE ARGB8888 pixels into a destination buffer with a specified
/// memory [`Orientation`].
///
/// Source dimensions `(src_w, src_h)` describe the blob's intrinsic size.
/// Destination dimensions `(dst_w, dst_h)` describe the buffer layout the
/// platform's scan-out reads. The decoder iterates the compressed stream
/// in source raster order and computes each pixel's destination slot from
/// [`Orientation`], so the landing memory matches the platform's native
/// read pattern.
///
/// Expected dimensions by orientation:
/// * [`Orientation::Identity`] and [`Orientation::Rot180`]: `(dst_w, dst_h) == (src_w, src_h)`.
/// * [`Orientation::Rot90Cw`] and [`Orientation::Rot90Ccw`]: `(dst_w, dst_h) == (src_h, src_w)`.
///
/// No heap allocation is performed — suitable for `no_std`.
pub fn decode_argb_into_rotated(
    src_w: usize,
    src_h: usize,
    palette_rgb565: &[u16],
    stream: &[u8],
    out: &mut [u8],
    dst_w: usize,
    dst_h: usize,
    orientation: Orientation,
) -> Result<(), Error> {
    use consts::*;

    // Validate dst dims match the chosen orientation.
    let dims_ok = match orientation {
        Orientation::Identity | Orientation::Rot180 => dst_w == src_w && dst_h == src_h,
        Orientation::Rot90Cw | Orientation::Rot90Ccw => dst_w == src_h && dst_h == src_w,
    };
    if !dims_ok {
        return Err(Error::SizeMismatch);
    }

    let total = src_w * src_h;
    if out.len() < dst_w * dst_h * 4 {
        return Err(Error::SizeMismatch);
    }

    // Precompute ARGB u32 palette for fast lookup.
    let mut pal_argb = [0u32; MAX_PALETTE];
    for (i, &c) in palette_rgb565.iter().enumerate() {
        pal_argb[i] = rgb565_to_argb_u32(c);
    }

    let base_ptr = out.as_mut_ptr() as *mut u32;

    // Inline helper: source linear pos → destination linear pos under
    // the chosen orientation. Kept inline so the compiler can hoist the
    // orientation branch out of the hot loop.
    let to_dst = |pos: usize| -> usize {
        let sx = pos % src_w;
        let sy = pos / src_w;
        let (dx, dy) = match orientation {
            Orientation::Identity => (sx, sy),
            Orientation::Rot90Cw => (src_h - 1 - sy, sx),
            Orientation::Rot90Ccw => (sy, src_w - 1 - sx),
            Orientation::Rot180 => (src_w - 1 - sx, src_h - 1 - sy),
        };
        dy * dst_w + dx
    };

    let write_one = |pos: usize, argb: u32| unsafe {
        base_ptr.add(to_dst(pos)).write_volatile(argb);
    };

    let mut pos: usize = 0;
    let mut recent_idx: u8 = 0;
    let mut i = 0;

    while i < stream.len() && pos < total {
        let b = stream[i];
        i += 1;
        match b {
            ENCODE_KEY_SINGLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let c = ((stream[i] as u16) << 8) | (stream[i + 1] as u16);
                i += 2;
                write_one(pos, rgb565_to_argb_u32(c));
                pos += 1;
            }
            ENCODE_KEY_DOUBLE_INLINE_PIXEL => {
                if i + 1 >= stream.len() {
                    return Err(Error::Truncated);
                }
                let c = ((stream[i] as u16) << 8) | (stream[i + 1] as u16);
                i += 2;
                let argb = rgb565_to_argb_u32(c);
                write_one(pos, argb);
                pos += 1;
                if pos < total {
                    write_one(pos, argb);
                    pos += 1;
                }
            }
            ENCODE_KEY_LONG_REPEAT => {
                if i >= stream.len() {
                    return Err(Error::Truncated);
                }
                let add = stream[i] as usize;
                i += 1;
                let count = (SHORT_REPEAT_MAX as usize + 1) + add;
                let idx = recent_idx as usize;
                if idx >= palette_rgb565.len() {
                    return Err(Error::Truncated);
                }
                let argb = pal_argb[idx];
                for _ in 0..count {
                    if pos >= total {
                        break;
                    }
                    write_one(pos, argb);
                    pos += 1;
                }
            }
            data => {
                if (data as usize) < palette_rgb565.len() {
                    recent_idx = data;
                    write_one(pos, pal_argb[data as usize]);
                    pos += 1;
                } else {
                    let base = palette_rgb565.len() as u8;
                    let count = (data.saturating_sub(base)).saturating_add(1) as usize;
                    let idx = recent_idx as usize;
                    if idx >= palette_rgb565.len() {
                        return Err(Error::Truncated);
                    }
                    let argb = pal_argb[idx];
                    for _ in 0..count {
                        if pos >= total {
                            break;
                        }
                        write_one(pos, argb);
                        pos += 1;
                    }
                }
            }
        }
    }
    if pos != total {
        return Err(Error::SizeMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn round_trip_rgba_to_argb() {
        // 4x2 image: 2 colors, red and blue
        let w = 4;
        let h = 2;
        let mut rgba = vec![0u8; w * h * 4];
        // Row 0: red pixels
        for x in 0..w {
            let off = x * 4;
            rgba[off] = 0xFF; // R
            rgba[off + 1] = 0x00; // G
            rgba[off + 2] = 0x00; // B
            rgba[off + 3] = 0xFF; // A
        }
        // Row 1: blue pixels
        for x in 0..w {
            let off = (w + x) * 4;
            rgba[off] = 0x00;
            rgba[off + 1] = 0x00;
            rgba[off + 2] = 0xFF;
            rgba[off + 3] = 0xFF;
        }

        let (palette, stream) = encode_rgba(w, h, &rgba).unwrap();

        // Write blob and parse it back
        let mut blob = Vec::new();
        write_rle_blob(w as u16, h as u16, &palette, &stream, &mut blob);
        let (bw, bh, pal_bytes, bstream) = parse_rle_blob(&blob).unwrap();
        assert_eq!(bw, w as u16);
        assert_eq!(bh, h as u16);

        // Reconstruct palette from bytes
        let pal_count = pal_bytes.len() / 2;
        let mut pal: Vec<u16> = Vec::with_capacity(pal_count);
        for i in 0..pal_count {
            pal.push(u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]));
        }

        // Decode into ARGB buffer
        let mut argb_buf = vec![0u8; w * h * 4];
        decode_argb_into(w, h, &pal, bstream, &mut argb_buf).unwrap();

        // Verify red pixels (row 0) — ARGB u32 on little-endian = bytes [B, G, R, A]
        for x in 0..w {
            let off = x * 4;
            let pixel = u32::from_ne_bytes([
                argb_buf[off],
                argb_buf[off + 1],
                argb_buf[off + 2],
                argb_buf[off + 3],
            ]);
            let a = (pixel >> 24) & 0xFF;
            let r = (pixel >> 16) & 0xFF;
            // Red channel: 0xFF -> RGB565 -> back = 0xFF (5-bit: 31, expanded: 31<<3|31>>2 = 255)
            assert_eq!(a, 0xFF);
            assert!(r >= 0xF8, "red channel too low: {:#X}", r);
        }
        // Verify blue pixels (row 1)
        for x in 0..w {
            let off = (w + x) * 4;
            let pixel = u32::from_ne_bytes([
                argb_buf[off],
                argb_buf[off + 1],
                argb_buf[off + 2],
                argb_buf[off + 3],
            ]);
            let a = (pixel >> 24) & 0xFF;
            let b = pixel & 0xFF;
            assert_eq!(a, 0xFF);
            assert!(b >= 0xF8, "blue channel too low: {:#X}", b);
        }
    }

    #[test]
    fn parse_blob_bad_magic() {
        let data = b"NOPE\x00\x00\x00\x00\x00\x00";
        assert_eq!(parse_rle_blob(data), Err(Error::BadMagic));
    }

    #[test]
    fn parse_blob_truncated() {
        let data = b"RLEC\x01";
        assert_eq!(parse_rle_blob(data), Err(Error::Truncated));
    }

    /// Build an RGBA image with four distinctly-colored corner pixels so
    /// orientation-aware decoding can be verified by reading those corners
    /// out of the rotated destination buffer.
    fn corner_marker_image(w: usize, h: usize) -> Vec<u8> {
        let mut rgba = vec![0u8; w * h * 4];
        let set = |rgba: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8| {
            let off = (y * w + x) * 4;
            rgba[off] = r;
            rgba[off + 1] = g;
            rgba[off + 2] = b;
            rgba[off + 3] = 0xFF;
        };
        // Fill background gray (so RLE produces runs).
        for y in 0..h {
            for x in 0..w {
                set(&mut rgba, x, y, 0x40, 0x40, 0x40);
            }
        }
        set(&mut rgba, 0, 0, 0xF8, 0x00, 0x00); // TL red (5-bit safe)
        set(&mut rgba, w - 1, 0, 0x00, 0xFC, 0x00); // TR green
        set(&mut rgba, 0, h - 1, 0x00, 0x00, 0xF8); // BL blue
        set(&mut rgba, w - 1, h - 1, 0xF8, 0xFC, 0x00); // BR yellow
        rgba
    }

    fn pixel_at(buf: &[u8], w: usize, x: usize, y: usize) -> u32 {
        let off = (y * w + x) * 4;
        u32::from_ne_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
    }

    /// Helper: decode the marker image with a given orientation and check
    /// that each source corner lands at the expected destination corner.
    fn check_orientation(
        orientation: Orientation,
        src_corner: (usize, usize),
        dst_corner: (usize, usize),
        src_w: usize,
        src_h: usize,
    ) {
        let rgba = corner_marker_image(src_w, src_h);
        let (palette, stream) = encode_rgba(src_w, src_h, &rgba).unwrap();
        let mut blob = Vec::new();
        write_rle_blob(src_w as u16, src_h as u16, &palette, &stream, &mut blob);
        let (_, _, pal_bytes, bstream) = parse_rle_blob(&blob).unwrap();
        let pal_count = pal_bytes.len() / 2;
        let mut pal: Vec<u16> = Vec::with_capacity(pal_count);
        for i in 0..pal_count {
            pal.push(u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]));
        }

        let (dst_w, dst_h) = match orientation {
            Orientation::Identity | Orientation::Rot180 => (src_w, src_h),
            Orientation::Rot90Cw | Orientation::Rot90Ccw => (src_h, src_w),
        };
        let mut out = vec![0u8; dst_w * dst_h * 4];
        decode_argb_into_rotated(
            src_w,
            src_h,
            &pal,
            bstream,
            &mut out,
            dst_w,
            dst_h,
            orientation,
        )
        .unwrap();

        // Fetch source corner colour from the ORIGINAL rgba, then compare to
        // the pixel at the expected destination corner.
        let src_px = pixel_at(&rgba, src_w, src_corner.0, src_corner.1);
        let src_rgba_r = (src_px >> 0) & 0xFF;
        let src_rgba_g = (src_px >> 8) & 0xFF;
        let src_rgba_b = (src_px >> 16) & 0xFF;
        let dst_px = pixel_at(&out, dst_w, dst_corner.0, dst_corner.1);
        let dst_r = (dst_px >> 16) & 0xFF;
        let dst_g = (dst_px >> 8) & 0xFF;
        let dst_b = dst_px & 0xFF;
        // RGB565 round-trip loses the low bits — compare with tolerance.
        assert!(
            (dst_r as i32 - src_rgba_r as i32).abs() <= 8
                && (dst_g as i32 - src_rgba_g as i32).abs() <= 8
                && (dst_b as i32 - src_rgba_b as i32).abs() <= 8,
            "orientation {:?}: src{:?} RGB({:#X},{:#X},{:#X}) ≠ dst{:?} RGB({:#X},{:#X},{:#X})",
            orientation,
            src_corner,
            src_rgba_r,
            src_rgba_g,
            src_rgba_b,
            dst_corner,
            dst_r,
            dst_g,
            dst_b,
        );
    }

    #[test]
    fn decode_rotated_identity() {
        // src (0,0) → dst (0,0) and each corner stays put.
        check_orientation(Orientation::Identity, (0, 0), (0, 0), 4, 3);
        check_orientation(Orientation::Identity, (3, 0), (3, 0), 4, 3);
        check_orientation(Orientation::Identity, (0, 2), (0, 2), 4, 3);
        check_orientation(Orientation::Identity, (3, 2), (3, 2), 4, 3);
    }

    #[test]
    fn decode_rotated_cw_90() {
        // 90° CW: src (0,0) → dst (sh-1, 0). With sh=3, sw=4:
        //   src TL(0,0) → dst(2, 0)     (top-right of dst)
        //   src TR(3,0) → dst(2, 3)     (bottom-right of dst)
        //   src BL(0,2) → dst(0, 0)     (top-left of dst)
        //   src BR(3,2) → dst(0, 3)     (bottom-left of dst)
        check_orientation(Orientation::Rot90Cw, (0, 0), (2, 0), 4, 3);
        check_orientation(Orientation::Rot90Cw, (3, 0), (2, 3), 4, 3);
        check_orientation(Orientation::Rot90Cw, (0, 2), (0, 0), 4, 3);
        check_orientation(Orientation::Rot90Cw, (3, 2), (0, 3), 4, 3);
    }

    #[test]
    fn decode_rotated_ccw_90() {
        // 90° CCW: src (sx,sy) → dst (sy, sw-1-sx). With sw=4, sh=3:
        //   src TL(0,0) → dst(0, 3)     (bottom-left of dst)
        //   src TR(3,0) → dst(0, 0)     (top-left of dst)
        //   src BL(0,2) → dst(2, 3)     (bottom-right of dst)
        //   src BR(3,2) → dst(2, 0)     (top-right of dst)
        check_orientation(Orientation::Rot90Ccw, (0, 0), (0, 3), 4, 3);
        check_orientation(Orientation::Rot90Ccw, (3, 0), (0, 0), 4, 3);
        check_orientation(Orientation::Rot90Ccw, (0, 2), (2, 3), 4, 3);
        check_orientation(Orientation::Rot90Ccw, (3, 2), (2, 0), 4, 3);
    }

    #[test]
    fn decode_rotated_180() {
        // 180°: each corner flips to the diagonally opposite corner.
        check_orientation(Orientation::Rot180, (0, 0), (3, 2), 4, 3);
        check_orientation(Orientation::Rot180, (3, 0), (0, 2), 4, 3);
        check_orientation(Orientation::Rot180, (0, 2), (3, 0), 4, 3);
        check_orientation(Orientation::Rot180, (3, 2), (0, 0), 4, 3);
    }

    #[test]
    fn decode_rotated_dim_mismatch_rejected() {
        let rgba = corner_marker_image(4, 3);
        let (palette, stream) = encode_rgba(4, 3, &rgba).unwrap();
        let mut blob = Vec::new();
        write_rle_blob(4, 3, &palette, &stream, &mut blob);
        let (_, _, pal_bytes, bstream) = parse_rle_blob(&blob).unwrap();
        let pal_count = pal_bytes.len() / 2;
        let mut pal: Vec<u16> = Vec::with_capacity(pal_count);
        for i in 0..pal_count {
            pal.push(u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]));
        }
        // Rot90Ccw needs dst = (src_h, src_w) = (3, 4). Pass (4, 3) → mismatch.
        let mut bad = vec![0u8; 4 * 3 * 4];
        let res =
            decode_argb_into_rotated(4, 3, &pal, bstream, &mut bad, 4, 3, Orientation::Rot90Ccw);
        assert_eq!(res, Err(Error::SizeMismatch));
    }
}
