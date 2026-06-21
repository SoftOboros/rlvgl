// SPDX-License-Identifier: MIT
//! Icon and layout constants for the SCXML Tutorial Demo App.
//!
//! # Icon source notes
//! `ICON_DP` and `HERO_DP` are the authentic Dining Philosophers table
//! illustration from the scjson tutorial, transcoded to RLE via
//! `rlvgl-creator` per SCTD-00 §6.4 (tutorial assets are the preferred
//! source; provenance MUST be preserved). `ICON_MEDIA` / `ICON_INFO`
//! remain Lucide-derived supplemental glyphs reused from the disco-demo
//! asset tree (SCTD-00 §6.6: "Lucide MAY fill gaps when tutorial assets
//! are absent or unsuitable").
//!
//! ## Provenance — DP table illustration
//! Source: `vendor/scjson/tutorial/Examples/Qt/DiningPhilosophers/Images/`
//! `dininig_philosophers.svg` (600×600 viewBox; 5 philosophers + forks).
//! Pipeline (deterministic, re-runnable):
//! ```text
//! rlvgl-creator svg dininig_philosophers.svg out/        # -> 600x600 .raw
//! sips -z 60 60   <raw->png> dp_table_60.png             # 60px selector icon
//! sips -z 150 150 <raw->png> dp_table_150.png            # 150px hero
//! rlvgl-creator compress dp_table_<N>.png dp_table_<N>.rle
//! ```
//! The tutorial source is read-only; only transcoded copies live here.

/// Selector icon for the Dining Philosophers machines (faithful + interactive):
/// the tutorial table illustration scaled to the 60 px selector strip.
pub static ICON_DP: &[u8] = include_bytes!("../assets/icons/dp_table_60.rle");

/// Backdrop for the live Philosophers Table: the tutorial table illustration
/// at 150 px. The table overlays per-seat state discs on it (see
/// [`crate::philosophers`]); the disc centers are the philosopher coordinates
/// from this same SVG, so the overlay registers exactly on the art.
pub static HERO_DP: &[u8] = include_bytes!("../assets/icons/dp_table_150.rle");

/// Selector icon for Media Player stub (play48 glyph — supplemental Lucide).
pub static ICON_MEDIA: &[u8] = include_bytes!("../assets/icons/media48.rle");

/// Info/about icon (info glyph — supplemental Lucide).
pub static ICON_INFO: &[u8] = include_bytes!("../assets/icons/info.rle");

/// Focus highlight border color.
pub const FOCUS_HIGHLIGHT_COLOR: rlvgl_core::widget::Color =
    rlvgl_core::widget::Color(0, 180, 255, 255);

/// Focus highlight border width in pixels.
pub const FOCUS_BORDER_WIDTH: u8 = 2;
