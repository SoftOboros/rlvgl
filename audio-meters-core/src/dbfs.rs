//! dBFS newtype and display-time calibration offset.
//!
//! Per `docs/audio-meters/00-concepts.md` §9, calibration to dBu / dBV /
//! dBSPL is a display-time additive offset. It MUST NOT enter the ballistic
//! state machine. The helper here is intentionally trivial — it exists so
//! both the Rust core and the TypeScript port apply the offset at the same
//! point in the pipeline.

/// Decibels relative to digital full scale.
///
/// Per AES17, `0 dBFS` is the level of a full-scale sine wave. Newtype only;
/// no arithmetic ops are exposed because all meter math happens against
/// raw `f32` for cross-runtime parity (the TS port has no equivalent
/// newtype, and the JSON fixtures encode raw numbers).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dbfs(pub f32);

impl Dbfs {
    /// Construct a dBFS value from an `f32`.
    pub const fn new(db: f32) -> Self {
        Self(db)
    }

    /// Inner value as `f32`.
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// Apply a calibration offset to a dBFS reading.
///
/// `offset_db` is added directly: `dbu = dbfs + offset_db`. The offset is
/// per-installation (the studio chose `0 dBu = -20 dBFS`, `0 dBu = -18 dBFS`,
/// etc.). Skin descriptors carry a default; widgets may override.
#[inline]
pub fn apply_calibration(dbfs: f32, offset_db: f32) -> f32 {
    dbfs + offset_db
}
