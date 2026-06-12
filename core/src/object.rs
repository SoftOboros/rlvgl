//! LVGL-parity object metadata and tree helpers.
//!
//! This module is the additive object substrate for the LPAR initiative. It
//! deliberately leaves [`Widget`](crate::widget::Widget) and
//! [`WidgetNode`](crate::WidgetNode) untouched while providing a richer node
//! carrier for LVGL-like flags, states, ordering, detach semantics, hidden-aware
//! drawing, hit testing, and the LPAR-04 event/focus/lifecycle runtime.
//!
//! # LPAR-04 additions
//!
//! - [`ObjectEvent`] — the object-semantic event vocabulary.
//! - [`GestureDir`] — directional swipe summary payload for
//!   [`ObjectEvent::Gesture`].
//! - [`ObjectFlags::EVENT_BUBBLE`] — opt-in per-object bubbling flag.
//! - Per-node handler lists (trickle, target, bubble phase).
//! - [`dispatch_object_event`] — the routing entry point.
//! - [`DispatchInput`] and [`Disposition`] — dispatch input/output types.
//! - [`EventContext`] — dispatch context exposed to handlers.
//! - Lifecycle emission from mutation helpers
//!   ([`append_child`](ObjectNode::append_child),
//!   [`insert_child`](ObjectNode::insert_child),
//!   [`detach_child`](ObjectNode::detach_child)).

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::WidgetNode;
use crate::event::{Event, Key};
use crate::renderer::Renderer;
use crate::widget::{Rect, Widget};

// ---------------------------------------------------------------------------
// ObjectEvent — the §5.3 v1 code set
// ---------------------------------------------------------------------------

/// Directional swipe summary delivered as the payload of
/// [`ObjectEvent::Gesture`].
///
/// Derived from `DragStart`/`DragMove`/`DragEnd` displacement at the dispatch
/// layer (LPAR-04 §9.6).  Scroll begin/end/throw semantics belong to LPAR-05
/// and are **not** modelled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureDir {
    /// Swipe toward the top of the screen.
    Up,
    /// Swipe toward the bottom of the screen.
    Down,
    /// Swipe toward the left edge of the screen.
    Left,
    /// Swipe toward the right edge of the screen.
    Right,
}

/// Object-semantic event vocabulary for the LPAR-04 dispatch model.
///
/// This is the **object-level** event type, distinct from the device/recognizer
/// stream vocabulary in [`crate::event::Event`].  Stream events flow through
/// pipelines and legacy dispatch; `ObjectEvent` codes are delivered only
/// through [`dispatch_object_event`].
///
/// This enum is `#[non_exhaustive]`: consumers **must** include a wildcard
/// arm.  New codes will be added in later LPAR phases under the Specification
/// Required policy.
///
/// # LVGL analogue table (LPAR-04 §5.3 + LPAR-05 §6)
///
/// | Variant | Trigger | LVGL analogue |
/// |---|---|---|
/// | [`Pressed`](Self::Pressed) | Stream `PressDown` at target | `LV_EVENT_PRESSED` |
/// | [`Released`](Self::Released) | Contact ended over target | `LV_EVENT_RELEASED` |
/// | [`Clicked`](Self::Clicked) | Stream `PressRelease` at target | `LV_EVENT_CLICKED` |
/// | [`DoubleClicked`](Self::DoubleClicked) | Stream `DoubleTap` at target | `LV_EVENT_DOUBLE_CLICKED` |
/// | [`LongPressed`](Self::LongPressed) | Long-press recognizer output at target | `LV_EVENT_LONG_PRESSED` |
/// | [`LongPressedRepeat`](Self::LongPressedRepeat) | Long-press repeat output | `LV_EVENT_LONG_PRESSED_REPEAT` |
/// | [`Key`](Self::Key) | Key delivery to focused object | `LV_EVENT_KEY` |
/// | [`Rotary`](Self::Rotary) | Encoder diff in editing mode | `LV_EVENT_ROTARY` |
/// | [`Focused`](Self::Focused) | Object gained focus | `LV_EVENT_FOCUSED` |
/// | [`Defocused`](Self::Defocused) | Object lost focus | `LV_EVENT_DEFOCUSED` |
/// | [`Gesture`](Self::Gesture) | Directional swipe summary | `LV_EVENT_GESTURE` |
/// | [`Attached`](Self::Attached) | Subtree root attached via `append_child`/`insert_child` | `LV_EVENT_CHILD_CREATED` (inverted) |
/// | [`Detached`](Self::Detached) | Subtree root detached via `detach_child` | `LV_EVENT_DELETE` (narrowed) |
/// | [`ChildChanged`](Self::ChildChanged) | Parent notified after child add/remove/reorder | `LV_EVENT_CHILD_CHANGED` |
/// | [`ScrollBegin`](Self::ScrollBegin) | Scroll session began on this container | `LV_EVENT_SCROLL_BEGIN` |
/// | [`Scroll`](Self::Scroll) | Scroll offset changed | `LV_EVENT_SCROLL` |
/// | [`ScrollEnd`](Self::ScrollEnd) | Scroll session ended and offset settled | `LV_EVENT_SCROLL_END` |
/// | [`ScrollThrow`](Self::ScrollThrow) | Throw/momentum phase initiated | `LV_EVENT_SCROLL_THROW_BEGIN` |
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectEvent {
    /// A stable contact began at the given coordinates.
    ///
    /// Derived from stream [`Event::PressDown`] resolved at this target.
    /// Use for visual press feedback (e.g. button highlight).
    Pressed {
        /// Horizontal coordinate of the contact.
        x: i32,
        /// Vertical coordinate of the contact.
        y: i32,
    },
    /// A contact ended over this object (any cause).
    ///
    /// Always paired with a preceding [`Pressed`](Self::Pressed).
    Released {
        /// Horizontal coordinate at release.
        x: i32,
        /// Vertical coordinate at release.
        y: i32,
    },
    /// A short clean tap was completed on this object.
    ///
    /// Derived from stream [`Event::PressRelease`].  Contacts that crossed
    /// the drag threshold produce no `Clicked` (INPUT-00 §6 click-vs-drag
    /// suppression).
    Clicked {
        /// Horizontal coordinate of the tap.
        x: i32,
        /// Vertical coordinate of the tap.
        y: i32,
    },
    /// Two consecutive short taps were detected on this object.
    ///
    /// Derived from stream [`Event::DoubleTap`].
    DoubleClicked {
        /// Horizontal coordinate of the second tap.
        x: i32,
        /// Vertical coordinate of the second tap.
        y: i32,
    },
    /// A held contact reached the long-press threshold.
    ///
    /// Emitted once per contact by the long-press recognizer (LPAR-04 §9).
    /// Disarmed by `DragStart` before emission.
    LongPressed {
        /// Horizontal coordinate of the held contact.
        x: i32,
        /// Vertical coordinate of the held contact.
        y: i32,
    },
    /// A long-pressed contact crossed another repeat interval.
    ///
    /// Emitted every repeat period after the initial [`LongPressed`](Self::LongPressed).
    LongPressedRepeat {
        /// Horizontal coordinate of the held contact.
        x: i32,
        /// Vertical coordinate of the held contact.
        y: i32,
    },
    /// A key event was delivered to this focused object.
    ///
    /// Delivered when this object has focus and a keypad/encoder key event
    /// is received.  In navigate mode, encoder presses map to
    /// [`Key::Enter`]; in editing mode, all keys route here.
    Key(
        /// The key that was pressed or repeated.
        Key,
    ),
    /// Encoder rotation delta delivered to this focused object in editing mode.
    ///
    /// In navigate mode, rotation drives `focus_next`/`focus_prev` instead.
    Rotary {
        /// Net rotation steps (positive = clockwise).
        diff: i32,
    },
    /// This object gained keyboard focus.
    ///
    /// The [`ObjectStates::FOCUSED`] bit is set before this event is emitted.
    Focused,
    /// This object lost keyboard focus.
    ///
    /// The [`ObjectStates::FOCUSED`] bit is cleared before this event is emitted.
    Defocused,
    /// A directional swipe was recognized over this object.
    ///
    /// Derived from drag displacement (LPAR-04 §9.6).  Scroll semantics are
    /// LPAR-05 non-goals.
    Gesture(
        /// Direction of the swipe.
        GestureDir,
    ),
    /// This node was attached to a live tree via
    /// [`append_child`](ObjectNode::append_child) or
    /// [`insert_child`](ObjectNode::insert_child).
    ///
    /// Delivered synchronously to the attached subtree root after the
    /// structural change.
    Attached,
    /// This node was detached from its parent via
    /// [`detach_child`](ObjectNode::detach_child).
    ///
    /// Delivered synchronously to the detached subtree root after the
    /// structural change.  This is the lifecycle event name reserved by
    /// LPAR-02 §6.10.
    Detached,
    /// A child was added to or removed from this node.
    ///
    /// Delivered synchronously to the **parent** after
    /// [`append_child`](ObjectNode::append_child),
    /// [`insert_child`](ObjectNode::insert_child), or
    /// [`detach_child`](ObjectNode::detach_child).
    ChildChanged,
    /// Scroll session began on this container.
    ///
    /// Emitted once per scroll session when a drag gesture transitions the
    /// container into active scrolling. The offset is unchanged at this point;
    /// the first [`Scroll`](Self::Scroll) event carries the new offset.
    ///
    /// LVGL analogue: `LV_EVENT_SCROLL_BEGIN`.
    ScrollBegin,
    /// Scroll offset changed.
    ///
    /// Emitted each tick/frame that the offset advances — during active drag
    /// scrolling and during throw deceleration. The payload carries the new
    /// logical scroll offset after the change.
    ///
    /// LVGL analogue: `LV_EVENT_SCROLL`.
    Scroll {
        /// New horizontal scroll offset in logical pixels.
        scroll_x: i32,
        /// New vertical scroll offset in logical pixels.
        scroll_y: i32,
    },
    /// Scroll session ended and the offset has settled.
    ///
    /// Emitted once per scroll session, after the final [`Scroll`](Self::Scroll)
    /// event (after throw deceleration or snap settle completes, or after a
    /// drag ended with zero residual velocity).
    ///
    /// LVGL analogue: `LV_EVENT_SCROLL_END`.
    ScrollEnd,
    /// Throw/momentum phase initiated.
    ///
    /// Emitted once when the finger lifted with enough residual velocity to
    /// initiate throw/momentum. Emitted between the last drag-driven
    /// [`Scroll`](Self::Scroll) and the first momentum-driven
    /// [`Scroll`](Self::Scroll).
    ///
    /// LVGL analogue: `LV_EVENT_SCROLL_THROW_BEGIN`.
    ScrollThrow {
        /// Estimated horizontal throw velocity in logical pixels per tick.
        vel_x: i32,
        /// Estimated throw velocity in logical pixels per tick.
        vel_y: i32,
    },
}

// ---------------------------------------------------------------------------
// EventContext — dispatch context exposed to handlers
// ---------------------------------------------------------------------------

/// Dispatch context passed to every event handler during propagation.
///
/// `target_tag` and `current_tag` are the test-automation tags of the dispatch
/// target and the node currently running its handlers, respectively.  Tags are
/// optional; a `None` value means the node carries no tag.
#[derive(Debug, Clone, Copy)]
pub struct EventContext {
    /// Test-automation tag of the ultimate dispatch target.
    pub target_tag: Option<&'static str>,
    /// Test-automation tag of the node whose handlers are currently executing.
    pub current_tag: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// Handler type alias
// ---------------------------------------------------------------------------

/// Signature for an event handler stored on an [`ObjectNode`].
///
/// Receives the [`ObjectEvent`] and the current [`EventContext`].  Returns
/// `true` to consume the event and stop propagation.
pub type ObjectHandler = Box<dyn FnMut(&ObjectEvent, EventContext) -> bool>;

// ---------------------------------------------------------------------------
// ObjectFlags
// ---------------------------------------------------------------------------

/// LVGL-like object behavior flags.
///
/// Flags are stored as a compact bitset. Use the associated constants with
/// [`contains`](Self::contains), [`insert`](Self::insert), and
/// [`remove`](Self::remove).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFlags(u32);

impl ObjectFlags {
    /// No flags set.
    pub const EMPTY: Self = Self(0);
    /// Skip drawing and targeting for the object and its subtree.
    pub const HIDDEN: Self = Self(1 << 0);
    /// Draw the object but exclude it from default interaction targeting.
    pub const DISABLED: Self = Self(1 << 1);
    /// Allow the object to be selected by pointer hit testing.
    pub const CLICKABLE: Self = Self(1 << 2);
    /// Allow the object to participate in future focus traversal.
    pub const FOCUSABLE: Self = Self(1 << 3);
    /// Allow the object to participate in future scroll behavior.
    pub const SCROLLABLE: Self = Self(1 << 4);
    /// Opt-in bubbling: propagate events toward root after the target phase
    /// (LPAR-04 §6.4; mirrors `LV_OBJ_FLAG_EVENT_BUBBLE`).
    ///
    /// Default: clear.  When clear, bubble phase stops at this node.
    pub const EVENT_BUBBLE: Self = Self(1 << 5);

    /// Return a flag set from raw bits, dropping unknown bits.
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::all_bits())
    }

    /// Return the raw flag bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Return `true` when all flags in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Insert all flags in `other`.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Remove all flags in `other`.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Set or clear all flags in `other`.
    pub fn set(&mut self, other: Self, enabled: bool) {
        if enabled {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    const fn all_bits() -> u32 {
        Self::HIDDEN.0
            | Self::DISABLED.0
            | Self::CLICKABLE.0
            | Self::FOCUSABLE.0
            | Self::SCROLLABLE.0
            | Self::EVENT_BUBBLE.0
    }
}

impl Default for ObjectFlags {
    fn default() -> Self {
        Self::EMPTY
    }
}

// ---------------------------------------------------------------------------
// ObjectStates
// ---------------------------------------------------------------------------

/// LVGL-like object state bits.
///
/// The default state is represented by no bits set. Style resolution consumes
/// these bits in later LPAR phases; this module only stores and queries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStates(u32);

impl ObjectStates {
    /// No state bits set.
    pub const DEFAULT: Self = Self(0);
    /// Disabled state for style selection.
    pub const DISABLED: Self = Self(1 << 0);
    /// Focused state for style selection and focus bookkeeping.
    pub const FOCUSED: Self = Self(1 << 1);
    /// Pressed state for pointer or key activation.
    pub const PRESSED: Self = Self(1 << 2);
    /// Checked state for toggleable controls.
    pub const CHECKED: Self = Self(1 << 3);
    /// Edited state for text or value editing modes.
    pub const EDITED: Self = Self(1 << 4);

    /// Return a state set from raw bits, dropping unknown bits.
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::all_bits())
    }

    /// Return the raw state bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Return `true` when all state bits in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Insert all state bits in `other`.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Remove all state bits in `other`.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Set or clear all state bits in `other`.
    pub fn set(&mut self, other: Self, enabled: bool) {
        if enabled {
            self.insert(other);
        } else {
            self.remove(other);
        }
    }

    const fn all_bits() -> u32 {
        Self::DISABLED.0 | Self::FOCUSED.0 | Self::PRESSED.0 | Self::CHECKED.0 | Self::EDITED.0
    }
}

impl Default for ObjectStates {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// ---------------------------------------------------------------------------
// ObjectMeta
// ---------------------------------------------------------------------------

/// Cross-cutting metadata stored by an [`ObjectNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectMeta {
    flags: ObjectFlags,
    states: ObjectStates,
    detached: bool,
}

impl ObjectMeta {
    /// Create default object metadata.
    pub const fn new() -> Self {
        Self {
            flags: ObjectFlags::EMPTY,
            states: ObjectStates::DEFAULT,
            detached: false,
        }
    }

    /// Return the object flags.
    pub const fn flags(&self) -> ObjectFlags {
        self.flags
    }

    /// Mutably borrow the object flags.
    pub fn flags_mut(&mut self) -> &mut ObjectFlags {
        &mut self.flags
    }

    /// Return the object state bits.
    pub const fn states(&self) -> ObjectStates {
        self.states
    }

    /// Mutably borrow the object state bits.
    pub fn states_mut(&mut self) -> &mut ObjectStates {
        &mut self.states
    }

    /// Return whether this node has been detached from a live object tree.
    pub const fn is_detached(&self) -> bool {
        self.detached
    }

    fn set_detached(&mut self, detached: bool) {
        self.detached = detached;
    }
}

// ---------------------------------------------------------------------------
// DispatchInput and Disposition
// ---------------------------------------------------------------------------

/// The input to [`dispatch_object_event`].
///
/// Pointer-positioned events carry coordinates used for hit-test target
/// resolution.  Key and encoder events route to the focused node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchInput {
    /// A pointer stream event to be delivered to the hit-test target.
    ///
    /// The wrapped [`Event`] must be a pointer-positioned variant
    /// (`PressDown`, `PressRelease`, `DoubleTap`, `LongPress`, etc.).  The
    /// `x`/`y` coordinates are used for hit testing.
    Pointer {
        /// Pointer horizontal coordinate for hit-test resolution.
        x: i32,
        /// Pointer vertical coordinate for hit-test resolution.
        y: i32,
        /// The stream event to translate into an [`ObjectEvent`].
        event: Event,
    },
    /// An [`ObjectEvent`] to deliver to the hit-test target at `(x, y)`.
    PointerObject {
        /// Pointer horizontal coordinate for hit-test resolution.
        x: i32,
        /// Pointer vertical coordinate for hit-test resolution.
        y: i32,
        /// The object-semantic event to dispatch.
        event: ObjectEvent,
    },
    /// An [`ObjectEvent`] to deliver to the currently focused node.
    Focused {
        /// The object-semantic event to dispatch.
        event: ObjectEvent,
    },
    /// An [`ObjectEvent`] targeted at the node identified by a structural path,
    /// bypassing hit-test resolution.
    ///
    /// The event is delivered to that node's target handlers, then bubbles
    /// through ancestors using the existing per-node [`ObjectFlags::EVENT_BUBBLE`]
    /// gating — identically to how any other event bubbles. The
    /// [`ObjectFlags::EVENT_BUBBLE`] flag is never mutated (LPAR-05 §6.3/§16).
    Container {
        /// Structural path from root to the target node (list of child indices).
        path: Vec<usize>,
        /// The object-semantic event to dispatch.
        event: ObjectEvent,
    },
}

/// The outcome of a [`dispatch_object_event`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The event was consumed by a handler during dispatch.
    Consumed,
    /// The event completed all phases without any handler consuming it.
    Unconsumed,
    /// No target could be resolved (no hit-test result or no focused node).
    NoTarget,
}

// ---------------------------------------------------------------------------
// ObjectNode
// ---------------------------------------------------------------------------

/// Additive LVGL-like object tree node.
///
/// `ObjectNode` mirrors the current widget-tree ownership shape while keeping
/// object metadata in the node layer. Existing [`WidgetNode`](crate::WidgetNode)
/// users remain source-compatible; new parity work can opt into this richer
/// carrier.
///
/// # LPAR-04 handler storage
///
/// Each node owns three per-phase handler lists:
///
/// - **Trickle handlers** run root→target *before* the target phase.
/// - **Target handlers** run at the target node.
/// - **Bubble handlers** run target→root *after* the target phase, but only
///   while the node has [`ObjectFlags::EVENT_BUBBLE`] set.
///
/// Handlers are closures stored as `Box<dyn FnMut(…)>` and are invoked in
/// registration order.  A handler returning `true` consumes the event and
/// stops all remaining phases.
///
/// # Mutation and dispatch
///
/// Tree mutation helpers ([`append_child`](Self::append_child),
/// [`insert_child`](Self::insert_child), [`detach_child`](Self::detach_child))
/// MUST NOT be called from inside an active dispatch.  They emit lifecycle
/// events ([`ObjectEvent::Attached`], [`ObjectEvent::Detached`],
/// [`ObjectEvent::ChildChanged`]) synchronously after the structural change,
/// outside any active trickle/target/bubble traversal.
pub struct ObjectNode {
    widget: Rc<RefCell<dyn Widget>>,
    children: Vec<ObjectNode>,
    tag: Option<&'static str>,
    meta: ObjectMeta,
    /// Handlers run root→target (trickle/preprocess phase).
    trickle_handlers: Vec<ObjectHandler>,
    /// Handlers run at the target node (target phase).
    target_handlers: Vec<ObjectHandler>,
    /// Handlers run target→root (bubble phase; gated by `EVENT_BUBBLE` flag).
    bubble_handlers: Vec<ObjectHandler>,
    /// Optional scroll state, present when `ObjectFlags::SCROLLABLE` is set.
    ///
    /// Stored as a boxed value to keep `ObjectNode` from growing for
    /// non-scroll nodes.
    pub(crate) scroll: Option<Box<crate::scroll::ScrollState>>,
    /// Optional node-resident animation entries (LPAR-06 §6.1).
    ///
    /// Stored as a boxed value to keep `ObjectNode` from growing for
    /// non-animated nodes. Lazily allocated by
    /// [`ObjectAnims::bind`](crate::object_anim::ObjectAnims::bind).
    pub(crate) anims: Option<Box<crate::object_anim::NodeAnimSet>>,
    /// Optional LPAR-07 style slot holding local and shared style entries.
    ///
    /// `None` for nodes that carry no style overrides, keeping `ObjectNode`
    /// small. Lazily allocated by [`ObjectNode::add_local_style`] or
    /// [`ObjectNode::add_style`].
    pub(crate) style: Option<Box<crate::style_cascade::StyleState>>,
}

impl ObjectNode {
    /// Create a new object node with no children, tag, flags, states, or handlers.
    pub fn new(widget: Rc<RefCell<dyn Widget>>) -> Self {
        Self {
            widget,
            children: Vec::new(),
            tag: None,
            meta: ObjectMeta::new(),
            trickle_handlers: Vec::new(),
            target_handlers: Vec::new(),
            bubble_handlers: Vec::new(),
            scroll: None,
            anims: None,
            style: None,
        }
    }

    /// Recursively adopt a compatibility [`WidgetNode`] into an object tree.
    ///
    /// The widget handle, child order, and test-automation tag are preserved.
    /// New object metadata is initialized to defaults for every adopted node.
    /// Nodes adopted this way start with empty handler lists.
    pub fn adopt(node: WidgetNode) -> Self {
        let mut object = Self::new(node.widget);
        object.tag = node.tag;
        object.children = node.children.into_iter().map(Self::adopt).collect();
        object
    }

    /// Attach a test-automation tag to this node.
    pub fn with_tag(mut self, tag: &'static str) -> Self {
        self.tag = Some(tag);
        self
    }

    /// Return the widget handle stored by this node.
    pub fn widget(&self) -> &Rc<RefCell<dyn Widget>> {
        &self.widget
    }

    /// Return this node's test-automation tag.
    pub const fn tag(&self) -> Option<&'static str> {
        self.tag
    }

    /// Return immutable access to the node's object metadata.
    pub const fn meta(&self) -> &ObjectMeta {
        &self.meta
    }

    /// Return mutable access to the node's object metadata.
    pub fn meta_mut(&mut self) -> &mut ObjectMeta {
        &mut self.meta
    }

    /// Return this node's object flags.
    pub const fn flags(&self) -> ObjectFlags {
        self.meta.flags()
    }

    /// Return this node's object state bits.
    pub const fn states(&self) -> ObjectStates {
        self.meta.states()
    }

    /// Set or clear a flag on this node.
    pub fn set_flag(&mut self, flag: ObjectFlags, enabled: bool) {
        self.meta.flags_mut().set(flag, enabled);
    }

    /// Set or clear a state bit on this node.
    pub fn set_state(&mut self, state: ObjectStates, enabled: bool) {
        self.meta.states_mut().set(state, enabled);
    }

    /// Return whether this node is detached from a live object tree.
    pub const fn is_detached(&self) -> bool {
        self.meta.is_detached()
    }

    /// Return this node's child list.
    pub fn children(&self) -> &[ObjectNode] {
        &self.children
    }

    /// Return this node's mutable child list.
    pub fn children_mut(&mut self) -> &mut Vec<ObjectNode> {
        &mut self.children
    }

    // -----------------------------------------------------------------------
    // Scroll state accessors (LPAR-05 §5)
    // -----------------------------------------------------------------------

    /// Attach scroll state to this node, enabling it as a scroll container.
    ///
    /// The node's `SCROLLABLE` flag is set automatically. This is the primary
    /// way to make a node a scroll container (LPAR-05 §5).
    pub fn set_scroll_state(&mut self, state: Box<crate::scroll::ScrollState>) {
        self.meta.flags_mut().insert(ObjectFlags::SCROLLABLE);
        self.scroll = Some(state);
    }

    /// Return an immutable reference to the scroll state, if any.
    pub fn scroll_state(&self) -> Option<&crate::scroll::ScrollState> {
        self.scroll.as_deref()
    }

    /// Return a mutable reference to the scroll state, if any.
    pub fn scroll_state_mut(&mut self) -> Option<&mut crate::scroll::ScrollState> {
        self.scroll.as_deref_mut()
    }

    // -----------------------------------------------------------------------
    // Style accessors (LPAR-07 §7.1)
    // -----------------------------------------------------------------------

    /// Add a locally owned style patch keyed by `selector`.
    ///
    /// Local style entries take priority over added (shared) entries in the
    /// cascade. Within local entries, the last-added entry wins when multiple
    /// selectors match (§7.2 reverse-registration-order rule).
    ///
    /// The style slot is lazily allocated on the first call.
    pub fn add_local_style(
        &mut self,
        patch: crate::style_cascade::StylePatch,
        selector: crate::style_cascade::Selector,
    ) {
        let slot = self
            .style
            .get_or_insert_with(|| Box::new(crate::style_cascade::StyleState::new()));
        crate::style_cascade::push_local(slot, patch, selector);
    }

    /// Add a shared (added) style patch reference keyed by `selector`.
    ///
    /// Added entries have lower precedence than local entries. Within added
    /// entries, the last-added entry wins when multiple selectors match (§7.2).
    ///
    /// The `'static` lifetime constraint is conservative in v1; see LPAR-07
    /// §7.1 for the deferred object-lifetime-scoped extension.
    ///
    /// The style slot is lazily allocated on the first call.
    pub fn add_style(
        &mut self,
        patch: &'static crate::style_cascade::StylePatch,
        selector: crate::style_cascade::Selector,
    ) {
        let slot = self
            .style
            .get_or_insert_with(|| Box::new(crate::style_cascade::StyleState::new()));
        crate::style_cascade::push_added(slot, patch, selector);
    }

    /// Add a default-theme style patch keyed by `selector` (LPAR-07 §9.1).
    ///
    /// Theme entries resolve at the **lowest** style precedence — below local
    /// and added styles — so a widget or application style always wins over the
    /// theme regardless of registration order. Themes write here via
    /// [`LparTheme::apply_to_node`](crate::theme::LparTheme::apply_to_node).
    ///
    /// The style slot is lazily allocated on the first call.
    pub fn add_theme_style(
        &mut self,
        patch: crate::style_cascade::StylePatch,
        selector: crate::style_cascade::Selector,
    ) {
        let slot = self
            .style
            .get_or_insert_with(|| Box::new(crate::style_cascade::StyleState::new()));
        crate::style_cascade::push_theme(slot, patch, selector);
    }

    /// Clear all default-theme style entries on this node (for theme re-apply).
    ///
    /// Returns the number of entries removed; `0` when the style slot is `None`.
    pub fn clear_theme_styles(&mut self) -> usize {
        match self.style.as_deref_mut() {
            Some(slot) => crate::style_cascade::clear_theme(slot),
            None => 0,
        }
    }

    /// Remove local style entries whose selector matches `(part, states)`.
    ///
    /// A selector matches the `(part, states)` query when the selector's part
    /// equals `part` and all state bits in `states` are present in the
    /// selector's state mask (same predicate as cascade matching).
    ///
    /// Returns the number of entries removed. Returns `0` when the style slot
    /// is `None`.
    ///
    /// To remove all local styles for a part regardless of state, call
    /// [`remove_all_local_styles_by_part`](Self::remove_all_local_styles_by_part).
    /// To clear everything, call [`remove_all_local_styles`](Self::remove_all_local_styles).
    pub fn remove_local_styles(
        &mut self,
        part: crate::style_cascade::Part,
        states: ObjectStates,
    ) -> usize {
        match self.style.as_deref_mut() {
            Some(slot) => crate::style_cascade::remove_local_matching(slot, part, states),
            None => 0,
        }
    }

    /// Remove all local style entries for `part` regardless of state mask.
    ///
    /// This is the wildcard form described in §7.5 — removes every local entry
    /// whose part equals `part`, regardless of its state selector.
    ///
    /// Returns the number of entries removed.
    pub fn remove_all_local_styles_by_part(&mut self, part: crate::style_cascade::Part) -> usize {
        match self.style.as_deref_mut() {
            Some(slot) => crate::style_cascade::remove_all_local_by_part(slot, part),
            None => 0,
        }
    }

    /// Remove all local style entries from this node (unconditional clear).
    ///
    /// Returns the number of entries removed.
    pub fn remove_all_local_styles(&mut self) -> usize {
        match self.style.as_deref_mut() {
            Some(slot) => crate::style_cascade::remove_all_local(slot),
            None => 0,
        }
    }

    // -----------------------------------------------------------------------
    // Handler registration (LPAR-04 §6)
    // -----------------------------------------------------------------------

    /// Register a trickle-phase handler on this node.
    ///
    /// Trickle handlers run root→target before the target phase.  A handler
    /// returning `true` consumes the event and stops all phases.
    pub fn add_trickle_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&ObjectEvent, EventContext) -> bool + 'static,
    {
        self.trickle_handlers.push(Box::new(handler));
    }

    /// Register a target-phase handler on this node.
    ///
    /// Target handlers run at the node when it is the dispatch target, after
    /// the trickle phase.  A handler returning `true` consumes the event.
    pub fn add_target_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&ObjectEvent, EventContext) -> bool + 'static,
    {
        self.target_handlers.push(Box::new(handler));
    }

    /// Register a bubble-phase handler on this node.
    ///
    /// Bubble handlers run target→root after the target phase, but only while
    /// [`ObjectFlags::EVENT_BUBBLE`] is set on the target (and on each
    /// ancestor traversed).  A handler returning `true` consumes the event.
    pub fn add_bubble_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&ObjectEvent, EventContext) -> bool + 'static,
    {
        self.bubble_handlers.push(Box::new(handler));
    }

    /// Invoke all registered target-phase handlers for `event`.
    ///
    /// This is the direct handler-invocation path used by lifecycle delivery
    /// (Attached/Detached/ChildChanged/Focused/Defocused), which runs outside
    /// the trickle/bubble dispatch phases.
    ///
    /// Returns `true` if any handler consumed the event.
    pub fn invoke_handlers_for(&mut self, event: &ObjectEvent) -> bool {
        let ctx = EventContext {
            target_tag: self.tag,
            current_tag: self.tag,
        };
        for handler in &mut self.target_handlers {
            if handler(event, ctx) {
                return true;
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Mutation helpers — emit lifecycle events (LPAR-04 §6.6)
    // -----------------------------------------------------------------------

    /// Append a child node and return its index.
    ///
    /// After the structural change, emits [`ObjectEvent::Attached`] to the
    /// added subtree root's target handlers, then [`ObjectEvent::ChildChanged`]
    /// to this node's target handlers.
    ///
    /// # Dispatch constraint
    ///
    /// This method MUST NOT be called from inside an active
    /// [`dispatch_object_event`] traversal (LPAR-02 §7.4 / LPAR-04 §6.6).
    pub fn append_child(&mut self, mut child: ObjectNode) -> usize {
        child.set_detached_recursive(false);
        self.children.push(child);
        let idx = self.children.len() - 1;
        // Lifecycle: Attached → subtree root.
        self.children[idx].invoke_handlers_for(&ObjectEvent::Attached);
        // Lifecycle: ChildChanged → parent.
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        idx
    }

    /// Insert a child node at `index`.
    ///
    /// Returns `false` when `index` is greater than the current child count.
    ///
    /// After the structural change, emits [`ObjectEvent::Attached`] to the
    /// added subtree root's target handlers, then [`ObjectEvent::ChildChanged`]
    /// to this node's target handlers.
    ///
    /// # Dispatch constraint
    ///
    /// This method MUST NOT be called from inside an active
    /// [`dispatch_object_event`] traversal (LPAR-02 §7.4 / LPAR-04 §6.6).
    pub fn insert_child(&mut self, index: usize, mut child: ObjectNode) -> bool {
        if index > self.children.len() {
            return false;
        }
        child.set_detached_recursive(false);
        self.children.insert(index, child);
        // Lifecycle: Attached → subtree root.
        self.children[index].invoke_handlers_for(&ObjectEvent::Attached);
        // Lifecycle: ChildChanged → parent.
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        true
    }

    /// Detach and return the child at `index`.
    ///
    /// The returned subtree is marked detached recursively.  After the
    /// structural change, emits [`ObjectEvent::Detached`] to the detached
    /// subtree root's target handlers, then [`ObjectEvent::ChildChanged`] to
    /// this node's target handlers.
    ///
    /// # Dispatch constraint
    ///
    /// This method MUST NOT be called from inside an active
    /// [`dispatch_object_event`] traversal (LPAR-02 §7.4 / LPAR-04 §6.6).
    pub fn detach_child(&mut self, index: usize) -> Option<ObjectNode> {
        if index >= self.children.len() {
            return None;
        }
        let mut child = self.children.remove(index);
        child.set_detached_recursive(true);
        // Lifecycle: Detached → detached subtree root.
        child.invoke_handlers_for(&ObjectEvent::Detached);
        // Lifecycle: ChildChanged → parent.
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        Some(child)
    }

    // -----------------------------------------------------------------------
    // Reorder helpers (emit ChildChanged → parent on an effective move,
    // per the LPAR-04 §5.3 "add/remove/reorder" rule)
    // -----------------------------------------------------------------------

    /// Move the child at `index` to the front of the draw order.
    ///
    /// The front is the highest index and is considered visually topmost for
    /// hit testing. On a successful move, emits [`ObjectEvent::ChildChanged`]
    /// to this node's target handlers (LPAR-04 §5.3).
    pub fn raise_child(&mut self, index: usize) -> bool {
        if index >= self.children.len() {
            return false;
        }
        let child = self.children.remove(index);
        self.children.push(child);
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        true
    }

    /// Move the child at `index` to the back of the draw order.
    ///
    /// On a successful move, emits [`ObjectEvent::ChildChanged`] to this node's
    /// target handlers (LPAR-04 §5.3).
    pub fn lower_child(&mut self, index: usize) -> bool {
        if index >= self.children.len() {
            return false;
        }
        let child = self.children.remove(index);
        self.children.insert(0, child);
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        true
    }

    /// Move child `from` directly before child `before`.
    ///
    /// On an effective move (not a `from == before` no-op), emits
    /// [`ObjectEvent::ChildChanged`] to this node's target handlers
    /// (LPAR-04 §5.3).
    pub fn move_child_before(&mut self, from: usize, before: usize) -> bool {
        let len = self.children.len();
        if from >= len || before >= len {
            return false;
        }
        if from == before {
            return true;
        }
        let child = self.children.remove(from);
        let target = if from < before { before - 1 } else { before };
        self.children.insert(target, child);
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        true
    }

    /// Move child `from` directly after child `after`.
    ///
    /// On an effective move (not a `from == after` no-op), emits
    /// [`ObjectEvent::ChildChanged`] to this node's target handlers
    /// (LPAR-04 §5.3).
    pub fn move_child_after(&mut self, from: usize, after: usize) -> bool {
        let len = self.children.len();
        if from >= len || after >= len {
            return false;
        }
        if from == after {
            return true;
        }
        let child = self.children.remove(from);
        let mut target = after + 1;
        if from < target {
            target -= 1;
        }
        if target > self.children.len() {
            target = self.children.len();
        }
        self.children.insert(target, child);
        self.invoke_handlers_for(&ObjectEvent::ChildChanged);
        true
    }

    // -----------------------------------------------------------------------
    // Draw and hit-test (unchanged from LPAR-02)
    // -----------------------------------------------------------------------

    /// Recursively draw this object subtree.
    ///
    /// Hidden or detached subtrees are skipped. Visible nodes draw parent first
    /// and then children in sibling order.
    pub fn draw(&self, renderer: &mut dyn Renderer) {
        if self.is_hidden_or_detached() {
            return;
        }
        self.widget.borrow().draw(renderer);
        for child in &self.children {
            child.draw(renderer);
        }
    }

    /// Return the topmost targetable node at `x`, `y`.
    ///
    /// Hit testing searches children in reverse sibling order before testing
    /// the node itself. Hidden and detached subtrees are skipped. Disabled
    /// nodes are not targetable, but their visible children remain eligible in
    /// this base object phase.
    pub fn hit_test(&self, x: i32, y: i32) -> Option<&ObjectNode> {
        if self.is_hidden_or_detached() {
            return None;
        }
        for child in self.children.iter().rev() {
            if let Some(hit) = child.hit_test(x, y) {
                return Some(hit);
            }
        }
        if self.is_targetable_at(x, y) {
            Some(self)
        } else {
            None
        }
    }

    /// Union of this widget's bounds with every visible descendant's bounds,
    /// in logical coordinates (LPAR-03 "subtree visual extent").
    ///
    /// Returns `None` when this node is hidden or detached, or when neither
    /// this widget nor any visible descendant has positive-area bounds.
    /// Zero-area bounds contribute nothing; hidden or detached subtrees are
    /// excluded entirely, exactly mirroring [`draw`](Self::draw) skipping.
    ///
    /// # Ordering contract (LPAR-03 §15)
    ///
    /// A hidden root yields `None` **by design**: invalidation callers order
    /// their operations as *compute-then-hide* (capture the extent before
    /// setting [`ObjectFlags::HIDDEN`]) and *show-then-compute* (clear the
    /// flag, then capture). The same applies to detach: capture the extent
    /// before [`detach_child`](Self::detach_child), then push the captured
    /// rect into the invalidation planner.
    pub fn visible_subtree_extent(&self) -> Option<Rect> {
        if self.is_hidden_or_detached() {
            return None;
        }
        let bounds = self.widget.borrow().bounds();
        let mut extent = if bounds.width > 0 && bounds.height > 0 {
            Some(bounds)
        } else {
            None
        };
        for child in &self.children {
            if let Some(child_extent) = child.visible_subtree_extent() {
                extent = Some(match extent {
                    Some(current) => current.union(child_extent),
                    None => child_extent,
                });
            }
        }
        extent
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn set_detached_recursive(&mut self, detached: bool) {
        self.meta.set_detached(detached);
        for child in &mut self.children {
            child.set_detached_recursive(detached);
        }
    }

    fn is_hidden_or_detached(&self) -> bool {
        self.is_detached() || self.flags().contains(ObjectFlags::HIDDEN)
    }

    /// Public(crate) version used by `focus.rs` for ancestor-visibility checks.
    pub(crate) fn is_hidden_or_detached_pub(&self) -> bool {
        self.is_hidden_or_detached()
    }

    fn is_targetable_at(&self, x: i32, y: i32) -> bool {
        let flags = self.flags();
        flags.contains(ObjectFlags::CLICKABLE)
            && !flags.contains(ObjectFlags::DISABLED)
            && rect_contains(self.widget.borrow().bounds(), x, y)
    }
}

// ---------------------------------------------------------------------------
// dispatch_object_event — the LPAR-04 routing entry point
// ---------------------------------------------------------------------------

/// Dispatch an [`ObjectEvent`] through the `root` tree using
/// trickle → target → bubble propagation (LPAR-04 §6).
///
/// # Target resolution
///
/// - [`DispatchInput::Pointer`]: translates the stream `Event` into an
///   `ObjectEvent` and resolves the target via `hit_test(x, y)`.
/// - [`DispatchInput::PointerObject`]: dispatches the provided `ObjectEvent`
///   to the hit-test target at `(x, y)`.
/// - [`DispatchInput::Focused`]: dispatches to the node whose
///   [`ObjectStates::FOCUSED`] bit is set.
///
/// # Phase order
///
/// 1. **Trickle** — root→target, trickle handlers only.
/// 2. **Target** — target node: target handlers, then (for `Pointer` inputs)
///    the wrapped [`Widget::handle_event`].
/// 3. **Bubble** — target→root, bubble handlers; continues past the target
///    only when [`ObjectFlags::EVENT_BUBBLE`] is set on the target.
///
/// Any handler (or `Widget::handle_event` returning `true`) that consumes the
/// event terminates all remaining phases.
///
/// # Dispatch constraint
///
/// Mutation helpers ([`ObjectNode::append_child`] etc.) MUST NOT be called
/// from inside a handler during an active dispatch (LPAR-02 §7.4).
pub fn dispatch_object_event(root: &mut ObjectNode, input: DispatchInput) -> Disposition {
    // ----- 1. Resolve target path -----
    let (target_path, object_event, raw_stream_event) = match resolve_target(root, &input) {
        None => return Disposition::NoTarget,
        Some(t) => t,
    };

    let target_tag = node_at_path(root, &target_path).tag();

    // ----- 2. Trickle phase: root → target -----
    // Visit nodes [0..len] where path[0..i] is the prefix of the target path.
    // We visit ancestor nodes only (0..len), not the target itself.
    for depth in 0..target_path.len() {
        let node_path = &target_path[..depth];
        let ctx = EventContext {
            target_tag,
            current_tag: node_at_path(root, node_path).tag(),
        };
        let consumed = invoke_trickle(root, node_path, &object_event, ctx);
        if consumed {
            return Disposition::Consumed;
        }
    }

    // ----- 3. Target phase -----
    {
        let ctx = EventContext {
            target_tag,
            current_tag: target_tag,
        };
        let target_node = node_at_path_mut(root, &target_path);
        // Run target handlers.
        for handler in &mut target_node.target_handlers {
            if handler(&object_event, ctx) {
                return Disposition::Consumed;
            }
        }
        // For pointer stream inputs, also invoke Widget::handle_event.
        if let Some(ref ev) = raw_stream_event
            && target_node.widget.borrow_mut().handle_event(ev)
        {
            return Disposition::Consumed;
        }
    }

    // ----- 4. Bubble phase: target → root -----
    // The event is delivered to the target's bubble handlers, then ascends to
    // each ancestor's bubble handlers. Ascent from a node to its parent
    // continues only while that node has EVENT_BUBBLE set — the per-object rule
    // LVGL uses (LPAR-04 §6.4). A broken flag chain stops the bubble, so a
    // target whose ancestor clears the flag does not reach the root.
    let mut depth = target_path.len();
    loop {
        let node_path = &target_path[..depth];
        let ctx = EventContext {
            target_tag,
            current_tag: node_at_path(root, node_path).tag(),
        };
        if invoke_bubble(root, node_path, &object_event, ctx) {
            return Disposition::Consumed;
        }
        if depth == 0 {
            break;
        }
        // Continue to the parent only if THIS node opts into bubbling.
        if !node_at_path(root, node_path)
            .flags()
            .contains(ObjectFlags::EVENT_BUBBLE)
        {
            break;
        }
        depth -= 1;
    }

    Disposition::Unconsumed
}

// ---------------------------------------------------------------------------
// dispatch_object_event internals
// ---------------------------------------------------------------------------

/// Resolve target path, object event, and optional raw stream event.
fn resolve_target(
    root: &ObjectNode,
    input: &DispatchInput,
) -> Option<(Vec<usize>, ObjectEvent, Option<Event>)> {
    match input {
        DispatchInput::Pointer { x, y, event } => {
            let obj_ev = stream_event_to_object_event(event)?;
            let path = hit_test_path(root, *x, *y)?;
            Some((path, obj_ev, Some(event.clone())))
        }
        DispatchInput::PointerObject { x, y, event } => {
            let path = hit_test_path(root, *x, *y)?;
            Some((path, event.clone(), None))
        }
        DispatchInput::Focused { event } => {
            let path = find_focused_path_ro(root)?;
            Some((path, event.clone(), None))
        }
        DispatchInput::Container { path, event } => Some((path.clone(), event.clone(), None)),
    }
}

/// Map a stream [`Event`] to an [`ObjectEvent`] (pointer events only).
///
/// Returns `None` for events that have no direct pointer-target mapping
/// (e.g. `Tick`, `KeyDown`, `Encoder`).
fn stream_event_to_object_event(ev: &Event) -> Option<ObjectEvent> {
    match ev {
        Event::PressDown { x, y } => Some(ObjectEvent::Pressed { x: *x, y: *y }),
        Event::PressRelease { x, y } => Some(ObjectEvent::Clicked { x: *x, y: *y }),
        Event::DoubleTap { x, y } => Some(ObjectEvent::DoubleClicked { x: *x, y: *y }),
        Event::LongPress { x, y } => Some(ObjectEvent::LongPressed { x: *x, y: *y }),
        Event::LongPressRepeat { x, y } => Some(ObjectEvent::LongPressedRepeat { x: *x, y: *y }),
        Event::PointerUp { x, y } => Some(ObjectEvent::Released { x: *x, y: *y }),
        _ => None,
    }
}

/// Hit-test and return the path to the topmost targetable node, as a sequence
/// of child indices from the root.
fn hit_test_path(root: &ObjectNode, x: i32, y: i32) -> Option<Vec<usize>> {
    hit_test_path_recursive(root, x, y, &mut Vec::new())
}

fn hit_test_path_recursive(
    node: &ObjectNode,
    x: i32,
    y: i32,
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    if node.is_hidden_or_detached() {
        return None;
    }
    for (i, child) in node.children.iter().enumerate().rev() {
        path.push(i);
        if let Some(p) = hit_test_path_recursive(child, x, y, path) {
            return Some(p);
        }
        path.pop();
    }
    if node.is_targetable_at(x, y) {
        Some(path.clone())
    } else {
        None
    }
}

/// Find the path to the focused node (read-only).
fn find_focused_path_ro(root: &ObjectNode) -> Option<Vec<usize>> {
    find_focused_ro_recursive(root, &mut Vec::new())
}

fn find_focused_ro_recursive(node: &ObjectNode, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if node.states().contains(ObjectStates::FOCUSED) {
        return Some(path.clone());
    }
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        if let Some(p) = find_focused_ro_recursive(child, path) {
            return Some(p);
        }
        path.pop();
    }
    None
}

/// Navigate to a node by structural path (immutable).
fn node_at_path<'a>(root: &'a ObjectNode, path: &[usize]) -> &'a ObjectNode {
    let mut current = root;
    for &idx in path {
        current = &current.children[idx];
    }
    current
}

/// Navigate to a node by structural path (mutable).
fn node_at_path_mut<'a>(root: &'a mut ObjectNode, path: &[usize]) -> &'a mut ObjectNode {
    let mut current = root;
    for &idx in path {
        current = &mut current.children[idx];
    }
    current
}

/// Invoke trickle handlers at the node at `node_path`.
fn invoke_trickle(
    root: &mut ObjectNode,
    node_path: &[usize],
    event: &ObjectEvent,
    ctx: EventContext,
) -> bool {
    let node = node_at_path_mut(root, node_path);
    for handler in &mut node.trickle_handlers {
        if handler(event, ctx) {
            return true;
        }
    }
    false
}

/// Invoke bubble handlers at the node at `node_path`.
fn invoke_bubble(
    root: &mut ObjectNode,
    node_path: &[usize],
    event: &ObjectEvent,
    ctx: EventContext,
) -> bool {
    let node = node_at_path_mut(root, node_path);
    for handler in &mut node.bubble_handlers {
        if handler(event, ctx) {
            return true;
        }
    }
    false
}

fn rect_contains(rect: Rect, x: i32, y: i32) -> bool {
    rect.width > 0
        && rect.height > 0
        && x >= rect.x
        && y >= rect.y
        && x < rect.x + rect.width
        && y < rect.y + rect.height
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
    use crate::renderer::Renderer;
    use crate::widget::{Color, Widget};

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct TestWidget {
        tag: &'static str,
        bounds: Rect,
        /// If set, handle_event returns true for this event type.
        consume_clicks: bool,
    }

    impl TestWidget {
        fn node(tag: &'static str, bounds: Rect) -> ObjectNode {
            ObjectNode::new(Rc::new(RefCell::new(Self {
                tag,
                bounds,
                consume_clicks: false,
            })))
            .with_tag(tag)
        }
    }

    impl Widget for TestWidget {
        fn bounds(&self) -> Rect {
            self.bounds
        }

        fn draw(&self, renderer: &mut dyn Renderer) {
            renderer.draw_text(
                (self.bounds.x, self.bounds.y),
                self.tag,
                Color(0, 0, 0, 255),
            );
        }

        fn handle_event(&mut self, event: &Event) -> bool {
            self.consume_clicks && matches!(event, Event::PressRelease { .. })
        }
    }

    struct TestRenderer {
        draws: Vec<&'static str>,
    }

    impl Renderer for TestRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}

        fn draw_text(&mut self, _position: (i32, i32), text: &str, _color: Color) {
            match text {
                "root" => self.draws.push("root"),
                "a" => self.draws.push("a"),
                "b" => self.draws.push("b"),
                "c" => self.draws.push("c"),
                "grand" => self.draws.push("grand"),
                _ => {}
            }
        }
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    // -----------------------------------------------------------------------
    // Existing LPAR-02 tests (must pass unchanged)
    // -----------------------------------------------------------------------

    #[test]
    fn flags_and_states_store_and_clear() {
        let mut node = TestWidget::node("root", rect(0, 0, 10, 10));

        node.set_flag(ObjectFlags::CLICKABLE, true);
        node.set_flag(ObjectFlags::FOCUSABLE, true);
        node.set_state(ObjectStates::FOCUSED, true);

        assert!(node.flags().contains(ObjectFlags::CLICKABLE));
        assert!(node.flags().contains(ObjectFlags::FOCUSABLE));
        assert!(node.states().contains(ObjectStates::FOCUSED));

        node.set_flag(ObjectFlags::FOCUSABLE, false);
        node.set_state(ObjectStates::FOCUSED, false);

        assert!(!node.flags().contains(ObjectFlags::FOCUSABLE));
        assert!(!node.states().contains(ObjectStates::FOCUSED));
    }

    #[test]
    fn draw_skips_hidden_subtree() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut hidden = TestWidget::node("a", rect(0, 0, 10, 10));
        hidden.set_flag(ObjectFlags::HIDDEN, true);
        hidden.append_child(TestWidget::node("grand", rect(0, 0, 10, 10)));
        root.append_child(hidden);
        root.append_child(TestWidget::node("b", rect(0, 0, 10, 10)));

        let mut renderer = TestRenderer { draws: Vec::new() };
        root.draw(&mut renderer);

        assert_eq!(renderer.draws, vec!["root", "b"]);
    }

    #[test]
    fn hit_test_returns_topmost_clickable_node() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut a = TestWidget::node("a", rect(0, 0, 20, 20));
        let mut b = TestWidget::node("b", rect(0, 0, 20, 20));
        a.set_flag(ObjectFlags::CLICKABLE, true);
        b.set_flag(ObjectFlags::CLICKABLE, true);
        root.append_child(a);
        root.append_child(b);

        assert_eq!(root.hit_test(5, 5).and_then(ObjectNode::tag), Some("b"));

        root.children_mut()[1].set_flag(ObjectFlags::DISABLED, true);
        assert_eq!(root.hit_test(5, 5).and_then(ObjectNode::tag), Some("a"));

        root.children_mut()[0].set_flag(ObjectFlags::HIDDEN, true);
        assert_eq!(root.hit_test(5, 5).and_then(ObjectNode::tag), None);
    }

    #[test]
    fn child_reorder_helpers_update_sibling_order() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        root.append_child(TestWidget::node("a", rect(0, 0, 10, 10)));
        root.append_child(TestWidget::node("b", rect(0, 0, 10, 10)));
        root.append_child(TestWidget::node("c", rect(0, 0, 10, 10)));

        assert!(root.raise_child(0));
        assert_eq!(child_tags(&root), vec!["b", "c", "a"]);

        assert!(root.lower_child(1));
        assert_eq!(child_tags(&root), vec!["c", "b", "a"]);

        assert!(root.move_child_before(2, 1));
        assert_eq!(child_tags(&root), vec!["c", "a", "b"]);

        assert!(root.move_child_after(0, 2));
        assert_eq!(child_tags(&root), vec!["a", "b", "c"]);

        assert!(!root.raise_child(3));
        assert!(!root.move_child_before(0, 3));
    }

    #[test]
    fn detach_child_marks_returned_subtree_detached() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut child = TestWidget::node("a", rect(0, 0, 10, 10));
        child.append_child(TestWidget::node("grand", rect(0, 0, 10, 10)));
        root.append_child(child);

        let detached = root.detach_child(0).expect("child is present");

        assert!(root.children().is_empty());
        assert!(detached.is_detached());
        assert!(detached.children()[0].is_detached());
    }

    #[test]
    fn adopt_preserves_widget_node_shape() {
        let mut widget_root = crate::WidgetNode::new(Rc::new(RefCell::new(TestWidget {
            tag: "root",
            bounds: rect(0, 0, 100, 100),
            consume_clicks: false,
        })))
        .with_tag("root");
        widget_root.children.push(
            crate::WidgetNode::new(Rc::new(RefCell::new(TestWidget {
                tag: "a",
                bounds: rect(0, 0, 10, 10),
                consume_clicks: false,
            })))
            .with_tag("a"),
        );

        let object_root = ObjectNode::adopt(widget_root);

        assert_eq!(object_root.tag(), Some("root"));
        assert_eq!(object_root.children()[0].tag(), Some("a"));
        assert_eq!(object_root.flags(), ObjectFlags::EMPTY);
        assert_eq!(object_root.children()[0].states(), ObjectStates::DEFAULT);
    }

    #[test]
    fn visible_subtree_extent_unions_children() {
        let mut root = TestWidget::node("root", rect(10, 10, 20, 20));
        root.append_child(TestWidget::node("a", rect(40, 5, 10, 10)));
        root.append_child(TestWidget::node("b", rect(0, 30, 10, 10)));

        assert_eq!(root.visible_subtree_extent(), Some(rect(0, 5, 50, 35)));
    }

    #[test]
    fn visible_subtree_extent_excludes_hidden_child() {
        let mut root = TestWidget::node("root", rect(0, 0, 10, 10));
        let mut hidden = TestWidget::node("a", rect(50, 50, 10, 10));
        hidden.set_flag(ObjectFlags::HIDDEN, true);
        root.append_child(hidden);

        assert_eq!(root.visible_subtree_extent(), Some(rect(0, 0, 10, 10)));
    }

    #[test]
    fn visible_subtree_extent_skips_zero_area_bounds() {
        let mut root = TestWidget::node("root", rect(0, 0, 0, 0));
        root.append_child(TestWidget::node("a", rect(5, 5, 10, 10)));

        assert_eq!(root.visible_subtree_extent(), Some(rect(5, 5, 10, 10)));
    }

    #[test]
    fn visible_subtree_extent_is_none_for_hidden_or_detached_root() {
        let mut hidden = TestWidget::node("a", rect(0, 0, 10, 10));
        hidden.set_flag(ObjectFlags::HIDDEN, true);
        assert_eq!(hidden.visible_subtree_extent(), None);

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        root.append_child(TestWidget::node("b", rect(0, 0, 10, 10)));
        let detached = root.detach_child(0).expect("child is present");
        assert_eq!(detached.visible_subtree_extent(), None);
    }

    #[test]
    fn hide_flow_computes_extent_before_hiding() {
        use crate::invalidation::{InvalidationList, PresentPlan};

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut panel = TestWidget::node("a", rect(10, 10, 20, 20));
        panel.append_child(TestWidget::node("grand", rect(25, 25, 20, 20)));
        root.append_child(panel);

        let extent = root.children()[0].visible_subtree_extent();
        assert_eq!(extent, Some(rect(10, 10, 35, 35)));
        root.children_mut()[0].set_flag(ObjectFlags::HIDDEN, true);
        assert_eq!(root.children()[0].visible_subtree_extent(), None);

        let mut list = InvalidationList::<4>::new(rect(0, 0, 100, 100));
        list.push_opt(extent);
        assert_eq!(list.plan(), PresentPlan::Rects(&[rect(10, 10, 35, 35)]));
    }

    #[test]
    fn detach_flow_computes_extent_before_detaching() {
        use crate::invalidation::{InvalidationList, PresentPlan};

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut panel = TestWidget::node("a", rect(60, 60, 30, 30));
        panel.append_child(TestWidget::node("grand", rect(80, 80, 30, 30)));
        root.append_child(panel);

        let extent = root.children()[0]
            .visible_subtree_extent()
            .expect("visible subtree has an extent");
        assert_eq!(extent, rect(60, 60, 50, 50));
        let detached = root.detach_child(0).expect("child is present");
        assert_eq!(detached.visible_subtree_extent(), None);

        let mut list = InvalidationList::<4>::new(rect(0, 0, 100, 100));
        list.push(extent);
        assert_eq!(list.plan(), PresentPlan::Rects(&[rect(60, 60, 40, 40)]));
    }

    // -----------------------------------------------------------------------
    // LPAR-04: ObjectEvent tests
    // -----------------------------------------------------------------------

    /// Build a clickable node in a 10x10 box at position (x,y).
    fn clickable(tag: &'static str, x: i32, y: i32) -> ObjectNode {
        let mut n = TestWidget::node(tag, rect(x, y, 10, 10));
        n.set_flag(ObjectFlags::CLICKABLE, true);
        n
    }

    // -----------------------------------------------------------------------
    // LPAR-04: dispatch phase order trickle → target → bubble
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_phase_order_trickle_target_bubble() {
        let visit_log: Rc<RefCell<Vec<(&'static str, &'static str)>>> =
            Rc::new(RefCell::new(Vec::new()));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));

        // Child node "a" at (0,0) is the click target.
        let mut a = clickable("a", 0, 0);
        // "a" opts into bubbling.
        a.set_flag(ObjectFlags::EVENT_BUBBLE, true);
        // Append child BEFORE registering handlers so the Attached lifecycle
        // event does not pollute the visit log.
        root.append_child(a);

        // Register handlers after append so lifecycle emission is isolated.

        // Root carries a trickle handler (records "root/trickle").
        {
            let log = visit_log.clone();
            root.add_trickle_handler(move |_ev, ctx| {
                log.borrow_mut()
                    .push((ctx.current_tag.unwrap_or("?"), "trickle"));
                false
            });
        }
        // Root also carries a bubble handler.
        {
            let log = visit_log.clone();
            root.add_bubble_handler(move |_ev, ctx| {
                log.borrow_mut()
                    .push((ctx.current_tag.unwrap_or("?"), "bubble"));
                false
            });
        }
        // "a" has a target handler.
        {
            let log = visit_log.clone();
            root.children_mut()[0].add_target_handler(move |_ev, ctx| {
                log.borrow_mut()
                    .push((ctx.current_tag.unwrap_or("?"), "target"));
                false
            });
        }

        let ev = DispatchInput::PointerObject {
            x: 5,
            y: 5,
            event: ObjectEvent::Clicked { x: 5, y: 5 },
        };
        let result = dispatch_object_event(&mut root, ev);

        assert_eq!(result, Disposition::Unconsumed);
        // Expected visit order: root/trickle → a/target → root/bubble
        let log = visit_log.borrow();
        assert_eq!(
            *log,
            vec![("root", "trickle"), ("a", "target"), ("root", "bubble"),]
        );
    }

    // -----------------------------------------------------------------------
    // LPAR-04: stop-on-consume halts remaining phases
    // -----------------------------------------------------------------------

    #[test]
    fn stop_on_consume_at_target_prevents_bubble() {
        let bubble_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        {
            let called = bubble_called.clone();
            root.add_bubble_handler(move |_ev, _ctx| {
                *called.borrow_mut() = true;
                false
            });
        }

        let mut a = clickable("a", 0, 0);
        // Consuming target handler.
        a.add_target_handler(|_ev, _ctx| true);
        a.set_flag(ObjectFlags::EVENT_BUBBLE, true);
        root.append_child(a);

        let result = dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert_eq!(result, Disposition::Consumed);
        assert!(
            !*bubble_called.borrow(),
            "bubble must not run after consume"
        );
    }

    #[test]
    fn stop_on_consume_at_trickle_prevents_target_and_bubble() {
        let target_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let bubble_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));

        // Append child first, then register handlers so lifecycle emission
        // during append_child does not set the flags prematurely.
        let mut a = clickable("a", 0, 0);
        a.set_flag(ObjectFlags::EVENT_BUBBLE, true);
        root.append_child(a);

        // Consuming trickle handler at root (registered after child is in tree).
        root.add_trickle_handler(|_ev, _ctx| true);

        {
            let tc = target_called.clone();
            root.children_mut()[0].add_target_handler(move |_ev, _ctx| {
                *tc.borrow_mut() = true;
                false
            });
        }
        {
            let bc = bubble_called.clone();
            root.add_bubble_handler(move |_ev, _ctx| {
                *bc.borrow_mut() = true;
                false
            });
        }

        let result = dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert_eq!(result, Disposition::Consumed);
        assert!(!*target_called.borrow());
        assert!(!*bubble_called.borrow());
    }

    // -----------------------------------------------------------------------
    // LPAR-04: EVENT_BUBBLE gating
    // -----------------------------------------------------------------------

    #[test]
    fn bubble_only_when_event_bubble_flag_set() {
        let bubble_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        {
            let called = bubble_called.clone();
            root.add_bubble_handler(move |_ev, _ctx| {
                *called.borrow_mut() = true;
                false
            });
        }

        // "a" does NOT have EVENT_BUBBLE.
        let a = clickable("a", 0, 0);
        root.append_child(a);

        dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert!(
            !*bubble_called.borrow(),
            "bubble must not run without EVENT_BUBBLE"
        );
    }

    #[test]
    fn bubble_runs_when_event_bubble_flag_set() {
        let bubble_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        {
            let called = bubble_called.clone();
            root.add_bubble_handler(move |_ev, _ctx| {
                *called.borrow_mut() = true;
                false
            });
        }

        let mut a = clickable("a", 0, 0);
        a.set_flag(ObjectFlags::EVENT_BUBBLE, true);
        root.append_child(a);

        dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert!(*bubble_called.borrow(), "bubble must run with EVENT_BUBBLE");
    }

    #[test]
    fn bubble_stops_at_ancestor_without_event_bubble_flag() {
        // root → b → a(target). a opts into bubbling, b does NOT. The event
        // must reach b's bubble handler but stop there — never reaching root.
        let b_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let root_called: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut b = TestWidget::node("b", rect(0, 0, 20, 20));
        let mut a = clickable("a", 0, 0);
        a.set_flag(ObjectFlags::EVENT_BUBBLE, true);
        b.append_child(a);
        // b deliberately has NO EVENT_BUBBLE flag.
        root.append_child(b);

        // Register handlers after the tree is built so lifecycle emission
        // during append does not run them.
        {
            let called = b_called.clone();
            root.children_mut()[0].add_bubble_handler(move |_ev, _ctx| {
                *called.borrow_mut() = true;
                false
            });
        }
        {
            let called = root_called.clone();
            root.add_bubble_handler(move |_ev, _ctx| {
                *called.borrow_mut() = true;
                false
            });
        }

        dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert!(
            *b_called.borrow(),
            "bubble must reach the flagged target's parent"
        );
        assert!(
            !*root_called.borrow(),
            "bubble must stop at an ancestor that lacks EVENT_BUBBLE"
        );
    }

    #[test]
    fn reorder_emits_child_changed_to_parent() {
        let count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        root.append_child(TestWidget::node("a", rect(0, 0, 10, 10)));
        root.append_child(TestWidget::node("b", rect(0, 0, 10, 10)));
        root.append_child(TestWidget::node("c", rect(0, 0, 10, 10)));

        // Registered after appends, so only reorders are counted.
        {
            let c = count.clone();
            root.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::ChildChanged) {
                    *c.borrow_mut() += 1;
                }
                false
            });
        }

        assert!(root.raise_child(0));
        assert!(root.lower_child(1));
        assert!(root.move_child_before(2, 0));
        assert!(root.move_child_after(0, 2));
        assert_eq!(
            *count.borrow(),
            4,
            "each effective reorder emits ChildChanged"
        );

        // A no-op move emits nothing.
        assert!(root.move_child_before(1, 1));
        assert_eq!(
            *count.borrow(),
            4,
            "a no-op move must not emit ChildChanged"
        );
    }

    // -----------------------------------------------------------------------
    // LPAR-04: pointer target resolution via hit_test
    // -----------------------------------------------------------------------

    #[test]
    fn pointer_target_resolved_via_hit_test() {
        let target_seen: Rc<RefCell<Option<&'static str>>> = Rc::new(RefCell::new(None));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));

        let mut a = clickable("a", 0, 0);
        {
            let ts = target_seen.clone();
            a.add_target_handler(move |_ev, ctx| {
                *ts.borrow_mut() = ctx.target_tag;
                false
            });
        }
        root.append_child(a);

        dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 5,
                y: 5,
                event: ObjectEvent::Clicked { x: 5, y: 5 },
            },
        );

        assert_eq!(*target_seen.borrow(), Some("a"));
    }

    #[test]
    fn pointer_no_target_returns_no_target() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        // root has no CLICKABLE children; nothing to hit at (50,50).
        let result = dispatch_object_event(
            &mut root,
            DispatchInput::PointerObject {
                x: 50,
                y: 50,
                event: ObjectEvent::Clicked { x: 50, y: 50 },
            },
        );
        assert_eq!(result, Disposition::NoTarget);
    }

    // -----------------------------------------------------------------------
    // LPAR-04: key/encoder via focus
    // -----------------------------------------------------------------------

    #[test]
    fn key_event_resolves_to_focused_node() {
        let key_seen: Rc<RefCell<Option<&'static str>>> = Rc::new(RefCell::new(None));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut a = TestWidget::node("a", rect(0, 0, 10, 10));
        a.set_flag(ObjectFlags::FOCUSABLE, true);
        {
            let ks = key_seen.clone();
            a.add_target_handler(move |_ev, ctx| {
                *ks.borrow_mut() = ctx.target_tag;
                false
            });
        }
        root.append_child(a);

        // Manually give "a" focus.
        root.children_mut()[0].set_state(ObjectStates::FOCUSED, true);

        let result = dispatch_object_event(
            &mut root,
            DispatchInput::Focused {
                event: ObjectEvent::Key(crate::event::Key::Enter),
            },
        );

        assert_eq!(result, Disposition::Unconsumed);
        assert_eq!(*key_seen.borrow(), Some("a"));
    }

    #[test]
    fn key_event_no_focused_returns_no_target() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        root.append_child(TestWidget::node("a", rect(0, 0, 10, 10)));

        let result = dispatch_object_event(
            &mut root,
            DispatchInput::Focused {
                event: ObjectEvent::Key(crate::event::Key::Enter),
            },
        );
        assert_eq!(result, Disposition::NoTarget);
    }

    // -----------------------------------------------------------------------
    // LPAR-04: Pointer stream event mapping
    // -----------------------------------------------------------------------

    #[test]
    fn stream_press_release_maps_to_clicked() {
        let ev_seen: Rc<RefCell<Option<ObjectEvent>>> = Rc::new(RefCell::new(None));

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut a = clickable("a", 0, 0);
        {
            let es = ev_seen.clone();
            a.add_target_handler(move |ev, _ctx| {
                *es.borrow_mut() = Some(ev.clone());
                false
            });
        }
        root.append_child(a);

        dispatch_object_event(
            &mut root,
            DispatchInput::Pointer {
                x: 5,
                y: 5,
                event: Event::PressRelease { x: 5, y: 5 },
            },
        );

        assert_eq!(*ev_seen.borrow(), Some(ObjectEvent::Clicked { x: 5, y: 5 }));
    }

    #[test]
    fn stream_tick_produces_no_target() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        root.append_child(clickable("a", 0, 0));

        // Tick has no pointer-coordinate mapping; should return NoTarget.
        let result = dispatch_object_event(
            &mut root,
            DispatchInput::Pointer {
                x: 5,
                y: 5,
                event: Event::Tick,
            },
        );
        assert_eq!(result, Disposition::NoTarget);
    }

    // -----------------------------------------------------------------------
    // LPAR-04: Widget::handle_event at target phase
    // -----------------------------------------------------------------------

    #[test]
    fn widget_handle_event_at_target_phase_can_consume() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        // Build a node whose widget returns true for PressRelease.
        let widget = Rc::new(RefCell::new(TestWidget {
            tag: "a",
            bounds: rect(0, 0, 10, 10),
            consume_clicks: true,
        }));
        let mut a = ObjectNode::new(widget).with_tag("a");
        a.set_flag(ObjectFlags::CLICKABLE, true);
        root.append_child(a);

        let result = dispatch_object_event(
            &mut root,
            DispatchInput::Pointer {
                x: 5,
                y: 5,
                event: Event::PressRelease { x: 5, y: 5 },
            },
        );
        assert_eq!(result, Disposition::Consumed);
    }

    // -----------------------------------------------------------------------
    // LPAR-04: Lifecycle events
    // -----------------------------------------------------------------------

    #[test]
    fn append_child_emits_attached_and_child_changed() {
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let mut parent = TestWidget::node("parent", rect(0, 0, 100, 100));
        {
            let l = log.clone();
            parent.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::ChildChanged) {
                    l.borrow_mut().push("parent:child_changed");
                }
                false
            });
        }

        let mut child = TestWidget::node("child", rect(0, 0, 10, 10));
        {
            let l = log.clone();
            child.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::Attached) {
                    l.borrow_mut().push("child:attached");
                }
                false
            });
        }

        parent.append_child(child);

        assert_eq!(
            *log.borrow(),
            vec!["child:attached", "parent:child_changed"]
        );
    }

    #[test]
    fn insert_child_emits_attached_and_child_changed() {
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

        let mut parent = TestWidget::node("parent", rect(0, 0, 100, 100));
        {
            let l = log.clone();
            parent.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::ChildChanged) {
                    l.borrow_mut().push("parent:child_changed");
                }
                false
            });
        }

        let mut child = TestWidget::node("child", rect(0, 0, 10, 10));
        {
            let l = log.clone();
            child.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::Attached) {
                    l.borrow_mut().push("child:attached");
                }
                false
            });
        }

        let ok = parent.insert_child(0, child);
        assert!(ok);
        assert_eq!(
            *log.borrow(),
            vec!["child:attached", "parent:child_changed"]
        );
    }

    #[test]
    fn detach_child_emits_detached_and_child_changed() {
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let mut parent = TestWidget::node("parent", rect(0, 0, 100, 100));
        {
            let l = log.clone();
            parent.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::ChildChanged) {
                    l.borrow_mut().push("parent:child_changed");
                }
                false
            });
        }

        let mut child = TestWidget::node("child", rect(0, 0, 10, 10));
        {
            let l = log.clone();
            child.add_target_handler(move |ev, _ctx| {
                if matches!(ev, ObjectEvent::Detached) {
                    l.borrow_mut().push("child:detached");
                }
                false
            });
        }

        parent.append_child(child);
        log.borrow_mut().clear(); // clear the Attached/ChildChanged from append.

        let detached = parent.detach_child(0).expect("has child");
        drop(detached);

        assert_eq!(
            *log.borrow(),
            vec!["child:detached", "parent:child_changed"]
        );
    }

    #[test]
    fn detach_child_still_marks_subtree_detached() {
        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut child = TestWidget::node("a", rect(0, 0, 10, 10));
        child.append_child(TestWidget::node("grand", rect(0, 0, 10, 10)));
        root.append_child(child);

        let detached = root.detach_child(0).expect("child is present");

        assert!(root.children().is_empty());
        assert!(detached.is_detached());
        assert!(detached.children()[0].is_detached());
    }

    // -----------------------------------------------------------------------
    // LPAR-04: tag-based focus targeting must not exist
    // -----------------------------------------------------------------------

    #[test]
    fn focus_path_not_tag_based() {
        // Verify that focus_path takes a &[usize] structural path, not a tag.
        // This test documents the API shape; it would fail to compile if the
        // wrong API were provided.
        use crate::focus::FocusGroup;

        let mut root = TestWidget::node("root", rect(0, 0, 100, 100));
        let mut a = TestWidget::node("a", rect(0, 0, 10, 10));
        a.set_flag(ObjectFlags::FOCUSABLE, true);
        root.append_child(a);

        let fg = FocusGroup::new();
        // Structural path [0] → child 0 of root ("a").
        let ok = fg.focus_path(&mut root, &[0usize]);
        assert!(ok);
        assert!(root.children()[0].states().contains(ObjectStates::FOCUSED));
    }

    fn child_tags(root: &ObjectNode) -> Vec<&'static str> {
        root.children().iter().filter_map(ObjectNode::tag).collect()
    }
}
