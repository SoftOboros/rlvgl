// SPDX-License-Identifier: MIT
//! Star Wars–style vertical text crawl preset.
//!
//! [`StarCrawl`] is a type alias for [`TextCrawl`] parameterised with
//! [`FrameRoundedRate`] and [`StarField`] — the classic Star Wars
//! opening crawl look: dark starfield background, yellow text
//! scrolling upward. Callers who want the canonical preset can use
//! [`disco_demo_preset`] to skip the generic path entirely.

use rlvgl_core::packed_font::PackedFont;
use rlvgl_platform::blit::Surface;

use super::text::TextCrawl;
use crate::motion::background::StarField;
use crate::motion::direction::Direction;
use crate::motion::jumbo::JumboBuffer;
use crate::motion::rate::FrameRoundedRate;

/// Vertical Star Wars crawl: text scrolls up over a starfield.
pub type StarCrawl<'buf> = TextCrawl<'buf, FrameRoundedRate, StarField>;

/// Construct a [`StarCrawl`] configured with the disco demo defaults.
///
/// Rate: 30 px/s at 30 Hz = 1 px/frame, matching the hardware star
/// crawl. Background: default [`StarField`] (200 stars, seed
/// `0xDEADBEEF`, bg `0xFF0A0A20`). Text colour: `0xFFFFD700` (the
/// canonical yellow).
pub fn disco_demo_preset<'buf>(
    font: &'static PackedFont,
    lines: &'static [&'static str],
    jumbo_bg: JumboBuffer<'buf>,
    text_src: Surface<'buf>,
    scanline: &'buf mut [u8],
) -> StarCrawl<'buf> {
    TextCrawl::new(
        Direction::Up,
        FrameRoundedRate::new(30, 30),
        StarField::default(),
        font,
        0xFFFF_D700,
        lines,
        jumbo_bg,
        text_src,
        scanline,
    )
}
