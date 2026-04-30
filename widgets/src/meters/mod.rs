//! Audio-meter widgets and supporting types.
//!
//! See `docs/audio-meters/` for the initiative overview, concepts gate
//! (AM-00), scale schema (AM-03), skin schema (AM-04a), and the
//! `LedBargraph` chapter (AM-05).

pub mod bargraph;
pub mod lufs_gauge;
pub mod lufs_gauge_strict;
pub mod multi_channel;
pub mod needle;
pub mod numeric;
/// Hand-authored runtime [`skin::Scale`] / [`skin::Skin`] constants
/// matching the JSON descriptors under `assets/audio-meters/`. Replaced
/// by `rlvgl-creator` codegen output in AM-04b.
pub mod presets;
pub mod skin;
pub mod stereo;

pub use bargraph::LedBargraph;
pub use lufs_gauge::LufsGauge;
pub use lufs_gauge_strict::LufsGaugeStrict;
pub use multi_channel::{MultiChannel, split_horizontal_n, split_vertical_n};
pub use needle::NeedleVu;
pub use numeric::NumericPeak;
pub use skin::{
    Layout, MeterColorId, MeterType, Orientation, Palette, Scale, SecondaryColors, Skin,
    SkinAssets, TickLabel, Zone, rgb, rgba,
};
pub use stereo::{MeterWidget, StereoPair, split_horizontal};
