// SPDX-License-Identifier: MIT
//! Star Wars–style vertical text crawl preset.
//!
//! [`StarCrawl`] is a type alias for [`TextCrawl`] parameterised with
//! [`SubPixelRate`] and [`StarField`] — the classic Star Wars opening
//! crawl look: dark starfield background, yellow text scrolling upward.
//! The sub-pixel rate model lets the engine match a declared
//! wall-clock speed (40 px/s on the legacy hardware crawl) on any host
//! frame rate without quantising to whole-pixel steps.
//!
//! Callers who want the canonical preset can use [`disco_demo_preset`]
//! to skip the generic path entirely.

use rlvgl_core::packed_font::PackedFont;
use rlvgl_platform::blit::Surface;

use super::text::TextCrawl;
use crate::motion::background::StarField;
use crate::motion::direction::Direction;
use crate::motion::jumbo::JumboBuffer;
use crate::motion::rate::SubPixelRate;

/// Target scroll speed used by the disco demo preset, in pixels per
/// second. Matches the legacy STM32H747I-DISCO hardware star crawl.
pub const DISCO_PRESET_PIXELS_PER_SEC: u32 = 40;

/// Line-spacing multiplier for the disco demo preset, expressed as a
/// rational `(num, den)` applied to the font height.
///
/// The hardware crawl uses `3 / 2` (1.5× font height). The simulator
/// intentionally sits noticeably *above* that — `2 / 1` (2× font
/// height) — so each line has a clear air-gap on a host display where
/// screen sizes can vary widely from the embedded target.
pub const DISCO_PRESET_LINE_SPACING: (u32, u32) = (2, 1);

/// Vertical Star Wars crawl: text scrolls up over a starfield.
pub type StarCrawl<'buf> = TextCrawl<'buf, SubPixelRate, StarField>;

/// Construct a [`StarCrawl`] configured with the disco demo defaults.
///
/// Rate: `40 px/s` sub-pixel advance at the supplied `frame_hz`, so
/// the visible motion matches `40 px/s` regardless of host refresh
/// rate (hardware runs at 30 Hz, the simulator at 60 Hz; both scroll
/// at the same wall-clock speed). Line spacing is 2× the font height
/// so simulator text has generous breathing room above the hardware's
/// 1.5× baseline. Background: default [`StarField`] (200 stars, seed
/// `0xDEADBEEF`, bg `0xFF0A0A20`). Text colour: `0xFFFFD700` (the
/// canonical yellow).
#[allow(clippy::too_many_arguments)]
pub fn disco_demo_preset<'buf>(
    font: &'static PackedFont,
    lines: &'static [&'static str],
    frame_hz: u32,
    jumbo_bg: JumboBuffer<'buf>,
    text_src: Surface<'buf>,
    scanline: &'buf mut [u8],
) -> StarCrawl<'buf> {
    let mut crawl = TextCrawl::new(
        Direction::Up,
        SubPixelRate::new(DISCO_PRESET_PIXELS_PER_SEC, frame_hz),
        StarField::default(),
        font,
        0xFFFF_D700,
        lines,
        jumbo_bg,
        text_src,
        scanline,
    );
    let (num, den) = DISCO_PRESET_LINE_SPACING;
    let spacing = (font.height as u32 * num / den.max(1)) as u16;
    crawl.line_spacing = spacing.max(1);
    crawl
}
