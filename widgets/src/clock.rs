//! Analog clock widget with sub-pixel anti-aliased hand rotation.
//!
//! The clock is composed from z-ordered [`ClockLayer`]s. Each layer reports
//! its bbox and self-induced dirty rect; the widget unions them, compositor
//! restores from pristine background under the union, then layers paint in
//! z-order. The face background lives in the framebuffer's pristine copy —
//! this widget animates moving parts only.
//!
//! Driver path:
//! 1. Application predicts present-time `target_time` and calls
//!    [`Clock::set_target_time`]; angles + dirty union are computed and the
//!    method returns a [`TickOutcome`] for telemetry.
//! 2. Compositor reads [`Widget::clear_region`] (consumes dirty union) and
//!    queues pristine restore via DMA2D (or equivalent).
//! 3. Compositor calls [`Widget::draw`]; layers whose bbox intersects the
//!    union repaint in z-order.

use alloc::boxed::Box;
use alloc::vec::Vec;
use rlvgl_core::event::Event;
use rlvgl_core::raster::{Obb, PointF};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Time-of-day input to the clock.
///
/// Sub-second precision via fractional seconds; the driver supplies absolute
/// time so missed ticks don't accumulate drift.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClockTime {
    /// Seconds since midnight, fractional. May exceed 86400; the angle
    /// derivation reduces modulo each hand's period.
    pub seconds_of_day: f64,
}

/// Hand angles in radians. `0` is 12 o'clock; angles increase clockwise.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HandAngles {
    /// Hour hand angle (period 12 h).
    pub hour: f32,
    /// Minute hand angle (period 1 h).
    pub minute: f32,
    /// Second hand angle (period 1 min).
    pub second: f32,
}

/// Snapshot of clock state for one frame: source time and derived angles.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClockState {
    /// Source target time supplied by the driver.
    pub time: ClockTime,
    /// Angles derived once per tick by [`Clock::set_target_time`].
    pub angles: HandAngles,
}

impl ClockTime {
    /// Convert this time to clock-hand angles.
    pub fn to_angles(self) -> HandAngles {
        const TAU: f32 = core::f32::consts::TAU;
        let s = self.seconds_of_day as f32;
        HandAngles {
            hour: TAU * frac01(s / 43_200.0),
            minute: TAU * frac01(s / 3_600.0),
            second: TAU * frac01(s / 60.0),
        }
    }
}

/// Outcome of a [`Clock::set_target_time`] call. Reports dirty pixel count
/// and how many layers will repaint, for telemetry pairing with DWT cycle
/// counters.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TickOutcome {
    /// Dirty union was empty after the first paint; no draw work needed.
    Skipped,
    /// Normal incremental frame.
    Painted {
        /// Pixel count of the dirty union (`width * height`).
        dirty_px: u32,
        /// Number of layers whose bbox intersects the dirty union.
        layers_painted: u8,
    },
    /// First frame after construction or [`Clock::invalidate`]. Entire face
    /// is repainted. Sample this for "worst case" telemetry baselines.
    FullRepaint {
        /// Pixel count of the dirty union.
        dirty_px: u32,
        /// Number of layers in this paint.
        layers_painted: u8,
    },
}

/// One visual component of a clock — a hand, center cap, sub-second dot,
/// digital readout, etc. Layers compose in insertion (z-) order.
pub trait ClockLayer {
    /// Pixels this layer might write at `state`, in absolute framebuffer
    /// coordinates. Used for cross-layer dirty union math.
    fn bbox(&self, state: &ClockState, bounds: Rect) -> Rect;

    /// Self-induced dirty between `prev` and `state`. An empty rect means
    /// "I have not changed"; the compositor will still call `paint` if
    /// another layer's dirty rect intersects this layer's `bbox`.
    fn dirty(&self, prev: Option<&ClockState>, state: &ClockState, bounds: Rect) -> Rect;

    /// Paint this layer at `state`. Layers must paint within their declared
    /// `bbox` — the compositor relies on that for dirty-rect correctness.
    fn paint(&self, renderer: &mut dyn Renderer, state: &ClockState, bounds: Rect);
}

/// Which hand-angle component an [`AnalogHand`] reads from [`HandAngles`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum HandKind {
    /// Hour hand.
    Hour,
    /// Minute hand.
    Minute,
    /// Second hand.
    Second,
}

/// Analog hand laid out as an oriented bounding box rotating around the
/// face center. Geometry is expressed as fractions of face radius so the
/// same hand renders correctly at any clock size.
pub struct AnalogHand {
    /// Which angle this hand follows.
    pub kind: HandKind,
    /// Length from pivot to tip, fraction of face radius.
    pub length: f32,
    /// Length from pivot to tail end, fraction of face radius (typically
    /// `0.0..=0.25`). `0.0` = no tail past the pivot.
    pub tail: f32,
    /// Hand width, fraction of face radius.
    pub width: f32,
    /// Hand color (alpha modulated by AA coverage at edges).
    pub color: Color,
}

impl AnalogHand {
    /// Construct an hour hand with sensible defaults.
    pub const fn hour(color: Color) -> Self {
        Self {
            kind: HandKind::Hour,
            length: 0.55,
            tail: 0.10,
            width: 0.040,
            color,
        }
    }

    /// Construct a minute hand with sensible defaults.
    pub const fn minute(color: Color) -> Self {
        Self {
            kind: HandKind::Minute,
            length: 0.80,
            tail: 0.12,
            width: 0.028,
            color,
        }
    }

    /// Construct a second hand with sensible defaults.
    pub const fn second(color: Color) -> Self {
        Self {
            kind: HandKind::Second,
            length: 0.92,
            tail: 0.18,
            width: 0.012,
            color,
        }
    }

    fn obb(&self, state: &ClockState, bounds: Rect) -> Obb {
        let r = (bounds.width.min(bounds.height) as f32) * 0.5;
        let cx = bounds.x as f32 + bounds.width as f32 * 0.5;
        let cy = bounds.y as f32 + bounds.height as f32 * 0.5;
        let len = (self.length + self.tail) * r;
        let width = self.width * r;
        let angle = match self.kind {
            HandKind::Hour => state.angles.hour,
            HandKind::Minute => state.angles.minute,
            HandKind::Second => state.angles.second,
        };
        // Clock convention: 0 rad = 12 o'clock (straight up, i.e. -y on
        // a screen-coord framebuffer), increasing clockwise. The OBB's
        // major axis points at the hand's tip:
        //   12 o'clock → (0, -1), 3 o'clock → (1, 0), 6 → (0, 1), 9 → (-1, 0).
        let cos_t = libm::sinf(angle);
        let sin_t = -libm::cosf(angle);
        // OBB center is offset from the pivot toward the tip by half the
        // tip-vs-tail asymmetry.
        let offset = ((self.length - self.tail) * 0.5) * r;
        let center = PointF::new(cx + cos_t * offset, cy + sin_t * offset);
        Obb::from_axis(center, len, width, cos_t, sin_t)
    }
}

/// Sub-second indicator: a small disc orbiting the face center at the
/// sub-second fractional rate. Completes one full orbit per wall-clock
/// second, giving visible per-frame motion at 30 Hz / 60 Hz tick rates
/// — useful for confirming the driver is delivering target times to the
/// widget on time, and for visualising end-to-end render pipeline
/// latency.
pub struct SubsecondDot {
    /// Orbit radius as a fraction of face radius (typical `0.4..=0.7`).
    pub orbit_radius: f32,
    /// Dot radius as a fraction of face radius (typical `0.015..=0.04`).
    pub dot_radius: f32,
    /// Dot color (alpha modulated by AA coverage at edges).
    pub color: Color,
}

impl SubsecondDot {
    /// Default dot at 65% orbit radius, 2% dot radius.
    pub const fn standard(color: Color) -> Self {
        Self {
            orbit_radius: 0.65,
            dot_radius: 0.020,
            color,
        }
    }

    fn position(&self, state: &ClockState, bounds: Rect) -> (PointF, f32) {
        let r = (bounds.width.min(bounds.height) as f32) * 0.5;
        let cx = bounds.x as f32 + bounds.width as f32 * 0.5;
        let cy = bounds.y as f32 + bounds.height as f32 * 0.5;
        // Sub-second fraction in [0, 1). `as i64 as f64` floor works for
        // any non-negative `seconds_of_day` and avoids needing
        // `f64::floor` (not in core).
        let s = state.time.seconds_of_day;
        let i = s as i64 as f64;
        let frac = (s - i) as f32;
        let angle = core::f32::consts::TAU * frac;
        let orbit = self.orbit_radius * r;
        let dot = self.dot_radius * r;
        // Same convention as hands: 0 = 12 o'clock (up), clockwise.
        let cos_t = libm::sinf(angle);
        let sin_t = -libm::cosf(angle);
        let center = PointF::new(cx + cos_t * orbit, cy + sin_t * orbit);
        (center, dot)
    }

    fn bbox_for(&self, state: &ClockState, bounds: Rect) -> Rect {
        let (center, dot) = self.position(state, bounds);
        let pad = dot + 1.0;
        Rect {
            x: (center.x - pad) as i32 - 1,
            y: (center.y - pad) as i32 - 1,
            width: (pad * 2.0) as i32 + 3,
            height: (pad * 2.0) as i32 + 3,
        }
    }
}

impl ClockLayer for SubsecondDot {
    fn bbox(&self, state: &ClockState, bounds: Rect) -> Rect {
        self.bbox_for(state, bounds)
    }

    fn dirty(&self, prev: Option<&ClockState>, state: &ClockState, bounds: Rect) -> Rect {
        let cur = self.bbox_for(state, bounds);
        match prev {
            None => cur,
            Some(p) => {
                if p.time.seconds_of_day == state.time.seconds_of_day {
                    Rect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    }
                } else {
                    union_rect(cur, self.bbox_for(p, bounds))
                }
            }
        }
    }

    fn paint(&self, renderer: &mut dyn Renderer, state: &ClockState, bounds: Rect) {
        let (center, dot) = self.position(state, bounds);
        renderer.fill_disc_aa(center, dot, self.color);
    }
}

/// Filled circular cap at the face center, drawn on top of the hands to
/// hide the small visual gap where the three hand pivots meet. Static
/// after first paint; the same union-expansion pass that protects tick
/// marks ensures pristine restore covers the cap before any blend.
pub struct CenterCap {
    /// Cap radius as a fraction of face radius (typical `0.03..=0.06`).
    pub radius: f32,
    /// Cap color (alpha modulated by AA coverage at edges).
    pub color: Color,
}

impl CenterCap {
    /// Default cap sized to comfortably cover the typical pivot region.
    pub const fn standard(color: Color) -> Self {
        Self {
            radius: 0.04,
            color,
        }
    }

    fn center_radius(&self, bounds: Rect) -> (PointF, f32) {
        let r = (bounds.width.min(bounds.height) as f32) * 0.5;
        let cx = bounds.x as f32 + bounds.width as f32 * 0.5;
        let cy = bounds.y as f32 + bounds.height as f32 * 0.5;
        (PointF::new(cx, cy), self.radius * r)
    }
}

impl ClockLayer for CenterCap {
    fn bbox(&self, _state: &ClockState, bounds: Rect) -> Rect {
        let (center, r) = self.center_radius(bounds);
        let pad = r + 1.0;
        Rect {
            x: (center.x - pad) as i32 - 1,
            y: (center.y - pad) as i32 - 1,
            width: (pad * 2.0) as i32 + 3,
            height: (pad * 2.0) as i32 + 3,
        }
    }

    fn dirty(&self, prev: Option<&ClockState>, state: &ClockState, bounds: Rect) -> Rect {
        match prev {
            None => self.bbox(state, bounds),
            Some(_) => Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }

    fn paint(&self, renderer: &mut dyn Renderer, _state: &ClockState, bounds: Rect) {
        let (center, r) = self.center_radius(bounds);
        renderer.fill_disc_aa(center, r, self.color);
    }
}

/// Static tick mark on the clock face. Each tick is its own
/// [`ClockLayer`], so a hand sweeping across the face only triggers
/// repaint of the few ticks it crosses, not the whole face. Geometry is
/// expressed as fractions of face radius for size-agnostic scaling.
///
/// The angle convention matches the hands: `0` is 12 o'clock; angle
/// increases clockwise.
pub struct TickMark {
    /// Angle in radians; `0` = 12 o'clock, clockwise.
    pub angle: f32,
    /// Distance from face center to outer end of the mark, fraction of
    /// face radius. `1.0` = exactly at the face edge.
    pub outer_radius: f32,
    /// Length along the radial direction, fraction of face radius.
    pub length: f32,
    /// Width perpendicular to the radial direction, fraction of face radius.
    pub width: f32,
    /// Mark color (alpha modulated by AA coverage at edges).
    pub color: Color,
}

impl TickMark {
    fn obb(&self, bounds: Rect) -> Obb {
        let r = (bounds.width.min(bounds.height) as f32) * 0.5;
        let cx = bounds.x as f32 + bounds.width as f32 * 0.5;
        let cy = bounds.y as f32 + bounds.height as f32 * 0.5;
        let outer = self.outer_radius * r;
        let inner = (self.outer_radius - self.length).max(0.0) * r;
        let mid_r = (outer + inner) * 0.5;
        let len = outer - inner;
        let width = self.width * r;
        // OBB major axis = radial direction; same `(sin, -cos)` mapping as
        // the hands so 0 rad points up at 12 o'clock.
        let cos_t = libm::sinf(self.angle);
        let sin_t = -libm::cosf(self.angle);
        let center = PointF::new(cx + cos_t * mid_r, cy + sin_t * mid_r);
        Obb::from_axis(center, len, width, cos_t, sin_t)
    }
}

impl ClockLayer for TickMark {
    fn bbox(&self, _state: &ClockState, bounds: Rect) -> Rect {
        self.obb(bounds).aabb()
    }

    fn dirty(&self, prev: Option<&ClockState>, _state: &ClockState, bounds: Rect) -> Rect {
        match prev {
            None => self.obb(bounds).aabb(),
            Some(_) => Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }

    fn paint(&self, renderer: &mut dyn Renderer, _state: &ClockState, bounds: Rect) {
        renderer.fill_obb_aa(self.obb(bounds), self.color);
    }
}

/// Helper that constructs a standard set of [`TickMark`] layers (12 hour
/// marks and optionally 48 minute marks at non-hour positions) and pushes
/// them onto a [`Clock`] in z-order. Call this *before* pushing hand
/// layers so the hands paint on top of the face.
///
/// Sizes are deliberately conservative defaults; for custom faces
/// construct individual [`TickMark`]s and push them with
/// [`Clock::push_layer`].
pub struct ClockFace {
    /// Color for hour marks (12 marks at cardinal positions).
    pub hour_color: Color,
    /// Color for minute marks; `None` disables them.
    pub minute_color: Option<Color>,
    /// Outer radius / length / width of hour marks (fractions of radius).
    pub hour_size: TickSize,
    /// Outer radius / length / width of minute marks.
    pub minute_size: TickSize,
}

/// Geometry triple shared by hour and minute mark configurations.
#[derive(Copy, Clone, Debug)]
pub struct TickSize {
    /// Distance from face center to outer end of mark, fraction of radius.
    pub outer_radius: f32,
    /// Length along the radial direction, fraction of radius.
    pub length: f32,
    /// Width perpendicular to the radial direction, fraction of radius.
    pub width: f32,
}

impl ClockFace {
    /// Standard 12-hour-mark + 48-minute-mark face.
    pub const fn standard(hour_color: Color, minute_color: Color) -> Self {
        Self {
            hour_color,
            minute_color: Some(minute_color),
            hour_size: TickSize {
                outer_radius: 0.96,
                length: 0.10,
                width: 0.030,
            },
            minute_size: TickSize {
                outer_radius: 0.96,
                length: 0.05,
                width: 0.012,
            },
        }
    }

    /// 12 hour marks, no minute marks.
    pub const fn hours_only(color: Color) -> Self {
        Self {
            hour_color: color,
            minute_color: None,
            hour_size: TickSize {
                outer_radius: 0.96,
                length: 0.10,
                width: 0.030,
            },
            minute_size: TickSize {
                outer_radius: 0.0,
                length: 0.0,
                width: 0.0,
            },
        }
    }

    /// Push tick layers onto `clock` in z-order: minute marks first (so
    /// hour marks paint on top at cardinal positions if their bboxes
    /// overlap).
    pub fn push_layers(&self, clock: &mut Clock) {
        if let Some(minute_color) = self.minute_color {
            for i in 0..60 {
                if i % 5 == 0 {
                    continue;
                }
                let angle = core::f32::consts::TAU * i as f32 / 60.0;
                clock.push_layer(TickMark {
                    angle,
                    outer_radius: self.minute_size.outer_radius,
                    length: self.minute_size.length,
                    width: self.minute_size.width,
                    color: minute_color,
                });
            }
        }
        for i in 0..12 {
            let angle = core::f32::consts::TAU * i as f32 / 12.0;
            clock.push_layer(TickMark {
                angle,
                outer_radius: self.hour_size.outer_radius,
                length: self.hour_size.length,
                width: self.hour_size.width,
                color: self.hour_color,
            });
        }
    }
}

impl ClockLayer for AnalogHand {
    fn bbox(&self, state: &ClockState, bounds: Rect) -> Rect {
        self.obb(state, bounds).aabb()
    }

    fn dirty(&self, prev: Option<&ClockState>, state: &ClockState, bounds: Rect) -> Rect {
        let cur = self.bbox(state, bounds);
        match prev {
            None => cur,
            Some(p) => {
                let prev_angle = match self.kind {
                    HandKind::Hour => p.angles.hour,
                    HandKind::Minute => p.angles.minute,
                    HandKind::Second => p.angles.second,
                };
                let cur_angle = match self.kind {
                    HandKind::Hour => state.angles.hour,
                    HandKind::Minute => state.angles.minute,
                    HandKind::Second => state.angles.second,
                };
                if prev_angle == cur_angle {
                    Rect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    }
                } else {
                    union_rect(cur, self.bbox(p, bounds))
                }
            }
        }
    }

    fn paint(&self, renderer: &mut dyn Renderer, state: &ClockState, bounds: Rect) {
        renderer.fill_obb_aa(self.obb(state, bounds), self.color);
    }
}

/// Analog clock widget. Owns layers in z-order; composes them per frame
/// via dirty-rect union math.
pub struct Clock {
    bounds: Rect,
    layers: Vec<Box<dyn ClockLayer>>,
    state: Option<ClockState>,
    dirty_union: Option<Rect>,
    last_outcome: TickOutcome,
    needs_full_repaint: bool,
}

impl Clock {
    /// Create a new clock occupying `bounds`. The first call to
    /// [`set_target_time`](Self::set_target_time) will return
    /// [`TickOutcome::FullRepaint`].
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            layers: Vec::new(),
            state: None,
            dirty_union: None,
            last_outcome: TickOutcome::Skipped,
            needs_full_repaint: true,
        }
    }

    /// Append a layer. Layers paint in insertion order (later layers on top).
    pub fn push_layer<L: ClockLayer + 'static>(&mut self, layer: L) {
        self.layers.push(Box::new(layer));
        self.needs_full_repaint = true;
    }

    /// Force the next [`set_target_time`](Self::set_target_time) call to be
    /// treated as a full repaint. Use after extent changes, theme changes,
    /// or when the widget re-enters visibility.
    pub fn invalidate(&mut self) {
        self.needs_full_repaint = true;
    }

    /// Telemetry from the most recent `set_target_time` call.
    pub fn last_outcome(&self) -> TickOutcome {
        self.last_outcome
    }

    /// Compute the next frame's plan from the driver-supplied target time.
    /// Returns telemetry about what will happen during draw.
    ///
    /// Idempotent on a fixed `(target_time, bounds)` — safe for the driver
    /// to skip frames; absolute-time math means no accumulator drift.
    pub fn set_target_time(&mut self, target_time: ClockTime) -> TickOutcome {
        let new_state = ClockState {
            time: target_time,
            angles: target_time.to_angles(),
        };
        let prev: Option<&ClockState> = if self.needs_full_repaint {
            None
        } else {
            self.state.as_ref()
        };

        let mut union: Option<Rect> = None;
        for layer in &self.layers {
            let d = layer.dirty(prev, &new_state, self.bounds);
            if d.width > 0 && d.height > 0 {
                union = Some(match union {
                    None => d,
                    Some(u) => union_rect(u, d),
                });
            }
        }

        // Expand the union to fixpoint: any layer whose bbox intersects
        // the current union will paint, and its full bbox must be inside
        // the union so the compositor's pristine restore covers every
        // pixel the layer will write. Without this, static layers with
        // partial overlap (e.g. a tick mark partially crossed by a hand
        // sweep) re-blend AA edges over un-restored pixels each frame and
        // drift darker over time.
        if let Some(mut u) = union {
            loop {
                let mut grew = false;
                for layer in &self.layers {
                    let bb = layer.bbox(&new_state, self.bounds);
                    if bb.width == 0 || bb.height == 0 {
                        continue;
                    }
                    if rects_intersect(bb, u) {
                        let merged = union_rect(u, bb);
                        if merged != u {
                            u = merged;
                            grew = true;
                        }
                    }
                }
                if !grew {
                    break;
                }
            }
            union = Some(u);
        }

        let union_clipped = union.and_then(|u| rect_intersect(u, self.bounds));

        let outcome = match (self.needs_full_repaint, union_clipped) {
            (_, None) => TickOutcome::Skipped,
            (full, Some(r)) => {
                let dirty_px = (r.width as u32).saturating_mul(r.height as u32);
                let layers_painted = self
                    .layers
                    .iter()
                    .filter(|l| rects_intersect(l.bbox(&new_state, self.bounds), r))
                    .count() as u8;
                if full {
                    TickOutcome::FullRepaint {
                        dirty_px,
                        layers_painted,
                    }
                } else {
                    TickOutcome::Painted {
                        dirty_px,
                        layers_painted,
                    }
                }
            }
        };

        self.dirty_union = union_clipped;
        self.state = Some(new_state);
        self.needs_full_repaint = false;
        self.last_outcome = outcome;
        outcome
    }
}

impl Widget for Clock {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let union = self.dirty_union.unwrap_or(self.bounds);
        for layer in &self.layers {
            if rects_intersect(layer.bbox(state, self.bounds), union) {
                layer.paint(renderer, state, self.bounds);
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        self.dirty_union.take()
    }
}

#[inline]
fn frac01(x: f32) -> f32 {
    let i = x as i64 as f32;
    let f = x - i;
    if f < 0.0 { f + 1.0 } else { f }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    if a.width == 0 || a.height == 0 {
        return b;
    }
    if b.width == 0 || b.height == 0 {
        return a;
    }
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.width).max(b.x + b.width);
    let y1 = (a.y + a.height).max(b.y + b.height);
    Rect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}

fn rect_intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    if x1 > x0 && y1 > y0 {
        Some(Rect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    } else {
        None
    }
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    rect_intersect(a, b).is_some()
}
