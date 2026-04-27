//! Runtime `Skin` types for audio-meter widgets.
//!
//! Path-(b) integration shape: typed `Skin` / `Scale` structs the
//! application constructs (literal `const`, build.rs codegen, or runtime
//! deserialisation in host-side tools). The widget consumes a
//! `&'static Skin` and never parses JSON itself — embedded targets stay
//! `no_std` and alloc-free.
//!
//! Path (a) — compile-time codegen of `const SkinStatic` modules from
//! `assets/audio-meters/skins/*.json` via `rlvgl-creator` — is deferred
//! to AM-04b. The on-disk JSON schema and these struct shapes were
//! designed so codegen produces literals of these exact types.
//!
//! See `docs/audio-meters/04-skins.md` for the descriptor schema and
//! `docs/audio-meters/05-led-bargraph.md` for the widget's consumption
//! pattern.

use rlvgl_core::widget::Color;

/// Concepts §7 colour-zone identifier. Skins map this to a concrete
/// `Color` via [`Palette::color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterColorId {
    /// Well below alignment level. Conventionally green.
    Safe,
    /// Around alignment level. Green / amber depending on skin.
    Nominal,
    /// Approaching headroom limit. Amber / yellow.
    Caution,
    /// At or above safe peak threshold. Red.
    Hot,
    /// Clipped / illegal level. Bright red / flashing.
    Over,
}

/// Concrete colours for the §7 zone identifiers. Every skin populates
/// all five.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Colour for [`MeterColorId::Safe`].
    pub safe: Color,
    /// Colour for [`MeterColorId::Nominal`].
    pub nominal: Color,
    /// Colour for [`MeterColorId::Caution`].
    pub caution: Color,
    /// Colour for [`MeterColorId::Hot`].
    pub hot: Color,
    /// Colour for [`MeterColorId::Over`].
    pub over: Color,
}

impl Palette {
    /// Look up the concrete colour for a zone identifier.
    pub const fn color(&self, id: MeterColorId) -> Color {
        match id {
            MeterColorId::Safe => self.safe,
            MeterColorId::Nominal => self.nominal,
            MeterColorId::Caution => self.caution,
            MeterColorId::Hot => self.hot,
            MeterColorId::Over => self.over,
        }
    }
}

/// Optional non-zone colours. `None` means "widget chooses default".
#[derive(Debug, Clone, Copy, Default)]
pub struct SecondaryColors {
    /// Background fill behind the meter, if widget paints one.
    pub background: Option<Color>,
    /// Frame / bezel colour.
    pub frame: Option<Color>,
    /// Scale label text colour.
    pub scale_text: Option<Color>,
    /// Minor tick mark colour.
    pub minor_tick: Option<Color>,
    /// Major tick mark colour.
    pub major_tick: Option<Color>,
    /// Needle colour for analog meters.
    pub needle: Option<Color>,
    /// Pivot-point colour for analog meters.
    pub needle_pivot: Option<Color>,
    /// Off-state colour for unlit LED segments.
    pub led_off: Option<Color>,
    /// Peak-hold pip colour.
    pub peak_hold: Option<Color>,
}

/// Layout orientation for a meter widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// LEDs run left-to-right; needle swings horizontally.
    Horizontal,
    /// LEDs run bottom-to-top; needle swings vertically.
    Vertical,
}

/// Widget family. Skins target one family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterType {
    /// LED bargraph (vertical or horizontal).
    Bargraph,
    /// Analog needle.
    Needle,
    /// Numeric readout.
    Numeric,
    /// LUFS gauge with multi-band display.
    LufsGauge,
}

/// Layout hints. Final pixel placement is the widget's responsibility.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// Orientation. Drives LED progression and label placement.
    pub orientation: Orientation,
    /// Width-to-height ratio. Used by container code that wants to
    /// preserve the skin's intended proportions.
    pub aspect_ratio: f32,
    /// Number of LED segments for `MeterType::Bargraph`. Other meter
    /// types ignore this.
    pub led_count: u32,
    /// Hold time for the peak pip in milliseconds. `0` disables peak
    /// hold.
    pub peak_hold_ms: f32,
}

/// One zone within a [`Scale`]. Boundaries are in dBFS-domain (the
/// scale's `pivot.input_dbfs` ties dBFS to displayed labels).
#[derive(Debug, Clone, Copy)]
pub struct Zone {
    /// Lower edge, inclusive (display dB).
    pub from_db: f32,
    /// Upper edge, exclusive (display dB).
    pub to_db: f32,
    /// Which §7 colour identifier this zone uses.
    pub color: MeterColorId,
}

/// A printed label on a major tick. Mirror of one entry in the JSON
/// `ticks.labels` map. Value is in scale-units; the entry's `value`
/// MUST match one of [`Scale::majors`] within 1e-3.
#[derive(Debug, Clone, Copy)]
pub struct TickLabel {
    /// Major tick this label sits on (in scale-units).
    pub value: f32,
    /// Display string (e.g. `"0"`, `"+3"`, `"−20"`).
    pub label: &'static str,
}

/// Runtime `Scale` — mirror of `assets/audio-meters/schema/scale.schema.json`.
///
/// Authored as a `const` literal in app code (or generated by
/// `rlvgl-creator` in AM-04b). All borrowed slices are `&'static` so
/// the entire Scale lives in flash on embedded targets.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    /// Scale identifier. See concepts §6.
    pub id: &'static str,
    /// Display unit string for tick labels (e.g. `"dBVU"`, `"dBFS"`).
    pub label_units: &'static str,
    /// Lowest value on the visible scale (in scale units).
    pub range_min_db: f32,
    /// Highest value on the visible scale (in scale units).
    pub range_max_db: f32,
    /// Numeric scale-units coordinate of the pivot. Lies within
    /// `[range_min_db, range_max_db]`.
    pub pivot_value: f32,
    /// Display text at the pivot tick (e.g. `"0"`, `"4"`, `"−23"`).
    pub pivot_label: &'static str,
    /// dBFS sample value that should produce a reading at the pivot.
    /// Together with `pivot_value` this defines the dBFS → scale-units
    /// offset: `scale_units = dbfs + (pivot_value - pivot_input_dbfs)`.
    pub pivot_input_dbfs: f32,
    /// Optional alternate-units calibration. When the user wants
    /// labels in dBu / dBV / dBSPL instead of `label_units`, the
    /// alt-units value is `scale_units + offset_db`. Does **not**
    /// affect zone lookup or needle / bargraph positioning — those
    /// always work in scale-units domain.
    pub calibration_offset_db: Option<f32>,
    /// Major-tick positions, in scale-units (strictly ascending).
    pub majors: &'static [f32],
    /// Minor ticks between each pair of consecutive majors. `0` = none.
    pub minors_per_major_division: u32,
    /// Optional per-major labels. Only majors listed here render
    /// labels; widgets fall back to a numeric format for majors
    /// missing an entry. Empty slice means "no labels".
    pub tick_labels: &'static [TickLabel],
    /// Colour zones partitioning `[range_min_db, range_max_db]`.
    pub zones: &'static [Zone],
}

/// Runtime `Skin` — mirror of
/// `assets/audio-meters/schema/skin.schema.json`.
#[derive(Debug, Clone, Copy)]
pub struct Skin {
    /// Skin identifier. Matches `assets/audio-meters/skins/<id>.json`.
    pub id: &'static str,
    /// Human-readable picker title.
    pub title: &'static str,
    /// Bound [`Scale`].
    pub scale: &'static Scale,
    /// Default ballistic (widget MAY override at instantiation).
    pub default_ballistic: rlvgl_audio_meters_core::Ballistic,
    /// Widget family.
    pub meter_type: MeterType,
    /// Concrete §7 colours.
    pub palette: Palette,
    /// Non-zone colours.
    pub secondary: SecondaryColors,
    /// Geometry hints.
    pub layout: Layout,
}

impl Scale {
    /// Project a dBFS-domain reading into scale-units. Widgets use this
    /// for zone lookup and for needle / bargraph positioning. See
    /// concepts §3 (Scale glossary entry) and §9 (widget update
    /// contract): ballistic state stays in dBFS-domain; *only* the
    /// rendered value lives in scale-units.
    #[inline]
    pub fn dbfs_to_scale_units(&self, dbfs: f32) -> f32 {
        dbfs + (self.pivot_value - self.pivot_input_dbfs)
    }

    /// Look up the printed label for a major tick at `value` (in
    /// scale-units). Returns `None` if no label is registered — the
    /// widget should fall back to a numeric format.
    pub fn label_for_major(&self, value: f32) -> Option<&'static str> {
        for t in self.tick_labels {
            if (t.value - value).abs() < 1e-3 {
                return Some(t.label);
            }
        }
        None
    }

    /// Look up the §7 colour identifier whose zone covers `scale_value`
    /// (a value in scale-units, **not** dBFS — call
    /// [`Self::dbfs_to_scale_units`] first if you have a dBFS reading).
    /// If `scale_value` is below `range_min_db`, returns the first
    /// zone's colour; if at or above `range_max_db`, returns the last
    /// zone's colour.
    pub fn zone_color_for(&self, scale_value: f32) -> MeterColorId {
        // Zones are partition-ordered (validated at AM-03 by both
        // runtimes); first zone whose `to_db` exceeds `scale_value` wins.
        for z in self.zones {
            if scale_value < z.to_db {
                return z.color;
            }
        }
        // scale_value >= range_max_db: clamp to last zone.
        self.zones
            .last()
            .map(|z| z.color)
            .unwrap_or(MeterColorId::Safe)
    }
}

/// Build a 24-bit-RGB fully-opaque [`Color`]. `const`-usable, mirrors
/// the `#RRGGBB` hex codes in the JSON skin descriptors.
#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color(r, g, b, 0xff)
}

/// Build an RGBA [`Color`]. `const`-usable, mirrors `#RRGGBBAA` codes.
#[inline]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color(r, g, b, a)
}
