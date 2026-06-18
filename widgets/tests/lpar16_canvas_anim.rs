// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixtures for the LPAR-15 Core media widgets.
//!
//! Discharges two rows of the LPAR-16 §6 per-phase fixture ledger for LPAR-15:
//! `CanvasWidget` and `AnimImage`. Both are §5.B **kind 1 — determinism
//! fixtures**: each operation is replayed and the output asserted bit-identical
//! across runs, PLUS a concrete expected-value assertion so the test fails on a
//! stably-wrong result, not only on non-determinism (LPAR-16 §5.B mandate).
//!
//! - Canvas is not tick-driven; the determinism form used here is
//!   reproducible-draw-sequence (same primitive sequence → identical buffer).
//! - AnimImage rides the LPAR-06 §7.2 tick-determinism invariant: a fixed
//!   synthetic tick stream yields a bit-identical frame-index sequence.

use rlvgl_core::event::Event;
use rlvgl_core::image::{ImageData, ImageDescriptor, PixelFormat};
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::anim_image::{AnimImage, AnimImageLoopMode, FrameSource};
use rlvgl_widgets::canvas::CanvasWidget;

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ──────────────────────────────────────────────────────────────────────────
// CanvasWidget — reproducible-draw-sequence determinism (§5.B kind 1)
// ──────────────────────────────────────────────────────────────────────────

/// Run a fixed primitive sequence into an 8×8 buffer and return the pixels.
fn render_canvas() -> CanvasWidget {
    let mut cw = CanvasWidget::new(rect(0, 0, 40, 40), 8, 8);
    cw.fill(Color(10, 20, 30, 255));
    cw.fill_rect(rect(2, 2, 3, 3), Color(200, 100, 50, 255));
    cw.draw_line(0, 0, 7, 7, Color(255, 255, 255, 255));
    cw.draw_pixel(7, 0, Color(1, 2, 3, 255));
    cw
}

#[test]
fn canvas_draw_sequence_is_deterministic_and_correct() {
    // Determinism: the same primitive sequence yields a bit-identical buffer.
    let first = render_canvas().inner().pixels();
    let second = render_canvas().inner().pixels();
    assert_eq!(
        first, second,
        "identical primitive sequence → identical buffer"
    );

    // Concrete expected values — the mandatory kind-1 anti-"stably-wrong"
    // assertion. The diagonal line is drawn last, so it wins over the
    // fill_rect on the (3,3)/(4,4) overlap.
    let cw = render_canvas();
    let px = cw.inner();
    assert_eq!(
        px.get_pixel(0, 0),
        Some(Color(255, 255, 255, 255)),
        "line endpoint (0,0) is white"
    );
    assert_eq!(
        px.get_pixel(7, 7),
        Some(Color(255, 255, 255, 255)),
        "line endpoint (7,7) is white"
    );
    assert_eq!(
        px.get_pixel(2, 3),
        Some(Color(200, 100, 50, 255)),
        "fill_rect interior off the diagonal keeps the rect color"
    );
    assert_eq!(
        px.get_pixel(7, 0),
        Some(Color(1, 2, 3, 255)),
        "draw_pixel overrides the background"
    );
    assert_eq!(
        px.get_pixel(6, 0),
        Some(Color(10, 20, 30, 255)),
        "untouched cell keeps the flood-fill background"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// AnimImage — synthetic-tick-stream determinism (§5.B kind 1; LPAR-06 §7.2)
// ──────────────────────────────────────────────────────────────────────────

/// Build `n` 1×1 placeholder frames (pixel data is irrelevant to frame timing).
fn make_frames(n: usize) -> FrameSource {
    let mut frames: Vec<ImageDescriptor<'static>> = Vec::new();
    for _ in 0..n {
        frames.push(ImageDescriptor::new(
            PixelFormat::Argb8888,
            1,
            1,
            ImageData::Borrowed(&[]),
            None,
        ));
    }
    FrameSource::Decoded(frames)
}

/// Drive `ticks` `Event::Tick`s through a fresh 4-frame Loop animation at one
/// tick per frame, returning the frame index observed after each tick.
fn tick_sequence(ticks: usize) -> Vec<usize> {
    let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(4));
    a.set_ticks_per_frame(1);
    a.set_loop_mode(AnimImageLoopMode::Loop);
    let mut seq = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        a.handle_event(&Event::Tick);
        seq.push(a.current_frame_index());
    }
    seq
}

#[test]
fn anim_image_tick_stream_is_deterministic_and_correct() {
    // Determinism: identical tick stream → identical frame-index sequence
    // (LPAR-06 §7.2 tick-determinism invariant, which AnimImage inherits).
    let first = tick_sequence(12);
    let second = tick_sequence(12);
    assert_eq!(
        first, second,
        "identical tick stream → identical frame sequence"
    );

    // Concrete expected sequence: 4 frames, 1 tick/frame, Loop, starting at 0.
    // 0→1→2→3→wrap→0, three full cycles over 12 ticks.
    assert_eq!(
        first,
        vec![1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0],
        "deterministic Loop cadence over a 12-tick stream"
    );
}
