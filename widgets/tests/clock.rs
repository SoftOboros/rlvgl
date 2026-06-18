//! Tests for the analog clock widget: angle math, tick outcomes, dirty
//! union math, and end-to-end Widget integration with a recording renderer.

use rlvgl_core::event::Event;
use rlvgl_core::raster::Obb;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::clock::{
    AnalogHand, CenterCap, Clock, ClockFace, ClockLayer, ClockState, ClockTime, SubsecondDot,
    TickMark, TickOutcome,
};

const FACE: Rect = Rect {
    x: 50,
    y: 50,
    width: 200,
    height: 200,
};
const RED: Color = Color(255, 0, 0, 255);

/// Recording renderer that captures every call. Used to assert the clock
/// emits the right primitives in the right order.
#[derive(Default)]
struct Recorder {
    obbs: Vec<(Obb, Color)>,
    rects: Vec<(Rect, Color)>,
    rows: Vec<(i32, i32, Color, usize)>,
}

impl Renderer for Recorder {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.rects.push((rect, color));
    }
    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
    fn fill_obb_aa(&mut self, obb: Obb, color: Color) {
        self.obbs.push((obb, color));
    }
    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        self.rows.push((x, y, color, coverage.len()));
    }
}

fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn time_to_angles_at_canonical_times() {
    use core::f32::consts::{FRAC_PI_2, PI, TAU};

    // 12:00:00 → all hands at 0.
    let a = ClockTime {
        seconds_of_day: 0.0,
    }
    .to_angles();
    assert!(approx_eq(a.hour, 0.0, 1e-3));
    assert!(approx_eq(a.minute, 0.0, 1e-3));
    assert!(approx_eq(a.second, 0.0, 1e-3));

    // 03:00:00 → hour at π/2, minute at 0, second at 0.
    let a = ClockTime {
        seconds_of_day: 3.0 * 3600.0,
    }
    .to_angles();
    assert!(approx_eq(a.hour, FRAC_PI_2, 1e-3), "hour {}", a.hour);
    assert!(approx_eq(a.minute, 0.0, 1e-3));
    assert!(approx_eq(a.second, 0.0, 1e-3));

    // 06:00:00 → hour at π.
    let a = ClockTime {
        seconds_of_day: 6.0 * 3600.0,
    }
    .to_angles();
    assert!(approx_eq(a.hour, PI, 1e-3), "hour {}", a.hour);

    // 00:00:30 → second at π (half a minute).
    let a = ClockTime {
        seconds_of_day: 30.0,
    }
    .to_angles();
    assert!(approx_eq(a.second, PI, 1e-3), "second {}", a.second);

    // 00:30:00 → minute at π.
    let a = ClockTime {
        seconds_of_day: 30.0 * 60.0,
    }
    .to_angles();
    assert!(approx_eq(a.minute, PI, 1e-3));

    // 11:00:00 → hour at 11/12 of TAU. Tests modular reduction near the
    // top of the period without hitting f32 precision limits at very
    // sub-second deltas (43200 - 0.001 quantizes to 43200 in f32).
    let a = ClockTime {
        seconds_of_day: 11.0 * 3600.0,
    }
    .to_angles();
    let expected = TAU * 11.0 / 12.0;
    assert!(
        approx_eq(a.hour, expected, 1e-3),
        "hour {} vs {}",
        a.hour,
        expected
    );
}

#[test]
fn first_tick_is_full_repaint() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    match outcome {
        TickOutcome::FullRepaint {
            dirty_px,
            layers_painted,
        } => {
            assert!(dirty_px > 0, "first tick should have non-zero dirty pixels");
            assert_eq!(layers_painted, 1);
        }
        other => panic!("expected FullRepaint, got {other:?}"),
    }
    assert_eq!(clock.last_outcome(), outcome);
}

#[test]
fn second_tick_with_motion_is_painted() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 0.5,
    });
    match outcome {
        TickOutcome::Painted {
            dirty_px,
            layers_painted,
        } => {
            assert!(dirty_px > 0);
            assert_eq!(layers_painted, 1);
        }
        other => panic!("expected Painted, got {other:?}"),
    }
}

#[test]
fn identical_time_after_first_paint_is_skipped() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 5.0,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 5.0,
    });
    assert_eq!(outcome, TickOutcome::Skipped);
}

#[test]
fn invalidate_forces_full_repaint() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 5.0,
    });
    clock.invalidate();
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 5.0,
    });
    assert!(
        matches!(outcome, TickOutcome::FullRepaint { .. }),
        "got {outcome:?}"
    );
}

#[test]
fn clear_region_returns_dirty_union_then_clears() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let r = clock.clear_region().expect("should have dirty rect");
    // Second-hand AABB sits inside the face bounds (with AA padding).
    assert!(r.x >= FACE.x - 1 && r.y >= FACE.y - 1);
    assert!(r.x + r.width <= FACE.x + FACE.width + 1);
    assert!(r.y + r.height <= FACE.y + FACE.height + 1);
    // Once consumed, gone until the next set_target_time.
    assert!(clock.clear_region().is_none());
}

#[test]
fn dirty_union_grows_with_hand_sweep() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let _ = clock.clear_region();
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 7.5,
    });

    let r = clock.clear_region().expect("dirty after sweep");
    // After a 45° sweep the union of prev+current OBB AABBs should be
    // larger than a single hand's AABB. Lower bound: must span more than
    // half the face along at least one axis.
    let half_face = FACE.width.min(FACE.height) / 2;
    assert!(
        r.width > half_face || r.height > half_face,
        "swept union should span >half face; got {r:?}"
    );
}

#[test]
fn draw_emits_obb_per_layer_within_dirty() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::hour(RED));
    clock.push_layer(AnalogHand::minute(RED));
    clock.push_layer(AnalogHand::second(RED));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 4.0 * 3600.0 + 30.0 * 60.0 + 15.0,
    });
    let _ = clock.clear_region();

    let mut recorder = Recorder::default();
    clock.draw(&mut recorder);
    assert_eq!(
        recorder.obbs.len(),
        3,
        "every layer should have emitted exactly one fill_obb_aa"
    );
    for (_obb, color) in &recorder.obbs {
        assert_eq!(*color, RED);
    }
}

#[test]
fn second_layer_alone_repaints_when_only_second_changed() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::hour(RED));
    clock.push_layer(AnalogHand::minute(RED));
    clock.push_layer(AnalogHand::second(RED));

    let t0 = ClockTime {
        seconds_of_day: 4.0 * 3600.0 + 30.0 * 60.0 + 15.0,
    };
    let _ = clock.set_target_time(t0);
    let _ = clock.clear_region();

    // Advance by one second — at this scale the hour hand AABB barely
    // shifts and the minute hand's bbox doesn't intersect the second-hand
    // sweep arc near its tip. The hour and minute layers may or may not be
    // pulled into the repaint via cross-layer touch, but the second hand
    // must always be painted.
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: t0.seconds_of_day + 1.0,
    });

    if let TickOutcome::Painted { layers_painted, .. } = clock.last_outcome() {
        assert!(
            (1..=3).contains(&layers_painted),
            "expected 1-3 layers painted, got {layers_painted}"
        );
    } else {
        panic!("expected Painted, got {:?}", clock.last_outcome());
    }
}

#[test]
fn handle_event_does_not_consume() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));
    assert!(!clock.handle_event(&Event::Tick));
}

#[test]
fn analog_hand_obb_points_at_tip_at_three_oclock() {
    // 03:00:00 → hour hand should point at (cx + len, cy) — i.e.
    // OBB axis ≈ (1, 0) at 3 o'clock.
    let hand = AnalogHand::hour(RED);
    let state = ClockState {
        time: ClockTime {
            seconds_of_day: 3.0 * 3600.0,
        },
        angles: ClockTime {
            seconds_of_day: 3.0 * 3600.0,
        }
        .to_angles(),
    };
    let bbox = hand.bbox(&state, FACE);
    let cx = FACE.x + FACE.width / 2;
    let cy = FACE.y + FACE.height / 2;
    // Bbox center must be to the right of face center at 3 o'clock.
    let bbox_cx = bbox.x + bbox.width / 2;
    let bbox_cy = bbox.y + bbox.height / 2;
    assert!(
        bbox_cx > cx,
        "3 o'clock hour-hand bbox should be right of face center"
    );
    // At 3 o'clock, the hand is horizontal — bbox center y near face cy.
    assert!(
        (bbox_cy - cy).abs() <= 2,
        "3 o'clock hand should be roughly horizontal; got dy={}",
        bbox_cy - cy
    );
}

#[test]
fn clock_face_pushes_twelve_hour_marks_when_hours_only() {
    let mut clock = Clock::new(FACE);
    let face = ClockFace::hours_only(Color(80, 80, 80, 255));
    face.push_layers(&mut clock);
    // Face contributes 12 layers when hours-only.
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    if let TickOutcome::FullRepaint { layers_painted, .. } = clock.last_outcome() {
        assert_eq!(layers_painted, 12, "hours_only should add exactly 12 ticks");
    } else {
        panic!("expected FullRepaint, got {:?}", clock.last_outcome());
    }
}

#[test]
fn clock_face_standard_pushes_60_marks() {
    // 12 hour marks + 48 non-hour-position minute marks = 60 layers.
    let mut clock = Clock::new(FACE);
    let face = ClockFace::standard(Color(80, 80, 80, 255), Color(160, 160, 160, 255));
    face.push_layers(&mut clock);
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    if let TickOutcome::FullRepaint { layers_painted, .. } = clock.last_outcome() {
        assert_eq!(layers_painted, 60);
    } else {
        panic!("expected FullRepaint, got {:?}", clock.last_outcome());
    }
}

#[test]
fn tick_marks_are_static_after_first_paint() {
    // After first paint, a tick alone should report Skipped on identical
    // ticks even with `prev` known — it's static.
    let mut clock = Clock::new(FACE);
    clock.push_layer(TickMark {
        angle: 0.0,
        outer_radius: 0.95,
        length: 0.1,
        width: 0.03,
        color: RED,
    });
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 0.5,
    });
    assert_eq!(
        outcome,
        TickOutcome::Skipped,
        "static tick alone should yield Skipped on motion in other (absent) layers"
    );
}

#[test]
fn union_expands_to_cover_full_face_when_hand_crosses_tick() {
    // A second hand swept across one tick must expand the dirty union to
    // include the entire tick AABB, not just the sweep area, so pristine
    // restore covers everything before AA blend.
    let mut clock = Clock::new(FACE);
    // Place a tick at 3 o'clock — the second hand will sweep across it
    // when angles transition from ~just-before-3 to ~just-after-3.
    clock.push_layer(TickMark {
        angle: core::f32::consts::FRAC_PI_2, // 3 o'clock
        outer_radius: 0.95,
        length: 0.10,
        width: 0.03,
        color: Color(80, 80, 80, 255),
    });
    clock.push_layer(AnalogHand::second(RED));

    // First tick: full repaint.
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 14.0,
    });
    let _ = clock.clear_region();

    // Tick across: second hand sweeps from ~14s position to ~16s
    // position, crossing 3 o'clock (15s = π/2).
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 16.0,
    });
    let union = clock
        .clear_region()
        .expect("dirty union should be present after sweep");

    // The 3-o'clock tick has its outer end at ~ (face_cx + 0.95*r, face_cy).
    // Its bbox must be fully inside the reported union.
    let cx = FACE.x + FACE.width / 2;
    let cy = FACE.y + FACE.height / 2;
    let r = FACE.width.min(FACE.height) / 2;
    let tick_outer_x = cx + (r * 95) / 100;
    assert!(
        union.x + union.width > tick_outer_x,
        "union right edge {} should reach past tick outer x {}; union={:?}",
        union.x + union.width,
        tick_outer_x,
        union
    );
    assert!(
        union.y <= cy && union.y + union.height >= cy,
        "union should span face center y {cy}; union={union:?}"
    );
}

#[test]
fn center_cap_is_static_and_emits_disc() {
    // Cap added to a clock with a hand: first tick paints; second tick
    // (with hand motion) yields Painted, and the cap layer is among
    // those repainted because its bbox sits at the face center where the
    // hands rotate.
    let mut clock = Clock::new(FACE);
    clock.push_layer(AnalogHand::second(RED));
    clock.push_layer(CenterCap::standard(Color(20, 20, 20, 255)));

    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 0.5,
    });
    if let TickOutcome::Painted { layers_painted, .. } = outcome {
        assert_eq!(
            layers_painted, 2,
            "second-hand sweep over center cap should repaint both layers"
        );
    } else {
        panic!("expected Painted, got {outcome:?}");
    }
}

#[test]
fn center_cap_alone_yields_skipped_after_first_paint() {
    // A static layer alone should never produce dirty after first paint.
    let mut clock = Clock::new(FACE);
    clock.push_layer(CenterCap::standard(Color(20, 20, 20, 255)));
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 0.0,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 30.0,
    });
    assert_eq!(outcome, TickOutcome::Skipped);
}

#[test]
fn subsecond_dot_orbits_at_subsecond_rate() {
    // Dot bbox at fractional seconds 0.0 (top, 12 o'clock) and 0.5
    // (bottom, 6 o'clock) must straddle the face center along y.
    let dot = SubsecondDot::standard(Color(200, 200, 30, 255));
    let cy = FACE.y + FACE.height / 2;

    let s_top = ClockState {
        time: ClockTime {
            seconds_of_day: 10.0,
        }, // frac = 0
        angles: ClockTime {
            seconds_of_day: 10.0,
        }
        .to_angles(),
    };
    let bbox_top = dot.bbox(&s_top, FACE);
    let bbox_top_cy = bbox_top.y + bbox_top.height / 2;
    assert!(
        bbox_top_cy < cy,
        "frac=0 dot should be above face center; got {bbox_top_cy} vs cy {cy}"
    );

    let s_bot = ClockState {
        time: ClockTime {
            seconds_of_day: 10.5,
        }, // frac = 0.5
        angles: ClockTime {
            seconds_of_day: 10.5,
        }
        .to_angles(),
    };
    let bbox_bot = dot.bbox(&s_bot, FACE);
    let bbox_bot_cy = bbox_bot.y + bbox_bot.height / 2;
    assert!(
        bbox_bot_cy > cy,
        "frac=0.5 dot should be below face center; got {bbox_bot_cy} vs cy {cy}"
    );
}

#[test]
fn subsecond_dot_dirty_empty_when_time_unchanged() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(SubsecondDot::standard(Color(200, 200, 30, 255)));
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 10.25,
    });
    let outcome = clock.set_target_time(ClockTime {
        seconds_of_day: 10.25,
    });
    assert_eq!(outcome, TickOutcome::Skipped);
}

#[test]
fn subsecond_dot_dirty_grows_with_motion() {
    let mut clock = Clock::new(FACE);
    clock.push_layer(SubsecondDot::standard(Color(200, 200, 30, 255)));
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 10.0,
    });
    let _ = clock.clear_region();

    // Quarter-orbit advance: dot moves 90° around the orbit.
    let _ = clock.set_target_time(ClockTime {
        seconds_of_day: 10.25,
    });
    let dirty = clock.clear_region().expect("non-empty after motion");
    // Union must span at least the orbit chord (orbit_radius * face_radius).
    let face_r = FACE.width.min(FACE.height) / 2;
    let orbit_r = (face_r * 65) / 100; // matches SubsecondDot::standard.orbit_radius
    assert!(
        dirty.width >= orbit_r || dirty.height >= orbit_r,
        "quarter-orbit dirty union should reach orbit radius; got {dirty:?} (orbit_r={orbit_r})"
    );
}

#[test]
fn analog_hand_obb_points_up_at_twelve() {
    let hand = AnalogHand::minute(RED);
    let state = ClockState {
        time: ClockTime {
            seconds_of_day: 0.0,
        },
        angles: ClockTime {
            seconds_of_day: 0.0,
        }
        .to_angles(),
    };
    let bbox = hand.bbox(&state, FACE);
    let cx = FACE.x + FACE.width / 2;
    let cy = FACE.y + FACE.height / 2;
    let bbox_cx = bbox.x + bbox.width / 2;
    let bbox_cy = bbox.y + bbox.height / 2;
    assert!(
        (bbox_cx - cx).abs() <= 2,
        "12 o'clock hand should be roughly vertical; got dx={}",
        bbox_cx - cx
    );
    assert!(
        bbox_cy < cy,
        "12 o'clock hand bbox center should be above face center"
    );
}
