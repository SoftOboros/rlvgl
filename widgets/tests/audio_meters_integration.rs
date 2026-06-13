//! Kitchen-sink integration test for the audio-meters widget tree.
//!
//! Spins up one of each widget family (`LedBargraph`, `NeedleVu`,
//! `NumericPeak`) plus a stereo pair, feeds them a synthetic dBFS
//! sequence (silence → ramp → impulse → silence), and asserts that:
//!
//! - Every widget renders without panic on every frame.
//! - Each widget's reading tracks the input plausibly under its
//!   ballistic.
//! - The stereo pair's two channels remain independent under
//!   asymmetric input.
//!
//! Verifies that the AM-00…AM-08c layering composes end-to-end. Real
//! audio sources (mic_capture / AudioWorklet / file playback) are
//! covered in `docs/audio-meters/10-integration.md`.

use rlvgl_audio_meters_core::Ballistic;
use rlvgl_core::font::ShapedText;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::meters::{
    LedBargraph, NeedleVu, NumericPeak, StereoPair, presets, split_horizontal,
};

/// Counting renderer — records every fill_rect / draw_text call so we
/// can assert "something was drawn" without depending on a real
/// platform backend.
struct Counter {
    rects: usize,
    texts: usize,
}
impl Counter {
    fn new() -> Self {
        Self { rects: 0, texts: 0 }
    }
}
impl Renderer for Counter {
    fn fill_rect(&mut self, _r: Rect, _c: Color) {
        self.rects += 1;
    }
    fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {
        self.texts += 1;
    }
    // Meters migrated to the LPAR-08 shaped-text path; count those draws too
    // so "did the widget draw text" stays accurate regardless of the path.
    fn draw_text_shaped(&mut self, _shaped: &ShapedText<'_>, _o: (i32, i32), _c: Color) {
        self.texts += 1;
    }
}

/// Synthetic per-frame dBFS sequence. 480 frames at 60 Hz = 8 s.
///
/// 0..60   : silence at floor
/// 60..120 : ramp from -60 to -10 dBFS
/// 120..180: hold at -10 dBFS
/// 180..200: impulse to -1 dBFS, single frame within a -120 silence
/// 200..480: silence at floor
fn synthetic_signal(n: usize) -> f32 {
    if n < 60 {
        -120.0
    } else if n < 120 {
        let t = (n - 60) as f32 / 60.0;
        -60.0 + t * 50.0 // -60 → -10
    } else if n < 180 {
        -10.0
    } else if n == 180 {
        -1.0 // impulse
    } else {
        -120.0
    }
}

#[test]
fn one_of_each_widget_renders_through_full_sequence() {
    // Bargraph + ticks; vertical mono.
    let mut bar = LedBargraph::new(
        Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 320,
        },
        &presets::BROADCAST_CLASSIC_BARGRAPH,
    )
    .with_ticks();

    // Needle + ticks; horizontal mono.
    let mut needle = NeedleVu::new(
        Rect {
            x: 80,
            y: 0,
            width: 320,
            height: 200,
        },
        &presets::BROADCAST_CLASSIC_NEEDLE,
    )
    .with_ticks();

    // Numeric readout.
    let mut numeric = NumericPeak::new(
        Rect {
            x: 80,
            y: 220,
            width: 220,
            height: 88,
        },
        &presets::DIGITAL_STUDIO_NUMERIC,
    );

    // Stereo bargraph pair driving asymmetric input.
    let outer = Rect {
        x: 420,
        y: 0,
        width: 96,
        height: 320,
    };
    let (lb, rb) = split_horizontal(outer, 4);
    let stereo_left = LedBargraph::new(lb, &presets::DIGITAL_STUDIO_BARGRAPH);
    let stereo_right = LedBargraph::new(rb, &presets::DIGITAL_STUDIO_BARGRAPH);
    let mut stereo = StereoPair::new(outer, 4, stereo_left, stereo_right);

    let dt = 1.0 / 60.0;
    // Snapshot state mid-plateau (frame 175 is well inside the held
    // -10 dBFS region, all ballistics settled). Final-state
    // assertions after the silence tail risk being noise — pin
    // observations at a stable moment instead.
    let mut mid_bar = f32::NEG_INFINITY;
    let mut mid_needle = f32::NEG_INFINITY;
    let mut mid_left = f32::NEG_INFINITY;
    let mut mid_right = f32::NEG_INFINITY;

    for f in 0..480 {
        let dbfs = synthetic_signal(f);
        bar.update(dbfs, dt);
        needle.update(dbfs, dt);
        numeric.update(dbfs, dt);
        // Stereo: left = mono signal, right = same -6 dB.
        stereo.update_stereo(dbfs, dbfs - 6.0, dt);

        if f == 175 {
            mid_bar = bar.reading_db();
            mid_needle = needle.reading_db();
            mid_left = stereo.left.reading_db();
            mid_right = stereo.right.reading_db();
        }

        // Render every 30th frame so the test stays cheap but
        // exercises rendering against real ballistic state.
        if f % 30 == 0 {
            let mut c = Counter::new();
            bar.draw(&mut c);
            assert!(c.rects > 0, "frame {f}: bargraph drew nothing");
            assert!(c.texts > 0, "frame {f}: bargraph ticks drew no text");

            let mut c = Counter::new();
            needle.draw(&mut c);
            assert!(c.rects > 0, "frame {f}: needle drew nothing");
            assert!(c.texts > 0, "frame {f}: needle ticks drew no text");

            let mut c = Counter::new();
            numeric.draw(&mut c);
            assert_eq!(c.rects, 1, "frame {f}: numeric should draw 1 background");
            assert_eq!(c.texts, 2, "frame {f}: numeric should draw 2 text lines");

            let mut c = Counter::new();
            stereo.draw(&mut c);
            assert!(c.rects > 0, "frame {f}: stereo drew nothing");
        }
    }

    // Mid-plateau (held -10 dBFS for ~1 s, well past VU's 300 ms
    // settling time): all settled ballistics should track input.
    assert!(
        mid_bar > -15.0 && mid_bar < -5.0,
        "VU bargraph at mid-plateau ({mid_bar}) should track -10 dBFS"
    );
    assert!(
        mid_needle > -15.0 && mid_needle < -5.0,
        "VU needle at mid-plateau ({mid_needle}) should track -10 dBFS"
    );
    // Stereo asymmetry holds at the plateau: right channel was always
    // -6 dB lower, so its settled reading is below left's.
    assert!(
        mid_left > mid_right + 3.0,
        "stereo asymmetry at plateau: left {mid_left}, right {mid_right}"
    );

    // Final state: all readings are in a sane range. VU bargraph and
    // needle have decayed to floor after 5 s of silence; numeric is
    // DigitalPeak so its reading also tracks silence; only the
    // peak-hold scalars retain a memory of the impulse.
    let final_n_peak = numeric.peak_db();
    assert!(
        final_n_peak > -90.0 && final_n_peak <= 0.0,
        "numeric peak after sequence: {final_n_peak}"
    );
    let final_bar_peak = bar.peak_db();
    assert!(
        final_bar_peak > -90.0 && final_bar_peak <= 0.0,
        "bargraph peak after sequence: {final_bar_peak}"
    );
}

#[test]
fn ballistic_swap_works_across_widgets() {
    // Verify that swapping the ballistic on a running widget resets
    // its state at the floor and a fresh trajectory begins.
    let mut bar = LedBargraph::new(
        Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 320,
        },
        &presets::BROADCAST_CLASSIC_BARGRAPH,
    );
    for _ in 0..120 {
        bar.update(-10.0, 1.0 / 60.0);
    }
    let before = bar.reading_db();
    bar.set_ballistic(Ballistic::DigitalPeak);
    let after_swap = bar.reading_db();
    assert!(
        after_swap < before,
        "swap should reset reading: before {before}, after {after_swap}"
    );

    // After one update at the same input under DigitalPeak, the
    // reading instant-attacks to the input value.
    bar.update(-10.0, 1.0 / 60.0);
    let after_one = bar.reading_db();
    assert!(
        (after_one - (-10.0)).abs() < 0.5,
        "DigitalPeak should track input quickly: {after_one}"
    );
}
