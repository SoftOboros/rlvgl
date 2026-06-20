// SPDX-License-Identifier: MIT
//! Icon and layout constants for the SCXML Tutorial Demo App.
//!
//! # Icon source notes
//! These are Lucide-derived supplemental glyphs reused from the
//! disco-demo asset tree (SCTD-00 §6.6: "Lucide MAY fill gaps when
//! tutorial assets are absent or unsuitable"). Tutorial-asset icons
//! (from Qt/DiningPhilosophers/Images/ and
//! Qt/SkodaBoleroInfotainment/Qml/Images/) require transcoding to RLE
//! and are deferred to a follow-up per SCTD-00 §6.4.

/// Selector icon for Dining Philosophers (cpu48 glyph — supplemental Lucide).
pub static ICON_DP: &[u8] = include_bytes!("../assets/icons/dp48.rle");

/// Selector icon for Media Player stub (play48 glyph — supplemental Lucide).
pub static ICON_MEDIA: &[u8] = include_bytes!("../assets/icons/media48.rle");

/// Info/about icon (info glyph — supplemental Lucide).
pub static ICON_INFO: &[u8] = include_bytes!("../assets/icons/info.rle");

/// Focus highlight border color.
pub const FOCUS_HIGHLIGHT_COLOR: rlvgl_core::widget::Color =
    rlvgl_core::widget::Color(0, 180, 255, 255);

/// Focus highlight border width in pixels.
pub const FOCUS_BORDER_WIDTH: u8 = 2;
