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
//!           (SHORT_REPEAT_MAX + 1 + next_byte) pixels.
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
                if idx >= palette_rgb565.len() { return Err(Error::Truncated); }
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
                    if idx >= palette_rgb565.len() { return Err(Error::Truncated); }
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
pub fn encode_rgba(
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<(Vec<u16>, Vec<u8>), Error> {
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
    let mut palette: Vec<u16> = pairs.iter().take(take).map(|p| p.0).collect();
    if palette.len() > MAX_PALETTE { return Err(Error::PaletteTooLarge); }
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
    let flush_run = |palette: &Vec<u16>, out: &mut Vec<u8>, base: u8, cur_idx: Option<u8>, run_color: Option<u16>, run_len: usize| {
        let mut emitted = 0usize;
        if run_len == 0 { return 0usize; }
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
                (None, 0, _) => { run_color = Some(c); cur_idx = this_idx; run_len = 1; }
                (Some(rc), n, _) if rc == c => { run_len = n + 1; }
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

