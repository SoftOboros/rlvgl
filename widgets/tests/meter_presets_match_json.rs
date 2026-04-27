//! Cross-checks the hand-authored Skin / Scale constants in
//! `widgets::meters::presets` against the canonical JSON descriptors
//! under `assets/audio-meters/`. This test catches the most common
//! edit-skew failure: someone tweaks a colour or peak-hold value in
//! the JSON without updating the corresponding `pub static` here, or
//! vice versa.
//!
//! AM-04b will replace `presets.rs` with codegen output; until then,
//! this is the editor-discipline net.

use std::{collections::BTreeMap, fs, path::PathBuf};

use rlvgl_widgets::meters::{
    Layout, MeterColorId, MeterType, Orientation, Palette, Scale, SecondaryColors, Skin, Zone,
    presets,
};
use rlvgl_widgets::meters::skin::TickLabel;
use serde::Deserialize;

#[derive(Deserialize)]
struct JsonRange {
    min: f32,
    max: f32,
}

#[derive(Deserialize)]
struct JsonPivot {
    value: f32,
    label: String,
    input_dbfs: f32,
}

#[derive(Deserialize)]
struct JsonCalibration {
    #[allow(dead_code)]
    to: String,
    offset_db: f32,
}

#[derive(Deserialize)]
struct JsonTicks {
    majors: Vec<f32>,
    minors_per_major_division: u32,
    #[serde(default)]
    labels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct JsonZone {
    from_db: f32,
    to_db: f32,
    color: String,
}

#[derive(Deserialize)]
struct JsonScale {
    id: String,
    label_units: String,
    range_db: JsonRange,
    pivot: JsonPivot,
    calibration_default: Option<JsonCalibration>,
    ticks: JsonTicks,
    zones: Vec<JsonZone>,
    #[allow(dead_code)]
    compatible_ballistics: Vec<String>,
}

#[derive(Deserialize)]
struct JsonPalette {
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
struct JsonLayout {
    orientation: String,
    aspect_ratio: f32,
    led_count: Option<u32>,
    peak_hold_ms: Option<f32>,
}

#[derive(Deserialize)]
struct JsonSkin {
    id: String,
    title: String,
    scale_id: String,
    default_ballistic: String,
    meter_type: String,
    palette: JsonPalette,
    secondary_colors: Option<BTreeMap<String, String>>,
    layout: JsonLayout,
}

fn assets_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("assets")
        .join("audio-meters")
}

fn parse_hex(s: &str) -> (u8, u8, u8, u8) {
    assert!(s.starts_with('#'));
    let hex = &s[1..];
    assert!(hex.len() == 6 || hex.len() == 8, "bad hex: {s}");
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
    let a = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).unwrap()
    } else {
        0xff
    };
    (r, g, b, a)
}

fn assert_color_match(
    skin_id: &str,
    field: &str,
    runtime: rlvgl_core::widget::Color,
    json_hex: &str,
) {
    let (r, g, b, a) = parse_hex(json_hex);
    let runtime_tup = (runtime.0, runtime.1, runtime.2, runtime.3);
    assert_eq!(
        runtime_tup,
        (r, g, b, a),
        "{skin_id}.{field}: runtime = {runtime_tup:?}, JSON = {json_hex}"
    );
}

fn assert_scale_match(runtime: &Scale, json_path: &PathBuf) {
    let text = fs::read_to_string(json_path).unwrap();
    let j: JsonScale = serde_json::from_str(&text).unwrap();
    assert_eq!(runtime.id, j.id, "scale id");
    assert_eq!(runtime.label_units, j.label_units);
    assert_eq!(runtime.range_min_db, j.range_db.min);
    assert_eq!(runtime.range_max_db, j.range_db.max);
    assert_eq!(runtime.pivot_label, j.pivot.label);
    assert_eq!(runtime.pivot_input_dbfs, j.pivot.input_dbfs);
    assert_eq!(runtime.pivot_value, j.pivot.value, "{}: pivot_value", j.id);
    assert_eq!(
        runtime.calibration_offset_db,
        j.calibration_default.as_ref().map(|c| c.offset_db),
        "{}: calibration",
        j.id
    );
    assert_eq!(
        runtime.majors,
        j.ticks.majors.as_slice(),
        "{}: majors",
        j.id
    );
    assert_eq!(
        runtime.minors_per_major_division, j.ticks.minors_per_major_division,
        "{}: minors_per_major_division",
        j.id
    );
    assert_tick_labels_match(&j.id, runtime.tick_labels, &j.ticks.labels);
    assert_eq!(runtime.zones.len(), j.zones.len(), "{}: zone count", j.id);
    for (i, (rz, jz)) in runtime.zones.iter().zip(j.zones.iter()).enumerate() {
        assert_eq!(rz.from_db, jz.from_db, "{} zone[{}].from_db", j.id, i);
        assert_eq!(rz.to_db, jz.to_db, "{} zone[{}].to_db", j.id, i);
        let expected = match jz.color.as_str() {
            "Safe" => MeterColorId::Safe,
            "Nominal" => MeterColorId::Nominal,
            "Caution" => MeterColorId::Caution,
            "Hot" => MeterColorId::Hot,
            "Over" => MeterColorId::Over,
            other => panic!("unknown color in JSON: {other}"),
        };
        assert_eq!(rz.color, expected, "{} zone[{}].color", j.id, i);
    }
}

fn assert_tick_labels_match(
    scale_id: &str,
    runtime: &[TickLabel],
    json: &BTreeMap<String, String>,
) {
    assert_eq!(
        runtime.len(),
        json.len(),
        "{}: tick_labels count differs (runtime {} vs json {})",
        scale_id,
        runtime.len(),
        json.len()
    );
    for tl in runtime {
        // JSON keys are stringified numeric majors; parse with normal
        // ASCII minus, JSON does not use unicode minus in keys.
        let key = format!("{:.0}", tl.value);
        let expected = json
            .get(&key)
            .unwrap_or_else(|| panic!("{}: tick label key `{}` missing in JSON", scale_id, key));
        assert_eq!(
            tl.label, *expected,
            "{}: tick label for {} differs",
            scale_id, key
        );
    }
}

fn assert_skin_match(runtime: &Skin, json_path: &PathBuf) {
    let text = fs::read_to_string(json_path).unwrap();
    let j: JsonSkin = serde_json::from_str(&text).unwrap();
    assert_eq!(runtime.id, j.id);
    assert_eq!(runtime.title, j.title);
    assert_eq!(runtime.scale.id, j.scale_id);

    let bal = format!("{:?}", runtime.default_ballistic);
    assert_eq!(bal, j.default_ballistic, "{}: default_ballistic", j.id);

    let mt = match runtime.meter_type {
        MeterType::Bargraph => "bargraph",
        MeterType::Needle => "needle",
        MeterType::Numeric => "numeric",
        MeterType::LufsGauge => "lufs_gauge",
    };
    assert_eq!(mt, j.meter_type, "{}: meter_type", j.id);

    assert_color_match(&j.id, "palette.Safe", runtime.palette.safe, &j.palette.safe);
    assert_color_match(
        &j.id,
        "palette.Nominal",
        runtime.palette.nominal,
        &j.palette.nominal,
    );
    assert_color_match(
        &j.id,
        "palette.Caution",
        runtime.palette.caution,
        &j.palette.caution,
    );
    assert_color_match(&j.id, "palette.Hot", runtime.palette.hot, &j.palette.hot);
    assert_color_match(&j.id, "palette.Over", runtime.palette.over, &j.palette.over);

    if let Some(secs) = j.secondary_colors {
        check_secondary(&j.id, &runtime.secondary, &secs);
    }

    let orient = match runtime.layout.orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    };
    assert_eq!(orient, j.layout.orientation, "{}: orientation", j.id);
    assert_eq!(
        runtime.layout.aspect_ratio, j.layout.aspect_ratio,
        "{}: aspect_ratio",
        j.id
    );
    if let Some(n) = j.layout.led_count {
        assert_eq!(runtime.layout.led_count, n, "{}: led_count", j.id);
    }
    if let Some(p) = j.layout.peak_hold_ms {
        assert_eq!(runtime.layout.peak_hold_ms, p, "{}: peak_hold_ms", j.id);
    }
}

fn check_secondary(skin_id: &str, runtime: &SecondaryColors, json: &BTreeMap<String, String>) {
    macro_rules! check {
        ($key:literal, $field:ident) => {
            if let Some(hex) = json.get($key) {
                let rt = runtime.$field.unwrap_or_else(|| {
                    panic!("{}: secondary.{} missing in runtime", skin_id, $key)
                });
                assert_color_match(skin_id, concat!("secondary.", $key), rt, hex);
            }
        };
    }
    check!("background", background);
    check!("frame", frame);
    check!("scale_text", scale_text);
    check!("minor_tick", minor_tick);
    check!("major_tick", major_tick);
    check!("needle", needle);
    check!("needle_pivot", needle_pivot);
    check!("led_off", led_off);
    check!("peak_hold", peak_hold);
}

#[test]
fn scales_match_json() {
    let dir = assets_root().join("scales");
    assert_scale_match(&presets::SCALE_VU_BROADCAST, &dir.join("vu_broadcast.json"));
    assert_scale_match(&presets::SCALE_VU_EBU, &dir.join("vu_ebu.json"));
    assert_scale_match(&presets::SCALE_DIGITAL_PEAK, &dir.join("digital_peak.json"));
    assert_scale_match(
        &presets::SCALE_LUFS_EBU_R128,
        &dir.join("lufs_ebu_r128.json"),
    );
}

#[test]
fn skins_match_json() {
    let dir = assets_root().join("skins");
    assert_skin_match(
        &presets::BROADCAST_CLASSIC_BARGRAPH,
        &dir.join("broadcast_classic_bargraph.json"),
    );
    assert_skin_match(
        &presets::EBU_CLASSIC_BARGRAPH,
        &dir.join("ebu_classic_bargraph.json"),
    );
    assert_skin_match(
        &presets::DIGITAL_STUDIO_BARGRAPH,
        &dir.join("digital_studio_bargraph.json"),
    );
    assert_skin_match(
        &presets::DIGITAL_STUDIO_NUMERIC,
        &dir.join("digital_studio_numeric.json"),
    );
    assert_skin_match(
        &presets::BROADCAST_CLASSIC_NEEDLE,
        &dir.join("broadcast_classic_needle.json"),
    );
    assert_skin_match(
        &presets::LUFS_EBU_R128_GAUGE,
        &dir.join("lufs_ebu_r128_gauge.json"),
    );
}

/// `Zone` is required to be exported so this integration test sees it.
fn _zone_export_check(_z: &Zone) {}
fn _palette_export_check(_p: &Palette) {}
fn _layout_export_check(_l: &Layout) {}
