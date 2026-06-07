//! Console demo for the rlvgl audio-meters Rust widgets. Mirror of
//! `audio-meters-widgets/ts/examples/cli-demo.ts`.
//!
//! Drives all four widget families (LedBargraph, NeedleVu,
//! NumericPeak, LufsGauge) with a synthetic 12-second dBFS sequence
//! and prints per-frame readings plus an ASCII bargraph to stdout.
//!
//! Run with:
//!
//!   cargo run --release --example audio_meters_console -p rlvgl-widgets
//!
//! (Release mode is slightly faster but not required — the demo
//! finishes in well under a second on debug builds.)
//!
//! Replace `synthetic()` with a real audio source's dBFS detector to
//! drive a live signal. See
//! `docs/audio-meters/10-integration.md` for the documented wiring
//! recipes.

use rlvgl_core::widget::Rect;
use rlvgl_widgets::meters::{LedBargraph, LufsGauge, NeedleVu, NumericPeak, Scale, presets};

/// Synthetic dBFS sequence over 12 seconds. Same shape as the TS
/// cli-demo so output can be compared frame-for-frame between
/// runtimes.
fn synthetic(t: f32) -> f32 {
    if t < 2.0 {
        -120.0
    } else if t < 4.0 {
        -60.0 + ((t - 2.0) / 2.0) * 50.0
    } else if t < 7.0 {
        -10.0
    } else if t < 9.0 {
        let cycle = (t - 7.0) % 0.5;
        if cycle < 0.1 { -1.0 } else { -10.0 }
    } else {
        -120.0
    }
}

fn ascii_bar(dbfs: f32, scale: &Scale) -> String {
    let sv = scale.dbfs_to_scale_units(dbfs);
    let lo = scale.range_min_db;
    let hi = scale.range_max_db;
    let t = ((sv - lo) / (hi - lo)).clamp(0.0, 1.0);
    let n: i32 = 16;
    let lit = (t * n as f32 + 0.5) as i32;
    let mut out = String::with_capacity((n as usize) + 2);
    out.push('▕');
    for i in 0..n {
        out.push(if i < lit { '▆' } else { '·' });
    }
    out.push('▏');
    out
}

fn main() {
    let mut bar = LedBargraph::new(
        Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 320,
        },
        &presets::BROADCAST_CLASSIC_BARGRAPH,
    );
    let mut needle = NeedleVu::new(
        Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        },
        &presets::BROADCAST_CLASSIC_NEEDLE,
    );
    let mut numeric = NumericPeak::new(
        Rect {
            x: 0,
            y: 0,
            width: 220,
            height: 88,
        },
        &presets::DIGITAL_STUDIO_NUMERIC,
    );
    let mut lufs = LufsGauge::new(
        Rect {
            x: 0,
            y: 0,
            width: 280,
            height: 140,
        },
        &presets::LUFS_EBU_R128_GAUGE,
    );

    let dt: f32 = 1.0 / 60.0;
    let total_frames: usize = 12 * 60;
    let print_every: usize = 30;

    println!("== rlvgl audio meters Rust console demo ==");
    println!("  t   | Bargraph (VU)        | Needle (VU) | Numeric (dBFS)       | LUFS gauge");
    println!(
        "------+----------------------+-------------+----------------------+----------------------------"
    );

    for f in 0..total_frames {
        let t = f as f32 * dt;
        let dbfs = synthetic(t);
        bar.update(dbfs, dt);
        let _ = needle.update(dbfs, dt);
        let _ = numeric.update(dbfs, dt);
        lufs.update(dbfs, dt);

        if f % print_every == 0 {
            let bar_str = ascii_bar(bar.reading_db(), bar.skin().scale);
            let ang_deg = needle.needle_angle_rad().to_degrees() as i32;
            let needle_str = format!("{:>4}°", ang_deg);
            let num_str = format!(
                "{:>7.1} PK {:>7.1}",
                numeric.reading_db(),
                numeric.peak_db()
            );
            let lufs_str = format!(
                "I={:>6.1} S={:>6.1} M={:>6.1}",
                lufs.integrated_db(),
                lufs.short_term_db(),
                lufs.momentary_db()
            );
            println!(
                "{:>4.1}  | {} | {:<11} | {} | {}",
                t, bar_str, needle_str, num_str, lufs_str
            );
        }
    }

    println!(
        "\nDone — replace `synthetic()` with real dBFS from your audio source. \
         See docs/audio-meters/10-integration.md for wiring recipes."
    );
}
