//! Validates the canonical scale set under
//! `assets/audio-meters/scales/`. This test holds the cross-runtime
//! contract: every scale checked into the asset package MUST parse, MUST
//! pass internal-consistency checks, and MUST be loadable by both
//! runtimes (Rust here, TS in `audio-meters-core/ts/test/scales.test.ts`).
//!
//! The `Scale` struct here is test-local. Public Scale types for widgets
//! land with AM-05; this file is just the validator.

use std::{collections::BTreeSet, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RangeDb {
    min: f32,
    max: f32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pivot {
    label: String,
    input_dbfs: f32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CalibrationDefault {
    to: String,
    offset_db: f32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ticks {
    majors: Vec<f32>,
    minors_per_major_division: u32,
    #[serde(default)]
    labels: std::collections::BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Zone {
    from_db: f32,
    to_db: f32,
    color: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Scale {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    id: String,
    label_units: String,
    range_db: RangeDb,
    pivot: Pivot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calibration_default: Option<CalibrationDefault>,
    ticks: Ticks,
    zones: Vec<Zone>,
    compatible_ballistics: Vec<String>,
}

const ALLOWED_COLORS: &[&str] = &["Safe", "Nominal", "Caution", "Hot", "Over"];
const ALLOWED_BALLISTICS: &[&str] = &[
    "Vu",
    "PpmTypeI",
    "PpmTypeIIa",
    "PpmTypeIIb",
    "DigitalPeak",
    "Rms",
    "LufsM",
    "LufsS",
    "LufsI",
    "Instant",
];

fn scales_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has a parent")
        .join("assets")
        .join("audio-meters")
        .join("scales")
}

fn validate(scale: &Scale, file_stem: &str) {
    assert_eq!(
        scale.id, file_stem,
        "scale id `{}` MUST match filename stem `{}`",
        scale.id, file_stem
    );
    assert!(
        scale.range_db.min < scale.range_db.max,
        "{}: range_db.min ({}) < range_db.max ({})",
        scale.id,
        scale.range_db.min,
        scale.range_db.max
    );

    // Pivot input_dbfs is in dBFS-domain; no constraint relative to range.
    assert!(
        scale.pivot.input_dbfs.is_finite(),
        "{}: pivot.input_dbfs must be finite",
        scale.id
    );

    // Majors sorted ascending, all within range_db.
    assert!(scale.ticks.majors.len() >= 2, "{}: need ≥ 2 majors", scale.id);
    for w in scale.ticks.majors.windows(2) {
        assert!(
            w[0] < w[1],
            "{}: ticks.majors must be strictly ascending; got {} ≥ {}",
            scale.id,
            w[0],
            w[1]
        );
    }
    let first = *scale.ticks.majors.first().unwrap();
    let last = *scale.ticks.majors.last().unwrap();
    assert!(
        (first - scale.range_db.min).abs() < 1e-3,
        "{}: first major ({}) should equal range_db.min ({})",
        scale.id,
        first,
        scale.range_db.min
    );
    assert!(
        (last - scale.range_db.max).abs() < 1e-3,
        "{}: last major ({}) should equal range_db.max ({})",
        scale.id,
        last,
        scale.range_db.max
    );

    // Zones partition range_db without gap or overlap.
    assert!(!scale.zones.is_empty(), "{}: at least one zone", scale.id);
    let mut prev_to = scale.range_db.min;
    for (i, z) in scale.zones.iter().enumerate() {
        assert!(
            ALLOWED_COLORS.contains(&z.color.as_str()),
            "{} zone[{}]: color `{}` not in §7 enum",
            scale.id,
            i,
            z.color
        );
        assert!(
            (z.from_db - prev_to).abs() < 1e-3,
            "{} zone[{}]: from_db ({}) must abut previous to_db ({})",
            scale.id,
            i,
            z.from_db,
            prev_to
        );
        assert!(
            z.to_db > z.from_db,
            "{} zone[{}]: to_db ({}) must exceed from_db ({})",
            scale.id,
            i,
            z.to_db,
            z.from_db
        );
        prev_to = z.to_db;
    }
    assert!(
        (prev_to - scale.range_db.max).abs() < 1e-3,
        "{}: last zone to_db ({}) must equal range_db.max ({})",
        scale.id,
        prev_to,
        scale.range_db.max
    );

    // Ballistic identifiers known.
    assert!(
        !scale.compatible_ballistics.is_empty(),
        "{}: need ≥ 1 compatible_ballistics",
        scale.id
    );
    let unique: BTreeSet<_> = scale.compatible_ballistics.iter().collect();
    assert_eq!(
        unique.len(),
        scale.compatible_ballistics.len(),
        "{}: compatible_ballistics has duplicates",
        scale.id
    );
    for b in &scale.compatible_ballistics {
        assert!(
            ALLOWED_BALLISTICS.contains(&b.as_str()),
            "{}: ballistic `{}` not in §5 enum",
            scale.id,
            b
        );
    }

    // Labels reference declared majors.
    for key in scale.ticks.labels.keys() {
        let parsed: f32 = key
            .parse()
            .unwrap_or_else(|_| panic!("{}: tick label key `{}` not a number", scale.id, key));
        let known = scale.ticks.majors.iter().any(|m| (m - parsed).abs() < 1e-3);
        assert!(
            known,
            "{}: tick label key `{}` not in majors {:?}",
            scale.id, key, scale.ticks.majors
        );
    }
}

#[test]
fn canonical_scales_load_and_validate() {
    let dir = scales_dir();
    let mut found = 0;
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();

    for path in entries {
        let text = fs::read_to_string(&path).unwrap();
        let scale: Scale = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        validate(&scale, &stem);
        found += 1;
    }

    // The §6 enumeration in the concepts doc lists six canonical scales.
    // Lower bound here so adding more scales does not break the test.
    assert!(
        found >= 6,
        "expected ≥ 6 canonical scales under {}, found {}",
        dir.display(),
        found
    );
}
