//! LVGL v9 binary image (`.bin`) codec.
//!
//! This is a *separate, deliberately interoperable* format from the crate's
//! native `RLEC` palette codec (see the crate root). It produces files that
//! LVGL v9's image decoder (`lv_bin_decoder`) can consume directly, so rlvgl
//! assets can be handed to an LVGL build. The two formats share no bytes:
//! `RLEC` is a palette-indexed run codec with its own container; this module
//! emits the upstream `lv_image_header_t` layout.
//!
//! # Format (LVGL v9, little-endian)
//!
//! 12-byte `lv_image_header_t`:
//!
//! | offset | size | field                                   |
//! |--------|------|-----------------------------------------|
//! | 0      | 1    | magic = `0x19` ([`LV_IMAGE_HEADER_MAGIC`]) |
//! | 1      | 1    | color format ([`LvglCf::code`])         |
//! | 2      | 2    | flags (`0x08` = compressed)             |
//! | 4      | 2    | width                                   |
//! | 6      | 2    | height                                  |
//! | 8      | 2    | stride (bytes per row)                  |
//! | 10     | 2    | reserved (`0`)                          |
//!
//! Uncompressed: header immediately followed by the pixel data section
//! (`height * stride` bytes). Compressed (`--rle`): header, then a 12-byte
//! `lv_image_compressed_t` (`method:u32`, `compressed_size:u32`,
//! `decompressed_size:u32`), then the compressed payload.
//!
//! Per-pixel byte order matches upstream `LVGLImage.py`:
//! * `RGB565`   — little-endian `u16` `(r>>3)<<11 | (g>>2)<<5 | (b>>3)`
//! * `RGB888`   — `B, G, R`
//! * `ARGB8888` — `B, G, R, A`
//! * `XRGB8888` — `B, G, R, 0xFF`
//!
//! The reference for every constant here is upstream
//! `scripts/LVGLImage.py` (LVGL v9).

use alloc::vec::Vec;

use crate::Error;

/// LVGL v9 image header magic byte (`lv_image_header_t.magic`).
pub const LV_IMAGE_HEADER_MAGIC: u8 = 0x19;

/// `lv_image_header_t.flags` bit set when the data section is compressed.
pub const LV_IMAGE_FLAGS_COMPRESSED: u16 = 0x08;

/// Size of the `lv_image_header_t` in bytes.
pub const LV_IMAGE_HEADER_SIZE: usize = 12;

/// Size of the `lv_image_compressed_t` prefix in bytes.
pub const LV_IMAGE_COMPRESSED_HEADER_SIZE: usize = 12;

/// Color formats this codec can emit, with their upstream `lv_color_format_t`
/// integer codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LvglCf {
    /// 16-bit `RGB565`, little-endian (`0x12`). The most compact opaque format.
    Rgb565,
    /// 24-bit `RGB888`, stored `B, G, R` (`0x0F`).
    Rgb888,
    /// 32-bit `ARGB8888`, stored `B, G, R, A` (`0x10`).
    Argb8888,
    /// 32-bit `XRGB8888`, stored `B, G, R, 0xFF` (`0x11`); alpha ignored.
    Xrgb8888,
}

impl LvglCf {
    /// Upstream `lv_color_format_t` integer code written to header byte 1.
    pub fn code(self) -> u8 {
        match self {
            LvglCf::Rgb888 => 0x0F,
            LvglCf::Argb8888 => 0x10,
            LvglCf::Xrgb8888 => 0x11,
            LvglCf::Rgb565 => 0x12,
        }
    }

    /// Bytes per pixel in the stored data section.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            LvglCf::Rgb565 => 2,
            LvglCf::Rgb888 => 3,
            LvglCf::Argb8888 | LvglCf::Xrgb8888 => 4,
        }
    }

    /// Upstream C enum name, for generated `lv_image_dsc_t` descriptors.
    pub fn lv_name(self) -> &'static str {
        match self {
            LvglCf::Rgb565 => "LV_COLOR_FORMAT_RGB565",
            LvglCf::Rgb888 => "LV_COLOR_FORMAT_RGB888",
            LvglCf::Argb8888 => "LV_COLOR_FORMAT_ARGB8888",
            LvglCf::Xrgb8888 => "LV_COLOR_FORMAT_XRGB8888",
        }
    }
}

/// Compression method recorded in `lv_image_compressed_t.method`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LvglCompress {
    /// No compression; data section is raw pixels.
    None,
    /// LVGL run-length encoding (`lv_rle`).
    Rle,
}

impl LvglCompress {
    /// Upstream method code (`NONE = 0`, `RLE = 1`, `LZ4 = 2`).
    pub fn code(self) -> u32 {
        match self {
            LvglCompress::None => 0,
            LvglCompress::Rle => 1,
        }
    }
}

/// Encode one RGBA8888 frame's pixel data section (no header) in `cf`'s
/// stored byte order. Length is `width * height * cf.bytes_per_pixel()`.
pub fn encode_pixels(
    width: usize,
    height: usize,
    rgba: &[u8],
    cf: LvglCf,
) -> Result<Vec<u8>, Error> {
    if rgba.len() < width * height * 4 {
        return Err(Error::SizeMismatch);
    }
    let mut out = Vec::with_capacity(width * height * cf.bytes_per_pixel());
    for i in 0..width * height {
        let px = &rgba[i * 4..i * 4 + 4];
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        match cf {
            LvglCf::Rgb565 => {
                let c = crate::rgba_to_rgb565(px);
                out.extend_from_slice(&c.to_le_bytes());
            }
            LvglCf::Rgb888 => out.extend_from_slice(&[b, g, r]),
            LvglCf::Argb8888 => out.extend_from_slice(&[b, g, r, a]),
            LvglCf::Xrgb8888 => out.extend_from_slice(&[b, g, r, 0xFF]),
        }
    }
    Ok(out)
}

/// Build the 12-byte `lv_image_header_t`.
pub fn header_bytes(width: u16, height: u16, cf: LvglCf, flags: u16, stride: u16) -> [u8; 12] {
    header_bytes_raw(width, height, cf.code(), flags, stride)
}

/// Build the 12-byte `lv_image_header_t` from a raw `lv_color_format_t` code.
/// Used by the alpha-only path, whose formats live in [`LvglAlphaCf`].
pub fn header_bytes_raw(width: u16, height: u16, cf_code: u8, flags: u16, stride: u16) -> [u8; 12] {
    let mut h = [0u8; 12];
    h[0] = LV_IMAGE_HEADER_MAGIC;
    h[1] = cf_code;
    h[2..4].copy_from_slice(&flags.to_le_bytes());
    h[4..6].copy_from_slice(&width.to_le_bytes());
    h[6..8].copy_from_slice(&height.to_le_bytes());
    h[8..10].copy_from_slice(&stride.to_le_bytes());
    // bytes 10..12 reserved, left zero.
    h
}

/// Row stride in bytes for a tightly-packed `cf` image: `width * bpp`.
pub fn stride(width: usize, cf: LvglCf) -> usize {
    width * cf.bytes_per_pixel()
}

/// Encode an uncompressed LVGL v9 `.bin`: header + pixel data.
pub fn encode_bin(width: usize, height: usize, rgba: &[u8], cf: LvglCf) -> Result<Vec<u8>, Error> {
    let pixels = encode_pixels(width, height, rgba, cf)?;
    let stride = stride(width, cf) as u16;
    let mut out = Vec::with_capacity(LV_IMAGE_HEADER_SIZE + pixels.len());
    out.extend_from_slice(&header_bytes(width as u16, height as u16, cf, 0, stride));
    out.extend_from_slice(&pixels);
    Ok(out)
}

/// Encode a `LV_IMAGE_COMPRESS_RLE` LVGL v9 `.bin`: header (compressed flag) +
/// `lv_image_compressed_t` + RLE payload.
pub fn encode_bin_rle(
    width: usize,
    height: usize,
    rgba: &[u8],
    cf: LvglCf,
) -> Result<Vec<u8>, Error> {
    let pixels = encode_pixels(width, height, rgba, cf)?;
    let blk = cf.bytes_per_pixel();
    let compressed = rle_compress(&pixels, blk);
    let stride = stride(width, cf) as u16;

    let mut out = Vec::with_capacity(
        LV_IMAGE_HEADER_SIZE + LV_IMAGE_COMPRESSED_HEADER_SIZE + compressed.len(),
    );
    out.extend_from_slice(&header_bytes(
        width as u16,
        height as u16,
        cf,
        LV_IMAGE_FLAGS_COMPRESSED,
        stride,
    ));
    out.extend_from_slice(&LvglCompress::Rle.code().to_le_bytes());
    out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    out.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// LVGL `lv_rle` compress over `blk`-byte units.
///
/// Control byte grammar (matches `lv_rle_decompress`):
/// * high bit **clear** (`0x00..=0x7F`): a *repeat* of `count` copies of the
///   single following block.
/// * high bit **set** (`0x80 | count`): a *literal* run of `count` blocks
///   copied verbatim.
///
/// `count` is `1..=127`. Input is assumed already block-aligned (the encoders
/// here build it from `width * height` whole pixels, so it always is).
pub fn rle_compress(data: &[u8], blk: usize) -> Vec<u8> {
    let mut out = Vec::new();
    if blk == 0 || data.is_empty() {
        return out;
    }
    let nblk = data.len() / blk;
    let block = |k: usize| &data[k * blk..k * blk + blk];

    let mut i = 0;
    while i < nblk {
        // Measure the repeat run starting at block `i`.
        let mut run = 1;
        while i + run < nblk && run < 127 && block(i + run) == block(i) {
            run += 1;
        }
        if run >= 2 {
            out.push(run as u8); // high bit clear => repeat
            out.extend_from_slice(block(i));
            i += run;
        } else {
            // Literal run: consume blocks until a repeat of >= 2 begins or 127.
            let start = i;
            let mut lit = 0;
            while i < nblk && lit < 127 {
                let repeats_next = i + 1 < nblk && block(i + 1) == block(i);
                if repeats_next {
                    break;
                }
                i += 1;
                lit += 1;
            }
            out.push(0x80 | lit as u8); // high bit set => literal
            out.extend_from_slice(&data[start * blk..(start + lit) * blk]);
        }
    }
    out
}

/// Inverse of [`rle_compress`]. `out_len` is the expected decompressed length.
pub fn rle_decompress(data: &[u8], blk: usize, out_len: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(out_len);
    if blk == 0 {
        return Err(Error::Unsupported);
    }
    let mut i = 0;
    while i < data.len() {
        let ctrl = data[i];
        i += 1;
        if ctrl & 0x80 != 0 {
            let cnt = (ctrl & 0x7F) as usize;
            let bytes = cnt * blk;
            if i + bytes > data.len() {
                return Err(Error::Truncated);
            }
            out.extend_from_slice(&data[i..i + bytes]);
            i += bytes;
        } else {
            let cnt = ctrl as usize;
            if i + blk > data.len() {
                return Err(Error::Truncated);
            }
            let unit = &data[i..i + blk];
            i += blk;
            for _ in 0..cnt {
                out.extend_from_slice(unit);
            }
        }
    }
    Ok(out)
}

/// A decoded LVGL `.bin`: `(width, height, color format, RGBA8888 pixels)`.
pub type DecodedLvgl = (u16, u16, LvglCf, Vec<u8>);

/// Parse + decode an LVGL v9 `.bin` (uncompressed or RLE) back to RGBA8888.
///
/// Supports the four [`LvglCf`] this codec emits; other color formats return
/// [`Error::Unsupported`]. Primarily for round-trip tests and a future
/// LVGL → rlvgl import path.
pub fn decode_bin(data: &[u8]) -> Result<DecodedLvgl, Error> {
    if data.len() < LV_IMAGE_HEADER_SIZE {
        return Err(Error::Truncated);
    }
    if data[0] != LV_IMAGE_HEADER_MAGIC {
        return Err(Error::BadMagic);
    }
    let cf = match data[1] {
        0x12 => LvglCf::Rgb565,
        0x0F => LvglCf::Rgb888,
        0x10 => LvglCf::Argb8888,
        0x11 => LvglCf::Xrgb8888,
        _ => return Err(Error::Unsupported),
    };
    let flags = u16::from_le_bytes([data[2], data[3]]);
    let width = u16::from_le_bytes([data[4], data[5]]);
    let height = u16::from_le_bytes([data[6], data[7]]);
    let (w, h) = (width as usize, height as usize);
    let bpp = cf.bytes_per_pixel();
    let raw_len = w * h * bpp;

    let pixels = if flags & LV_IMAGE_FLAGS_COMPRESSED != 0 {
        let base = LV_IMAGE_HEADER_SIZE;
        if data.len() < base + LV_IMAGE_COMPRESSED_HEADER_SIZE {
            return Err(Error::Truncated);
        }
        let method =
            u32::from_le_bytes([data[base], data[base + 1], data[base + 2], data[base + 3]]);
        if method != LvglCompress::Rle.code() {
            return Err(Error::Unsupported);
        }
        let clen = u32::from_le_bytes([
            data[base + 4],
            data[base + 5],
            data[base + 6],
            data[base + 7],
        ]) as usize;
        let payload_start = base + LV_IMAGE_COMPRESSED_HEADER_SIZE;
        if data.len() < payload_start + clen {
            return Err(Error::Truncated);
        }
        rle_decompress(&data[payload_start..payload_start + clen], bpp, raw_len)?
    } else {
        if data.len() < LV_IMAGE_HEADER_SIZE + raw_len {
            return Err(Error::Truncated);
        }
        data[LV_IMAGE_HEADER_SIZE..LV_IMAGE_HEADER_SIZE + raw_len].to_vec()
    };

    if pixels.len() < raw_len {
        return Err(Error::Truncated);
    }
    let rgba = decode_pixels(&pixels, w * h, cf);
    Ok((width, height, cf, rgba))
}

/// Convert a stored pixel data section back to RGBA8888.
fn decode_pixels(pixels: &[u8], count: usize, cf: LvglCf) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(count * 4);
    let bpp = cf.bytes_per_pixel();
    for i in 0..count {
        let p = &pixels[i * bpp..i * bpp + bpp];
        match cf {
            LvglCf::Rgb565 => {
                let c = u16::from_le_bytes([p[0], p[1]]);
                rgba.extend_from_slice(&crate::rgb565_to_rgba(c));
            }
            LvglCf::Rgb888 => rgba.extend_from_slice(&[p[2], p[1], p[0], 0xFF]),
            LvglCf::Argb8888 => rgba.extend_from_slice(&[p[2], p[1], p[0], p[3]]),
            LvglCf::Xrgb8888 => rgba.extend_from_slice(&[p[2], p[1], p[0], 0xFF]),
        }
    }
    rgba
}

// ---------------------------------------------------------------------------
// Alpha-only ("coverage") formats — A8 / A4.
//
// These store a single coverage channel; the fill color is applied at draw
// time (LVGL's image-recolor style, or rlvgl's `blend_alpha_bin_into_argb`).
// Ideal for monochrome line-art icons: one asset retints to any color at a
// fraction of an `ARGB8888` asset's size.
// ---------------------------------------------------------------------------

/// Where an alpha-only encode derives its coverage channel from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageSource {
    /// Source alpha if the image has real transparency, else luminance.
    Auto,
    /// Always the source alpha channel (transparent-background icons).
    Alpha,
    /// Always luminance (`white-on-black` opaque mask art; white = ink).
    Luminance,
}

/// LVGL alpha-only color formats. The stored byte(s) are coverage; color is
/// applied when drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LvglAlphaCf {
    /// 8-bit alpha (`0x0E`), 1 byte/px — smooth, no dithering needed.
    A8,
    /// 4-bit alpha (`0x0D`), 2 px/byte (pixel 0 = high nibble), nibble `n`
    /// expands to `n * 17`. Floyd–Steinberg dithered to approximate the ramp.
    A4,
}

impl LvglAlphaCf {
    /// Upstream `lv_color_format_t` code written to header byte 1.
    pub fn code(self) -> u8 {
        match self {
            LvglAlphaCf::A8 => 0x0E,
            LvglAlphaCf::A4 => 0x0D,
        }
    }

    /// Bits per pixel in the packed data section.
    pub fn bits_per_pixel(self) -> usize {
        match self {
            LvglAlphaCf::A8 => 8,
            LvglAlphaCf::A4 => 4,
        }
    }

    /// Row stride in bytes: `width * bpp` rounded up to a byte boundary.
    pub fn stride(self, width: usize) -> usize {
        (width * self.bits_per_pixel()).div_ceil(8)
    }

    /// Upstream C enum name, for generated `lv_image_dsc_t` descriptors.
    pub fn lv_name(self) -> &'static str {
        match self {
            LvglAlphaCf::A8 => "LV_COLOR_FORMAT_A8",
            LvglAlphaCf::A4 => "LV_COLOR_FORMAT_A4",
        }
    }
}

/// Rec. 601 luma of an RGBA pixel: `(77R + 150G + 29B) >> 8`. Deterministic.
fn luma(px: &[u8]) -> u8 {
    ((77 * px[0] as u32 + 150 * px[1] as u32 + 29 * px[2] as u32) >> 8) as u8
}

/// Resolve [`CoverageSource::Auto`] for a frame: alpha if any pixel is
/// non-opaque, else luminance.
fn resolve_coverage(rgba: &[u8], count: usize, src: CoverageSource) -> CoverageSource {
    match src {
        CoverageSource::Auto => {
            if (0..count).any(|i| rgba[i * 4 + 3] != 0xFF) {
                CoverageSource::Alpha
            } else {
                CoverageSource::Luminance
            }
        }
        other => other,
    }
}

/// 8-bit coverage of one pixel under a resolved (non-`Auto`) source.
fn coverage_at(px: &[u8], src: CoverageSource) -> u8 {
    match src {
        CoverageSource::Luminance => luma(px),
        // Alpha, or an unresolved Auto (treated as alpha) — px[3].
        _ => px[3],
    }
}

/// Floyd–Steinberg dither an 8-bit coverage plane to 4-bit values (`0..=15`),
/// one per pixel in raster order. Deterministic (no RNG).
fn dither_a4(width: usize, height: usize, cover: &[u8]) -> Vec<u8> {
    let mut work: Vec<i32> = cover.iter().map(|&c| c as i32).collect();
    let mut out = alloc::vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let old = work[idx].clamp(0, 255);
            let q = (old * 15 + 127) / 255; // nearest of 16 levels
            out[idx] = q as u8;
            let err = old - q * 17; // q*17 is the reconstructed 8-bit value
            let mut diffuse = |xx: i32, yy: i32, num: i32| {
                if xx >= 0 && (xx as usize) < width && (yy as usize) < height {
                    work[yy as usize * width + xx as usize] += err * num / 16;
                }
            };
            diffuse(x as i32 + 1, y as i32, 7);
            diffuse(x as i32 - 1, y as i32 + 1, 3);
            diffuse(x as i32, y as i32 + 1, 5);
            diffuse(x as i32 + 1, y as i32 + 1, 1);
        }
    }
    out
}

/// Pack 4-bit values (pixel 0 in the high nibble) into byte-aligned rows.
fn pack_a4(width: usize, height: usize, q4: &[u8]) -> Vec<u8> {
    let stride = LvglAlphaCf::A4.stride(width);
    let mut out = alloc::vec![0u8; stride * height];
    for y in 0..height {
        for x in 0..width {
            let n = q4[y * width + x] & 0x0F;
            let byte = y * stride + x / 2;
            if x % 2 == 0 {
                out[byte] |= n << 4;
            } else {
                out[byte] |= n;
            }
        }
    }
    out
}

/// Encode an alpha-only pixel data section (no header) from an RGBA frame.
pub fn encode_alpha_pixels(
    width: usize,
    height: usize,
    rgba: &[u8],
    cf: LvglAlphaCf,
    src: CoverageSource,
) -> Result<Vec<u8>, Error> {
    let count = width * height;
    if rgba.len() < count * 4 {
        return Err(Error::SizeMismatch);
    }
    let src = resolve_coverage(rgba, count, src);
    let cover: Vec<u8> = (0..count)
        .map(|i| coverage_at(&rgba[i * 4..i * 4 + 4], src))
        .collect();
    match cf {
        // A8: stride == width, so raster-order coverage is the data section.
        LvglAlphaCf::A8 => Ok(cover),
        LvglAlphaCf::A4 => {
            let q4 = dither_a4(width, height, &cover);
            Ok(pack_a4(width, height, &q4))
        }
    }
}

/// Encode an alpha-only LVGL v9 `.bin` (optionally RLE-compressed).
pub fn encode_alpha_bin(
    width: usize,
    height: usize,
    rgba: &[u8],
    cf: LvglAlphaCf,
    src: CoverageSource,
    rle: bool,
) -> Result<Vec<u8>, Error> {
    let pixels = encode_alpha_pixels(width, height, rgba, cf, src)?;
    let stride = cf.stride(width) as u16;
    let mut out = Vec::new();
    if rle {
        // LVGL RLE block size for sub-byte formats is ceil(bpp/8) == 1 byte.
        let compressed = rle_compress(&pixels, 1);
        out.extend_from_slice(&header_bytes_raw(
            width as u16,
            height as u16,
            cf.code(),
            LV_IMAGE_FLAGS_COMPRESSED,
            stride,
        ));
        out.extend_from_slice(&LvglCompress::Rle.code().to_le_bytes());
        out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        out.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        out.extend_from_slice(&compressed);
    } else {
        out.extend_from_slice(&header_bytes_raw(
            width as u16,
            height as u16,
            cf.code(),
            0,
            stride,
        ));
        out.extend_from_slice(&pixels);
    }
    Ok(out)
}

/// A decoded alpha-only `.bin`: `(width, height, format, 8-bit coverage plane)`.
pub type DecodedAlpha = (u16, u16, LvglAlphaCf, Vec<u8>);

/// Parse + decode an alpha-only LVGL `.bin` (A8/A4, uncompressed or RLE) into
/// an 8-bit coverage plane in raster order (A4 nibbles expanded `n * 17`).
pub fn decode_alpha_bin(data: &[u8]) -> Result<DecodedAlpha, Error> {
    if data.len() < LV_IMAGE_HEADER_SIZE {
        return Err(Error::Truncated);
    }
    if data[0] != LV_IMAGE_HEADER_MAGIC {
        return Err(Error::BadMagic);
    }
    let cf = match data[1] {
        0x0E => LvglAlphaCf::A8,
        0x0D => LvglAlphaCf::A4,
        _ => return Err(Error::Unsupported),
    };
    let flags = u16::from_le_bytes([data[2], data[3]]);
    let width = u16::from_le_bytes([data[4], data[5]]);
    let height = u16::from_le_bytes([data[6], data[7]]);
    let (w, h) = (width as usize, height as usize);
    let stride = cf.stride(w);
    let data_len = stride * h;

    let packed = if flags & LV_IMAGE_FLAGS_COMPRESSED != 0 {
        let base = LV_IMAGE_HEADER_SIZE;
        if data.len() < base + LV_IMAGE_COMPRESSED_HEADER_SIZE {
            return Err(Error::Truncated);
        }
        let method =
            u32::from_le_bytes([data[base], data[base + 1], data[base + 2], data[base + 3]]);
        if method != LvglCompress::Rle.code() {
            return Err(Error::Unsupported);
        }
        let clen = u32::from_le_bytes([
            data[base + 4],
            data[base + 5],
            data[base + 6],
            data[base + 7],
        ]) as usize;
        let start = base + LV_IMAGE_COMPRESSED_HEADER_SIZE;
        if data.len() < start + clen {
            return Err(Error::Truncated);
        }
        rle_decompress(&data[start..start + clen], 1, data_len)?
    } else {
        if data.len() < LV_IMAGE_HEADER_SIZE + data_len {
            return Err(Error::Truncated);
        }
        data[LV_IMAGE_HEADER_SIZE..LV_IMAGE_HEADER_SIZE + data_len].to_vec()
    };
    if packed.len() < data_len {
        return Err(Error::Truncated);
    }

    let mut cover = alloc::vec![0u8; w * h];
    match cf {
        LvglAlphaCf::A8 => {
            for y in 0..h {
                cover[y * w..y * w + w].copy_from_slice(&packed[y * stride..y * stride + w]);
            }
        }
        LvglAlphaCf::A4 => {
            for y in 0..h {
                for x in 0..w {
                    let byte = packed[y * stride + x / 2];
                    let nib = if x % 2 == 0 { byte >> 4 } else { byte & 0x0F };
                    cover[y * w + x] = nib * 17;
                }
            }
        }
    }
    Ok((width, height, cf, cover))
}

/// Source-over composite a `fill` color through an 8-bit `coverage` plane onto
/// an ARGB8888 destination buffer.
///
/// `dst` byte order is `[B, G, R, A]` per pixel (little-endian ARGB8888 —
/// rlvgl's framebuffer layout). This is the rlvgl coverage+tint draw path:
/// the icon stores only coverage; the caller supplies the fill color at draw
/// time. Destination alpha is left fully opaque.
pub fn blend_coverage_into_argb(
    width: usize,
    height: usize,
    coverage: &[u8],
    fill: (u8, u8, u8),
    dst: &mut [u8],
) -> Result<(), Error> {
    let count = width * height;
    if coverage.len() < count || dst.len() < count * 4 {
        return Err(Error::SizeMismatch);
    }
    let (fr, fg, fb) = (fill.0 as u32, fill.1 as u32, fill.2 as u32);
    for i in 0..count {
        let a = coverage[i] as u32;
        let ia = 255 - a;
        let d = &mut dst[i * 4..i * 4 + 4];
        d[0] = ((fb * a + d[0] as u32 * ia + 127) / 255) as u8;
        d[1] = ((fg * a + d[1] as u32 * ia + 127) / 255) as u8;
        d[2] = ((fr * a + d[2] as u32 * ia + 127) / 255) as u8;
        d[3] = 0xFF;
    }
    Ok(())
}

/// Parse an alpha-only LVGL `.bin` and composite it onto an ARGB8888 buffer
/// with `fill`. Returns the image dimensions. See [`blend_coverage_into_argb`].
pub fn blend_alpha_bin_into_argb(
    bin: &[u8],
    fill: (u8, u8, u8),
    dst: &mut [u8],
) -> Result<(u16, u16), Error> {
    let (w, h, _cf, cover) = decode_alpha_bin(bin)?;
    blend_coverage_into_argb(w as usize, h as usize, &cover, fill, dst)?;
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn solid(w: usize, h: usize, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut v = vec![0u8; w * h * 4];
        for px in v.chunks_exact_mut(4) {
            px.copy_from_slice(&[r, g, b, a]);
        }
        v
    }

    #[test]
    fn header_layout_is_lvgl_v9() {
        let h = header_bytes(100, 50, LvglCf::Rgb565, 0, 200);
        assert_eq!(h[0], 0x19); // magic
        assert_eq!(h[1], 0x12); // RGB565
        assert_eq!(&h[2..4], &[0, 0]); // flags
        assert_eq!(&h[4..6], &100u16.to_le_bytes()); // width
        assert_eq!(&h[6..8], &50u16.to_le_bytes()); // height
        assert_eq!(&h[8..10], &200u16.to_le_bytes()); // stride
        assert_eq!(&h[10..12], &[0, 0]); // reserved
    }

    #[test]
    fn rgb565_pixel_byte_order_is_little_endian() {
        // Pure red -> RGB565 0xF800 -> LE bytes [0x00, 0xF8].
        let px = encode_pixels(1, 1, &[0xFF, 0x00, 0x00, 0xFF], LvglCf::Rgb565).unwrap();
        assert_eq!(px, vec![0x00, 0xF8]);
    }

    #[test]
    fn argb8888_pixel_byte_order_is_bgra() {
        // R=0x11 G=0x22 B=0x33 A=0x44 -> stored B,G,R,A.
        let px = encode_pixels(1, 1, &[0x11, 0x22, 0x33, 0x44], LvglCf::Argb8888).unwrap();
        assert_eq!(px, vec![0x33, 0x22, 0x11, 0x44]);
    }

    #[test]
    fn rgb888_pixel_byte_order_is_bgr() {
        let px = encode_pixels(1, 1, &[0x11, 0x22, 0x33, 0xFF], LvglCf::Rgb888).unwrap();
        assert_eq!(px, vec![0x33, 0x22, 0x11]);
    }

    #[test]
    fn uncompressed_bin_round_trips_argb8888() {
        let (w, h) = (8, 4);
        let rgba = solid(w, h, 0x10, 0x20, 0x30, 0x40);
        let bin = encode_bin(w, h, &rgba, LvglCf::Argb8888).unwrap();
        assert_eq!(bin.len(), LV_IMAGE_HEADER_SIZE + w * h * 4);
        let (dw, dh, cf, out) = decode_bin(&bin).unwrap();
        assert_eq!((dw, dh, cf), (w as u16, h as u16, LvglCf::Argb8888));
        assert_eq!(out, rgba);
    }

    #[test]
    fn rle_round_trips_and_sets_compressed_flag() {
        let (w, h) = (16, 16); // solid -> highly compressible
        let rgba = solid(w, h, 0x00, 0x80, 0xFF, 0xFF);
        let bin = encode_bin_rle(w, h, &rgba, LvglCf::Argb8888).unwrap();
        let flags = u16::from_le_bytes([bin[2], bin[3]]);
        assert_ne!(flags & LV_IMAGE_FLAGS_COMPRESSED, 0);
        // Compressed payload must be far smaller than the raw section.
        assert!(bin.len() < LV_IMAGE_HEADER_SIZE + w * h * 4 / 2);
        let (_, _, _, out) = decode_bin(&bin).unwrap();
        assert_eq!(out, rgba);
    }

    #[test]
    fn rle_round_trips_mixed_runs_and_literals() {
        // A gradient row forces literal runs; repeated rows force repeats.
        let (w, h) = (32, 8);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let off = (y * w + x) * 4;
                rgba[off] = (x * 8) as u8;
                rgba[off + 1] = (y * 32) as u8;
                rgba[off + 2] = 0x40;
                rgba[off + 3] = 0xFF;
            }
        }
        let bin = encode_bin_rle(w, h, &rgba, LvglCf::Rgb565).unwrap();
        let (_, _, _, out) = decode_bin(&bin).unwrap();
        // RGB565 is lossy; compare the round trip through the same quantizer.
        let requantized = {
            let px = encode_pixels(w, h, &rgba, LvglCf::Rgb565).unwrap();
            decode_pixels(&px, w * h, LvglCf::Rgb565)
        };
        assert_eq!(out, requantized);
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let mut bin = encode_bin(2, 2, &solid(2, 2, 1, 2, 3, 4), LvglCf::Rgb565).unwrap();
        bin[0] = 0x00;
        assert_eq!(decode_bin(&bin), Err(Error::BadMagic));
    }

    // --- alpha-only (A8 / A4) coverage formats ---

    /// Build an RGBA frame from a per-pixel `(r,g,b,a)` closure.
    fn make_rgba(w: usize, h: usize, f: impl Fn(usize, usize) -> (u8, u8, u8, u8)) -> Vec<u8> {
        let mut v = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let (r, g, b, a) = f(x, y);
                let o = (y * w + x) * 4;
                v[o..o + 4].copy_from_slice(&[r, g, b, a]);
            }
        }
        v
    }

    #[test]
    fn a8_coverage_auto_uses_alpha_when_transparent() {
        // Distinct alpha per column; RGB constant. Auto must pick the alpha.
        let alphas = [0u8, 0x80, 0xC0, 0xFF];
        let rgba = make_rgba(4, 1, |x, _| (10, 20, 30, alphas[x]));
        let px = encode_alpha_pixels(4, 1, &rgba, LvglAlphaCf::A8, CoverageSource::Auto).unwrap();
        assert_eq!(px, alphas);
    }

    #[test]
    fn a8_coverage_auto_falls_back_to_luminance_when_opaque() {
        // Fully opaque white pixel -> luminance 255; black -> 0.
        let rgba = make_rgba(2, 1, |x, _| {
            if x == 0 {
                (255, 255, 255, 255)
            } else {
                (0, 0, 0, 255)
            }
        });
        let px = encode_alpha_pixels(2, 1, &rgba, LvglAlphaCf::A8, CoverageSource::Auto).unwrap();
        assert_eq!(px[0], 255);
        assert_eq!(px[1], 0);
    }

    #[test]
    fn a4_packs_pixel0_in_high_nibble() {
        // coverage [255, 0] -> 4-bit [15, 0] -> 0xF0; stride = (2*4+7)/8 = 1.
        let rgba = make_rgba(2, 1, |x, _| (0, 0, 0, if x == 0 { 255 } else { 0 }));
        let px = encode_alpha_pixels(2, 1, &rgba, LvglAlphaCf::A4, CoverageSource::Alpha).unwrap();
        assert_eq!(px.len(), 1);
        assert_eq!(px[0], 0xF0);
    }

    #[test]
    fn a8_bin_round_trips_coverage_exactly() {
        let rgba = make_rgba(8, 4, |x, y| (0, 0, 0, ((x + y * 8) * 7 % 256) as u8));
        let bin =
            encode_alpha_bin(8, 4, &rgba, LvglAlphaCf::A8, CoverageSource::Alpha, false).unwrap();
        assert_eq!(bin[1], 0x0E); // A8 cf code
        assert_eq!(u16::from_le_bytes([bin[8], bin[9]]), 8); // stride == width
        let (w, h, cf, cover) = decode_alpha_bin(&bin).unwrap();
        assert_eq!((w, h, cf), (8, 4, LvglAlphaCf::A8));
        let expect: Vec<u8> = (0..32).map(|i| (i * 7 % 256) as u8).collect();
        assert_eq!(cover, expect);
    }

    #[test]
    fn a4_rle_round_trips_and_sets_flags() {
        // Vertical alpha ramp -> dither produces runs the RLE can shrink.
        let (w, h) = (16, 16);
        let rgba = make_rgba(w, h, |_, y| (0, 0, 0, (y * 17) as u8));
        let bin =
            encode_alpha_bin(w, h, &rgba, LvglAlphaCf::A4, CoverageSource::Alpha, true).unwrap();
        assert_eq!(bin[1], 0x0D); // A4
        assert_ne!(
            u16::from_le_bytes([bin[2], bin[3]]) & LV_IMAGE_FLAGS_COMPRESSED,
            0
        );
        let plain =
            encode_alpha_bin(w, h, &rgba, LvglAlphaCf::A4, CoverageSource::Alpha, false).unwrap();
        // RLE vs uncompressed decode to identical coverage.
        let (_, _, _, a) = decode_alpha_bin(&bin).unwrap();
        let (_, _, _, b) = decode_alpha_bin(&plain).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn blend_coverage_is_source_over_with_fill() {
        // dst starts black; fill is pure red. coverage 0 -> black, 255 -> red,
        // 128 -> ~half.
        let mut dst = vec![0u8; 3 * 4]; // 3 px ARGB8888 [B,G,R,A]
        let cover = [0u8, 128, 255];
        blend_coverage_into_argb(3, 1, &cover, (255, 0, 0), &mut dst).unwrap();
        // px0: untouched black
        assert_eq!(&dst[0..4], &[0, 0, 0, 0xFF]);
        // px1: ~half red in the R byte (index 2)
        assert!((dst[6] as i32 - 128).abs() <= 1, "got R={}", dst[6]);
        assert_eq!(dst[4], 0); // B
        assert_eq!(dst[5], 0); // G
                               // px2: full red
        assert_eq!(&dst[8..12], &[0, 0, 255, 0xFF]);
    }

    #[test]
    fn blend_alpha_bin_tints_a_decoded_icon() {
        // A8 bin: left opaque, right transparent. Tint green over white bg.
        let rgba = make_rgba(2, 1, |x, _| (0, 0, 0, if x == 0 { 255 } else { 0 }));
        let bin =
            encode_alpha_bin(2, 1, &rgba, LvglAlphaCf::A8, CoverageSource::Alpha, false).unwrap();
        let mut dst = vec![0xFFu8; 2 * 4]; // white bg
        let (w, h) = blend_alpha_bin_into_argb(&bin, (0, 255, 0), &mut dst).unwrap();
        assert_eq!((w, h), (2, 1));
        // px0 fully covered -> green [B,G,R,A] = [0,255,0,255]
        assert_eq!(&dst[0..4], &[0, 255, 0, 0xFF]);
        // px1 zero coverage -> background white preserved
        assert_eq!(&dst[4..8], &[0xFF, 0xFF, 0xFF, 0xFF]);
    }
}
