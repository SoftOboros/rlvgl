//! Hand-authored `Scale` and `Skin` constants matching the JSON files
//! under `assets/audio-meters/{scales,skins}/`. These are the path-(b)
//! integration shape from AM-05/06: app code references these
//! constants directly.
//!
//! The path-(a) replacement — `rlvgl-creator meters from-yaml` emitting
//! these constants from JSON — lands in AM-04b. When that ships, this
//! file becomes the codegen target rather than hand-authored.
//!
//! **Editing rule:** if you change a value here, you MUST also update
//! the corresponding JSON descriptor under
//! `assets/audio-meters/{scales,skins}/`. The `presets_match_json` test
//! will catch divergence on most fields.

use rlvgl_audio_meters_core::Ballistic;

use super::skin::{
    Layout, MeterColorId, MeterType, Orientation, Palette, Scale, SecondaryColors, Skin, TickLabel,
    Zone, rgb,
};

// ---- Scales ---------------------------------------------------------

/// `vu_broadcast` — US/SMPTE VU. `0 VU = -20 dBFS = +4 dBu`.
pub static SCALE_VU_BROADCAST: Scale = Scale {
    id: "vu_broadcast",
    label_units: "dBVU",
    range_min_db: -20.0,
    range_max_db: 3.0,
    pivot_value: 0.0,
    pivot_label: "0",
    pivot_input_dbfs: -20.0,
    calibration_offset_db: Some(24.0),
    majors: &[-20.0, -10.0, -7.0, -5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 3.0],
    minors_per_major_division: 4,
    tick_labels: &[
        TickLabel { value: -20.0, label: "−20" },
        TickLabel { value: -10.0, label: "−10" },
        TickLabel { value: -7.0,  label: "−7"  },
        TickLabel { value: -5.0,  label: "−5"  },
        TickLabel { value: -3.0,  label: "−3"  },
        TickLabel { value: -1.0,  label: "−1"  },
        TickLabel { value:  0.0,  label: "0"   },
        TickLabel { value:  1.0,  label: "+1"  },
        TickLabel { value:  2.0,  label: "+2"  },
        TickLabel { value:  3.0,  label: "+3"  },
    ],
    zones: &[
        Zone {
            from_db: -20.0,
            to_db: -3.0,
            color: MeterColorId::Safe,
        },
        Zone {
            from_db: -3.0,
            to_db: 0.0,
            color: MeterColorId::Nominal,
        },
        Zone {
            from_db: 0.0,
            to_db: 1.0,
            color: MeterColorId::Caution,
        },
        Zone {
            from_db: 1.0,
            to_db: 3.0,
            color: MeterColorId::Hot,
        },
    ],
};

/// `vu_ebu` — EBU VU. `0 VU = -18 dBFS = 0 dBu`.
pub static SCALE_VU_EBU: Scale = Scale {
    id: "vu_ebu",
    label_units: "dBVU",
    range_min_db: -18.0,
    range_max_db: 3.0,
    pivot_value: 0.0,
    pivot_label: "0",
    pivot_input_dbfs: -18.0,
    calibration_offset_db: Some(18.0),
    majors: &[-18.0, -10.0, -7.0, -5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 3.0],
    minors_per_major_division: 4,
    tick_labels: &[
        TickLabel { value: -18.0, label: "−18" },
        TickLabel { value: -10.0, label: "−10" },
        TickLabel { value: -7.0,  label: "−7"  },
        TickLabel { value: -5.0,  label: "−5"  },
        TickLabel { value: -3.0,  label: "−3"  },
        TickLabel { value: -1.0,  label: "−1"  },
        TickLabel { value:  0.0,  label: "0"   },
        TickLabel { value:  1.0,  label: "+1"  },
        TickLabel { value:  2.0,  label: "+2"  },
        TickLabel { value:  3.0,  label: "+3"  },
    ],
    zones: &[
        Zone {
            from_db: -18.0,
            to_db: -3.0,
            color: MeterColorId::Safe,
        },
        Zone {
            from_db: -3.0,
            to_db: 0.0,
            color: MeterColorId::Nominal,
        },
        Zone {
            from_db: 0.0,
            to_db: 1.0,
            color: MeterColorId::Caution,
        },
        Zone {
            from_db: 1.0,
            to_db: 3.0,
            color: MeterColorId::Hot,
        },
    ],
};

/// `digital_peak` — AES17 dBFS, range −60…0.
pub static SCALE_DIGITAL_PEAK: Scale = Scale {
    id: "digital_peak",
    label_units: "dBFS",
    range_min_db: -60.0,
    range_max_db: 0.0,
    pivot_value: 0.0,
    pivot_label: "0",
    pivot_input_dbfs: 0.0,
    calibration_offset_db: None,
    majors: &[-60.0, -50.0, -40.0, -30.0, -20.0, -12.0, -6.0, -3.0, 0.0],
    minors_per_major_division: 5,
    tick_labels: &[
        TickLabel { value: -60.0, label: "−60" },
        TickLabel { value: -50.0, label: "−50" },
        TickLabel { value: -40.0, label: "−40" },
        TickLabel { value: -30.0, label: "−30" },
        TickLabel { value: -20.0, label: "−20" },
        TickLabel { value: -12.0, label: "−12" },
        TickLabel { value:  -6.0, label: "−6"  },
        TickLabel { value:  -3.0, label: "−3"  },
        TickLabel { value:   0.0, label: "0"   },
    ],
    zones: &[
        Zone {
            from_db: -60.0,
            to_db: -12.0,
            color: MeterColorId::Safe,
        },
        Zone {
            from_db: -12.0,
            to_db: -3.0,
            color: MeterColorId::Nominal,
        },
        Zone {
            from_db: -3.0,
            to_db: -0.1,
            color: MeterColorId::Caution,
        },
        Zone {
            from_db: -0.1,
            to_db: 0.0,
            color: MeterColorId::Over,
        },
    ],
};

// ---- Skins ----------------------------------------------------------

/// `broadcast_classic_bargraph` — US LED bargraph; classic green/amber/red.
pub static BROADCAST_CLASSIC_BARGRAPH: Skin = Skin {
    id: "broadcast_classic_bargraph",
    title: "Broadcast Classic — LED Bargraph",
    scale: &SCALE_VU_BROADCAST,
    default_ballistic: Ballistic::Vu,
    meter_type: MeterType::Bargraph,
    palette: Palette {
        safe: rgb(0x1f, 0x9d, 0x4f),
        nominal: rgb(0x3e, 0xd2, 0x7a),
        caution: rgb(0xe6, 0xb2, 0x2c),
        hot: rgb(0xd2, 0x4b, 0x2e),
        over: rgb(0xff, 0x2a, 0x1f),
    },
    secondary: SecondaryColors {
        background: Some(rgb(0x0c, 0x0c, 0x0e)),
        frame: Some(rgb(0x2a, 0x2a, 0x2e)),
        scale_text: Some(rgb(0xcf, 0xcf, 0xd2)),
        minor_tick: Some(rgb(0x5b, 0x5b, 0x62)),
        major_tick: Some(rgb(0xa0, 0xa0, 0xa8)),
        needle: None,
        needle_pivot: None,
        led_off: Some(rgb(0x1c, 0x1c, 0x1f)),
        peak_hold: Some(rgb(0xff, 0xff, 0xff)),
    },
    layout: Layout {
        orientation: Orientation::Vertical,
        aspect_ratio: 0.18,
        led_count: 32,
        peak_hold_ms: 1200.0,
    },
};

/// `ebu_classic_bargraph` — EBU LED bargraph; teal-tinted Safe band.
pub static EBU_CLASSIC_BARGRAPH: Skin = Skin {
    id: "ebu_classic_bargraph",
    title: "EBU Classic — LED Bargraph",
    scale: &SCALE_VU_EBU,
    default_ballistic: Ballistic::Vu,
    meter_type: MeterType::Bargraph,
    palette: Palette {
        safe: rgb(0x1f, 0x7d, 0x8c),
        nominal: rgb(0x3e, 0xb1, 0xc4),
        caution: rgb(0xe6, 0xb2, 0x2c),
        hot: rgb(0xd2, 0x4b, 0x2e),
        over: rgb(0xff, 0x2a, 0x1f),
    },
    secondary: SecondaryColors {
        background: Some(rgb(0x0a, 0x0c, 0x0d)),
        frame: Some(rgb(0x23, 0x28, 0x2b)),
        scale_text: Some(rgb(0xd6, 0xdc, 0xde)),
        minor_tick: Some(rgb(0x56, 0x63, 0x6a)),
        major_tick: Some(rgb(0x9a, 0xa6, 0xad)),
        needle: None,
        needle_pivot: None,
        led_off: Some(rgb(0x16, 0x1a, 0x1c)),
        peak_hold: Some(rgb(0xff, 0xff, 0xff)),
    },
    layout: Layout {
        orientation: Orientation::Vertical,
        aspect_ratio: 0.18,
        led_count: 32,
        peak_hold_ms: 1500.0,
    },
};

/// `broadcast_classic_needle` — Cream-faced analog VU look over `vu_broadcast`.
pub static BROADCAST_CLASSIC_NEEDLE: Skin = Skin {
    id: "broadcast_classic_needle",
    title: "Broadcast Classic — Analog Needle",
    scale: &SCALE_VU_BROADCAST,
    default_ballistic: Ballistic::Vu,
    meter_type: MeterType::Needle,
    palette: Palette {
        safe: rgb(0x1f, 0x1f, 0x1f),
        nominal: rgb(0x1f, 0x1f, 0x1f),
        caution: rgb(0xa0, 0x7b, 0x1c),
        hot: rgb(0xa7, 0x2b, 0x1f),
        over: rgb(0xd1, 0x1a, 0x1a),
    },
    secondary: SecondaryColors {
        background: Some(rgb(0xf1, 0xe7, 0xc4)),
        frame: Some(rgb(0x3a, 0x2a, 0x18)),
        scale_text: Some(rgb(0x1a, 0x1a, 0x1a)),
        minor_tick: Some(rgb(0x3a, 0x2a, 0x18)),
        major_tick: Some(rgb(0x1a, 0x1a, 0x1a)),
        needle: Some(rgb(0x1a, 0x1a, 0x1a)),
        needle_pivot: Some(rgb(0x3a, 0x2a, 0x18)),
        led_off: None,
        peak_hold: Some(rgb(0xd1, 0x1a, 0x1a)),
    },
    layout: Layout {
        orientation: Orientation::Horizontal,
        aspect_ratio: 1.6,
        led_count: 0,
        peak_hold_ms: 0.0,
    },
};

/// `digital_studio_numeric` — Two-line numeric dBFS readout.
pub static DIGITAL_STUDIO_NUMERIC: Skin = Skin {
    id: "digital_studio_numeric",
    title: "Digital Studio — Numeric Readout",
    scale: &SCALE_DIGITAL_PEAK,
    default_ballistic: Ballistic::DigitalPeak,
    meter_type: MeterType::Numeric,
    palette: Palette {
        safe: rgb(0x3e, 0xd2, 0x7a),
        nominal: rgb(0x3e, 0xd2, 0x7a),
        caution: rgb(0xe6, 0xb2, 0x2c),
        hot: rgb(0xd2, 0x4b, 0x2e),
        over: rgb(0xff, 0x2a, 0x1f),
    },
    secondary: SecondaryColors {
        background: Some(rgb(0x08, 0x08, 0x0a)),
        frame: Some(rgb(0x1d, 0x21, 0x26)),
        scale_text: Some(rgb(0xdd, 0xe2, 0xe6)),
        minor_tick: None,
        major_tick: None,
        needle: None,
        needle_pivot: None,
        led_off: None,
        peak_hold: Some(rgb(0xff, 0xe3, 0x5a)),
    },
    layout: Layout {
        orientation: Orientation::Horizontal,
        aspect_ratio: 2.5,
        led_count: 0,
        peak_hold_ms: 1500.0,
    },
};

/// `digital_studio_bargraph` — High-resolution dBFS peak meter.
pub static DIGITAL_STUDIO_BARGRAPH: Skin = Skin {
    id: "digital_studio_bargraph",
    title: "Digital Studio — Peak Bargraph",
    scale: &SCALE_DIGITAL_PEAK,
    default_ballistic: Ballistic::DigitalPeak,
    meter_type: MeterType::Bargraph,
    palette: Palette {
        safe: rgb(0x2e, 0x7d, 0xd1),
        nominal: rgb(0x3e, 0xd2, 0x7a),
        caution: rgb(0xe6, 0xb2, 0x2c),
        hot: rgb(0xd2, 0x4b, 0x2e),
        over: rgb(0xff, 0x2a, 0x1f),
    },
    secondary: SecondaryColors {
        background: Some(rgb(0x08, 0x0a, 0x0c)),
        frame: Some(rgb(0x1d, 0x21, 0x26)),
        scale_text: Some(rgb(0xdd, 0xe2, 0xe6)),
        minor_tick: Some(rgb(0x4d, 0x55, 0x5c)),
        major_tick: Some(rgb(0x8e, 0x98, 0xa0)),
        needle: None,
        needle_pivot: None,
        led_off: Some(rgb(0x11, 0x14, 0x1a)),
        peak_hold: Some(rgb(0xff, 0xe3, 0x5a)),
    },
    layout: Layout {
        orientation: Orientation::Vertical,
        aspect_ratio: 0.16,
        led_count: 48,
        peak_hold_ms: 1500.0,
    },
};
