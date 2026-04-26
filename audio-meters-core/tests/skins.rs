//! Validates the canonical skin set under
//! `assets/audio-meters/skins/`. Mirror of
//! `audio-meters-core/ts/test/skins.test.ts`. Both runtimes implement
//! the same checks; divergence is a bug.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CalibrationOverride {
    #[allow(dead_code)]
    to: String,
    #[allow(dead_code)]
    offset_db: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Palette {
    #[serde(rename = "Safe")]
    safe: String,
    #[serde(rename = "Nominal")]
    nominal: String,
    #[serde(rename = "Caution")]
    caution: String,
    #[serde(rename = "Hot")]
    hot: String,
    #[serde(rename = "Over")]
    over: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    orientation: String,
    aspect_ratio: f32,
    #[serde(default)]
    led_count: Option<u32>,
    #[serde(default)]
    peak_hold_ms: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Skin {
    #[serde(rename = "$schema", default)]
    #[allow(dead_code)]
    schema: Option<String>,
    id: String,
    title: String,
    scale_id: String,
    default_ballistic: String,
    #[serde(default)]
    #[allow(dead_code)]
    calibration_override: Option<CalibrationOverride>,
    meter_type: String,
    palette: Palette,
    #[serde(default)]
    secondary_colors: Option<BTreeMap<String, String>>,
    layout: Layout,
    #[serde(default)]
    #[allow(dead_code)]
    assets: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
struct ScaleZone {
    #[allow(dead_code)]
    from_db: f32,
    #[allow(dead_code)]
    to_db: f32,
    #[allow(dead_code)]
    color: String,
}

#[derive(Deserialize)]
struct ScaleStub {
    id: String,
    compatible_ballistics: Vec<String>,
    #[allow(dead_code)]
    zones: Vec<ScaleZone>,
}

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

const ALLOWED_METER_TYPES: &[&str] = &["bargraph", "needle", "numeric", "lufs_gauge"];
const ALLOWED_ORIENTATIONS: &[&str] = &["horizontal", "vertical"];

fn assets_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("assets")
        .join("audio-meters")
}

fn is_hex_color(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn load_scales() -> BTreeMap<String, ScaleStub> {
    let dir = assets_root().join("scales");
    fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read scales: {e}"))
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|s| s.to_str()) == Some("json")).then_some(p)
        })
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap();
            let s: ScaleStub = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
            (s.id.clone(), s)
        })
        .collect()
}

fn validate(skin: &Skin, file_stem: &str, scales: &BTreeMap<String, ScaleStub>) {
    assert_eq!(
        skin.id, file_stem,
        "skin id `{}` MUST match filename stem `{}`",
        skin.id, file_stem
    );
    assert!(!skin.title.is_empty(), "{}: title required", skin.id);

    let scale = scales.get(&skin.scale_id).unwrap_or_else(|| {
        panic!(
            "{}: scale_id `{}` does not match any file under scales/",
            skin.id, skin.scale_id
        )
    });
    assert_eq!(
        scale.id, skin.scale_id,
        "internal: scale stub id mismatch ({} vs {})",
        scale.id, skin.scale_id
    );

    assert!(
        ALLOWED_BALLISTICS.contains(&skin.default_ballistic.as_str()),
        "{}: default_ballistic `{}` not in §5 enum",
        skin.id,
        skin.default_ballistic
    );
    // Advisory: warn (do not fail) if the ballistic is not in
    // compatible_ballistics. Concepts §6 documents this is allowed but
    // unconventional. Tests print to stderr but pass.
    if !scale
        .compatible_ballistics
        .contains(&skin.default_ballistic)
    {
        eprintln!(
            "[skins] note: {} pairs ballistic `{}` with scale `{}` (not in compatible_ballistics {:?})",
            skin.id, skin.default_ballistic, scale.id, scale.compatible_ballistics
        );
    }

    assert!(
        ALLOWED_METER_TYPES.contains(&skin.meter_type.as_str()),
        "{}: meter_type `{}` not in enum",
        skin.id,
        skin.meter_type
    );

    // Palette colours.
    for (name, value) in [
        ("Safe", &skin.palette.safe),
        ("Nominal", &skin.palette.nominal),
        ("Caution", &skin.palette.caution),
        ("Hot", &skin.palette.hot),
        ("Over", &skin.palette.over),
    ] {
        assert!(
            is_hex_color(value),
            "{} palette[{}]: `{}` not a valid hex colour",
            skin.id,
            name,
            value
        );
    }

    if let Some(secondary) = &skin.secondary_colors {
        let allowed: BTreeSet<_> = [
            "background",
            "frame",
            "scale_text",
            "minor_tick",
            "major_tick",
            "needle",
            "needle_pivot",
            "led_off",
            "peak_hold",
        ]
        .into_iter()
        .collect();
        for (k, v) in secondary {
            assert!(
                allowed.contains(k.as_str()),
                "{}: secondary_colors key `{}` not in schema",
                skin.id,
                k
            );
            assert!(
                is_hex_color(v),
                "{} secondary[{}]: `{}` not a valid hex colour",
                skin.id,
                k,
                v
            );
        }
    }

    assert!(
        ALLOWED_ORIENTATIONS.contains(&skin.layout.orientation.as_str()),
        "{}: layout.orientation `{}` not in enum",
        skin.id,
        skin.layout.orientation
    );
    assert!(
        skin.layout.aspect_ratio > 0.0 && skin.layout.aspect_ratio <= 100.0,
        "{}: aspect_ratio out of range ({})",
        skin.id,
        skin.layout.aspect_ratio
    );

    // led_count only meaningful for bargraph.
    if skin.meter_type == "bargraph" {
        let n = skin
            .layout
            .led_count
            .unwrap_or_else(|| panic!("{}: bargraph must declare led_count", skin.id));
        assert!((4..=256).contains(&n), "{}: led_count {n} out of range", skin.id);
    }

    if let Some(hold) = skin.layout.peak_hold_ms {
        assert!(
            (0.0..=60_000.0).contains(&hold),
            "{}: peak_hold_ms {hold} out of range",
            skin.id
        );
    }
}

#[test]
fn canonical_skins_load_and_validate() {
    let scales = load_scales();
    let dir = assets_root().join("skins");
    let entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();

    let mut count = 0;
    for path in entries {
        let text = fs::read_to_string(&path).unwrap();
        let skin: Skin = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        validate(&skin, &stem, &scales);
        count += 1;
    }

    assert!(
        count >= 3,
        "expected ≥ 3 canonical skins, found {} under {}",
        count,
        dir.display()
    );
}
