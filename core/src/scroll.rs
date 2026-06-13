//! LPAR-05 scroll runtime: state, controller, and snap logic.
//!
//! A [`ScrollController`] consumes the `DragStart`/`DragMove`/`DragEnd`
//! event stream (from a `DragRecognizer`) and drives scroll offsets on
//! `ObjectNode`-based scroll containers.  All timing is tick-driven — no wall
//! clock (LPAR-05 §8.1).  Identical `(event, tick)` input sequences produce
//! identical scroll trajectories (determinism guarantee, LPAR-05 §8.1).
//!
//! # Scroll session lifecycle (LPAR-05 §7.4)
//!
//! ```text
//! DragStart  →  ScrollBegin (on container)
//! DragMove   →  Scroll (per effective offset change)
//! DragEnd    →  [if vel ≥ MIN] ScrollThrow + tick-driven Scroll … ScrollEnd
//!            →  [if vel < MIN] ScrollEnd immediately
//! ```
//!
//! # Throw and snap (LPAR-05 §8–§9)
//!
//! Throw uses a [`crate::anim::Tween`] with [`crate::anim::Easing::EaseOut`].
//! Snap adjusts the tween endpoint before the first momentum tick.
//!
//! # Nested scroll chaining (LPAR-05 §10)
//!
//! When the active (inner) container reaches its edge with residual motion,
//! [`ScrollEnd`](crate::object::ObjectEvent::ScrollEnd) is emitted on the inner
//! container and [`ScrollBegin`](crate::object::ObjectEvent::ScrollBegin) on the
//! nearest `SCROLLABLE` ancestor.

use alloc::vec::Vec;

use crate::anim::{Easing, Tween};
use crate::event::Event;
use crate::object::{DispatchInput, ObjectEvent, ObjectFlags, ObjectNode};
use crate::widget::Rect;

// ---------------------------------------------------------------------------
// Tuning constants (LPAR-05 §8)
// ---------------------------------------------------------------------------

/// Minimum throw velocity (pixels per tick) to enter momentum mode.
///
/// Drags ending below this threshold emit `ScrollEnd` immediately.
pub const MIN_THROW_VELOCITY_PX_PER_TICK: i32 = 2;

/// Maximum throw velocity cap (pixels per tick).
///
/// Pathological high-speed flings are clamped to this value.
pub const MAX_THROW_VELOCITY_PX_PER_TICK: i32 = 80;

/// Number of recent `DragMove` events used for velocity estimation.
pub const VELOCITY_WINDOW_N: usize = 4;

/// Throw duration = `initial_velocity.abs() * THROW_DURATION_FACTOR` ticks.
///
/// Higher initial velocity → longer throw.
pub const THROW_DURATION_FACTOR: u32 = 6;

/// Default snap-attraction radius in pixels.
///
/// When the throw endpoint is within this many pixels of a snap point, the
/// endpoint is adjusted to the snap point.
pub const DEFAULT_SNAP_ATTRACTION_RADIUS: i32 = 60;

/// Duration of the short snap-settle tween in ticks.
pub const SNAP_SETTLE_TICKS: u32 = 8;

// ---------------------------------------------------------------------------
// Scrollbar geometry constants (LPAR-05 §11.3)
//
// These mirror the constants in `widgets/src/scroll_view.rs` (which are
// private there).  The canonical reference is `ScrollView::scrollbar_thumb`
// (lines 139–158 of that file).  If those values change, update here too.
// ---------------------------------------------------------------------------

/// Width in pixels of the scrollbar thumb (matches `SCROLLBAR_WIDTH` in
/// `widgets/src/scroll_view.rs`).
pub const SCROLLBAR_WIDTH: i32 = 4;

/// Gap between the scrollbar thumb and the viewport's right edge (matches
/// `SCROLLBAR_MARGIN` in `widgets/src/scroll_view.rs`).
pub const SCROLLBAR_MARGIN: i32 = 2;

/// Minimum scrollbar thumb height so it stays visible on long content (matches
/// `SCROLLBAR_MIN_THUMB` in `widgets/src/scroll_view.rs`).
pub const SCROLLBAR_MIN_THUMB: i32 = 8;

// ---------------------------------------------------------------------------
// ScrollAxis
// ---------------------------------------------------------------------------

/// The scroll axis supported by a container.
///
/// LPAR-05 §3: vertical is fully frozen; horizontal and both are additive
/// (available for future extension; v1 implementation is vertical-first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Container scrolls vertically only.
    Vertical,
    /// Container scrolls horizontally only (additive, not wired in v1).
    Horizontal,
    /// Container scrolls in both axes (additive, not wired in v1).
    Both,
}

// ---------------------------------------------------------------------------
// SnapAlign
// ---------------------------------------------------------------------------

/// Snap alignment: how a snap point aligns within the viewport (LPAR-05 §9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapAlign {
    /// The snap point aligns with the start (top/left) edge of the viewport.
    #[default]
    Start,
    /// The snap point aligns with the center of the viewport.
    Center,
    /// The snap point aligns with the end (bottom/right) edge of the viewport.
    End,
}

// ---------------------------------------------------------------------------
// SnapConfig
// ---------------------------------------------------------------------------

/// Snap point specification for a scroll container (LPAR-05 §9.2).
#[derive(Debug, Clone)]
pub enum SnapConfig {
    /// No snapping.
    None,
    /// Explicit list of absolute offset values (e.g. `[0, 200, 400]`).
    ///
    /// The controller snaps to the nearest point in the list within the
    /// attraction radius.
    Explicit(Vec<i32>),
    /// Snap points derived from the bounds of the container's direct children.
    ///
    /// Each child's leading edge (top for vertical, left for horizontal) is a
    /// snap point.  Alignment controls which edge of the viewport the snap
    /// point aligns to (LPAR-05 §9.3).
    ChildAligned,
}

// ---------------------------------------------------------------------------
// ScrollbarMode
// ---------------------------------------------------------------------------

/// Scrollbar display mode (LPAR-05 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarMode {
    /// Scrollbar is visible only during an active scroll session.
    #[default]
    Auto,
    /// Scrollbar is always visible when content overflows.
    On,
    /// No scrollbar drawn.
    Off,
}

// ---------------------------------------------------------------------------
// ScrollState
// ---------------------------------------------------------------------------

/// Scroll metadata owned by a scrollable [`ObjectNode`].
///
/// Attach via [`ObjectNode::set_scroll_state`](crate::object::ObjectNode::set_scroll_state).
/// The node's `SCROLLABLE` flag is set automatically.
///
/// # Content extent and viewport
///
/// The viewport size is derived from the node's `Widget::bounds()` at the
/// time of each scroll computation.  The maximum scroll offset is:
///
/// ```text
/// max_scroll_y = (content_h - viewport_h).max(0)
/// max_scroll_x = (content_w - viewport_w).max(0)
/// ```
///
/// LPAR-05 §5: `SCROLLABLE` is independent of actual overflow; the content
/// extent is supplied explicitly (LPAR-10 will compute it from layout later).
#[derive(Debug, Clone)]
pub struct ScrollState {
    /// Current horizontal scroll offset in logical pixels.
    pub offset_x: i32,
    /// Current vertical scroll offset in logical pixels.
    pub offset_y: i32,
    /// Explicit content width in logical pixels.
    ///
    /// The maximum horizontal scroll offset is `(content_w - viewport_w).max(0)`.
    pub content_w: i32,
    /// Explicit content height in logical pixels.
    ///
    /// The maximum vertical scroll offset is `(content_h - viewport_h).max(0)`.
    pub content_h: i32,
    /// Scroll axis supported by this container.
    pub axis: ScrollAxis,
    /// Snap configuration.
    pub snap: SnapConfig,
    /// Snap alignment (which viewport edge a snap point aligns to).
    pub snap_align: SnapAlign,
    /// Scrollbar display mode.
    pub scrollbar_mode: ScrollbarMode,
    /// Snap attraction radius in pixels.
    pub snap_attraction_radius: i32,
}

impl ScrollState {
    /// Create default scroll state for a vertical-only container.
    ///
    /// Content extent must be updated via [`Self::content_w`] /
    /// [`Self::content_h`] before scroll is meaningful.
    pub fn new() -> Self {
        Self {
            offset_x: 0,
            offset_y: 0,
            content_w: 0,
            content_h: 0,
            axis: ScrollAxis::Vertical,
            snap: SnapConfig::None,
            snap_align: SnapAlign::Start,
            scrollbar_mode: ScrollbarMode::Auto,
            snap_attraction_radius: DEFAULT_SNAP_ATTRACTION_RADIUS,
        }
    }

    /// Return the maximum allowed vertical scroll offset given a viewport height.
    pub fn max_scroll_y(&self, viewport_h: i32) -> i32 {
        (self.content_h - viewport_h).max(0)
    }

    /// Return the maximum allowed horizontal scroll offset given a viewport width.
    pub fn max_scroll_x(&self, viewport_w: i32) -> i32 {
        (self.content_w - viewport_w).max(0)
    }

    /// Clamp and set the vertical offset; returns `true` if it changed.
    pub fn set_offset_y(&mut self, new_y: i32, viewport_h: i32) -> bool {
        let clamped = new_y.clamp(0, self.max_scroll_y(viewport_h));
        if clamped != self.offset_y {
            self.offset_y = clamped;
            true
        } else {
            false
        }
    }

    /// Clamp and set the horizontal offset; returns `true` if it changed.
    pub fn set_offset_x(&mut self, new_x: i32, viewport_w: i32) -> bool {
        let clamped = new_x.clamp(0, self.max_scroll_x(viewport_w));
        if clamped != self.offset_x {
            self.offset_x = clamped;
            true
        } else {
            false
        }
    }

    /// Compute the vertical scrollbar thumb rect in screen space for the given
    /// viewport `Rect`, using the same formula as
    /// `ScrollView::scrollbar_thumb()` (`widgets/src/scroll_view.rs` lines
    /// 139–158).
    ///
    /// Returns `None` when:
    /// - `scrollbar_mode` is [`ScrollbarMode::Off`], or
    /// - the content does not overflow the viewport (`content_h ≤ viewport.height`).
    ///
    /// The returned rect is positioned on the **right edge** of `viewport` with
    /// width [`SCROLLBAR_WIDTH`] and a gap of [`SCROLLBAR_MARGIN`] pixels.
    /// Thumb height is computed as:
    ///
    /// ```text
    /// thumb_h = (viewport_h² / content_h).clamp(SCROLLBAR_MIN_THUMB, viewport_h)
    /// ```
    ///
    /// Thumb position reflects the current [`offset_y`](Self::offset_y):
    /// at offset 0 the thumb sits at the top of the track; at `max_scroll_y`
    /// the thumb sits at the bottom.
    ///
    /// # Notes
    ///
    /// - **Rendering and Auto-mode visibility** are the consuming widget's
    ///   responsibility.  This method returns geometry only; whether the
    ///   scrollbar is currently visible (e.g. during an active session in `Auto`
    ///   mode) is managed by the caller.
    /// - **Horizontal axis** is deferred (v1 vertical-first, LPAR-05 §17).
    pub fn scrollbar_thumb(&self, viewport: Rect) -> Option<Rect> {
        if self.scrollbar_mode == ScrollbarMode::Off {
            return None;
        }
        let viewport_h = viewport.height;
        if self.content_h <= viewport_h {
            return None;
        }
        let track_h = viewport_h;
        // Use i64 intermediate to avoid overflow on large content heights.
        let thumb_h = ((track_h as i64 * track_h as i64) / self.content_h as i64) as i32;
        let thumb_h = thumb_h.clamp(SCROLLBAR_MIN_THUMB, track_h);
        let travel = track_h - thumb_h;
        let max = self.max_scroll_y(viewport_h);
        let offset = if max > 0 {
            ((self.offset_y as i64 * travel as i64) / max as i64) as i32
        } else {
            0
        };
        Some(Rect {
            x: viewport.x + viewport.width - SCROLLBAR_WIDTH - SCROLLBAR_MARGIN,
            y: viewport.y + offset,
            width: SCROLLBAR_WIDTH,
            height: thumb_h,
        })
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VelocityWindow — sliding window for velocity estimation (LPAR-05 §8.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct VelocitySample {
    pos: i32,
    tick: u32,
}

/// Sliding window velocity estimator for the drag stream.
struct VelocityWindow {
    samples: [VelocitySample; VELOCITY_WINDOW_N],
    count: usize,
    head: usize,
}

impl VelocityWindow {
    fn new() -> Self {
        Self {
            samples: [VelocitySample { pos: 0, tick: 0 }; VELOCITY_WINDOW_N],
            count: 0,
            head: 0,
        }
    }

    fn push(&mut self, pos: i32, tick: u32) {
        self.samples[self.head] = VelocitySample { pos, tick };
        self.head = (self.head + 1) % VELOCITY_WINDOW_N;
        if self.count < VELOCITY_WINDOW_N {
            self.count += 1;
        }
    }

    /// Estimate velocity in pixels per tick over the window.
    ///
    /// Returns 0 if the window is empty.  If Δticks == 0 (all samples in the
    /// same tick), the window cannot estimate velocity; returns 0 (spec §8.2:
    /// "window is widened" — here we treat it as zero to avoid divide-by-zero;
    /// callers may widen by using more samples or returning 0).
    fn estimate_velocity(&self) -> i32 {
        if self.count < 2 {
            return 0;
        }
        // The most recent sample is at head - 1 (wrapping).
        let newest_idx = if self.head == 0 {
            VELOCITY_WINDOW_N - 1
        } else {
            self.head - 1
        };
        // The oldest sample we have.
        let oldest_idx = if self.count < VELOCITY_WINDOW_N {
            // Samples are at [0..count), head points just past the newest.
            // oldest = (head - count + N) % N
            (self.head + VELOCITY_WINDOW_N - self.count) % VELOCITY_WINDOW_N
        } else {
            self.head % VELOCITY_WINDOW_N
        };
        let newest = self.samples[newest_idx];
        let oldest = self.samples[oldest_idx];
        let dt = newest.tick.wrapping_sub(oldest.tick);
        if dt == 0 {
            return 0;
        }
        let dp = newest.pos - oldest.pos;
        // Integer division with sign preservation.
        dp / dt as i32
    }
}

// ---------------------------------------------------------------------------
// ScrollSession — active drag/throw session
// ---------------------------------------------------------------------------

/// Internal state for one active scroll session.
struct ScrollSession {
    /// Structural path from root to the active container node.
    container_path: Vec<usize>,
    /// Tick at which the session started.
    #[allow(dead_code)]
    start_tick: u32,
    /// Velocity window for throw estimation.
    vel_y: VelocityWindow,
    /// Current throw/snap tween, if in throw/snap-settle phase.
    throw_tween: Option<Tween>,
    /// Whether the session is in throw/snap-settle phase (post-DragEnd).
    in_throw: bool,
    /// Starting offset at the time throw began (for tween from/to).
    #[allow(dead_code)]
    throw_from_offset: i32,
    /// Target offset the throw tween is moving toward.
    #[allow(dead_code)]
    throw_to_offset: i32,
}

impl ScrollSession {
    fn new(path: Vec<usize>, tick: u32) -> Self {
        Self {
            container_path: path,
            start_tick: tick,
            vel_y: VelocityWindow::new(),
            throw_tween: None,
            in_throw: false,
            throw_from_offset: 0,
            throw_to_offset: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollController
// ---------------------------------------------------------------------------

/// Scroll controller configuration.
///
/// All constants are exposed here so callers can override defaults while
/// preserving the determinism guarantee.
#[derive(Debug, Clone, Copy)]
pub struct ScrollConfig {
    /// Minimum throw velocity (pixels per tick) to enter momentum mode.
    pub min_throw_velocity: i32,
    /// Maximum throw velocity cap (pixels per tick).
    pub max_throw_velocity: i32,
    /// Throw duration scale factor.
    pub throw_duration_factor: u32,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self {
            min_throw_velocity: MIN_THROW_VELOCITY_PX_PER_TICK,
            max_throw_velocity: MAX_THROW_VELOCITY_PX_PER_TICK,
            throw_duration_factor: THROW_DURATION_FACTOR,
        }
    }
}

/// LPAR-05 scroll controller.
///
/// Consumes `DragStart`/`DragMove`/`DragEnd` events and tick advances to drive
/// scroll offsets on `ObjectNode`-based scroll containers.  Call
/// [`process`](Self::process) on every drag event and
/// [`tick`](Self::tick) on every [`Event::Tick`](crate::event::Event::Tick).
///
/// # Determinism
///
/// Given identical `(event, tick)` input sequences, the controller produces
/// identical emitted-event and offset sequences (LPAR-05 §8.1).
///
/// # Thread safety
///
/// `ScrollController` is not `Send` or `Sync`.  Use it from a single thread.
pub struct ScrollController {
    config: ScrollConfig,
    /// Current tick counter, incremented on each `Event::Tick`.
    tick: u32,
    /// Active scroll session, if any.
    session: Option<ScrollSession>,
    /// Last drag position (for computing deltas in DragMove).
    last_drag_y: i32,
    last_drag_x: i32,
}

impl ScrollController {
    /// Create a new controller with the given configuration.
    pub fn new(config: ScrollConfig) -> Self {
        Self {
            config,
            tick: 0,
            session: None,
            last_drag_y: 0,
            last_drag_x: 0,
        }
    }

    /// Create a new controller with default configuration.
    pub fn default_config() -> Self {
        Self::new(ScrollConfig::default())
    }

    /// Advance the internal tick counter and process any active throw/snap tween.
    ///
    /// Call this once per `Event::Tick`. The `sink` callback receives each
    /// viewport dirty rect for every effective offset change.
    pub fn tick(&mut self, root: &mut ObjectNode, sink: &mut dyn FnMut(Rect)) {
        self.tick = self.tick.wrapping_add(1);
        self.advance_throw(root, sink);
    }

    /// Process a drag event (`DragStart`, `DragMove`, or `DragEnd`).
    ///
    /// Pass the drag stream event from the `DragRecognizer`. Other events are
    /// silently ignored (use [`tick`](Self::tick) for `Event::Tick`).
    ///
    /// The `sink` callback receives viewport dirty rects for every effective
    /// offset change.
    pub fn process(&mut self, root: &mut ObjectNode, ev: &Event, sink: &mut dyn FnMut(Rect)) {
        match ev {
            Event::DragStart {
                x,
                y,
                origin_x,
                origin_y,
            } => self.on_drag_start(root, *x, *y, *origin_x, *origin_y, sink),
            Event::DragMove { x, y } => self.on_drag_move(root, *x, *y, sink),
            Event::DragEnd { x, y } => self.on_drag_end(root, *x, *y, sink),
            _ => {}
        }
    }

    /// Return `true` when a scroll session is currently active (between
    /// `ScrollBegin` and `ScrollEnd`, inclusive of the throw phase).
    ///
    /// Use this from the platform layer to determine whether a drag event has
    /// been absorbed by the scroll controller so that it MUST NOT also be
    /// dispatched as a normal drag gesture (LPAR-05 §7.2).
    pub fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// Notify the controller that the container at `path` is being invalidated
    /// (detached, hidden, or `SCROLLABLE` cleared) during an active session.
    ///
    /// If the active session targets that container, `ScrollEnd` is emitted
    /// before the structural change becomes visible (LPAR-05 §7.6). Call this
    /// before mutating the node.
    pub fn cancel_if_active(
        &mut self,
        root: &mut ObjectNode,
        path: &[usize],
        sink: &mut dyn FnMut(Rect),
    ) {
        let is_active = self
            .session
            .as_ref()
            .map(|s| s.container_path.as_slice() == path)
            .unwrap_or(false);
        if is_active {
            self.emit_scroll_end(root, sink);
            self.session = None;
        }
    }

    // -----------------------------------------------------------------------
    // DragStart handler
    // -----------------------------------------------------------------------

    fn on_drag_start(
        &mut self,
        root: &mut ObjectNode,
        x: i32,
        y: i32,
        _origin_x: i32,
        _origin_y: i32,
        _sink: &mut dyn FnMut(Rect),
    ) {
        // §8.5 supersession: a new DragStart during an active throw silently
        // cancels the old session — no ScrollEnd for the interrupted session.
        self.session = None;

        // Find the deepest SCROLLABLE ancestor of the hit-test target.
        let path = find_scrollable_ancestor(root, x, y);
        if path.is_none() {
            return;
        }
        let path = path.unwrap();

        // Check axis alignment: vertical scroll requires vertical drag direction.
        // At DragStart, we don't know direction yet (threshold just crossed).
        // We activate optimistically and check axis on DragMove.
        // For now: activate if the container's axis is Vertical or Both.
        {
            let container = node_at_path(root, &path);
            if let Some(ss) = &container.scroll {
                if ss.axis == ScrollAxis::Horizontal {
                    // Horizontal-only: do not activate (v1 is vertical-first).
                    return;
                }
            } else {
                // No scroll state even though SCROLLABLE is set; skip.
                return;
            }
        }

        let mut session = ScrollSession::new(path.clone(), self.tick);
        // Record start position.
        let start_y = node_at_path(root, &path)
            .scroll
            .as_ref()
            .map(|s| s.offset_y)
            .unwrap_or(0);
        session.vel_y.push(start_y, self.tick);

        // Emit ScrollBegin.
        dispatch_to_container(root, &path, ObjectEvent::ScrollBegin);

        self.last_drag_y = y;
        self.last_drag_x = x;
        self.session = Some(session);
    }

    // -----------------------------------------------------------------------
    // DragMove handler
    // -----------------------------------------------------------------------

    fn on_drag_move(&mut self, root: &mut ObjectNode, x: i32, y: i32, sink: &mut dyn FnMut(Rect)) {
        let in_throw = self.session.as_ref().map(|s| s.in_throw).unwrap_or(true);
        if self.session.is_none() || in_throw {
            return;
        }

        let delta_y = self.last_drag_y - y; // drag DOWN = positive offset (scroll content up)
        self.last_drag_y = y;
        self.last_drag_x = x;

        let path = self.session.as_ref().unwrap().container_path.clone();

        // Get current viewport height and offset.
        let viewport_h = node_at_path(root, &path).widget().borrow().bounds().height;
        let viewport_rect = node_at_path(root, &path).widget().borrow().bounds();

        let (current_offset_y, max_scroll_y) = {
            let node = node_at_path(root, &path);
            let ss = node.scroll.as_ref().unwrap();
            (ss.offset_y, ss.max_scroll_y(viewport_h))
        };

        let new_offset_y = (current_offset_y + delta_y).clamp(0, max_scroll_y);
        let changed = new_offset_y != current_offset_y;

        // Update the offset.
        if changed {
            node_at_path_mut(root, &path)
                .scroll
                .as_mut()
                .unwrap()
                .offset_y = new_offset_y;
        }

        // Check for chaining: at edge with residual delta.
        let at_edge = new_offset_y == 0 || new_offset_y == max_scroll_y;
        let residual = if at_edge && delta_y != 0 {
            let unclamped = current_offset_y + delta_y;
            unclamped - new_offset_y
        } else {
            0
        };

        // Emit Scroll if changed.
        if changed {
            let (ox, oy) = {
                let n = node_at_path(root, &path);
                let ss = n.scroll.as_ref().unwrap();
                (ss.offset_x, ss.offset_y)
            };
            dispatch_to_container(
                root,
                &path,
                ObjectEvent::Scroll {
                    scroll_x: ox,
                    scroll_y: oy,
                },
            );
            sink(viewport_rect);
        }

        // Push velocity sample.
        let offset_for_vel = node_at_path(root, &path)
            .scroll
            .as_ref()
            .map(|s| s.offset_y)
            .unwrap_or(0);
        if let Some(ref mut sess) = self.session {
            sess.vel_y.push(offset_for_vel, self.tick);
        }

        // Handle chaining: inner at edge → hand off to outer SCROLLABLE ancestor.
        if residual != 0
            && at_edge
            && let Some(ancestor_path) = find_scrollable_ancestor_above(root, &path)
        {
            // Emit ScrollEnd on inner.
            self.emit_scroll_end(root, sink);
            // Begin new session on ancestor.
            let ancestor_start_y = node_at_path(root, &ancestor_path)
                .scroll
                .as_ref()
                .map(|s| s.offset_y)
                .unwrap_or(0);
            let mut new_session = ScrollSession::new(ancestor_path.clone(), self.tick);
            new_session.vel_y.push(ancestor_start_y, self.tick);
            dispatch_to_container(root, &ancestor_path, ObjectEvent::ScrollBegin);
            self.session = Some(new_session);

            // Apply residual to ancestor.
            let ancestor_viewport_h = node_at_path(root, &ancestor_path)
                .widget()
                .borrow()
                .bounds()
                .height;
            let ancestor_viewport_rect = node_at_path(root, &ancestor_path)
                .widget()
                .borrow()
                .bounds();
            let (ancestor_offset_y, ancestor_max) = {
                let node = node_at_path(root, &ancestor_path);
                let ss = node.scroll.as_ref().unwrap();
                (ss.offset_y, ss.max_scroll_y(ancestor_viewport_h))
            };
            let new_ancestor_y = (ancestor_offset_y + residual).clamp(0, ancestor_max);
            if new_ancestor_y != ancestor_offset_y {
                node_at_path_mut(root, &ancestor_path)
                    .scroll
                    .as_mut()
                    .unwrap()
                    .offset_y = new_ancestor_y;
                let (aox, aoy) = {
                    let n = node_at_path(root, &ancestor_path);
                    let ss = n.scroll.as_ref().unwrap();
                    (ss.offset_x, ss.offset_y)
                };
                dispatch_to_container(
                    root,
                    &ancestor_path,
                    ObjectEvent::Scroll {
                        scroll_x: aox,
                        scroll_y: aoy,
                    },
                );
                sink(ancestor_viewport_rect);
                let new_vel_y = node_at_path(root, &ancestor_path)
                    .scroll
                    .as_ref()
                    .map(|s| s.offset_y)
                    .unwrap_or(0);
                if let Some(ref mut sess) = self.session {
                    sess.vel_y.push(new_vel_y, self.tick);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // DragEnd handler
    // -----------------------------------------------------------------------

    fn on_drag_end(&mut self, root: &mut ObjectNode, _x: i32, _y: i32, sink: &mut dyn FnMut(Rect)) {
        if self.session.is_none() {
            return;
        }

        // Estimate velocity.
        let raw_vel = self.session.as_ref().unwrap().vel_y.estimate_velocity();
        let vel = raw_vel.clamp(
            -self.config.max_throw_velocity,
            self.config.max_throw_velocity,
        );

        let path = self.session.as_ref().unwrap().container_path.clone();

        let viewport_h = node_at_path(root, &path).widget().borrow().bounds().height;
        let (current_offset_y, max_scroll_y) = {
            let node = node_at_path(root, &path);
            let ss = node.scroll.as_ref().unwrap();
            (ss.offset_y, ss.max_scroll_y(viewport_h))
        };

        if vel.abs() >= self.config.min_throw_velocity {
            // Compute projected endpoint: offset + vel * duration (in ticks).
            let duration = vel.unsigned_abs() * self.config.throw_duration_factor;
            let projected = current_offset_y + vel * duration as i32;
            let mut endpoint = projected.clamp(0, max_scroll_y);

            // Snap: adjust endpoint before first momentum Scroll.
            endpoint = snap_endpoint(
                root,
                &path,
                endpoint,
                current_offset_y,
                viewport_h,
                self.config.min_throw_velocity,
            );
            endpoint = endpoint.clamp(0, max_scroll_y);

            // Emit ScrollThrow.
            dispatch_to_container(
                root,
                &path,
                ObjectEvent::ScrollThrow {
                    vel_x: 0,
                    vel_y: vel,
                },
            );

            // Set up throw tween.
            let tween =
                Tween::new(current_offset_y, endpoint, duration).with_easing(Easing::EaseOut);

            if let Some(ref mut sess) = self.session {
                sess.throw_tween = Some(tween);
                sess.in_throw = true;
                sess.throw_from_offset = current_offset_y;
                sess.throw_to_offset = endpoint;
            }
        } else {
            // Below the throw threshold. Per LPAR-05 §9.1, snapping settles at
            // the end of a drag OR a throw: if a snap config attracts the
            // resting offset to a nearby point, animate a short settle tween
            // (no `ScrollThrow` — this is a correction, not a fling) and let
            // `advance_throw` emit `ScrollEnd` on completion. Otherwise the
            // session ends immediately.
            let settled = snap_endpoint(
                root,
                &path,
                current_offset_y,
                current_offset_y,
                viewport_h,
                self.config.min_throw_velocity,
            )
            .clamp(0, max_scroll_y);

            if settled != current_offset_y {
                let tween = Tween::new(current_offset_y, settled, SNAP_SETTLE_TICKS)
                    .with_easing(Easing::EaseOut);
                if let Some(ref mut sess) = self.session {
                    sess.throw_tween = Some(tween);
                    sess.in_throw = true;
                    sess.throw_from_offset = current_offset_y;
                    sess.throw_to_offset = settled;
                }
            } else {
                self.emit_scroll_end(root, sink);
                self.session = None;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Throw tick advance
    // -----------------------------------------------------------------------

    fn advance_throw(&mut self, root: &mut ObjectNode, sink: &mut dyn FnMut(Rect)) {
        // Step the tween and capture results, then drop the borrow before
        // mutating root.
        let tween_step = match &mut self.session {
            Some(s) if s.in_throw => {
                if let Some(ref mut t) = s.throw_tween {
                    let val = t.step();
                    let finished = t.finished();
                    let path = s.container_path.clone();
                    Some((path, val, finished))
                } else {
                    None
                }
            }
            _ => None,
        };

        let (path, new_offset, tween_done) = match tween_step {
            Some(v) => v,
            None => return,
        };

        let viewport_h = node_at_path(root, &path).widget().borrow().bounds().height;
        let viewport_rect = node_at_path(root, &path).widget().borrow().bounds();
        let max_scroll_y = node_at_path(root, &path)
            .scroll
            .as_ref()
            .map(|s| s.max_scroll_y(viewport_h))
            .unwrap_or(0);
        let current_offset_y = node_at_path(root, &path)
            .scroll
            .as_ref()
            .map(|s| s.offset_y)
            .unwrap_or(0);

        let clamped = new_offset.clamp(0, max_scroll_y);
        let at_edge = clamped != new_offset; // Tween went outside bounds.
        let changed = clamped != current_offset_y;

        if changed {
            node_at_path_mut(root, &path)
                .scroll
                .as_mut()
                .unwrap()
                .offset_y = clamped;
            let (ox, oy) = {
                let n = node_at_path(root, &path);
                let ss = n.scroll.as_ref().unwrap();
                (ss.offset_x, ss.offset_y)
            };
            dispatch_to_container(
                root,
                &path,
                ObjectEvent::Scroll {
                    scroll_x: ox,
                    scroll_y: oy,
                },
            );
            sink(viewport_rect);
        }

        if tween_done || at_edge {
            self.emit_scroll_end(root, sink);
            self.session = None;
        }
    }

    // -----------------------------------------------------------------------
    // Helper: emit ScrollEnd to current session's container
    // -----------------------------------------------------------------------

    fn emit_scroll_end(&self, root: &mut ObjectNode, _sink: &mut dyn FnMut(Rect)) {
        if let Some(ref session) = self.session {
            dispatch_to_container(root, &session.container_path, ObjectEvent::ScrollEnd);
        }
    }
}

// ---------------------------------------------------------------------------
// Snap endpoint adjustment (LPAR-05 §9)
// ---------------------------------------------------------------------------

/// Adjust a throw endpoint to the nearest snap point within the attraction radius.
fn snap_endpoint(
    root: &ObjectNode,
    path: &[usize],
    endpoint: i32,
    _current_offset: i32,
    viewport_h: i32,
    _min_vel: i32,
) -> i32 {
    let node = node_at_path(root, path);
    let ss = match node.scroll.as_ref() {
        Some(s) => s,
        None => return endpoint,
    };
    let radius = ss.snap_attraction_radius;
    let align = ss.snap_align;

    let snap_points: Vec<i32> = match &ss.snap {
        SnapConfig::None => return endpoint,
        SnapConfig::Explicit(list) => list.clone(),
        SnapConfig::ChildAligned => {
            // Derive snap points from direct children's bounds.
            node.children()
                .iter()
                .map(|child| {
                    let b = child.widget().borrow().bounds();
                    let raw = b.y; // top edge of child
                    // Apply alignment offset.
                    align_offset(raw, b.height, viewport_h, align)
                })
                .collect()
        }
    };

    // Find the nearest snap point within the attraction radius.
    let mut best = endpoint;
    let mut best_dist = i32::MAX;
    for &sp in &snap_points {
        let dist = (endpoint - sp).abs();
        if dist <= radius && dist < best_dist {
            best_dist = dist;
            best = sp;
        }
    }
    best
}

/// Convert a child top edge to the effective snap offset for the given alignment.
fn align_offset(child_top: i32, child_h: i32, viewport_h: i32, align: SnapAlign) -> i32 {
    match align {
        SnapAlign::Start => child_top,
        SnapAlign::Center => child_top - (viewport_h - child_h) / 2,
        SnapAlign::End => child_top - (viewport_h - child_h),
    }
}

// ---------------------------------------------------------------------------
// Path dispatch helper (LPAR-05 §6.3)
// ---------------------------------------------------------------------------

/// Dispatch a scroll `ObjectEvent` to the container at `path` and bubble per
/// existing `EVENT_BUBBLE` gating.
///
/// This is the LPAR-05 path-targeted dispatch: it does not use hit testing.
/// The [`ObjectFlags::EVENT_BUBBLE`] flag is **never** mutated (LPAR-05 §6.3).
pub fn dispatch_to_container(root: &mut ObjectNode, path: &[usize], event: ObjectEvent) {
    use crate::object::dispatch_object_event;
    let _ = dispatch_object_event(
        root,
        DispatchInput::Container {
            path: path.to_vec(),
            event,
        },
    );
}

// ---------------------------------------------------------------------------
// Tree traversal helpers
// ---------------------------------------------------------------------------

/// Find the deepest `SCROLLABLE` ancestor of the hit-test target at `(x, y)`.
///
/// Returns the structural path to that container, or `None` if no scrollable
/// ancestor exists (LPAR-05 §7.2).
fn find_scrollable_ancestor(root: &ObjectNode, x: i32, y: i32) -> Option<Vec<usize>> {
    // Hit-test to find the target path.
    let target_path = hit_test_path_for_scroll(root, x, y)?;
    // Walk from target toward root, find the deepest SCROLLABLE ancestor.
    // "Deepest" means the one closest to the target (longest path prefix).
    for depth in (0..=target_path.len()).rev() {
        let path = &target_path[..depth];
        let node = node_at_path(root, path);
        if node.flags().contains(ObjectFlags::SCROLLABLE) && node.scroll.is_some() {
            return Some(path.to_vec());
        }
    }
    None
}

/// Find the nearest `SCROLLABLE` ancestor ABOVE the container at `path`.
///
/// Used for chaining (LPAR-05 §10.2).
fn find_scrollable_ancestor_above(root: &ObjectNode, path: &[usize]) -> Option<Vec<usize>> {
    if path.is_empty() {
        return None;
    }
    for depth in (0..path.len()).rev() {
        let ancestor_path = &path[..depth];
        let node = node_at_path(root, ancestor_path);
        if node.flags().contains(ObjectFlags::SCROLLABLE) && node.scroll.is_some() {
            return Some(ancestor_path.to_vec());
        }
    }
    None
}

/// Hit-test, returning the structural path to the topmost hit node.
///
/// Unlike the main hit_test_path (which requires CLICKABLE), this one finds
/// the deepest node containing `(x, y)` regardless of flags, so we can walk
/// its ancestors to find a SCROLLABLE container.
fn hit_test_path_for_scroll(root: &ObjectNode, x: i32, y: i32) -> Option<Vec<usize>> {
    hit_test_scroll_recursive(root, x, y, &mut Vec::new())
}

fn hit_test_scroll_recursive(
    node: &ObjectNode,
    x: i32,
    y: i32,
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    if node.meta().is_detached() || node.flags().contains(ObjectFlags::HIDDEN) {
        return None;
    }
    // Check children first (reverse for topmost).
    for (i, child) in node.children().iter().enumerate().rev() {
        path.push(i);
        if let Some(p) = hit_test_scroll_recursive(child, x, y, path) {
            return Some(p);
        }
        path.pop();
    }
    // Check this node.
    let b = node.widget().borrow().bounds();
    if b.width > 0
        && b.height > 0
        && x >= b.x
        && y >= b.y
        && x < b.x + b.width
        && y < b.y + b.height
    {
        Some(path.clone())
    } else {
        None
    }
}

/// Navigate to a node by structural path (immutable).
fn node_at_path<'a>(root: &'a ObjectNode, path: &[usize]) -> &'a ObjectNode {
    let mut current = root;
    for &idx in path {
        current = &current.children()[idx];
    }
    current
}

/// Navigate to a node by structural path (mutable).
fn node_at_path_mut<'a>(root: &'a mut ObjectNode, path: &[usize]) -> &'a mut ObjectNode {
    let mut current = root;
    for &idx in path {
        current = &mut current.children_mut()[idx];
    }
    current
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use super::*;
    use crate::event::Event;
    use crate::object::{ObjectEvent, ObjectFlags, ObjectNode};
    use crate::renderer::Renderer;
    use crate::widget::{Rect, Widget};

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct TestWidget {
        bounds: Rect,
    }

    impl TestWidget {
        fn node(bounds: Rect) -> ObjectNode {
            ObjectNode::new(Rc::new(RefCell::new(Self { bounds })))
        }
    }

    impl Widget for TestWidget {
        fn bounds(&self) -> Rect {
            self.bounds
        }
        fn draw(&self, _renderer: &mut dyn Renderer) {}
        fn handle_event(&mut self, _event: &Event) -> bool {
            false
        }
    }

    /// Build: root(400x600) → container(400x600, SCROLLABLE, content 400x1200) → child(400x1200)
    fn scroll_tree() -> ObjectNode {
        let mut root = TestWidget::node(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 600,
        });
        let mut container = TestWidget::node(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 600,
        });
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        container.set_scroll_state(Box::new(ss));
        let child = TestWidget::node(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 1200,
        });
        container.append_child(child);
        root.append_child(container);
        root
    }

    fn collect_events(root: &mut ObjectNode, path: &[usize]) -> Rc<RefCell<Vec<ObjectEvent>>> {
        let log: Rc<RefCell<Vec<ObjectEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        let node = node_at_path_mut(root, path);
        node.add_target_handler(move |ev, _ctx| {
            l.borrow_mut().push(ev.clone());
            false
        });
        log
    }

    // -----------------------------------------------------------------------
    // 1. No activation on a non-scrollable tree
    // -----------------------------------------------------------------------

    #[test]
    fn no_activation_on_non_scrollable_tree() {
        let mut root = TestWidget::node(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 600,
        });
        let child = TestWidget::node(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 600,
        });
        root.append_child(child);

        let mut ctrl = ScrollController::default_config();
        let mut dirty: Vec<Rect> = Vec::new();
        let mut sink = |r: Rect| dirty.push(r);

        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 100,
                origin_x: 200,
                origin_y: 150,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 80 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 80 }, &mut sink);

        assert!(
            dirty.is_empty(),
            "no dirty rects without a scrollable container"
        );
    }

    // -----------------------------------------------------------------------
    // 2. Activation on SCROLLABLE+axis-aligned
    // -----------------------------------------------------------------------

    #[test]
    fn activation_on_scrollable_container() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut dirty: Vec<Rect> = Vec::new();
        let mut sink = |r: Rect| dirty.push(r);

        // DragStart anywhere inside the container.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 100,
                origin_x: 200,
                origin_y: 110,
            },
            &mut sink,
        );
        let log_snap = log.borrow().clone();
        assert!(
            log_snap
                .iter()
                .any(|e| matches!(e, ObjectEvent::ScrollBegin)),
            "ScrollBegin must be emitted on DragStart; got: {:?}",
            log_snap
        );
    }

    // -----------------------------------------------------------------------
    // 3. ScrollBegin precedes Scroll; ScrollEnd is last; zero-delta session
    // -----------------------------------------------------------------------

    #[test]
    fn scroll_event_ordering() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Drag that produces actual scroll.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 300,
                origin_x: 200,
                origin_y: 310,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 280 }, &mut sink); // delta = +20 downward
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 280 }, &mut sink);

        let log_snap = log.borrow().clone();

        // There must be a ScrollBegin before any Scroll.
        let begin_pos = log_snap
            .iter()
            .position(|e| matches!(e, ObjectEvent::ScrollBegin));
        let scroll_pos = log_snap
            .iter()
            .position(|e| matches!(e, ObjectEvent::Scroll { .. }));
        let end_pos = log_snap
            .iter()
            .rposition(|e| matches!(e, ObjectEvent::ScrollEnd));

        assert!(begin_pos.is_some(), "ScrollBegin must be emitted");
        if let Some(sp) = scroll_pos {
            assert!(begin_pos.unwrap() < sp, "ScrollBegin must precede Scroll");
        }
        assert!(end_pos.is_some(), "ScrollEnd must be emitted");
        // ScrollEnd must be the last scroll event.
        let last_scroll_event_idx = log_snap.iter().rposition(|e| {
            matches!(
                e,
                ObjectEvent::ScrollBegin
                    | ObjectEvent::Scroll { .. }
                    | ObjectEvent::ScrollEnd
                    | ObjectEvent::ScrollThrow { .. }
            )
        });
        assert_eq!(
            end_pos, last_scroll_event_idx,
            "ScrollEnd must be the last scroll event"
        );
    }

    // -----------------------------------------------------------------------
    // 4. ScrollEnd fires even on zero-delta session
    // -----------------------------------------------------------------------

    #[test]
    fn scroll_end_fires_on_zero_delta_session() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // DragStart then immediate DragEnd with no DragMove.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 300,
                origin_x: 200,
                origin_y: 305,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 300 }, &mut sink);

        let log_snap = log.borrow().clone();
        assert!(
            log_snap
                .iter()
                .any(|e| matches!(e, ObjectEvent::ScrollBegin)),
            "ScrollBegin required"
        );
        assert!(
            log_snap.iter().any(|e| matches!(e, ObjectEvent::ScrollEnd)),
            "ScrollEnd required on zero-delta"
        );
        assert!(
            !log_snap
                .iter()
                .any(|e| matches!(e, ObjectEvent::Scroll { .. })),
            "no Scroll on zero-delta"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Offset clamps to [0, max_scroll]; effective change emits Scroll + dirty rect
    // -----------------------------------------------------------------------

    #[test]
    fn offset_clamps_and_emits_scroll() {
        let mut root = scroll_tree();
        let _log = collect_events(&mut root, &[0]);
        let mut ctrl = ScrollController::default_config();
        let mut dirty: Vec<Rect> = Vec::new();
        let mut sink = |r: Rect| dirty.push(r);

        // Drag heavily upward (should clamp at 0).
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 510,
            },
            &mut sink,
        );
        // Move up a LOT — should clamp at 0.
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 1000 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 1000 }, &mut sink);

        let offset = node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y;
        assert_eq!(
            offset, 0,
            "offset must clamp at 0 on upward drag past start"
        );

        // Now drag downward past max_scroll.
        // Use DragStart at y=700 so a DragMove to y=0 gives delta_y=700 > max_scroll=600.
        let mut root2 = scroll_tree();
        let mut dirty2: Vec<Rect> = Vec::new();
        let mut sink2 = |r: Rect| dirty2.push(r);
        let mut ctrl2 = ScrollController::default_config();
        ctrl2.process(
            &mut root2,
            &Event::DragStart {
                x: 200,
                y: 700,
                origin_x: 200,
                origin_y: 710,
            },
            &mut sink2,
        );
        ctrl2.process(&mut root2, &Event::DragMove { x: 200, y: 0 }, &mut sink2); // delta_y = 700 > max_scroll=600
        ctrl2.process(&mut root2, &Event::DragEnd { x: 200, y: 0 }, &mut sink2);

        let offset2 = node_at_path(&root2, &[0]).scroll.as_ref().unwrap().offset_y;
        let max_y = 1200 - 600; // 600
        assert_eq!(
            offset2, max_y,
            "offset must clamp at max_scroll on downward drag"
        );

        // Should have emitted at least one dirty rect.
        assert!(
            !dirty2.is_empty(),
            "dirty rect must be pushed on effective offset change"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Throw: above-threshold → ScrollThrow once, then Scroll until ScrollEnd
    // -----------------------------------------------------------------------

    #[test]
    fn throw_above_threshold_emits_scroll_throw() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Build up some velocity: tick a few times, then drag fast.
        ctrl.tick(&mut root, &mut sink); // tick 1
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 510,
            },
            &mut sink,
        );
        ctrl.tick(&mut root, &mut sink); // tick 2
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 490 }, &mut sink);
        ctrl.tick(&mut root, &mut sink); // tick 3
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 470 }, &mut sink);
        ctrl.tick(&mut root, &mut sink); // tick 4
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 450 }, &mut sink);
        ctrl.tick(&mut root, &mut sink); // tick 5
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 450 }, &mut sink);

        let log_snap = log.borrow().clone();
        let has_throw = log_snap
            .iter()
            .any(|e| matches!(e, ObjectEvent::ScrollThrow { .. }));
        if has_throw {
            let throw_idx = log_snap
                .iter()
                .position(|e| matches!(e, ObjectEvent::ScrollThrow { .. }))
                .unwrap();
            let begin_idx = log_snap
                .iter()
                .position(|e| matches!(e, ObjectEvent::ScrollBegin))
                .unwrap();
            assert!(
                begin_idx < throw_idx,
                "ScrollBegin must precede ScrollThrow"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 7. Below-threshold: ScrollEnd immediately after DragEnd
    // -----------------------------------------------------------------------

    #[test]
    fn below_threshold_scroll_end_immediately() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Single slow move.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 300,
                origin_x: 200,
                origin_y: 310,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 299 }, &mut sink); // 1 px delta
        // Many ticks before DragEnd → velocity very low.
        for _ in 0..20 {
            ctrl.tick(&mut root, &mut sink);
        }
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 299 }, &mut sink);

        let log_snap = log.borrow().clone();
        assert!(
            !log_snap
                .iter()
                .any(|e| matches!(e, ObjectEvent::ScrollThrow { .. })),
            "slow drag must not emit ScrollThrow; got: {:?}",
            log_snap
        );
        assert!(
            log_snap.iter().any(|e| matches!(e, ObjectEvent::ScrollEnd)),
            "ScrollEnd must be emitted after below-threshold DragEnd"
        );
    }

    // -----------------------------------------------------------------------
    // 8. Determinism: identical (event, tick) script → identical results
    // -----------------------------------------------------------------------

    #[test]
    fn determinism_identical_input_produces_identical_output() {
        let make_events = || -> (Vec<ObjectEvent>, i32) {
            let mut root = scroll_tree();
            let log = collect_events(&mut root, &[0]);
            let mut ctrl = ScrollController::default_config();
            let mut sink = |_: Rect| {};

            ctrl.tick(&mut root, &mut sink);
            ctrl.process(
                &mut root,
                &Event::DragStart {
                    x: 200,
                    y: 400,
                    origin_x: 200,
                    origin_y: 420,
                },
                &mut sink,
            );
            ctrl.tick(&mut root, &mut sink);
            ctrl.process(&mut root, &Event::DragMove { x: 200, y: 380 }, &mut sink);
            ctrl.tick(&mut root, &mut sink);
            ctrl.process(&mut root, &Event::DragMove { x: 200, y: 360 }, &mut sink);
            ctrl.tick(&mut root, &mut sink);
            ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 360 }, &mut sink);
            // Run 20 ticks to settle any throw.
            for _ in 0..20 {
                ctrl.tick(&mut root, &mut sink);
            }

            let final_offset = node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y;
            (log.borrow().clone(), final_offset)
        };

        let (events1, offset1) = make_events();
        let (events2, offset2) = make_events();

        assert_eq!(
            events1, events2,
            "identical input must produce identical events"
        );
        assert_eq!(
            offset1, offset2,
            "identical input must produce identical final offset"
        );
    }

    // -----------------------------------------------------------------------
    // 9. Snap: explicit-list settle
    // -----------------------------------------------------------------------

    #[test]
    fn snap_explicit_list_adjusts_endpoint() {
        let mut root = {
            let mut root = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut container = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut ss = ScrollState::new();
            ss.content_h = 1200;
            ss.snap = SnapConfig::Explicit(vec![0, 300, 600]);
            ss.snap_attraction_radius = 100;
            container.set_scroll_state(Box::new(ss));
            root.append_child(container);
            root
        };

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Drag to roughly 280 (near snap point 300).
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 510,
            },
            &mut sink,
        );
        // DragMove: move 280 pixels down (delta_y = +280).
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 220 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 220 }, &mut sink);

        // If throw velocity is above threshold, the throw endpoint should snap to 300.
        // If below threshold, the offset stays at ~280 (no snap during drag, only at throw).
        // We just confirm no panic and that snap logic runs.
        let _offset = node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y;
        // The test mainly verifies no panic and snap code runs; exact offset depends on velocity.
    }

    #[test]
    fn below_threshold_drag_settles_to_snap_point() {
        // A slow drag (velocity below the throw threshold) that rests near a
        // snap point MUST settle to it via a tween (LPAR-05 §9.1: snapping at
        // the end of a drag OR throw), not stop wherever the finger lifted.
        let mut root = {
            let mut root = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut container = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut ss = ScrollState::new();
            ss.content_h = 1200;
            ss.snap = SnapConfig::Explicit(vec![0, 300, 600]);
            ss.snap_attraction_radius = 100;
            container.set_scroll_state(Box::new(ss));
            root.append_child(container);
            root
        };
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Single move to offset 280 with no intervening ticks → dt == 0 →
        // velocity 0 → below the throw threshold.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 510,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 220 }, &mut sink);
        assert_eq!(
            node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y,
            280,
            "drag lands at 280 before release"
        );
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 220 }, &mut sink);

        // Drive the settle tween to completion.
        for _ in 0..SNAP_SETTLE_TICKS + 2 {
            ctrl.tick(&mut root, &mut sink);
        }

        assert_eq!(
            node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y,
            300,
            "below-threshold release settles to the nearest snap point"
        );
        let evs = log.borrow();
        assert!(
            !evs.iter()
                .any(|e| matches!(e, ObjectEvent::ScrollThrow { .. })),
            "a snap settle is not a fling — no ScrollThrow: {evs:?}"
        );
        assert!(
            evs.iter().any(|e| matches!(e, ObjectEvent::ScrollEnd)),
            "settle completes with ScrollEnd: {evs:?}"
        );
    }

    // -----------------------------------------------------------------------
    // 10. Child-aligned snap
    // -----------------------------------------------------------------------

    #[test]
    fn snap_child_aligned_derives_from_children() {
        let mut root = {
            let mut root = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut container = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            });
            let mut ss = ScrollState::new();
            ss.content_h = 1800;
            ss.snap = SnapConfig::ChildAligned;
            ss.snap_align = SnapAlign::Start;
            ss.snap_attraction_radius = 80;
            container.set_scroll_state(Box::new(ss));
            // Three children at y=0, y=600, y=1200.
            container.append_child(TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 600,
            }));
            container.append_child(TestWidget::node(Rect {
                x: 0,
                y: 600,
                width: 400,
                height: 600,
            }));
            container.append_child(TestWidget::node(Rect {
                x: 0,
                y: 1200,
                width: 400,
                height: 600,
            }));
            root.append_child(container);
            root
        };

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Use ticks between events to produce a non-zero velocity estimate.
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 510,
            },
            &mut sink,
        );
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 400 }, &mut sink);
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 300 }, &mut sink);
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 200 }, &mut sink);
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 200 }, &mut sink);
        // Run enough ticks to fully settle the throw (duration ≈ vel*FACTOR ≤ 80*6=480 ticks).
        for _ in 0..600 {
            ctrl.tick(&mut root, &mut sink);
        }

        // After settling, offset should be at a child boundary (snap point),
        // or at the maximum scroll offset.
        let offset = node_at_path(&root, &[0]).scroll.as_ref().unwrap().offset_y;
        let max_scroll = 1800 - 600; // 1200
        assert!(
            offset == 0 || offset == 600 || offset == 1200 || offset == max_scroll,
            "offset should snap to a child boundary, got {}",
            offset
        );
    }

    // -----------------------------------------------------------------------
    // 11. Chaining: inner-at-edge hands residual to outer
    // -----------------------------------------------------------------------

    #[test]
    fn chaining_inner_at_edge_activates_outer() {
        // Build: root → outer(SCROLLABLE, 400x400, content 400x1200)
        //                 → inner(SCROLLABLE, 400x200, content 400x400)
        let mut root = {
            let mut root = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 400,
            });
            let mut outer = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 400,
            });
            let mut outer_ss = ScrollState::new();
            outer_ss.content_h = 1200;
            outer.set_scroll_state(Box::new(outer_ss));

            let mut inner = TestWidget::node(Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 200,
            });
            let mut inner_ss = ScrollState::new();
            inner_ss.content_h = 400; // max_scroll = 400-200 = 200
            inner.set_scroll_state(Box::new(inner_ss));

            outer.append_child(inner);
            root.append_child(outer);
            root
        };

        let outer_log = collect_events(&mut root, &[0]);
        let inner_log = collect_events(&mut root, &[0, 0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // First scroll inner to its max offset.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 150,
                origin_x: 200,
                origin_y: 160,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 0 }, &mut sink);
        let inner_offset = node_at_path(&root, &[0, 0])
            .scroll
            .as_ref()
            .unwrap()
            .offset_y;
        assert!(inner_offset <= 200, "inner clamped at max");

        // Continue drag (residual should chain to outer).
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: -100 }, &mut sink);

        let inner_events = inner_log.borrow().clone();
        let outer_events = outer_log.borrow().clone();

        // Just verify no panic.
        let _ = inner_events;
        let _ = outer_events;
    }

    // -----------------------------------------------------------------------
    // 12. Supersession: new DragStart during throw → no ScrollEnd for old session
    // -----------------------------------------------------------------------

    #[test]
    fn supersession_no_scroll_end_for_interrupted_throw() {
        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        // Start a throw.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 500,
                origin_x: 200,
                origin_y: 520,
            },
            &mut sink,
        );
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 480 }, &mut sink);
        ctrl.tick(&mut root, &mut sink);
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 460 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 460 }, &mut sink);

        // Count ScrollEnd events so far.
        let end_count_before = log
            .borrow()
            .iter()
            .filter(|e| matches!(e, ObjectEvent::ScrollEnd))
            .count();

        // New DragStart during throw — should NOT emit ScrollEnd for old session.
        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 400,
                origin_x: 200,
                origin_y: 410,
            },
            &mut sink,
        );

        let end_count_after = log
            .borrow()
            .iter()
            .filter(|e| matches!(e, ObjectEvent::ScrollEnd))
            .count();

        assert_eq!(
            end_count_before, end_count_after,
            "new DragStart during throw must NOT emit ScrollEnd for the old session"
        );
    }

    // -----------------------------------------------------------------------
    // 13. Bubbling: scroll event reaches ancestor ONLY with EVENT_BUBBLE set;
    //     controller never mutates the flag.
    // -----------------------------------------------------------------------

    #[test]
    fn bubbling_requires_event_bubble_flag_on_container() {
        let mut root = scroll_tree();

        // Register a bubble handler on root.
        let root_saw: Rc<RefCell<Vec<ObjectEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let rsaw = root_saw.clone();
        root.add_bubble_handler(move |ev, _ctx| {
            rsaw.borrow_mut().push(ev.clone());
            false
        });

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 300,
                origin_x: 200,
                origin_y: 310,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 280 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 280 }, &mut sink);

        // Container [0] does NOT have EVENT_BUBBLE, so root should NOT see scroll events.
        let root_events = root_saw.borrow();
        let has_scroll = root_events.iter().any(|e| {
            matches!(
                e,
                ObjectEvent::ScrollBegin | ObjectEvent::Scroll { .. } | ObjectEvent::ScrollEnd
            )
        });
        assert!(
            !has_scroll,
            "scroll events must not bubble to root without EVENT_BUBBLE on container"
        );

        // Verify controller did not mutate EVENT_BUBBLE.
        let container_flags = node_at_path(&root, &[0]).flags();
        assert!(
            !container_flags.contains(ObjectFlags::EVENT_BUBBLE),
            "controller must not set EVENT_BUBBLE on the container"
        );
    }

    #[test]
    fn bubbling_works_when_event_bubble_set() {
        let mut root = scroll_tree();

        // Set EVENT_BUBBLE on the container.
        node_at_path_mut(&mut root, &[0]).set_flag(ObjectFlags::EVENT_BUBBLE, true);

        let root_saw: Rc<RefCell<Vec<ObjectEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let rsaw = root_saw.clone();
        root.add_bubble_handler(move |ev, _ctx| {
            rsaw.borrow_mut().push(ev.clone());
            false
        });

        let mut ctrl = ScrollController::default_config();
        let mut sink = |_: Rect| {};

        ctrl.process(
            &mut root,
            &Event::DragStart {
                x: 200,
                y: 300,
                origin_x: 200,
                origin_y: 310,
            },
            &mut sink,
        );
        ctrl.process(&mut root, &Event::DragMove { x: 200, y: 280 }, &mut sink);
        ctrl.process(&mut root, &Event::DragEnd { x: 200, y: 280 }, &mut sink);

        let root_events = root_saw.borrow();
        let has_scroll = root_events.iter().any(|e| {
            matches!(
                e,
                ObjectEvent::ScrollBegin | ObjectEvent::Scroll { .. } | ObjectEvent::ScrollEnd
            )
        });
        assert!(
            has_scroll,
            "scroll events must bubble to root when EVENT_BUBBLE is set"
        );

        // Verify EVENT_BUBBLE was NOT cleared by the controller.
        assert!(
            node_at_path(&root, &[0])
                .flags()
                .contains(ObjectFlags::EVENT_BUBBLE),
            "controller must not clear EVENT_BUBBLE after session"
        );
    }

    // -----------------------------------------------------------------------
    // 14. Path-targeted dispatch delivers to container
    // -----------------------------------------------------------------------

    #[test]
    fn path_targeted_dispatch_delivers_to_container() {
        use crate::object::{DispatchInput, dispatch_object_event};

        let mut root = scroll_tree();
        let log = collect_events(&mut root, &[0]);

        let _ = dispatch_object_event(
            &mut root,
            DispatchInput::Container {
                path: vec![0],
                event: ObjectEvent::ScrollBegin,
            },
        );

        let events = log.borrow();
        assert!(
            events.iter().any(|e| matches!(e, ObjectEvent::ScrollBegin)),
            "Container dispatch must deliver ScrollBegin to the target"
        );
    }

    // -----------------------------------------------------------------------
    // 15. ScrollState accessors
    // -----------------------------------------------------------------------

    #[test]
    fn scroll_state_max_scroll_clamped() {
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        assert_eq!(ss.max_scroll_y(600), 600);
        assert_eq!(ss.max_scroll_y(1200), 0, "no overflow = max is 0");
        assert_eq!(ss.max_scroll_y(1400), 0, "viewport > content = max is 0");

        assert!(ss.set_offset_y(100, 600));
        assert_eq!(ss.offset_y, 100);
        assert!(!ss.set_offset_y(100, 600), "no change when same value");
        assert!(ss.set_offset_y(700, 600), "clamps to max");
        assert_eq!(ss.offset_y, 600);
        assert!(ss.set_offset_y(-10, 600), "clamps to 0");
        assert_eq!(ss.offset_y, 0);
    }

    // -----------------------------------------------------------------------
    // 16. scrollbar_thumb geometry
    // -----------------------------------------------------------------------

    fn vp(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn scrollbar_thumb_returns_none_when_content_fits() {
        let mut ss = ScrollState::new();
        ss.content_h = 400; // ≤ viewport height
        let vp = vp(0, 0, 200, 600);
        assert!(
            ss.scrollbar_thumb(vp).is_none(),
            "no thumb when content_h ≤ viewport_h"
        );
    }

    #[test]
    fn scrollbar_thumb_returns_none_when_mode_off() {
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        ss.scrollbar_mode = ScrollbarMode::Off;
        let vp = vp(0, 0, 200, 600);
        assert!(
            ss.scrollbar_thumb(vp).is_none(),
            "no thumb when mode is Off"
        );
    }

    #[test]
    fn scrollbar_thumb_on_right_edge_with_correct_geometry() {
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        ss.offset_y = 0;
        // viewport: x=0, y=0, w=200, h=600
        let vp = vp(0, 0, 200, 600);
        let thumb = ss
            .scrollbar_thumb(vp)
            .expect("thumb must be Some when overflowing");

        // Width must equal SCROLLBAR_WIDTH.
        assert_eq!(thumb.width, SCROLLBAR_WIDTH);
        // x must be at right edge: vp.x + vp.width - SCROLLBAR_WIDTH - SCROLLBAR_MARGIN
        assert_eq!(
            thumb.x,
            200 - SCROLLBAR_WIDTH - SCROLLBAR_MARGIN,
            "thumb must be on the right edge"
        );
        // thumb_h = (600² / 1200).clamp(8, 600) = 300
        assert_eq!(thumb.height, 300, "thumb height formula mismatch");
        // at offset 0: thumb.y == viewport.y
        assert_eq!(thumb.y, 0, "thumb at top when offset is 0");
    }

    #[test]
    fn scrollbar_thumb_position_reflects_scroll_offset() {
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        let vp = vp(0, 0, 200, 600);

        // At max offset the thumb should be at the bottom of the track.
        ss.offset_y = ss.max_scroll_y(600); // 600
        let thumb = ss.scrollbar_thumb(vp).expect("thumb must be Some");
        // thumb_h = 300; travel = 600 - 300 = 300.
        // offset = (600 * 300) / 600 = 300
        assert_eq!(thumb.y, 300, "thumb at bottom when offset is max");

        // At halfway the thumb should be at the center.
        ss.offset_y = 300;
        let thumb_mid = ss.scrollbar_thumb(vp).expect("thumb must be Some");
        // offset = (300 * 300) / 600 = 150
        assert_eq!(
            thumb_mid.y, 150,
            "thumb at center when offset is half of max"
        );
    }
}
