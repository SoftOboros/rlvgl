//! Tests for animation primitives.
use rlvgl_core::animation::{
    Easing, FadeTransition, Fade, KeyFade, LoopMode, Motion, Slide, Timeline,
};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect};

#[test]
fn fade_updates_bg_color() {
    let mut style = Style::default();
    style.bg_color = Color(0, 0, 0, 255);
    let start_color = style.bg_color;
    let mut timeline = Timeline::new();
    timeline.add_fade(Fade::new(
        &mut style,
        start_color,
        Color(255, 0, 0, 255),
        100,
    ));

    timeline.tick(50);
    assert_eq!(style.bg_color, Color(127, 0, 0, 255));
    assert!(!timeline.is_empty());

    timeline.tick(50);
    assert_eq!(style.bg_color, Color(255, 0, 0, 255));
    assert!(timeline.is_empty());
}

#[test]
fn slide_moves_rect() {
    let mut rect = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let start = rect;
    let end = Rect {
        x: 10,
        y: 0,
        width: 10,
        height: 10,
    };
    let mut timeline = Timeline::new();
    timeline.add_slide(Slide::new(&mut rect, start, end, 100));

    timeline.tick(30);
    assert_eq!(rect.x, 3);
    assert!(!timeline.is_empty());

    timeline.tick(70);
    assert_eq!(rect.x, 10);
    assert!(timeline.is_empty());
}

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

#[test]
fn easing_linear_identity() {
    assert_eq!(Easing::Linear.apply(0.0), 0.0);
    assert_eq!(Easing::Linear.apply(0.5), 0.5);
    assert_eq!(Easing::Linear.apply(1.0), 1.0);
}

#[test]
fn easing_ease_in_quadratic() {
    assert_eq!(Easing::EaseIn.apply(0.0), 0.0);
    assert!((Easing::EaseIn.apply(0.5) - 0.25).abs() < 0.001);
    assert_eq!(Easing::EaseIn.apply(1.0), 1.0);
}

#[test]
fn easing_ease_out_quadratic() {
    assert_eq!(Easing::EaseOut.apply(0.0), 0.0);
    assert!((Easing::EaseOut.apply(0.5) - 0.75).abs() < 0.001);
    assert_eq!(Easing::EaseOut.apply(1.0), 1.0);
}

#[test]
fn easing_ease_in_out_endpoints() {
    assert_eq!(Easing::EaseInOut.apply(0.0), 0.0);
    assert!((Easing::EaseInOut.apply(0.5) - 0.5).abs() < 0.001);
    assert_eq!(Easing::EaseInOut.apply(1.0), 1.0);
}

#[test]
fn easing_step_quantizes() {
    let e = Easing::Step(4);
    assert_eq!(e.apply(0.0), 0.0);
    assert_eq!(e.apply(0.24), 0.0);
    assert_eq!(e.apply(0.25), 0.25);
    assert_eq!(e.apply(0.49), 0.25);
    assert_eq!(e.apply(0.5), 0.5);
    assert_eq!(e.apply(0.99), 0.75);
}

#[test]
fn easing_clamps_input() {
    assert_eq!(Easing::Linear.apply(-0.5), 0.0);
    assert_eq!(Easing::Linear.apply(1.5), 1.0);
}

#[test]
fn easing_bounce_endpoints() {
    assert!((Easing::Bounce.apply(0.0)).abs() < 0.01);
    assert!((Easing::Bounce.apply(1.0) - 1.0).abs() < 0.01);
}

// ---------------------------------------------------------------------------
// LoopMode
// ---------------------------------------------------------------------------

#[test]
fn loop_mode_once_finishes() {
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let start = rect;
    let end = Rect { x: 100, y: 0, width: 10, height: 10 };
    let mut m = Motion::new(&mut rect, start, end, 100);
    m.tick(100);
    assert!(m.finished());
    assert_eq!(rect.x, 100);
}

#[test]
fn loop_mode_repeat_cycles() {
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let start = rect;
    let end = Rect { x: 100, y: 0, width: 10, height: 10 };
    let mut m = Motion::new(&mut rect, start, end, 100).with_loop(LoopMode::Repeat(2));

    m.tick(50);
    assert!(!m.finished());
    assert_eq!(rect.x, 50);

    // Complete first cycle, start second
    m.tick(70);
    assert!(!m.finished());
    // elapsed=120, cycle 1, within=20 → t=0.2 → x=20
    assert_eq!(rect.x, 20);

    // Complete second cycle
    m.tick(100);
    assert!(m.finished());
    assert_eq!(rect.x, 100); // ends at final position
}

#[test]
fn loop_mode_repeat_infinite() {
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let start = rect;
    let end = Rect { x: 100, y: 0, width: 10, height: 10 };
    let mut m = Motion::new(&mut rect, start, end, 100).with_loop(LoopMode::Repeat(0));

    m.tick(250);
    assert!(!m.finished()); // infinite loop never finishes
    // elapsed=250, within=50 → t=0.5 → x=50
    assert_eq!(rect.x, 50);
}

#[test]
fn loop_mode_pingpong() {
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let start = rect;
    let end = Rect { x: 100, y: 0, width: 10, height: 10 };
    let mut m = Motion::new(&mut rect, start, end, 100).with_loop(LoopMode::PingPong(1));

    // Forward half
    m.tick(50);
    assert_eq!(rect.x, 50);

    // Reach end, start reverse
    m.tick(70);
    // elapsed=120, half_cycle=1 (reverse), within=20 → t=1.0-0.2=0.8 → x=80
    assert_eq!(rect.x, 80);

    // Complete round-trip
    m.tick(100);
    assert!(m.finished());
    assert_eq!(rect.x, 0); // returns to start
}

// ---------------------------------------------------------------------------
// Motion with easing
// ---------------------------------------------------------------------------

#[test]
fn motion_with_ease_in() {
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let start = rect;
    let end = Rect { x: 100, y: 0, width: 10, height: 10 };
    let mut m = Motion::new(&mut rect, start, end, 100).with_easing(Easing::EaseIn);

    m.tick(50);
    // EaseIn: t=0.5 → t²=0.25 → x=25
    assert_eq!(rect.x, 25);

    m.tick(50);
    assert_eq!(rect.x, 100);
}

// ---------------------------------------------------------------------------
// FadeTransition
// ---------------------------------------------------------------------------

#[test]
fn fade_transition_linear() {
    let mut alpha: u8 = 255;
    let mut ft = FadeTransition::new(&mut alpha, 255, 0, 100);

    ft.tick(50);
    assert_eq!(alpha, 127);

    ft.tick(50);
    assert_eq!(alpha, 0);
    assert!(ft.finished());
}

#[test]
fn fade_transition_with_easing_and_loop() {
    let mut alpha: u8 = 0;
    let mut ft = FadeTransition::new(&mut alpha, 0, 255, 100)
        .with_easing(Easing::EaseIn)
        .with_loop(LoopMode::Repeat(2));

    ft.tick(50);
    // EaseIn: t=0.5 → 0.25 → alpha=63
    assert_eq!(alpha, 63);
    assert!(!ft.finished());
}

// ---------------------------------------------------------------------------
// KeyFade
// ---------------------------------------------------------------------------

#[test]
fn key_fade_two_keys_equivalent_to_fade() {
    let mut alpha: u8 = 255;
    let mut kf = KeyFade::new(&mut alpha)
        .key(0, 255, Easing::Linear)
        .key(100, 0, Easing::Linear);

    kf.tick(50);
    assert_eq!(alpha, 127);

    kf.tick(50);
    assert_eq!(alpha, 0);
    assert!(kf.finished());
}

#[test]
fn key_fade_three_keys() {
    let mut alpha: u8 = 0;
    let mut kf = KeyFade::new(&mut alpha)
        .key(0, 0, Easing::Linear)
        .key(100, 255, Easing::Linear)
        .key(200, 128, Easing::Linear);

    // At t=50ms: segment 0→100, local t=0.5, alpha = 0 + 255*0.5 = 127
    kf.tick(50);
    assert_eq!(alpha, 127);

    // At t=150ms: segment 100→200, local t=0.5, alpha = 255 + (128-255)*0.5 = 191
    kf.tick(100);
    assert_eq!(alpha, 191);

    // At t=200ms: done
    kf.tick(50);
    assert_eq!(alpha, 128);
    assert!(kf.finished());
}

#[test]
fn key_fade_with_per_segment_easing() {
    let mut alpha: u8 = 0;
    let mut kf = KeyFade::new(&mut alpha)
        .key(0, 0, Easing::EaseIn)      // ease-in from 0→255
        .key(100, 255, Easing::Linear);  // (last key easing ignored)

    // At t=50ms: EaseIn on segment 0→100, local t=0.5, eased=0.25, alpha=63
    kf.tick(50);
    assert_eq!(alpha, 63);
}

#[test]
fn key_fade_looping() {
    let mut alpha: u8 = 0;
    let mut kf = KeyFade::new(&mut alpha)
        .key(0, 0, Easing::Linear)
        .key(100, 255, Easing::Linear)
        .with_loop(LoopMode::Repeat(0)); // infinite

    kf.tick(50);
    assert_eq!(alpha, 127);

    kf.tick(75);
    // elapsed=125, within=25 → t=0.25 → alpha=63
    assert_eq!(alpha, 63);
    assert!(!kf.finished());
}

// ---------------------------------------------------------------------------
// Timeline with new types
// ---------------------------------------------------------------------------

#[test]
fn timeline_mixes_all_types() {
    let mut style = Style::default();
    style.bg_color = Color(0, 0, 0, 255);
    let mut rect = Rect { x: 0, y: 0, width: 10, height: 10 };
    let mut alpha: u8 = 255;

    let mut tl = Timeline::new();
    tl.add_fade(Fade::new(&mut style, Color(0, 0, 0, 255), Color(255, 0, 0, 255), 100));
    tl.add_motion(Motion::new(
        &mut rect,
        Rect { x: 0, y: 0, width: 10, height: 10 },
        Rect { x: 100, y: 0, width: 10, height: 10 },
        100,
    ));
    tl.add_fade_transition(FadeTransition::new(&mut alpha, 255, 0, 100));

    assert!(!tl.is_empty());
    tl.tick(100);
    assert!(tl.is_empty());
    assert_eq!(style.bg_color, Color(255, 0, 0, 255));
    assert_eq!(rect.x, 100);
    assert_eq!(alpha, 0);
}

// ---------------------------------------------------------------------------
// Color::with_alpha
// ---------------------------------------------------------------------------

#[test]
fn color_with_alpha() {
    let c = Color(255, 128, 0, 200);
    let c2 = c.with_alpha(128);
    assert_eq!(c2.0, 255);
    assert_eq!(c2.1, 128);
    assert_eq!(c2.2, 0);
    // 200 * 128 / 255 = 100
    assert_eq!(c2.3, 100);
}

#[test]
fn color_with_alpha_full() {
    let c = Color(0, 0, 0, 255);
    assert_eq!(c.with_alpha(255).3, 255);
    assert_eq!(c.with_alpha(0).3, 0);
}
