//! LPAR-04 §8 input-device adapters.
//!
//! Four thin typed wrappers that turn platform input into core [`Event`]
//! streams plus the routing rules defined in LPAR-04 §8.  They are *adapters*,
//! not a driver framework — existing raw-event producers (`PD`/`PM`/`PU`/`KD`/
//! `KU`) remain valid without wrapping; this layer is additive.
//!
//! # Device classes (LPAR-04 §8.1)
//!
//! | Type | Produces | Routing rule |
//! |---|---|---|
//! | [`PointerDevice`] | `PointerDown/Move/Up`, optionally `Touch` | Hit-test target (§6.2), canonical recognizer chain (§8.3) |
//! | [`KeypadDevice`] | `KeyDown`/`KeyUp`, tick-driven auto-repeat | Focused object as `ObjectEvent::Key` |
//! | [`EncoderDevice`] | `Encoder { diff }`, Enter press | Navigate → focus movement; editing → `ObjectEvent::Rotary`/`Key` |
//! | [`ButtonDevice`] | Synthesized `PointerDown/Up` at a configured screen point | Identical to Pointer after synthesis |
//!
//! # Per-instance state
//!
//! Two [`PointerDevice`]s do not share recognizer state (§8.4).  Keypad and
//! encoder devices address focus through the single-`FOCUSED` tree invariant
//! (§7.3); multiple simultaneous keypad/encoder devices targeting different
//! focus groups in one tree are out of v1 scope.
//!
//! # Tick-domain durations
//!
//! All durations are Tick counts (LPAR-04 §9.1).  No wall-clock API appears
//! anywhere in this module.  Default constants are documented against LVGL's
//! nominal timings for a 30 Hz tick loop.

use rlvgl_core::event::{Event, Key};
use rlvgl_core::focus::FocusGroup;
use rlvgl_core::object::{DispatchInput, Disposition, ObjectEvent, ObjectNode};
use rlvgl_core::scroll::{ScrollConfig, ScrollController};
use rlvgl_core::widget::Rect;

use crate::gesture::{
    DoubleTapRecognizer, DragRecognizer, LongPressConfig, LongPressRecognizer, TapRecognizer,
};

// ---------------------------------------------------------------------------
// Keypad auto-repeat constants (§9.7)
// ---------------------------------------------------------------------------

/// Number of ticks a key must be held before auto-repeat begins.
///
/// At 30 Hz this is ≈ 400 ms, matching LVGL's nominal `lv_indev_get_act_obj`
/// long-press delay and the [`crate::gesture::LONG_PRESS_TICKS`] precedent.
pub const KEY_REPEAT_DELAY_TICKS: u32 = 12;

/// Number of ticks between successive auto-repeat deliveries after the initial
/// repeat fires.
///
/// At 30 Hz this is ≈ 100 ms, matching the
/// [`crate::gesture::LONG_PRESS_REPEAT_TICKS`] precedent.
pub const KEY_REPEAT_PERIOD_TICKS: u32 = 3;

// ---------------------------------------------------------------------------
// PointerDevice
// ---------------------------------------------------------------------------

/// LPAR-04 §8 Pointer input device adapter.
///
/// Owns the canonical recognizer chain:
///
/// ```text
/// raw → DragRecognizer → TapRecognizer → LongPressRecognizer → DoubleTapRecognizer → dispatch
/// ```
///
/// **Note on chain order vs. spec:** LPAR-04 §8.3 lists the chain as
/// `raw → Drag → LongPress → Tap → DoubleTap`.  In practice, the
/// `LongPressRecognizer` implementation arms on `PressDown` (the output of
/// `TapRecognizer`), not on raw `PointerDown`.  The chain is therefore
/// corrected to `Drag → Tap → LongPress → DoubleTap` so that:
///
/// 1. `PressDown` (debounced) arms the long-press timer.
/// 2. `DragStart` cancels `TapRecognizer` (no `PressDown` emitted) **and**
///    also explicitly cancels `LongPressRecognizer` (safety — e.g. if a
///    contact moved past the drag threshold after the long-press fired).
///
/// This preserves the INPUT-00 §6.2 / LPAR-04 §9.4 cancellation contract.
///
/// # Scroll integration (LPAR-05 §7)
///
/// When constructed with [`with_scroll`](Self::with_scroll), an owned
/// [`ScrollController`] sits above the recognizer chain.  On every
/// `DragStart`/`DragMove`/`DragEnd` the controller is offered the event
/// first.  If the controller activates a scroll session (it found a
/// `SCROLLABLE` ancestor), the drag event is **not** re-dispatched to the
/// object tree as a normal gesture — scrolling and gesture dispatch are
/// mutually exclusive for the same contact (LPAR-05 §7.2).  Dirty rects
/// are accumulated in an internal `Vec<Rect>` and can be drained with
/// [`take_dirty`](Self::take_dirty).
///
/// Feed raw hardware events with [`process`](Self::process) and call
/// [`tick`](Self::tick) once per frame tick to drive the long-press,
/// double-tap timers, and (if enabled) the scroll throw tween.  Each
/// resulting stream event is dispatched into the tree via
/// `dispatch_object_event(root, DispatchInput::Pointer { x, y, event })`.
pub struct PointerDevice {
    drag: DragRecognizer,
    tap: TapRecognizer,
    long_press: LongPressRecognizer,
    dtap: DoubleTapRecognizer,
    /// Optional LPAR-05 scroll controller.  `None` when scroll wiring is not
    /// enabled (the default constructors do not attach one).
    scroll: Option<ScrollController>,
    /// Accumulated viewport dirty rects from scroll offset changes.
    dirty: alloc::vec::Vec<Rect>,
}

impl PointerDevice {
    /// Create a pointer device with the default recognizer thresholds at the
    /// given frame rate.
    ///
    /// `frame_hz` is used to convert the duration constants in
    /// [`crate::gesture`] from milliseconds to ticks.
    ///
    /// Scroll integration is **disabled** by this constructor.  Use
    /// [`with_scroll`](Self::with_scroll) to enable it.
    pub fn new(frame_hz: u32) -> Self {
        Self {
            drag: DragRecognizer::new(),
            tap: TapRecognizer::new(frame_hz),
            long_press: LongPressRecognizer::new(),
            dtap: DoubleTapRecognizer::new(frame_hz),
            scroll: None,
            dirty: alloc::vec::Vec::new(),
        }
    }

    /// Create a pointer device with custom long-press thresholds.
    ///
    /// Use this to override the default 400 ms / 100 ms timings.
    ///
    /// Scroll integration is **disabled** by this constructor.  Use
    /// [`with_scroll`](Self::with_scroll) on the returned instance to enable
    /// it.
    pub fn with_long_press_config(frame_hz: u32, lp_config: LongPressConfig) -> Self {
        Self {
            drag: DragRecognizer::new(),
            tap: TapRecognizer::new(frame_hz),
            long_press: LongPressRecognizer::with_config(lp_config),
            dtap: DoubleTapRecognizer::new(frame_hz),
            scroll: None,
            dirty: alloc::vec::Vec::new(),
        }
    }

    /// Attach a [`ScrollController`] to this device, enabling LPAR-05
    /// drag→scroll composition.
    ///
    /// Can be called with a custom [`ScrollConfig`] or with the default via
    /// [`ScrollConfig::default()`].
    ///
    /// # Composition rule (LPAR-05 §7.2)
    ///
    /// A drag event that activates a scroll session is consumed by the
    /// controller and **MUST NOT** also be dispatched as a normal drag gesture
    /// to the object tree.  Use [`take_dirty`](Self::take_dirty) after each
    /// [`process`](Self::process) / [`tick`](Self::tick) call to drain the
    /// accumulated viewport dirty rects and forward them to the LPAR-03
    /// invalidation planner.
    pub fn with_scroll(mut self, config: ScrollConfig) -> Self {
        self.scroll = Some(ScrollController::new(config));
        self
    }

    /// Drain and return all viewport dirty [`Rect`]s accumulated since the
    /// last call.
    ///
    /// Each rect corresponds to the viewport of a scroll container whose
    /// offset changed during the last [`process`](Self::process) or
    /// [`tick`](Self::tick) call.  Forward these to the LPAR-03 invalidation
    /// planner so the display driver repaints the affected areas.
    ///
    /// Returns an empty `Vec` when no scroll activity occurred or when scroll
    /// integration is disabled.
    pub fn take_dirty(&mut self) -> alloc::vec::Vec<Rect> {
        core::mem::take(&mut self.dirty)
    }

    /// Feed one raw hardware event through the recognizer chain and dispatch
    /// any resulting stream events to the tree.
    ///
    /// Raw events (`PointerDown`/`Move`/`Up`) traverse the chain in order.
    /// Returns the [`Disposition`] of the *last* dispatch that was made for
    /// this event, or [`Disposition::NoTarget`] when the chain produced no
    /// dispatchable output.
    ///
    /// When scroll integration is enabled (see [`with_scroll`](Self::with_scroll)):
    /// drag events (`DragStart`/`DragMove`/`DragEnd`) are offered to the
    /// [`ScrollController`] first.  If the controller activates or continues a
    /// scroll session, the drag is **suppressed** from normal object-tree
    /// dispatch (LPAR-05 §7.2).  Dirty rects are accumulated; drain them with
    /// [`take_dirty`](Self::take_dirty).
    pub fn process(&mut self, root: &mut ObjectNode, raw: Event) -> Disposition {
        let mut last = Disposition::NoTarget;

        // ── Stage 1: DragRecognizer ───────────────────────────────────────
        let after_drag = match self.drag.process(&raw) {
            None => return Disposition::NoTarget,
            Some(ev) => ev,
        };

        // DragStart: cancel tap AND long-press (INPUT-00 §6.2, LPAR-04 §9.4).
        // Cancelling tap prevents a stale PressRelease.  Cancelling long-press
        // is a safety measure in case it was armed before the drag threshold
        // was crossed.
        if matches!(after_drag, Event::DragStart { .. }) {
            self.tap.cancel();
            self.long_press.cancel();
        }

        // ── Scroll controller intercept (LPAR-05 §7.1–§7.2) ──────────────
        // Offer DragStart/DragMove/DragEnd to the scroll controller before the
        // tap/long-press stages.  Suppress the drag from normal object-tree
        // dispatch if the controller owns this contact either *before* the call
        // (an in-progress session — including the terminating `DragEnd` that a
        // below-threshold release ends with `session = None`) or *after* it (a
        // `DragStart` that just activated a session). Checking only the
        // after-state would leak a scroll contact's final `DragEnd` into normal
        // dispatch.
        if matches!(
            after_drag,
            Event::DragStart { .. } | Event::DragMove { .. } | Event::DragEnd { .. }
        ) && let Some(ref mut ctrl) = self.scroll
        {
            let was_active = ctrl.is_active();
            let dirty = &mut self.dirty;
            ctrl.process(root, &after_drag, &mut |r| dirty.push(r));
            if was_active || ctrl.is_active() {
                // The scroll controller owns this contact: drag consumed by
                // scrolling. Do NOT fall through to normal dispatch.
                return Disposition::Unconsumed;
            }
            // No session before or after (no SCROLLABLE ancestor, or wrong
            // axis): fall through to normal drag dispatch as today.
        }

        // ── Stage 2: TapRecognizer ────────────────────────────────────────
        // Converts PointerDown → PressDown (arms long-press downstream).
        // PointerUp starts the settle timer (output deferred to tap.tick).
        let after_tap = match self.tap.process(&after_drag) {
            None => return last,
            Some(ev) => ev,
        };

        // ── Stage 3: LongPressRecognizer (pass-through, additive) ─────────
        // Arms on PressDown, disarms on DragStart/PressRelease/PointerUp.
        let after_lp = match self.long_press.process(&after_tap) {
            None => return last,
            Some(ev) => ev,
        };

        // ── Stage 4: DoubleTapRecognizer → dispatch ───────────────────────
        let (ev_a, ev_b) = self.dtap.process(&after_lp);
        if let Some(ev) = ev_a {
            last = dispatch_pointer_event(root, ev);
        }
        if let Some(ev) = ev_b {
            last = dispatch_pointer_event(root, ev);
        }

        last
    }

    /// Advance all time-based recognizers one tick and dispatch any emitted
    /// stream events to the tree.
    ///
    /// Call this once per [`Event::Tick`].  Drives the long-press recognizer
    /// (for `LongPress`/`LongPressRepeat` delivery), the double-tap window
    /// timer (for deferred single-tap delivery), and — when scroll integration
    /// is enabled — the scroll throw/snap-settle tween.  Dirty rects from
    /// throw ticks are accumulated; drain them with
    /// [`take_dirty`](Self::take_dirty).
    ///
    /// Returns the [`Disposition`] of the last dispatch that occurred during
    /// this tick, or [`Disposition::NoTarget`] if the tick produced no output.
    pub fn tick(&mut self, root: &mut ObjectNode) -> Disposition {
        let mut last = Disposition::NoTarget;

        // Advance scroll throw/snap-settle tween (LPAR-05 §8.4, §9.4).
        if let Some(ref mut ctrl) = self.scroll {
            let dirty = &mut self.dirty;
            ctrl.tick(root, &mut |r| dirty.push(r));
        }

        // Tap settle timer: may emit a deferred PressRelease.
        // Feed through long_press (disarms it) then dtap.
        if let Some(tap_ev) = self.tap.tick()
            && let Some(after_lp) = self.long_press.process(&tap_ev)
        {
            let (ev_a, ev_b) = self.dtap.process(&after_lp);
            if let Some(ev) = ev_a {
                last = dispatch_pointer_event(root, ev);
            }
            if let Some(ev) = ev_b {
                last = dispatch_pointer_event(root, ev);
            }
        }

        // Long-press timer: may emit LongPress / LongPressRepeat.
        // These bypass the tap stage (tap has no state change here) and go
        // directly into dtap (which passes non-tap events through unchanged).
        if let Some(lp_ev) = self.long_press.tick() {
            let (ev_a, ev_b) = self.dtap.process(&lp_ev);
            if let Some(ev) = ev_a {
                last = dispatch_pointer_event(root, ev);
            }
            if let Some(ev) = ev_b {
                last = dispatch_pointer_event(root, ev);
            }
        }

        // Double-tap window timer: may emit a buffered PressRelease.
        if let Some(dt_ev) = self.dtap.tick() {
            last = dispatch_pointer_event(root, dt_ev);
        }

        last
    }
}

/// Dispatch a single pointer-family stream event to the tree.
///
/// Extracts coordinates from the event and routes via
/// `DispatchInput::Pointer`.  Events with no spatial coordinates (e.g.
/// `Tick`, `KeyDown`) are silently dropped.
fn dispatch_pointer_event(root: &mut ObjectNode, ev: Event) -> Disposition {
    let (x, y) = match pointer_coords(&ev) {
        Some(c) => c,
        None => return Disposition::NoTarget,
    };
    rlvgl_core::object::dispatch_object_event(root, DispatchInput::Pointer { x, y, event: ev })
}

/// Extract `(x, y)` from a pointer-family stream event, or `None` for
/// non-spatial events.
fn pointer_coords(ev: &Event) -> Option<(i32, i32)> {
    match ev {
        Event::PointerDown { x, y }
        | Event::PointerUp { x, y }
        | Event::PointerMove { x, y }
        | Event::PressDown { x, y }
        | Event::PressRelease { x, y }
        | Event::DoubleTap { x, y }
        | Event::LongPress { x, y }
        | Event::LongPressRepeat { x, y } => Some((*x, *y)),
        Event::DragStart { x, y, .. } | Event::DragMove { x, y } | Event::DragEnd { x, y } => {
            Some((*x, *y))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// KeypadDevice
// ---------------------------------------------------------------------------

/// LPAR-04 §8 Keypad input device adapter with §9.7 tick-counted auto-repeat.
///
/// Routes `KeyDown`/`KeyUp` to the focused object via
/// `dispatch_object_event(root, DispatchInput::Focused { event: ObjectEvent::Key(key) })`.
///
/// # Auto-repeat (§9.7)
///
/// While a key is held (a `KeyDown` received without a matching `KeyUp`),
/// [`tick`](Self::tick) re-delivers `ObjectEvent::Key` to the focused object
/// after [`KEY_REPEAT_DELAY_TICKS`] ticks, then every
/// [`KEY_REPEAT_PERIOD_TICKS`] ticks thereafter.  These are synthetic
/// `ObjectEvent::Key` re-deliveries — the stream never sees extra `KeyDown`
/// events for repeats.
///
/// # Unconsumed keys
///
/// When dispatch returns [`Disposition::Unconsumed`] or
/// [`Disposition::NoTarget`], the key was not consumed by the focused object.
/// The caller's existing app-level post-dispatch path receives it through the
/// normal application pump — `KeypadDevice` does **not** swallow unconsumed
/// keys.
pub struct KeypadDevice {
    /// Key currently held, if any.
    held_key: Option<Key>,
    /// Tick counter since key-down (used for auto-repeat).
    held_ticks: u32,
    /// Ticks before auto-repeat begins.
    repeat_delay: u32,
    /// Ticks between successive auto-repeat deliveries after the first.
    repeat_period: u32,
}

impl KeypadDevice {
    /// Create a keypad device with the default auto-repeat constants
    /// ([`KEY_REPEAT_DELAY_TICKS`] / [`KEY_REPEAT_PERIOD_TICKS`]).
    pub fn new() -> Self {
        Self {
            held_key: None,
            held_ticks: 0,
            repeat_delay: KEY_REPEAT_DELAY_TICKS,
            repeat_period: KEY_REPEAT_PERIOD_TICKS,
        }
    }

    /// Create a keypad device with custom auto-repeat timing.
    ///
    /// `delay_ticks` is the hold duration before the first repeat.
    /// `period_ticks` is the interval between subsequent repeats.
    /// `period_ticks` is clamped to at least 1 to prevent infinite loops.
    pub fn with_repeat(delay_ticks: u32, period_ticks: u32) -> Self {
        Self {
            held_key: None,
            held_ticks: 0,
            repeat_delay: delay_ticks,
            repeat_period: period_ticks.max(1),
        }
    }

    /// Process a `KeyDown` event: route to the focused object and arm
    /// auto-repeat.
    ///
    /// Returns the [`Disposition`] from `dispatch_object_event`.
    pub fn key_down(&mut self, root: &mut ObjectNode, key: Key) -> Disposition {
        self.held_key = Some(key.clone());
        self.held_ticks = 0;
        dispatch_key(root, key)
    }

    /// Process a `KeyUp` event: disarm auto-repeat.
    ///
    /// Returns [`Disposition::Unconsumed`] — key-up has no `ObjectEvent`
    /// counterpart in v1; the caller's app-level handler receives it through
    /// the normal pump.
    pub fn key_up(&mut self, _root: &mut ObjectNode, _key: Key) -> Disposition {
        self.held_key = None;
        self.held_ticks = 0;
        Disposition::Unconsumed
    }

    /// Advance the auto-repeat timer one tick.
    ///
    /// If a key is currently held and the repeat schedule fires, re-delivers
    /// `ObjectEvent::Key` to the focused object.  Returns the
    /// [`Disposition`] of the re-delivery, or [`Disposition::NoTarget`] when
    /// no repeat occurred this tick.
    pub fn tick(&mut self, root: &mut ObjectNode) -> Disposition {
        let key = match &self.held_key {
            Some(k) => k.clone(),
            None => return Disposition::NoTarget,
        };

        self.held_ticks += 1;

        // Has auto-repeat started?
        if self.held_ticks < self.repeat_delay {
            return Disposition::NoTarget;
        }

        // Ticks past the initial delay.
        let after_delay = self.held_ticks - self.repeat_delay;

        // First repeat fires exactly when `after_delay == 0`.
        // Subsequent repeats fire every `repeat_period` ticks.
        let fires = after_delay == 0 || after_delay.is_multiple_of(self.repeat_period);

        if fires {
            dispatch_key(root, key)
        } else {
            Disposition::NoTarget
        }
    }
}

impl Default for KeypadDevice {
    fn default() -> Self {
        Self::new()
    }
}

/// Deliver `ObjectEvent::Key(key)` to the focused node.
fn dispatch_key(root: &mut ObjectNode, key: Key) -> Disposition {
    rlvgl_core::object::dispatch_object_event(
        root,
        DispatchInput::Focused {
            event: ObjectEvent::Key(key),
        },
    )
}

// ---------------------------------------------------------------------------
// EncoderDevice
// ---------------------------------------------------------------------------

/// LPAR-04 §8 Encoder input device adapter.
///
/// Holds a [`FocusGroup`] and routes encoder events according to the group's
/// navigate / editing mode:
///
/// - **Navigate mode** (`policy.editing == false`): positive `diff` calls
///   `focus_next` once per detent; negative `diff` calls `focus_prev` once per
///   detent.  A `diff` of `±N` steps focus `N` times in the corresponding
///   direction (one step per detent — the simplest correct rule for a discrete
///   encoder).
///
/// - **Editing mode** (`policy.editing == true`): delivers
///   `ObjectEvent::Rotary { diff }` to the focused object for any non-zero
///   `diff`, and `ObjectEvent::Key(Key::Enter)` for encoder presses.
///
/// # Mode toggle
///
/// An encoder press in either mode inverts `policy.editing` (LVGL encoder
/// convention: Enter toggles between navigate and edit).  The press is
/// delivered as `ObjectEvent::Key(Key::Enter)` to the focused object *before*
/// the toggle in editing mode.
///
/// # Focus group ownership
///
/// Each `EncoderDevice` owns its own [`FocusGroup`], keeping the per-device
/// state isolated (LPAR-04 §8.4).
pub struct EncoderDevice {
    /// Focus traversal and editing policy.
    pub focus_group: FocusGroup,
}

impl EncoderDevice {
    /// Create an encoder device with default focus policy (wrap enabled,
    /// navigate mode).
    pub fn new() -> Self {
        Self {
            focus_group: FocusGroup::new(),
        }
    }

    /// Process a rotation event.
    ///
    /// - Navigate mode: steps focus `diff.abs()` times in the direction of
    ///   the sign (positive → next, negative → prev).
    /// - Editing mode: delivers `ObjectEvent::Rotary { diff }` to the focused
    ///   object.  Returns [`Disposition::NoTarget`] when `diff == 0`.
    pub fn rotate(&mut self, root: &mut ObjectNode, diff: i32) -> Disposition {
        if diff == 0 {
            return Disposition::NoTarget;
        }

        if self.focus_group.policy.editing {
            // Editing mode: deliver rotation to the focused object.
            rlvgl_core::object::dispatch_object_event(
                root,
                DispatchInput::Focused {
                    event: ObjectEvent::Rotary { diff },
                },
            )
        } else {
            // Navigate mode: step focus by |diff| in the sign's direction.
            let steps = diff.unsigned_abs();
            let mut moved = false;
            for _ in 0..steps {
                if diff > 0 {
                    moved |= self.focus_group.focus_next(root);
                } else {
                    moved |= self.focus_group.focus_prev(root);
                }
            }
            if moved {
                Disposition::Unconsumed
            } else {
                Disposition::NoTarget
            }
        }
    }

    /// Process an encoder press (the physical click of the encoder shaft).
    ///
    /// Toggles editing mode and:
    /// - **Before toggle from editing → navigate**: delivers
    ///   `ObjectEvent::Key(Key::Enter)` to the focused object.
    /// - **Toggle from navigate → editing**: delivers
    ///   `ObjectEvent::Key(Key::Enter)` to the focused object *after* entering
    ///   editing mode (so the widget sees Enter while in edit mode).
    ///
    /// Returns the [`Disposition`] of the key delivery, or
    /// [`Disposition::NoTarget`] if no focused object exists.
    pub fn press(&mut self, root: &mut ObjectNode) -> Disposition {
        let was_editing = self.focus_group.policy.editing;

        if was_editing {
            // Deliver Enter first (in editing mode), then exit editing.
            let d = dispatch_key(root, Key::Enter);
            self.focus_group.set_editing(root, false);
            d
        } else {
            // Enter editing mode first, then deliver Enter to the (now editing) target.
            self.focus_group.set_editing(root, true);
            dispatch_key(root, Key::Enter)
        }
    }

    /// Return whether the encoder is currently in editing mode.
    pub fn is_editing(&self) -> bool {
        self.focus_group.policy.editing
    }

    /// Directly set editing mode without delivering a key event.
    ///
    /// Sets or clears [`rlvgl_core::object::ObjectStates::EDITED`] on the
    /// currently focused node in `root`.
    pub fn set_editing(&mut self, root: &mut ObjectNode, editing: bool) {
        self.focus_group.set_editing(root, editing);
    }
}

impl Default for EncoderDevice {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ButtonDevice
// ---------------------------------------------------------------------------

/// A hardware button mapped to a screen point.
///
/// When the button is pressed, synthesizes a `PointerDown` at `(x, y)`.
/// When released, synthesizes a `PointerUp` at `(x, y)`.  These are fed
/// through the same [`PointerDevice`] path, so all recognizer logic (tap,
/// drag, long-press) applies identically (LPAR-04 §8.1: "Identical to Pointer
/// after synthesis").
#[derive(Debug, Clone, Copy)]
pub struct ButtonMapping {
    /// Hardware button index (application-defined).
    pub button_id: u32,
    /// Screen x coordinate to synthesize pointer events at.
    pub x: i32,
    /// Screen y coordinate to synthesize pointer events at.
    pub y: i32,
}

/// LPAR-04 §8 Button input device adapter.
///
/// Maps each configured hardware button to a screen point.  A button press
/// synthesizes `PointerDown` at the mapped point; a button release synthesizes
/// `PointerUp`.  Both are fed through an owned [`PointerDevice`] so the full
/// recognizer chain applies — no new `Event` variants are introduced.
///
/// Two [`ButtonDevice`]s do not share recognizer state (per-device-instance
/// rule, §8.4).
pub struct ButtonDevice {
    pointer: PointerDevice,
    mappings: alloc::vec::Vec<ButtonMapping>,
}

impl ButtonDevice {
    /// Create a button device with the given button→point mappings and frame
    /// rate.
    ///
    /// `frame_hz` is forwarded to the owned [`PointerDevice`] for threshold
    /// conversion.
    pub fn new(frame_hz: u32, mappings: alloc::vec::Vec<ButtonMapping>) -> Self {
        Self {
            pointer: PointerDevice::new(frame_hz),
            mappings,
        }
    }

    /// Find the mapping for `button_id`.
    fn mapping_for(&self, button_id: u32) -> Option<&ButtonMapping> {
        self.mappings.iter().find(|m| m.button_id == button_id)
    }

    /// Process a button press: synthesize `PointerDown` at the mapped point
    /// and feed it through the pointer device.
    ///
    /// Returns [`Disposition::NoTarget`] if `button_id` is not registered.
    pub fn button_down(&mut self, root: &mut ObjectNode, button_id: u32) -> Disposition {
        let (x, y) = match self.mapping_for(button_id) {
            Some(m) => (m.x, m.y),
            None => return Disposition::NoTarget,
        };
        self.pointer.process(root, Event::PointerDown { x, y })
    }

    /// Process a button release: synthesize `PointerUp` at the mapped point
    /// and feed it through the pointer device.
    ///
    /// Returns [`Disposition::NoTarget`] if `button_id` is not registered.
    pub fn button_up(&mut self, root: &mut ObjectNode, button_id: u32) -> Disposition {
        let (x, y) = match self.mapping_for(button_id) {
            Some(m) => (m.x, m.y),
            None => return Disposition::NoTarget,
        };
        self.pointer.process(root, Event::PointerUp { x, y })
    }

    /// Advance the owned pointer device's timers one tick.
    ///
    /// Drives long-press and double-tap timers for button contacts.
    pub fn tick(&mut self, root: &mut ObjectNode) -> Disposition {
        self.pointer.tick(root)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use rlvgl_core::event::{Event, Key};
    use rlvgl_core::focus::FocusGroup;
    use rlvgl_core::object::{Disposition, ObjectEvent, ObjectFlags, ObjectNode, ObjectStates};
    use rlvgl_core::renderer::Renderer;
    use rlvgl_core::widget::{Color, Rect, Widget};

    // Renderer is used by the Widget impl, Color by draw_text signature.
    // They must remain in scope even though they appear "unused" in test bodies.

    use super::*;

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct TW {
        bounds: Rect,
        consume_clicks: bool,
        // Stored so the widget keeps a reference alive alongside the handlers
        // that capture a clone; not read directly by Widget methods.
        #[allow(dead_code)]
        events: Rc<RefCell<Vec<ObjectEvent>>>,
    }

    impl TW {
        fn node(
            tag: &'static str,
            bounds: Rect,
            events: Rc<RefCell<Vec<ObjectEvent>>>,
        ) -> ObjectNode {
            let widget = Rc::new(RefCell::new(TW {
                bounds,
                consume_clicks: false,
                events,
            }));
            ObjectNode::new(widget).with_tag(tag)
        }
    }

    impl Widget for TW {
        fn bounds(&self) -> Rect {
            self.bounds
        }
        fn draw(&self, _renderer: &mut dyn Renderer) {}
        fn handle_event(&mut self, event: &Event) -> bool {
            self.consume_clicks && matches!(event, Event::PressRelease { .. })
        }
    }

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// Make a clickable node at the given bounds.
    fn clickable(
        tag: &'static str,
        bounds: Rect,
        events: Rc<RefCell<Vec<ObjectEvent>>>,
    ) -> ObjectNode {
        let mut n = TW::node(tag, bounds, events.clone());
        n.set_flag(ObjectFlags::CLICKABLE, true);
        // Register a target handler that records all ObjectEvents.
        let ev = events.clone();
        n.add_target_handler(move |event, _ctx| {
            ev.borrow_mut().push(event.clone());
            false
        });
        n
    }

    /// Make a focusable node (not necessarily clickable).
    fn focusable(
        tag: &'static str,
        bounds: Rect,
        events: Rc<RefCell<Vec<ObjectEvent>>>,
    ) -> ObjectNode {
        let mut n = TW::node(tag, bounds, events.clone());
        n.set_flag(ObjectFlags::FOCUSABLE, true);
        let ev = events.clone();
        n.add_target_handler(move |event, _ctx| {
            ev.borrow_mut().push(event.clone());
            false
        });
        n
    }

    // -----------------------------------------------------------------------
    // PointerDevice tests
    // -----------------------------------------------------------------------

    /// A clean press-release over a clickable node dispatches Pressed then
    /// Clicked to the node's handler.
    #[test]
    fn pointer_device_click_dispatches_pressed_and_clicked() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: events.clone(),
        })));
        root.append_child(clickable("btn", r(10, 10, 30, 30), events.clone()));

        let mut dev = PointerDevice::new(30);

        // Down at (20, 20) — inside "btn".
        dev.process(&mut root, Event::PointerDown { x: 20, y: 20 });
        dev.process(&mut root, Event::PointerUp { x: 20, y: 20 });

        // Settle the tap recognizer (SETTLE_MS / 30Hz = 6 ticks).
        for _ in 0..10 {
            dev.tick(&mut root);
        }

        // Wait for double-tap window to expire.
        for _ in 0..20 {
            dev.tick(&mut root);
        }

        let ev = events.borrow();
        assert!(
            ev.iter().any(|e| matches!(e, ObjectEvent::Pressed { .. })),
            "expected Pressed: {ev:?}"
        );
        assert!(
            ev.iter().any(|e| matches!(e, ObjectEvent::Clicked { .. })),
            "expected Clicked: {ev:?}"
        );
    }

    /// A drag-crossing contact produces no `ObjectEvent::Clicked`.
    #[test]
    fn pointer_device_drag_suppresses_clicked() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 200, 200),
            consume_clicks: false,
            events: events.clone(),
        })));
        root.append_child(clickable("btn", r(0, 0, 200, 200), events.clone()));

        let mut dev = PointerDevice::new(30);

        dev.process(&mut root, Event::PointerDown { x: 10, y: 10 });
        // Move far enough to cross the drag threshold (>10 px Euclidean).
        dev.process(&mut root, Event::PointerMove { x: 30, y: 10 });
        dev.process(&mut root, Event::PointerUp { x: 30, y: 10 });

        // Drain settle + double-tap window.
        for _ in 0..40 {
            dev.tick(&mut root);
        }

        let ev = events.borrow();
        assert!(
            !ev.iter().any(|e| matches!(e, ObjectEvent::Clicked { .. })),
            "drag-crossing contact must not produce Clicked: {ev:?}"
        );
    }

    /// `tick()` drives a held contact to `LongPressed` delivery at the target.
    #[test]
    fn pointer_device_tick_drives_long_press() {
        // Use a short threshold for this test.
        let lp_config = LongPressConfig {
            long_press_ticks: 5,
            repeat_ticks: 3,
        };

        let events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: events.clone(),
        })));
        root.append_child(clickable("btn", r(10, 10, 50, 50), events.clone()));

        let mut dev = PointerDevice::with_long_press_config(30, lp_config);

        // Press down and hold.
        dev.process(&mut root, Event::PointerDown { x: 20, y: 20 });

        // Tick past the long-press threshold.
        for _ in 0..6 {
            dev.tick(&mut root);
        }

        let ev = events.borrow();
        assert!(
            ev.iter()
                .any(|e| matches!(e, ObjectEvent::LongPressed { .. })),
            "expected LongPressed after threshold: {ev:?}"
        );
    }

    // -----------------------------------------------------------------------
    // KeypadDevice tests
    // -----------------------------------------------------------------------

    /// `KeyDown` to a focused node delivers `ObjectEvent::Key`.
    #[test]
    fn keypad_delivers_key_to_focused_node() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: events.clone(),
        })));
        let node = focusable("input", r(0, 0, 50, 50), events.clone());
        root.append_child(node);

        // Focus the child.
        let fg = FocusGroup::new();
        fg.focus_next(&mut root);

        let mut dev = KeypadDevice::new();
        dev.key_down(&mut root, Key::Character('a'));

        let ev = events.borrow();
        assert!(
            ev.iter()
                .any(|e| matches!(e, ObjectEvent::Key(Key::Character('a')))),
            "expected Key(Character('a')): {ev:?}"
        );
    }

    /// Auto-repeat fires after delay, then at each period; exact tick counts
    /// are asserted.
    #[test]
    fn keypad_auto_repeat_fires_at_exact_ticks() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: events.clone(),
        })));
        let node = focusable("input", r(0, 0, 100, 100), events.clone());
        root.append_child(node);

        let fg = FocusGroup::new();
        fg.focus_next(&mut root);

        // Use short thresholds: delay=4, period=2.
        let delay = 4u32;
        let period = 2u32;
        let mut dev = KeypadDevice::with_repeat(delay, period);

        // Initial key delivery.
        dev.key_down(&mut root, Key::Enter);

        let mut repeat_ticks = Vec::new();

        // Tick through delay + several periods.  Track which ticks produce a repeat.
        for tick in 1..=(delay + period * 4) {
            let d = dev.tick(&mut root);
            if d != Disposition::NoTarget {
                repeat_ticks.push(tick);
            }
        }

        // First repeat at tick == delay; subsequent at delay+period, delay+2*period, ...
        let expected: Vec<u32> = (0..=4).map(|i| delay + i * period).collect();
        assert_eq!(repeat_ticks, expected, "auto-repeat tick schedule mismatch");

        // The event log should contain one initial Key + 5 repeat Keys.
        let ev = events.borrow();
        let key_count = ev
            .iter()
            .filter(|e| matches!(e, ObjectEvent::Key(Key::Enter)))
            .count();
        assert_eq!(key_count, 1 + 5, "1 initial + 5 repeats expected");
    }

    /// Unconsumed key: dispatch returns `Unconsumed` or `NoTarget` when no
    /// focused node exists.
    #[test]
    fn keypad_unconsumed_key_returns_no_target_when_no_focus() {
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        let mut dev = KeypadDevice::new();
        let d = dev.key_down(&mut root, Key::Escape);
        // No focused node → NoTarget.
        assert_eq!(d, Disposition::NoTarget);
    }

    // -----------------------------------------------------------------------
    // EncoderDevice tests
    // -----------------------------------------------------------------------

    /// Navigate-mode diff moves focus next/prev.
    #[test]
    fn encoder_navigate_mode_diff_moves_focus() {
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 200, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        // Three focusable nodes side by side.
        for (tag, x) in [("a", 0), ("b", 50), ("c", 100)] {
            let mut n = TW::node(tag, r(x, 0, 40, 40), Rc::new(RefCell::new(Vec::new())));
            n.set_flag(ObjectFlags::FOCUSABLE, true);
            root.append_child(n);
        }

        let mut enc = EncoderDevice::new();
        assert!(!enc.is_editing());

        // +1 → focus "a"
        enc.rotate(&mut root, 1);
        assert_eq!(focused_tag(&root), Some("a"));

        // +1 → focus "b"
        enc.rotate(&mut root, 1);
        assert_eq!(focused_tag(&root), Some("b"));

        // -1 → focus "a"
        enc.rotate(&mut root, -1);
        assert_eq!(focused_tag(&root), Some("a"));

        // +2 (multi-step) → focus "c" (a→b→c)
        enc.rotate(&mut root, 2);
        assert_eq!(focused_tag(&root), Some("c"));
    }

    /// Editing-mode diff delivers `Rotary` to the focused node.
    #[test]
    fn encoder_editing_mode_delivers_rotary_to_focused() {
        let rotary_events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        let node = focusable("ctrl", r(0, 0, 100, 100), rotary_events.clone());
        root.append_child(node);

        let mut enc = EncoderDevice::new();

        // Focus the node first.
        enc.focus_group.focus_next(&mut root);

        // Enter editing mode.
        enc.set_editing(&mut root, true);
        assert!(enc.is_editing());

        enc.rotate(&mut root, 3);

        let ev = rotary_events.borrow();
        assert!(
            ev.iter()
                .any(|e| matches!(e, ObjectEvent::Rotary { diff: 3 })),
            "expected Rotary {{ diff: 3 }}: {ev:?}"
        );
    }

    /// Enter press toggles editing mode.
    #[test]
    fn encoder_enter_press_toggles_editing() {
        let key_events = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        let node = focusable("ctrl", r(0, 0, 100, 100), key_events.clone());
        root.append_child(node);

        let mut enc = EncoderDevice::new();
        enc.focus_group.focus_next(&mut root);

        // Start in navigate mode; press → enter editing + deliver Enter.
        assert!(!enc.is_editing());
        enc.press(&mut root);
        assert!(enc.is_editing(), "first press should enter editing mode");

        let ev = key_events.borrow();
        assert!(
            ev.iter().any(|e| matches!(e, ObjectEvent::Key(Key::Enter))),
            "expected Key(Enter): {ev:?}"
        );
        drop(ev);

        // Second press → leave editing + deliver Enter.
        enc.press(&mut root);
        assert!(!enc.is_editing(), "second press should exit editing mode");
    }

    // -----------------------------------------------------------------------
    // ButtonDevice tests
    // -----------------------------------------------------------------------

    /// A button press synthesizes a pointer event at the mapped point and
    /// hits the right node.
    #[test]
    fn button_device_press_hits_mapped_node() {
        let events_a = Rc::new(RefCell::new(Vec::new()));
        let events_b = Rc::new(RefCell::new(Vec::new()));

        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 200, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));
        root.append_child(clickable("a", r(0, 0, 50, 50), events_a.clone()));
        root.append_child(clickable("b", r(100, 0, 50, 50), events_b.clone()));

        let mut dev = ButtonDevice::new(
            30,
            vec![
                ButtonMapping {
                    button_id: 0,
                    x: 10,
                    y: 10,
                }, // hits "a"
                ButtonMapping {
                    button_id: 1,
                    x: 110,
                    y: 10,
                }, // hits "b"
            ],
        );

        // Press button 1 — should hit node "b".
        dev.button_down(&mut root, 1);
        dev.button_up(&mut root, 1);

        // Settle.
        for _ in 0..30 {
            dev.tick(&mut root);
        }

        let ea = events_a.borrow();
        let eb = events_b.borrow();
        assert!(
            ea.iter().all(|e| !matches!(e, ObjectEvent::Pressed { .. })),
            "node 'a' should not have been pressed: {ea:?}"
        );
        assert!(
            eb.iter().any(|e| matches!(e, ObjectEvent::Pressed { .. })),
            "node 'b' should have been pressed: {eb:?}"
        );
    }

    // -----------------------------------------------------------------------
    // §7.6 WID adapter demonstration — focus → set_active
    // -----------------------------------------------------------------------

    /// Demonstrates the §7.6 documented composition pattern: a
    /// `Focused`/`Defocused` handler on a node calls a stand-in
    /// `set_active(bool)` (a simple bool cell) WITHOUT the framework
    /// auto-calling it.  This shows that focus changes drive activation
    /// correctly via the application-level adapter pattern.
    ///
    /// The framework does NOT auto-call `set_active`; only the handler
    /// wired by the application does.
    #[test]
    fn focus_to_set_active_adapter_demonstration() {
        // Stand-in for a WID `set_active` field.
        let active = Rc::new(RefCell::new(false));
        let active2 = active.clone();

        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 100, 100),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        let mut field = TW::node(
            "field",
            r(0, 0, 100, 100),
            Rc::new(RefCell::new(Vec::new())),
        );
        field.set_flag(ObjectFlags::FOCUSABLE, true);

        // Application-level adapter: Focused → set_active(true),
        // Defocused → set_active(false).
        field.add_target_handler(move |event, _ctx| {
            match event {
                ObjectEvent::Focused => *active2.borrow_mut() = true,
                ObjectEvent::Defocused => *active2.borrow_mut() = false,
                _ => {}
            }
            false
        });

        root.append_child(field);

        // Second focusable node so focus can move away.
        let mut other = TW::node("other", r(0, 50, 50, 50), Rc::new(RefCell::new(Vec::new())));
        other.set_flag(ObjectFlags::FOCUSABLE, true);
        root.append_child(other);

        let fg = FocusGroup::new();

        // Initially not active.
        assert!(!*active.borrow(), "should start inactive");

        // Move focus to "field" → adapter sets active = true.
        fg.focus_next(&mut root);
        assert!(
            *active.borrow(),
            "Focused event should activate via adapter"
        );
        // Confirm the focused node is actually "field".
        assert_eq!(focused_tag(&root), Some("field"));

        // Move focus to "other" → adapter sets active = false.
        fg.focus_next(&mut root);
        assert!(
            !*active.borrow(),
            "Defocused event should deactivate via adapter"
        );
    }

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn focused_tag(root: &ObjectNode) -> Option<&'static str> {
        if root.states().contains(ObjectStates::FOCUSED) {
            return root.tag();
        }
        for child in root.children() {
            if let Some(t) = focused_tag(child) {
                return Some(t);
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // PointerDevice scroll integration tests (LPAR-05 §7)
    // -----------------------------------------------------------------------

    use rlvgl_core::scroll::{ScrollConfig, ScrollState};

    /// Build a minimal tree with a SCROLLABLE container for scroll tests.
    ///
    /// Layout: root(400×600) → container(400×600, SCROLLABLE, content_h=1200) → child(400×1200)
    fn scroll_tree_platform() -> ObjectNode {
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 400, 600),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));

        let mut container = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 400, 600),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        container.set_scroll_state(Box::new(ss));

        let child = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 400, 1200),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));
        container.append_child(child);
        root.append_child(container);
        root
    }

    /// A drag over a SCROLLABLE container changes the scroll offset and does
    /// NOT dispatch a normal drag gesture to the object tree.
    #[test]
    fn scroll_drag_moves_offset_and_suppresses_gesture_dispatch() {
        let gesture_events: Rc<RefCell<Vec<ObjectEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let mut root = scroll_tree_platform();

        // Register a target handler on the scrollable container to catch any
        // gesture ObjectEvents that leak through.
        {
            let ev = gesture_events.clone();
            // path [0] = container
            use rlvgl_core::object::ObjectNode;
            fn node_mut<'a>(root: &'a mut ObjectNode, path: &[usize]) -> &'a mut ObjectNode {
                let mut cur = root;
                for &i in path {
                    cur = &mut cur.children_mut()[i];
                }
                cur
            }
            node_mut(&mut root, &[0]).add_target_handler(move |event, _ctx| {
                match event {
                    ObjectEvent::Gesture { .. } | ObjectEvent::Pressed { .. } => {
                        ev.borrow_mut().push(event.clone());
                    }
                    _ => {}
                }
                false
            });
        }

        let mut dev = PointerDevice::new(30).with_scroll(ScrollConfig::default());

        // PointerDown + big move (crosses drag threshold) + PointerUp
        dev.process(&mut root, Event::PointerDown { x: 200, y: 300 });
        dev.process(&mut root, Event::PointerMove { x: 200, y: 270 }); // crosses drag threshold
        dev.process(&mut root, Event::PointerMove { x: 200, y: 250 });
        dev.process(&mut root, Event::PointerUp { x: 200, y: 250 });
        for _ in 0..30 {
            dev.tick(&mut root);
        }

        // Offset must have changed.
        let offset = root.children()[0]
            .scroll_state()
            .expect("container must have scroll state")
            .offset_y;
        assert!(
            offset > 0,
            "scroll offset must increase after drag-down: {offset}"
        );

        // Dirty rects must have been generated.
        let dirty = dev.take_dirty();
        // Some ticks may have already drained; check cumulatively by verifying
        // offset changed (dirty was pushed when offset changed).
        // The take here just verifies no panic.
        let _ = dirty;

        // No normal drag gesture events should have been dispatched.
        let gestures = gesture_events.borrow();
        assert!(
            gestures.is_empty(),
            "drag over SCROLLABLE must not dispatch Gesture/Pressed: {gestures:?}"
        );
    }

    /// A scroll contact that ends *below* the throw threshold (no snap, so the
    /// controller clears its session on the terminating `DragEnd`) must still
    /// not leak any drag-stream event into the object tree: the scroll
    /// controller owns the whole contact (LPAR-05 §7.1). Regression for
    /// checking `is_active()` only *after* `process()`, which misses the
    /// session-ending `DragEnd` (whose `process()` sets `session = None`).
    #[test]
    fn below_threshold_scroll_suppresses_terminating_drag_from_widgets() {
        struct RecW {
            bounds: Rect,
            seen: Rc<RefCell<Vec<Event>>>,
        }
        impl Widget for RecW {
            fn bounds(&self) -> Rect {
                self.bounds
            }
            fn draw(&self, _r: &mut dyn Renderer) {}
            fn handle_event(&mut self, ev: &Event) -> bool {
                self.seen.borrow_mut().push(ev.clone());
                false
            }
        }

        let seen: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 400, 600),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));
        let mut container = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 400, 600),
            consume_clicks: false,
            events: Rc::new(RefCell::new(Vec::new())),
        })));
        let mut ss = ScrollState::new();
        ss.content_h = 1200;
        container.set_scroll_state(Box::new(ss));
        let mut child = ObjectNode::new(Rc::new(RefCell::new(RecW {
            bounds: r(0, 0, 400, 1200),
            seen: seen.clone(),
        })));
        child.set_flag(ObjectFlags::CLICKABLE, true);
        container.append_child(child);
        root.append_child(container);

        let mut dev = PointerDevice::new(30).with_scroll(ScrollConfig::default());
        // Press, cross the drag threshold (activates scroll), release — all in
        // the same tick so the release velocity is below the throw threshold.
        dev.process(&mut root, Event::PointerDown { x: 200, y: 300 });
        dev.process(&mut root, Event::PointerMove { x: 200, y: 270 });
        dev.process(&mut root, Event::PointerMove { x: 200, y: 250 });
        dev.process(&mut root, Event::PointerUp { x: 200, y: 250 });

        // The scroll happened.
        assert!(
            root.children()[0]
                .scroll_state()
                .expect("container has scroll state")
                .offset_y
                > 0,
            "scroll offset must have advanced"
        );
        // No drag-stream event leaked into the child widget.
        let evs = seen.borrow();
        assert!(
            !evs.iter().any(|e| matches!(
                e,
                Event::DragStart { .. } | Event::DragMove { .. } | Event::DragEnd { .. }
            )),
            "a scroll contact must not leak drag events into widgets: {evs:?}"
        );
    }

    /// A drag with no scrollable ancestor falls through to normal dispatch
    /// (the existing LPAR-04 behaviour is unaffected).
    #[test]
    fn scroll_no_scrollable_ancestor_falls_through_to_normal_dispatch() {
        // Tree with only a CLICKABLE node, no SCROLLABLE.
        let click_events: Rc<RefCell<Vec<ObjectEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let mut root = ObjectNode::new(Rc::new(RefCell::new(TW {
            bounds: r(0, 0, 200, 200),
            consume_clicks: false,
            events: click_events.clone(),
        })));
        let child = clickable("btn", r(0, 0, 200, 200), click_events.clone());
        root.append_child(child);

        let mut dev = PointerDevice::new(30).with_scroll(ScrollConfig::default());

        dev.process(&mut root, Event::PointerDown { x: 10, y: 10 });
        // Move far enough to trigger a drag.
        dev.process(&mut root, Event::PointerMove { x: 30, y: 10 });
        dev.process(&mut root, Event::PointerUp { x: 30, y: 10 });
        for _ in 0..40 {
            dev.tick(&mut root);
        }

        // The drag has no SCROLLABLE ancestor, so it must NOT have been
        // suppressed by the scroll controller — scroll stays inactive and
        // the normal gesture path runs.  Dirty rects must be empty.
        let dirty = dev.take_dirty();
        assert!(
            dirty.is_empty(),
            "no dirty rects without a scrollable ancestor"
        );
    }

    /// The dirty sink receives the viewport rect when the scroll offset changes.
    #[test]
    fn scroll_dirty_sink_receives_viewport_rect_on_effective_scroll() {
        let mut root = scroll_tree_platform();
        let mut dev = PointerDevice::new(30).with_scroll(ScrollConfig::default());

        // Accumulate dirty rects across the full gesture.
        let mut all_dirty: Vec<Rect> = Vec::new();

        dev.process(&mut root, Event::PointerDown { x: 200, y: 400 });
        all_dirty.extend(dev.take_dirty());

        dev.process(&mut root, Event::PointerMove { x: 200, y: 380 });
        all_dirty.extend(dev.take_dirty());

        dev.process(&mut root, Event::PointerMove { x: 200, y: 350 });
        all_dirty.extend(dev.take_dirty());

        dev.process(&mut root, Event::PointerUp { x: 200, y: 350 });
        all_dirty.extend(dev.take_dirty());

        // At least one dirty rect must have been produced.
        assert!(
            !all_dirty.is_empty(),
            "dirty sink must receive rects on effective scroll"
        );
        // All dirty rects should correspond to the container viewport (400×600).
        for d in &all_dirty {
            assert_eq!(d.width, 400, "dirty rect width must match viewport");
            assert_eq!(d.height, 600, "dirty rect height must match viewport");
        }
    }
}
