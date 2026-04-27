//! Audio-meter widgets and supporting types.
//!
//! See `docs/audio-meters/` for the initiative overview, concepts gate
//! (AM-00), scale schema (AM-03), skin schema (AM-04a), and the
//! `LedBargraph` chapter (AM-05).

pub mod bargraph;
pub mod needle;
pub mod numeric;
/// Hand-authored runtime [`skin::Scale`] / [`skin::Skin`] constants
/// matching the JSON descriptors under `assets/audio-meters/`. Replaced
/// by `rlvgl-creator` codegen output in AM-04b.
pub mod presets;
pub mod skin;

pub use bargraph::LedBargraph;
pub use needle::NeedleVu;
pub use numeric::NumericPeak;
pub use skin::{
    Layout, MeterColorId, MeterType, Orientation, Palette, Scale, SecondaryColors, Skin, TickLabel,
    Zone, rgb, rgba,
};
