// SPDX-License-Identifier: MIT
//! BBB-side buffer wiring for the Star Wars–style text crawl overlay.
//!
//! Port of `examples/disco-sim/src/crawl_buffers.rs` tuned for the
//! BeagleBone Black + NHD-7.0CTP-CAPE-P landscape panel. The motion
//! engine in [`rlvgl_widgets::motion`] is buffer-agnostic — every
//! non-trivial buffer it needs (jumbo background, A8 text source,
//! scanline scratch) crosses the API boundary as a borrowed slice —
//! so the only per-board work is sizing and allocating those buffers.
//!
//! On a 512 MiB AM3358 we can comfortably afford the ~7 MiB activation
//! cost (jumbo + A8 text + scanline). We allocate plain `Vec<u8>`s and
//! lean on [`Vec::leak`] for the `'static` lifetime the preset expects;
//! `RuntimeState::apply_effect_outcome` drops/deactivates any previous
//! instance before building a new one, so repeated activations leak one
//! buffer set apiece. Acceptable for a demo the user explicitly toggles.
//!
//! Font + text assets are reused from the STM32H747I-DISCO target via
//! `#[path]` so both runtimes see the same 24 px DejaVu Sans glyphs and
//! the same 119-line `README_CRAWL` body.

use rlvgl_core::packed_font::PackedFont;
use rlvgl_decomp::{decode_argb_into, parse_rle_blob};
use rlvgl_platform::blit::{PixelFmt, Surface};
use rlvgl_widgets::motion::crawl::{pre_render_text, star_crawl::disco_demo_preset};
use rlvgl_widgets::motion::{CrawlWindow, JumboBuffer, JumboOrientation, StarCrawl};

#[path = "../../stm32h747i-disco/src/fonts.rs"]
#[allow(unexpected_cfgs)]
mod fonts;

#[path = "../../stm32h747i-disco/src/readme_crawl.rs"]
mod readme_crawl;

/// 24 px DejaVu Sans — shared with the hardware UI font.
static CRAWL_FONT_DATA: &[u8] =
    include_bytes!("../../stm32h747i-disco/assets/fonts/DejaVuSans-24.bin");
static CRAWL_LOGO_RAW: &[u8] =
    include_bytes!("../../stm32h747i-disco/assets/icons/softoboros-letter-logo.raw");
static SPLASH_RLE: &[u8] = include_bytes!("../../stm32h747i-disco/assets/media/splash.rle");

/// Static [`PackedFont`] descriptor the crawl engine borrows at `'static`.
pub static CRAWL_FONT: PackedFont = PackedFont {
    height: 24,
    ascent: 22,
    glyphs: &fonts::DEJAVU_SANS_24_GLYPHS,
    data: CRAWL_FONT_DATA,
};

/// Full Star Wars crawl text. Shared with the hardware target via
/// `#[path]` inclusion so both runtimes see the same 119 lines.
pub use readme_crawl::README_CRAWL as CRAWL_LINES;

/// Bytes per ARGB8888 pixel.
const ARGB_BPP: usize = 4;
/// Jumbo scale along the scroll axis. 2× the visible extent so the
/// scroll cursor can slide through a pre-painted region without seams.
const JUMBO_SCALE: u8 = 2;
/// Pre-rendered A8 text column width.
const TEXT_WIDTH_PX: u32 = 600;
/// Pre-rendered A8 text column height. Large enough for the splash
/// graphic, crawl text, trailing logo, and the padding between them.
const TEXT_HEIGHT_PX: u32 = 6400;
/// STM32-style crawl viewport width, leaving the right-side icon bar exposed.
const CRAWL_VIEW_W: u32 = 720;
/// Crawl viewport height. Matches the full BBB panel height.
const CRAWL_VIEW_H: u32 = 480;
/// Hardware-matching line spacing: 1.5× a 24 px font = 36 px.
const CRAWL_LINE_SPACING_PX: u16 = 36;
/// Top margin above the splash crop, matching the STM32 crawl script.
const TOP_MARGIN_PX: u32 = 120;
/// Portrait splash crop source X offset.
const GRAPHIC_CROP_X: u32 = 48;
/// Portrait splash crop source Y offset.
const GRAPHIC_CROP_Y: u32 = 208;
/// Width/height of the square splash crop.
const GRAPHIC_SIZE: u32 = 384;
/// Gap between the splash crop and the first crawl line.
const GRAPHIC_GAP_PX: u32 = 40;
/// Gap between the end of the crawl text and the softoboros logo.
const LOGO_GAP_PX: u32 = 40;

fn logo_dimensions() -> Option<(u32, u32)> {
    (CRAWL_LOGO_RAW.len() >= 24).then(|| {
        (
            u32::from_le_bytes([
                CRAWL_LOGO_RAW[8],
                CRAWL_LOGO_RAW[9],
                CRAWL_LOGO_RAW[10],
                CRAWL_LOGO_RAW[11],
            ]),
            u32::from_le_bytes([
                CRAWL_LOGO_RAW[12],
                CRAWL_LOGO_RAW[13],
                CRAWL_LOGO_RAW[14],
                CRAWL_LOGO_RAW[15],
            ]),
        )
    })
}

fn blit_logo_a8(dst: &mut Surface<'_>, dst_y: u32) -> u32 {
    let Some((logo_w, logo_h)) = logo_dimensions() else {
        return 0;
    };
    let pixels = &CRAWL_LOGO_RAW[24..];
    let x_off = ((dst.width.saturating_sub(logo_w)) / 2) as usize;
    for row in 0..logo_h {
        let y = dst_y + row;
        if y >= dst.height {
            break;
        }
        let row_start = y as usize * dst.stride;
        for col in 0..logo_w {
            let src_off = ((row * logo_w + col) * 4) as usize;
            if src_off + 3 >= pixels.len() {
                break;
            }
            let alpha = pixels[src_off + 3];
            if alpha == 0 {
                continue;
            }
            let dst_off = row_start + x_off + col as usize;
            if dst_off >= dst.buf.len() {
                break;
            }
            dst.buf[dst_off] = dst.buf[dst_off].max(alpha);
        }
    }
    logo_h
}

fn blit_splash_crop_a8(dst: &mut Surface<'_>, dst_y: u32) -> u32 {
    let Ok((src_w, src_h, pal_bytes, stream)) = parse_rle_blob(SPLASH_RLE) else {
        return 0;
    };
    let src_w = u32::from(src_w);
    let src_h = u32::from(src_h);
    let mut palette = [0u16; 256];
    let pal_count = pal_bytes.len() / 2;
    for i in 0..pal_count.min(256) {
        palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
    }
    let mut splash = vec![0u8; src_w as usize * src_h as usize * ARGB_BPP];
    if decode_argb_into(
        src_w as usize,
        src_h as usize,
        &palette[..pal_count],
        stream,
        &mut splash,
    )
    .is_err()
    {
        return 0;
    }

    let x_off = ((dst.width.saturating_sub(GRAPHIC_SIZE)) / 2) as usize;
    for row in 0..GRAPHIC_SIZE {
        for col in 0..GRAPHIC_SIZE {
            let src_x = GRAPHIC_CROP_X + col;
            let src_y = GRAPHIC_CROP_Y + row;
            if src_x >= src_w || src_y >= src_h {
                continue;
            }
            let src_off = ((src_y * src_w + src_x) * ARGB_BPP as u32) as usize;
            if src_off + 2 >= splash.len() {
                continue;
            }
            let b = splash[src_off] as u32;
            let g = splash[src_off + 1] as u32;
            let r = splash[src_off + 2] as u32;
            let lum = (r * 77 + g * 150 + b * 29) >> 8;
            let alpha = 255u8.saturating_sub(lum as u8);
            if alpha == 0 {
                continue;
            }

            let dst_x = x_off + row as usize;
            let dst_row = dst_y + GRAPHIC_SIZE - 1 - col;
            if dst_row >= dst.height {
                continue;
            }
            let dst_off = dst_row as usize * dst.stride + dst_x;
            if dst_off >= dst.buf.len() {
                continue;
            }
            dst.buf[dst_off] = dst.buf[dst_off].max(alpha);
        }
    }
    GRAPHIC_SIZE
}

fn render_crawl_strip(text_src: &mut Surface<'_>) -> u32 {
    let mut cur_y = TOP_MARGIN_PX;
    let graphic_h = blit_splash_crop_a8(text_src, cur_y);
    if graphic_h > 0 {
        cur_y += graphic_h + GRAPHIC_GAP_PX;
    }
    let text_h = {
        let body_y = cur_y.min(text_src.height);
        let body_offset = body_y as usize * text_src.stride;
        let mut body = Surface::new(
            &mut text_src.buf[body_offset..],
            text_src.stride,
            PixelFmt::A8,
            text_src.width,
            text_src.height.saturating_sub(body_y),
        );
        pre_render_text(&CRAWL_FONT, CRAWL_LINES, CRAWL_LINE_SPACING_PX, &mut body)
    };
    cur_y += text_h + LOGO_GAP_PX;
    cur_y += blit_logo_a8(text_src, cur_y);
    cur_y
}

/// Build a [`CrawlWindow`] wrapping a freshly-allocated [`StarCrawl`]
/// engine sized like the STM32 demo's crawl viewport.
///
/// `frame_hz` must match the host's render loop cadence so the
/// sub-pixel rate model advances the scroll accumulator at the right
/// wall-clock speed (the disco preset targets 40 px/s).
pub fn build_star_crawl_window(
    visible_w: u32,
    visible_h: u32,
    frame_hz: u32,
) -> CrawlWindow<StarCrawl<'static>> {
    let visible_w = visible_w.max(1).min(CRAWL_VIEW_W);
    let visible_h = visible_h.max(1).min(CRAWL_VIEW_H);

    // Horizontal jumbo background so the starfield drifts right-to-left
    // while the text itself continues to crawl upward.
    let jumbo_stride = visible_w as usize * JUMBO_SCALE as usize * ARGB_BPP;
    let jumbo_len = jumbo_stride * visible_h as usize;
    let jumbo_vec: Vec<u8> = vec![0u8; jumbo_len];
    let jumbo_slice: &'static mut [u8] = Vec::leak(jumbo_vec);
    let jumbo = JumboBuffer::new(
        jumbo_slice,
        jumbo_stride,
        visible_w,
        visible_h,
        JUMBO_SCALE,
        JumboOrientation::Horizontal,
        PixelFmt::Argb8888,
    );

    // A8 text source: `TEXT_WIDTH_PX × TEXT_HEIGHT_PX` × 1 byte.
    let text_stride = TEXT_WIDTH_PX as usize;
    let text_len = text_stride * TEXT_HEIGHT_PX as usize;
    let text_vec: Vec<u8> = vec![0u8; text_len];
    let text_slice: &'static mut [u8] = Vec::leak(text_vec);
    let mut text_src = Surface::new(
        text_slice,
        text_stride,
        PixelFmt::A8,
        TEXT_WIDTH_PX,
        TEXT_HEIGHT_PX,
    );
    let text_height = render_crawl_strip(&mut text_src);

    // Scanline scratch: wide enough for the long axis × 4 bytes.
    let scanline_len = visible_w.max(visible_h) as usize * ARGB_BPP;
    let scanline_vec: Vec<u8> = vec![0u8; scanline_len];
    let scanline_slice: &'static mut [u8] = Vec::leak(scanline_vec);

    let mut crawl = disco_demo_preset(
        &CRAWL_FONT,
        CRAWL_LINES,
        frame_hz,
        jumbo,
        text_src,
        scanline_slice,
    );
    crawl.use_pre_rendered_text(text_height);

    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: visible_w as i32,
        height: visible_h as i32,
    };
    CrawlWindow::new(bounds, crawl)
}
