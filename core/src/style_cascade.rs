//! LPAR-07 style cascade substrate: `Part`, `Selector`, `StylePatch`,
//! `StyleState`, `InheritedContext`, and top-down cascade resolution.
//!
//! This module implements the LPAR-07 style cascade layer that resolves
//! `(node, part, state)` queries into a [`Style`] value for draw-time
//! consumption. It sits *above* [`Style`] — the cascade output is a
//! [`Style`] value assembled from per-property winner lookups, and the
//! draw path continues to use [`Style`] unchanged.
//!
//! # Cascade precedence (§7.2)
//!
//! For a query `(node, part, property)`:
//!
//! 0. **Transition override** (Tier 0) — highest precedence, set during
//!    an active style transition via [`start_transition`].
//! 1. **MPY local styles** — one sparse patch per exact selector.
//! 2. **Native local styles** — last-added wins among matching selectors.
//! 3. **Added (shared) styles** — last-added wins among matching selectors.
//! 4. **Theme styles** — last-added wins among matching selectors.
//! 5. For inheritable properties (`alpha` plus LPAR-08 text properties), take
//!    the value from the [`InheritedContext`] propagated top-down from
//!    ancestors.
//! 6. Property default value (from [`Style::default()`]).
//!
//! # Top-down inheritance (§7.3)
//!
//! Inheritance is propagated top-down during the resolve/draw traversal.
//! Each parent node resolves its own `alpha` and [`TextStyle`] for `MAIN`; the
//! resolved values become the [`InheritedContext`] handed to that node's
//! children. No parent pointer, no ancestor slice — the caller threads the
//! context down.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::anim::{ANIM_SCALE, Easing, Tween};
use crate::font::FontId;
use crate::object::ObjectStates;
use crate::object_anim::{ObjectAnimId, ObjectAnims};
use crate::style::Style;
use crate::widget::{Color, Rect};

// ---------------------------------------------------------------------------
// TransitionOverride — Tier-0 override slot for active style transitions
// ---------------------------------------------------------------------------

/// Live interpolated values for properties currently undergoing a style
/// transition (LPAR-07 §8).
///
/// Fields are `Some` while the corresponding property is being animated and
/// `None` once the animation completes or is cancelled. The override is stored
/// inside an `Rc<RefCell<TransitionOverride>>` on [`StyleState`] so that
/// animation apply closures captured during [`start_transition`] can write into
/// it without holding a borrow on the owning [`crate::object::ObjectNode`].
#[derive(Debug, Default, Clone)]
pub struct TransitionOverride {
    /// Overridden background color during a transition.
    pub bg_color: Option<Color>,
    /// Overridden border color during a transition.
    pub border_color: Option<Color>,
    /// Overridden border width during a transition.
    pub border_width: Option<u8>,
    /// Overridden alpha during a transition.
    pub alpha: Option<u8>,
    /// Overridden corner radius during a transition.
    pub radius: Option<u8>,
}

// ---------------------------------------------------------------------------
// AnimProp / AnimPropValue
// ---------------------------------------------------------------------------

/// Identifies which style property a transition animates (LPAR-07 §8.1).
///
/// Registration policy: **Specification Required** — adding a variant requires
/// a phase-doc amendment to the §8.1 table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimProp {
    /// The background color (`Style::bg_color`).
    BgColor,
    /// The border color (`Style::border_color`).
    BorderColor,
    /// The border width in pixels (`Style::border_width`).
    BorderWidth,
    /// The alpha / opacity (`Style::alpha`).
    Alpha,
    /// The corner radius in pixels (`Style::radius`).
    Radius,
}

/// Value of an animatable style property for transition endpoints (LPAR-07 §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimPropValue {
    /// A color-typed property value.
    Color(Color),
    /// A scalar property value (border width, alpha, or radius).
    Scalar(u8),
}

// ---------------------------------------------------------------------------
// TransitionDesc
// ---------------------------------------------------------------------------

/// Parameters describing a style transition animation (LPAR-07 §8.2).
#[derive(Debug, Clone, Copy)]
pub struct TransitionDesc {
    /// Duration of the transition in ticks.
    ///
    /// A value of `0` snaps the property immediately to the `to` value.
    pub duration_ticks: u32,
    /// Delay before the transition begins (ticks).
    pub delay_ticks: u32,
    /// Easing curve applied to the progress value.
    pub easing: Easing,
}

// ---------------------------------------------------------------------------
// Part
// ---------------------------------------------------------------------------

/// Sub-region or visual component of a widget used as a style selector key.
///
/// `Part` is a transparent newtype over `u32`. Named constants mirror LVGL's
/// `lv_part_t` vocabulary. Custom part ids are allocated in the `id >= 8`
/// range via [`Part::custom`].
///
/// Registration policy: **Specification Required**. Adding a named constant
/// requires a phase-doc amendment that updates the §6.1 table and cites the
/// owning widget phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Part(pub u32);

impl Part {
    /// The main body of the widget (`LV_PART_MAIN = 0`).
    pub const MAIN: Self = Self(0);
    /// The scroll-bar of a scrollable container (`LV_PART_SCROLLBAR = 1`).
    pub const SCROLLBAR: Self = Self(1);
    /// The filled portion of a slider or progress bar (`LV_PART_INDICATOR = 2`).
    pub const INDICATOR: Self = Self(2);
    /// The draggable thumb of a slider or knob widget (`LV_PART_KNOB = 3`).
    pub const KNOB: Self = Self(3);
    /// The currently selected item in a list or similar widget (`LV_PART_SELECTED = 4`).
    pub const SELECTED: Self = Self(4);
    /// Individual items inside a list or roller (`LV_PART_ITEMS = 5`).
    pub const ITEMS: Self = Self(5);
    /// The text-input cursor (`LV_PART_CURSOR = 6`).
    pub const CURSOR: Self = Self(6);

    /// Construct a custom part id.
    ///
    /// By convention `id >= 8` to avoid collisions with the named constants
    /// above (mirroring `LV_PART_CUSTOM_FIRST`). Widget phases use this to
    /// reserve widget-local part ids without a Specification Required amendment.
    ///
    /// # Example
    ///
    /// ```
    /// use rlvgl_core::style_cascade::Part;
    /// const MY_PART: Part = Part::custom(8);
    /// ```
    pub const fn custom(id: u32) -> Self {
        Self(id)
    }
}

// ---------------------------------------------------------------------------
// TextAlign / TextStyle
// ---------------------------------------------------------------------------

/// Horizontal text alignment resolved by the style cascade.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// Align text to the left edge.
    #[default]
    Left,
    /// Align text to the horizontal center.
    Center,
    /// Align text to the right edge.
    Right,
    /// Resolve automatically from paragraph direction; v1 maps this to left.
    Auto,
}

/// Fully resolved inheritable text properties.
///
/// This is the LPAR-08 text companion to the existing visual [`Style`].
/// Keeping it separate preserves the frozen public-field shape of [`Style`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    /// Text color.
    pub text_color: Color,
    /// Font registry identifier resolved by the display/platform at draw time.
    pub font_id: FontId,
    /// Extra pixels inserted between shaped glyphs.
    pub letter_spacing: i8,
    /// Extra pixels inserted between wrapped lines.
    pub line_spacing: i8,
    /// Horizontal text alignment.
    pub text_align: TextAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            text_color: Color(0, 0, 0, 255),
            font_id: FontId::DEFAULT,
            letter_spacing: 0,
            line_spacing: 0,
            text_align: TextAlign::Left,
        }
    }
}

// ---------------------------------------------------------------------------
// Selector
// ---------------------------------------------------------------------------

/// A `(Part, ObjectStates mask)` selector stored alongside a style patch.
///
/// A selector *matches* a query `(part, node_states)` when:
///
/// - `self.part == part`, **and**
/// - `(node_states & self.states) == self.states` — i.e. every state bit in
///   the selector is set on the node.
///
/// A selector constructed with [`ObjectStates::DEFAULT`] (bits == 0) as the
/// state mask matches **any** node regardless of which state bits are set
/// (§6.3, the DEFAULT-matches-any rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selector {
    /// The widget part this selector targets.
    pub part: Part,
    /// The required state bits. Zero (`DEFAULT`) matches any state.
    pub states: ObjectStates,
}

impl Selector {
    /// Create a selector for `part` that matches **any** state (DEFAULT mask).
    ///
    /// Use this as a base/always-on style entry for a given part.
    pub const fn part(part: Part) -> Self {
        Self {
            part,
            states: ObjectStates::DEFAULT,
        }
    }

    /// Create a selector for a specific `(part, states)` combination.
    pub const fn new(part: Part, states: ObjectStates) -> Self {
        Self { part, states }
    }

    /// Return `true` when this selector matches the given `(part, node_states)` query.
    ///
    /// Matching rules (§6.3):
    /// - parts must be equal.
    /// - every bit in `self.states` must be set in `node_states`; a zero mask
    ///   (`DEFAULT`) satisfies this for any `node_states`.
    pub const fn matches(&self, part: Part, node_states: ObjectStates) -> bool {
        self.part.0 == part.0 && node_states.contains(self.states)
    }
}

// ---------------------------------------------------------------------------
// StylePatch
// ---------------------------------------------------------------------------

/// A *partial* style entry: a per-property override record added to a node.
///
/// Each field is an `Option` — only fields that carry a `Some` value override
/// the lower-precedence or default value. This is the LVGL `lv_style_t`
/// analogue: a patch carries only the properties it intends to change.
///
/// Use [`StylePatch::builder`] (or set fields directly) to construct patches.
/// The cascade merges patches per-property — a patch that sets only `bg_color`
/// does not affect `border_color`, `alpha`, etc.
///
/// # Distinction from [`Style`]
///
/// [`Style`] (in `core::style`) is the *resolved*, fully-materialised property
/// bag used at draw time. `StylePatch` is the sparse "intent" stored in the
/// cascade; it is resolved into a [`Style`] by [`resolve`].
///
/// # LPAR-10 layout fields (§5.G)
///
/// The `padding_*`, `margin_*`, `gap_row`, and `gap_col` fields are additive
/// extensions for layout-related style properties.  They are resolved into a
/// [`crate::layout::LayoutStyle`] resolved struct by
/// [`crate::layout::resolve_layout_style`].  The frozen 5-field
/// [`crate::style::Style`] is **NOT** extended by LPAR-10.
///
/// Registration policy for new named layout properties: **Standards Action**
/// (cross-phase style contracts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StylePatch {
    /// Override for `Style::bg_color`.
    pub bg_color: Option<Color>,
    /// Override for `Style::border_color`.
    pub border_color: Option<Color>,
    /// Override for `Style::border_width`.
    pub border_width: Option<u8>,
    /// Override for `Style::alpha`.
    ///
    /// `alpha` is the one **inheritable** property in v1 (§7.3). When no
    /// patch in the cascade supplies a value, the resolved value falls back to
    /// the [`InheritedContext`] handed down from the parent node.
    pub alpha: Option<u8>,
    /// Override for `Style::radius`.
    pub radius: Option<u8>,
    /// Override for [`TextStyle::text_color`].
    pub text_color: Option<Color>,
    /// Override for [`TextStyle::font_id`].
    pub font_id: Option<FontId>,
    /// Override for [`TextStyle::letter_spacing`].
    pub letter_spacing: Option<i8>,
    /// Override for [`TextStyle::line_spacing`].
    pub line_spacing: Option<i8>,
    /// Override for [`TextStyle::text_align`].
    pub text_align: Option<TextAlign>,

    // --- LPAR-10 layout properties (§5.G) ---
    /// Top padding in pixels (resolves to `0` when `None`).
    pub padding_top: Option<i32>,
    /// Bottom padding in pixels (resolves to `0` when `None`).
    pub padding_bottom: Option<i32>,
    /// Left padding in pixels (resolves to `0` when `None`).
    pub padding_left: Option<i32>,
    /// Right padding in pixels (resolves to `0` when `None`).
    pub padding_right: Option<i32>,
    /// Top margin in pixels (resolves to `0` when `None`).
    pub margin_top: Option<i32>,
    /// Bottom margin in pixels (resolves to `0` when `None`).
    pub margin_bottom: Option<i32>,
    /// Left margin in pixels (resolves to `0` when `None`).
    pub margin_left: Option<i32>,
    /// Right margin in pixels (resolves to `0` when `None`).
    pub margin_right: Option<i32>,
    /// Row gap (spacing between rows / cross-axis tracks) in pixels.
    pub gap_row: Option<i32>,
    /// Column gap (spacing between columns / main-axis items) in pixels.
    pub gap_col: Option<i32>,
}

/// One of the twenty globally registered MPY local-style properties.
///
/// The stable numeric identifiers live in the MPY actor registry rather than
/// in this enum's Rust discriminants. This enum is the typed storage key used
/// after descriptor validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleProperty {
    /// Background color.
    BgColor,
    /// Border color.
    BorderColor,
    /// Border width.
    BorderWidth,
    /// Overall alpha.
    Alpha,
    /// Corner radius.
    Radius,
    /// Text color.
    TextColor,
    /// Font registry identifier.
    FontId,
    /// Inter-glyph spacing.
    LetterSpacing,
    /// Inter-line spacing.
    LineSpacing,
    /// Horizontal text alignment.
    TextAlign,
    /// Top padding.
    PaddingTop,
    /// Bottom padding.
    PaddingBottom,
    /// Left padding.
    PaddingLeft,
    /// Right padding.
    PaddingRight,
    /// Top margin.
    MarginTop,
    /// Bottom margin.
    MarginBottom,
    /// Left margin.
    MarginLeft,
    /// Right margin.
    MarginRight,
    /// Row gap.
    GapRow,
    /// Column gap.
    GapCol,
}

/// Typed value accepted by the local-style storage prerequisite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylePropertyValue {
    /// ARGB-converted native color.
    Color(Color),
    /// Unsigned scalar.
    U32(u32),
    /// Signed scalar.
    I32(i32),
    /// Registered text alignment.
    TextAlign(TextAlign),
}

/// Exact mutation of one MPY-owned property at one selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpyStyleUpdate {
    /// Set or replace the property.
    Set(StylePropertyValue),
    /// Remove only the MPY-owned property.
    Remove,
}

/// Failure while preparing or committing MPY-owned local-style storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpyStyleStorageError {
    /// The typed value does not match the selected property.
    TypeMismatch,
    /// The scalar is outside the native field's exact range.
    Range,
    /// Required retained storage could not be reserved.
    Capacity,
    /// The style storage changed after preparation.
    Stale,
    /// The private storage revision cannot advance without wrapping.
    RevisionExhausted,
}

/// Fully prepared exact MPY local-style update.
///
/// All vector growth and patch conversion occur before this value is returned.
/// The value intentionally exposes no mutable storage.
#[derive(Debug)]
pub struct PreparedMpyStyleMutation {
    owner: Rc<RefCell<TransitionOverride>>,
    expected_revision: u64,
    next_revision: u64,
    changed: bool,
    next_local: Vec<(Selector, StylePatch)>,
}

/// Successful MPY local-style storage commit awaiting explicit release.
#[derive(Debug)]
pub struct CommittedMpyStyleMutation {
    committed_revision: u64,
    retired: PreparedMpyStyleMutation,
}

/// Owning stale-commit error for one prepared MPY local-style update.
pub struct MpyStyleCommitError {
    cause: MpyStyleStorageError,
    prepared: PreparedMpyStyleMutation,
}

impl MpyStyleCommitError {
    /// Return the rejection cause.
    pub const fn cause(&self) -> MpyStyleStorageError {
        self.cause
    }

    /// Recover the prepared update for retry or deferred release.
    pub fn into_prepared(self) -> PreparedMpyStyleMutation {
        self.prepared
    }
}

impl core::fmt::Debug for MpyStyleCommitError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MpyStyleCommitError")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}

impl StylePatch {
    /// Return a new empty patch (all fields `None`).
    pub const fn new() -> Self {
        Self {
            bg_color: None,
            border_color: None,
            border_width: None,
            alpha: None,
            radius: None,
            text_color: None,
            font_id: None,
            letter_spacing: None,
            line_spacing: None,
            text_align: None,
            padding_top: None,
            padding_bottom: None,
            padding_left: None,
            padding_right: None,
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            gap_row: None,
            gap_col: None,
        }
    }

    /// Return a builder for constructing a patch via method chaining.
    pub fn builder() -> StylePatchBuilder {
        StylePatchBuilder(Self::new())
    }

    /// Return whether this patch contains no property override.
    pub const fn is_empty(&self) -> bool {
        self.bg_color.is_none()
            && self.border_color.is_none()
            && self.border_width.is_none()
            && self.alpha.is_none()
            && self.radius.is_none()
            && self.text_color.is_none()
            && self.font_id.is_none()
            && self.letter_spacing.is_none()
            && self.line_spacing.is_none()
            && self.text_align.is_none()
            && self.padding_top.is_none()
            && self.padding_bottom.is_none()
            && self.padding_left.is_none()
            && self.padding_right.is_none()
            && self.margin_top.is_none()
            && self.margin_bottom.is_none()
            && self.margin_left.is_none()
            && self.margin_right.is_none()
            && self.gap_row.is_none()
            && self.gap_col.is_none()
    }

    /// Read one typed property from this sparse patch.
    pub const fn property(&self, property: StyleProperty) -> Option<StylePropertyValue> {
        match property {
            StyleProperty::BgColor => color_value(self.bg_color),
            StyleProperty::BorderColor => color_value(self.border_color),
            StyleProperty::BorderWidth => u8_value(self.border_width),
            StyleProperty::Alpha => u8_value(self.alpha),
            StyleProperty::Radius => u8_value(self.radius),
            StyleProperty::TextColor => color_value(self.text_color),
            StyleProperty::FontId => match self.font_id {
                Some(value) => Some(StylePropertyValue::U32(value.0 as u32)),
                None => None,
            },
            StyleProperty::LetterSpacing => i8_value(self.letter_spacing),
            StyleProperty::LineSpacing => i8_value(self.line_spacing),
            StyleProperty::TextAlign => match self.text_align {
                Some(value) => Some(StylePropertyValue::TextAlign(value)),
                None => None,
            },
            StyleProperty::PaddingTop => i32_value(self.padding_top),
            StyleProperty::PaddingBottom => i32_value(self.padding_bottom),
            StyleProperty::PaddingLeft => i32_value(self.padding_left),
            StyleProperty::PaddingRight => i32_value(self.padding_right),
            StyleProperty::MarginTop => i32_value(self.margin_top),
            StyleProperty::MarginBottom => i32_value(self.margin_bottom),
            StyleProperty::MarginLeft => i32_value(self.margin_left),
            StyleProperty::MarginRight => i32_value(self.margin_right),
            StyleProperty::GapRow => i32_value(self.gap_row),
            StyleProperty::GapCol => i32_value(self.gap_col),
        }
    }

    fn set_property(
        &mut self,
        property: StyleProperty,
        value: StylePropertyValue,
    ) -> Result<(), MpyStyleStorageError> {
        match (property, value) {
            (StyleProperty::BgColor, StylePropertyValue::Color(value)) => {
                self.bg_color = Some(value);
            }
            (StyleProperty::BorderColor, StylePropertyValue::Color(value)) => {
                self.border_color = Some(value);
            }
            (StyleProperty::BorderWidth, StylePropertyValue::U32(value)) => {
                self.border_width =
                    Some(u8::try_from(value).map_err(|_| MpyStyleStorageError::Range)?);
            }
            (StyleProperty::Alpha, StylePropertyValue::U32(value)) => {
                self.alpha = Some(u8::try_from(value).map_err(|_| MpyStyleStorageError::Range)?);
            }
            (StyleProperty::Radius, StylePropertyValue::U32(value)) => {
                self.radius = Some(u8::try_from(value).map_err(|_| MpyStyleStorageError::Range)?);
            }
            (StyleProperty::TextColor, StylePropertyValue::Color(value)) => {
                self.text_color = Some(value);
            }
            (StyleProperty::FontId, StylePropertyValue::U32(value)) => {
                self.font_id = Some(FontId(
                    u16::try_from(value).map_err(|_| MpyStyleStorageError::Range)?,
                ));
            }
            (StyleProperty::LetterSpacing, StylePropertyValue::I32(value)) => {
                self.letter_spacing =
                    Some(i8::try_from(value).map_err(|_| MpyStyleStorageError::Range)?);
            }
            (StyleProperty::LineSpacing, StylePropertyValue::I32(value)) => {
                self.line_spacing =
                    Some(i8::try_from(value).map_err(|_| MpyStyleStorageError::Range)?);
            }
            (StyleProperty::TextAlign, StylePropertyValue::TextAlign(value)) => {
                self.text_align = Some(value);
            }
            (StyleProperty::PaddingTop, StylePropertyValue::I32(value)) => {
                self.padding_top = Some(value)
            }
            (StyleProperty::PaddingBottom, StylePropertyValue::I32(value)) => {
                self.padding_bottom = Some(value)
            }
            (StyleProperty::PaddingLeft, StylePropertyValue::I32(value)) => {
                self.padding_left = Some(value)
            }
            (StyleProperty::PaddingRight, StylePropertyValue::I32(value)) => {
                self.padding_right = Some(value)
            }
            (StyleProperty::MarginTop, StylePropertyValue::I32(value)) => {
                self.margin_top = Some(value)
            }
            (StyleProperty::MarginBottom, StylePropertyValue::I32(value)) => {
                self.margin_bottom = Some(value)
            }
            (StyleProperty::MarginLeft, StylePropertyValue::I32(value)) => {
                self.margin_left = Some(value)
            }
            (StyleProperty::MarginRight, StylePropertyValue::I32(value)) => {
                self.margin_right = Some(value)
            }
            (StyleProperty::GapRow, StylePropertyValue::I32(value)) => self.gap_row = Some(value),
            (StyleProperty::GapCol, StylePropertyValue::I32(value)) => self.gap_col = Some(value),
            _ => return Err(MpyStyleStorageError::TypeMismatch),
        }
        Ok(())
    }

    fn clear_property(&mut self, property: StyleProperty) {
        match property {
            StyleProperty::BgColor => self.bg_color = None,
            StyleProperty::BorderColor => self.border_color = None,
            StyleProperty::BorderWidth => self.border_width = None,
            StyleProperty::Alpha => self.alpha = None,
            StyleProperty::Radius => self.radius = None,
            StyleProperty::TextColor => self.text_color = None,
            StyleProperty::FontId => self.font_id = None,
            StyleProperty::LetterSpacing => self.letter_spacing = None,
            StyleProperty::LineSpacing => self.line_spacing = None,
            StyleProperty::TextAlign => self.text_align = None,
            StyleProperty::PaddingTop => self.padding_top = None,
            StyleProperty::PaddingBottom => self.padding_bottom = None,
            StyleProperty::PaddingLeft => self.padding_left = None,
            StyleProperty::PaddingRight => self.padding_right = None,
            StyleProperty::MarginTop => self.margin_top = None,
            StyleProperty::MarginBottom => self.margin_bottom = None,
            StyleProperty::MarginLeft => self.margin_left = None,
            StyleProperty::MarginRight => self.margin_right = None,
            StyleProperty::GapRow => self.gap_row = None,
            StyleProperty::GapCol => self.gap_col = None,
        }
    }
}

const fn color_value(value: Option<Color>) -> Option<StylePropertyValue> {
    match value {
        Some(value) => Some(StylePropertyValue::Color(value)),
        None => None,
    }
}

const fn u8_value(value: Option<u8>) -> Option<StylePropertyValue> {
    match value {
        Some(value) => Some(StylePropertyValue::U32(value as u32)),
        None => None,
    }
}

const fn i8_value(value: Option<i8>) -> Option<StylePropertyValue> {
    match value {
        Some(value) => Some(StylePropertyValue::I32(value as i32)),
        None => None,
    }
}

const fn i32_value(value: Option<i32>) -> Option<StylePropertyValue> {
    match value {
        Some(value) => Some(StylePropertyValue::I32(value)),
        None => None,
    }
}

/// Builder for [`StylePatch`] values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StylePatchBuilder(StylePatch);

impl StylePatchBuilder {
    /// Create a new builder starting from an empty patch.
    pub fn new() -> Self {
        Self(StylePatch::new())
    }

    /// Set the background color override.
    pub fn bg_color(mut self, color: Color) -> Self {
        self.0.bg_color = Some(color);
        self
    }

    /// Set the border color override.
    pub fn border_color(mut self, color: Color) -> Self {
        self.0.border_color = Some(color);
        self
    }

    /// Set the border width override.
    pub fn border_width(mut self, w: u8) -> Self {
        self.0.border_width = Some(w);
        self
    }

    /// Set the alpha (opacity) override.
    pub fn alpha(mut self, a: u8) -> Self {
        self.0.alpha = Some(a);
        self
    }

    /// Set the corner radius override.
    pub fn radius(mut self, r: u8) -> Self {
        self.0.radius = Some(r);
        self
    }

    /// Set the text color override.
    pub fn text_color(mut self, color: Color) -> Self {
        self.0.text_color = Some(color);
        self
    }

    /// Set the font registry identifier override.
    pub fn font_id(mut self, font_id: FontId) -> Self {
        self.0.font_id = Some(font_id);
        self
    }

    /// Set the letter spacing override in pixels.
    pub fn letter_spacing(mut self, spacing: i8) -> Self {
        self.0.letter_spacing = Some(spacing);
        self
    }

    /// Set the line spacing override in pixels.
    pub fn line_spacing(mut self, spacing: i8) -> Self {
        self.0.line_spacing = Some(spacing);
        self
    }

    /// Set the text alignment override.
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.0.text_align = Some(align);
        self
    }

    /// Consume the builder and return the constructed [`StylePatch`].
    pub fn build(self) -> StylePatch {
        self.0
    }
}

// ---------------------------------------------------------------------------
// InheritedContext
// ---------------------------------------------------------------------------

/// Inheritable property values threaded top-down through the resolve/draw
/// descent (§7.3).
///
/// At the root the context is empty (`InheritedContext::EMPTY`). After
/// resolving a node's `MAIN` properties, the caller constructs a *child
/// context* by calling [`InheritedContext::with_resolved`] and passes that
/// to each child's resolver call.
///
/// LPAR-08 extends the inherited set with text properties while preserving the
/// existing visual [`Style`] shape.
///
/// # No parent pointer
///
/// This type exists precisely *because* `ObjectNode` has no parent pointer
/// (LPAR-02 / LPAR-04 §7.2). The context travels down the call stack during
/// the traversal; a node never needs to walk upward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InheritedContext {
    /// Effective `alpha` from the nearest ancestor that resolved one, or `None`
    /// if no ancestor has set an alpha (falls to the §7.4 default `255`).
    pub alpha: Option<u8>,
    /// Effective inherited text color.
    pub text_color: Option<Color>,
    /// Effective inherited font identifier.
    pub font_id: Option<FontId>,
    /// Effective inherited letter spacing.
    pub letter_spacing: Option<i8>,
    /// Effective inherited line spacing.
    pub line_spacing: Option<i8>,
    /// Effective inherited text alignment.
    pub text_align: Option<TextAlign>,
}

impl InheritedContext {
    /// An empty context — no ancestor has supplied any inheritable value.
    ///
    /// Pass this as the root context in a top-down traversal.
    pub const EMPTY: Self = Self {
        alpha: None,
        text_color: None,
        font_id: None,
        letter_spacing: None,
        line_spacing: None,
        text_align: None,
    };

    /// Construct the context to pass to a node's children.
    ///
    /// `resolved_alpha` is the `alpha` the **current** node resolved (the
    /// value from the cascade, or the inherited value if no patch overrides it,
    /// or the §7.4 default). Children inherit this value if they carry no
    /// alpha override of their own.
    pub const fn with_resolved(resolved_alpha: u8) -> Self {
        let text = TextStyle {
            text_color: Color(0, 0, 0, 255),
            font_id: FontId::DEFAULT,
            letter_spacing: 0,
            line_spacing: 0,
            text_align: TextAlign::Left,
        };
        Self::with_resolved_styles(resolved_alpha, text)
    }

    /// Construct the context to pass to children after resolving both visual
    /// and text properties for the current node.
    pub const fn with_resolved_styles(resolved_alpha: u8, resolved_text: TextStyle) -> Self {
        Self {
            alpha: Some(resolved_alpha),
            text_color: Some(resolved_text.text_color),
            font_id: Some(resolved_text.font_id),
            letter_spacing: Some(resolved_text.letter_spacing),
            line_spacing: Some(resolved_text.line_spacing),
            text_align: Some(resolved_text.text_align),
        }
    }
}

// ---------------------------------------------------------------------------
// StyleState  (the per-node additive slot)
// ---------------------------------------------------------------------------

/// Per-node style slot holding local and shared style entries (LPAR-07 §7.1).
///
/// This type is stored in an `Option<Box<StyleState>>` on [`ObjectNode`]
/// (the `style` field), following the same lazy-alloc pattern as
/// `ScrollState` (LPAR-05) and `NodeAnimSet` (LPAR-06). The slot is `None`
/// for nodes that carry no style overrides, keeping `ObjectNode` small.
///
/// Style entries are keyed by [`Selector`] and carry a [`StylePatch`].
///
/// - **Transition override** (Tier 0) — highest precedence; written by
///   active [`start_transition`] animations.
/// - **MPY local entries** are one sparse patch per exact selector and take
///   priority over native local entries.
/// - **Native local entries** are owned by the node (`add_local_style`). They
///   take priority over added entries.
/// - **Added (shared) entries** are `'static` references (`add_style`).
///   Lower precedence than local entries.
///
/// Within each tier, the last-added entry wins for a given matching selector
/// (§7.2 reverse-registration-order rule).
pub struct StyleState {
    /// MPY-owned local style patches, one per exact selector.
    mpy_local: Vec<(Selector, StylePatch)>,
    /// Local style entries: `(selector, patch)` in registration order.
    local: Vec<(Selector, StylePatch)>,
    /// Added/shared style references: `(selector, patch)` in registration order.
    added: Vec<(Selector, &'static StylePatch)>,
    /// Default-theme entries (LPAR-07 §9.1): owned `(selector, patch)` applied
    /// at the **lowest** precedence — below local and added styles — so widget
    /// and application styles always win regardless of registration order.
    theme: Vec<(Selector, StylePatch)>,
    /// Tier-0 transition override slot.
    ///
    /// Shared with animation apply closures via `Rc<RefCell<...>>` so that
    /// closures can write interpolated values without holding a borrow on
    /// the owning node.
    pub(crate) transition_override: Rc<RefCell<TransitionOverride>>,
    /// Freshness guard for prepared MPY-local transactions.
    mpy_revision: u64,
}

impl StyleState {
    /// Create an empty style state with an empty transition override slot.
    pub fn new() -> Self {
        Self {
            mpy_local: Vec::new(),
            local: Vec::new(),
            added: Vec::new(),
            theme: Vec::new(),
            transition_override: Rc::new(RefCell::new(TransitionOverride::default())),
            mpy_revision: 0,
        }
    }

    /// Return a cloned handle to the transition override slot.
    ///
    /// Callers (typically [`start_transition`]) clone this handle so that
    /// animation apply closures can share ownership of the override cell
    /// without holding a borrow on the [`crate::object::ObjectNode`].
    pub fn transition_override_handle(&self) -> Rc<RefCell<TransitionOverride>> {
        Rc::clone(&self.transition_override)
    }

    // -----------------------------------------------------------------------
    // LPAR-10: read-only accessors for layout cascade resolution
    // -----------------------------------------------------------------------

    /// Return the local style entries as a `(Selector, StylePatch)` slice.
    ///
    /// Used by [`crate::layout::resolve_layout_style`] to iterate local entries
    /// in precedence order.
    pub fn local_entries(&self) -> &[(Selector, StylePatch)] {
        &self.local
    }

    /// Return MPY-owned local patches in selector registration order.
    pub fn mpy_local_entries(&self) -> &[(Selector, StylePatch)] {
        &self.mpy_local
    }

    /// Return the added (shared) style entries as a `(Selector, &StylePatch)` slice.
    ///
    /// Used by [`crate::layout::resolve_layout_style`] to iterate added entries
    /// in precedence order.
    pub fn added_entries(&self) -> &[(Selector, &'static StylePatch)] {
        &self.added
    }

    /// Return default-theme entries in registration order.
    pub fn theme_entries(&self) -> &[(Selector, StylePatch)] {
        &self.theme
    }

    /// Return the current private MPY-local storage revision.
    pub const fn mpy_revision(&self) -> u64 {
        self.mpy_revision
    }

    /// Prepare one exact MPY-local property set or removal.
    ///
    /// `maximum_selectors` is derived from the actor's finite applicability
    /// descriptor. All allocation and value conversion completes here.
    pub fn prepare_mpy_local_update(
        &self,
        selector: Selector,
        property: StyleProperty,
        update: MpyStyleUpdate,
        maximum_selectors: usize,
    ) -> Result<PreparedMpyStyleMutation, MpyStyleStorageError> {
        self.prepare_mpy_local_updates(&[(selector, property, update)], maximum_selectors)
    }

    /// Prepare an ordered group of exact MPY-local property updates.
    ///
    /// The group is applied to one owned replacement vector in submitted
    /// order, so repeated selectors and properties have deterministic
    /// last-write-wins behavior without allocating between operations.
    pub fn prepare_mpy_local_updates(
        &self,
        updates: &[(Selector, StyleProperty, MpyStyleUpdate)],
        maximum_selectors: usize,
    ) -> Result<PreparedMpyStyleMutation, MpyStyleStorageError> {
        let mut next_local = Vec::new();
        next_local
            .try_reserve_exact(self.mpy_local.len().saturating_add(updates.len()))
            .map_err(|_| MpyStyleStorageError::Capacity)?;
        next_local.extend_from_slice(&self.mpy_local);

        for (selector, property, update) in updates {
            let position = next_local
                .iter()
                .position(|(candidate, _)| candidate == selector);
            match (position, *update) {
                (Some(index), MpyStyleUpdate::Set(value)) => {
                    next_local[index].1.set_property(*property, value)?;
                }
                (Some(index), MpyStyleUpdate::Remove) => {
                    next_local[index].1.clear_property(*property);
                    if next_local[index].1.is_empty() {
                        next_local.remove(index);
                    }
                }
                (None, MpyStyleUpdate::Set(value)) => {
                    if next_local.len() >= maximum_selectors {
                        return Err(MpyStyleStorageError::Capacity);
                    }
                    let mut patch = StylePatch::new();
                    patch.set_property(*property, value)?;
                    next_local.push((*selector, patch));
                }
                (None, MpyStyleUpdate::Remove) => {}
            }
        }

        let changed = next_local != self.mpy_local;
        let next_revision = if changed {
            self.mpy_revision
                .checked_add(1)
                .ok_or(MpyStyleStorageError::RevisionExhausted)?
        } else {
            self.mpy_revision
        };
        Ok(PreparedMpyStyleMutation {
            owner: Rc::clone(&self.transition_override),
            expected_revision: self.mpy_revision,
            next_revision,
            changed,
            next_local,
        })
    }

    /// Check whether a prepared MPY-local update still owns this storage.
    pub fn prepared_mpy_local_update_is_current(
        &self,
        prepared: &PreparedMpyStyleMutation,
    ) -> bool {
        Rc::ptr_eq(&self.transition_override, &prepared.owner)
            && self.mpy_revision == prepared.expected_revision
    }

    /// Commit a prepared MPY-local update using only swaps and scalar stores.
    pub fn commit_mpy_local_update(
        &mut self,
        mut prepared: PreparedMpyStyleMutation,
    ) -> Result<CommittedMpyStyleMutation, MpyStyleCommitError> {
        if !self.prepared_mpy_local_update_is_current(&prepared) {
            return Err(MpyStyleCommitError {
                cause: MpyStyleStorageError::Stale,
                prepared,
            });
        }
        core::mem::swap(&mut self.mpy_local, &mut prepared.next_local);
        self.mpy_revision = prepared.next_revision;
        Ok(CommittedMpyStyleMutation {
            committed_revision: self.mpy_revision,
            retired: prepared,
        })
    }

    /// Release an uncommitted preparation after rejection or caller rollback.
    pub fn release_prepared_mpy_local_update(&self, prepared: PreparedMpyStyleMutation) {
        drop(prepared);
    }

    /// Release retained pre-commit storage after publication is complete.
    pub fn release_mpy_local_update(&self, committed: CommittedMpyStyleMutation) {
        drop(committed);
    }
}

impl PreparedMpyStyleMutation {
    /// Return whether this update changes durable MPY-owned storage.
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

impl CommittedMpyStyleMutation {
    /// Return the committed private storage revision.
    pub const fn revision(&self) -> u64 {
        self.committed_revision
    }

    /// Return whether durable MPY-owned storage changed.
    pub const fn changed(&self) -> bool {
        self.retired.changed
    }
}

impl Default for StyleState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObjectNode style methods (free functions operating on StyleState)
// ---------------------------------------------------------------------------
// These are not `impl ObjectNode` here to avoid a circular dependency between
// modules; ObjectNode adds the `pub(crate) style: Option<Box<StyleState>>`
// field and calls these free functions, or the methods are placed in object.rs
// directly (see below — we expose them via object.rs additions).
// Here we expose the free-function primitives so `object.rs` can delegate.

/// Add a locally owned style patch on a [`StyleState`].
///
/// Exposed so `ObjectNode::add_local_style` can delegate to this without
/// exposing `StyleState`'s internals at the object level.
pub fn push_local(state: &mut StyleState, patch: StylePatch, selector: Selector) {
    state.local.push((selector, patch));
}

/// Add a shared (added) style patch reference on a [`StyleState`].
pub fn push_added(state: &mut StyleState, patch: &'static StylePatch, selector: Selector) {
    state.added.push((selector, patch));
}

/// Add a default-theme style patch on a [`StyleState`] (LPAR-07 §9.1).
///
/// Theme entries resolve at the lowest style precedence — below local and
/// added styles — so a widget or application style always wins over the
/// theme regardless of which was registered first.
pub fn push_theme(state: &mut StyleState, patch: StylePatch, selector: Selector) {
    state.theme.push((selector, patch));
}

/// Remove all default-theme entries on a [`StyleState`].
///
/// Used to replace a theme (re-apply) without leaving stale entries; returns
/// the number of entries cleared.
pub fn clear_theme(state: &mut StyleState) -> usize {
    let n = state.theme.len();
    state.theme.clear();
    n
}

/// Remove local style entries whose selector matches `(part, states)`.
///
/// Matching uses the same predicate as [`Selector::matches`]. Returns the
/// number of entries removed.
///
/// Pass [`ObjectStates::DEFAULT`] with a wildcard intent: any selector whose
/// *part* matches and whose *state mask* is contained by `states` will be
/// removed. For a full "remove all" for a part, use
/// [`remove_all_local_by_part`]. For an unconditional full clear, use
/// [`remove_all_local`].
pub fn remove_local_matching(state: &mut StyleState, part: Part, states: ObjectStates) -> usize {
    let before = state.local.len();
    state.local.retain(|(sel, _)| !sel.matches(part, states));
    before - state.local.len()
}

/// Remove all local style entries for the given `part` regardless of state mask.
///
/// This is the "remove all local styles for a part" wildcard form described in
/// §7.5.
pub fn remove_all_local_by_part(state: &mut StyleState, part: Part) -> usize {
    let before = state.local.len();
    state.local.retain(|(sel, _)| sel.part != part);
    before - state.local.len()
}

/// Remove all local style entries on a node (full clear).
pub fn remove_all_local(state: &mut StyleState) -> usize {
    let count = state.local.len();
    state.local.clear();
    count
}

// ---------------------------------------------------------------------------
// resolve_styles / resolve — cascade resolution for (node-states, part, inherited_ctx)
// ---------------------------------------------------------------------------

/// Fully resolved visual and text styles for one cascade query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStyles {
    /// Fully materialized visual style for draw-time background/border use.
    pub style: Style,
    /// Fully materialized inheritable text style.
    pub text: TextStyle,
    /// Context to pass to this node's children during a top-down traversal.
    pub child_context: InheritedContext,
}

#[derive(Debug, Default)]
struct CascadeWinners {
    bg_color: Option<Color>,
    border_color: Option<Color>,
    border_width: Option<u8>,
    alpha: Option<u8>,
    radius: Option<u8>,
    text_color: Option<Color>,
    font_id: Option<FontId>,
    letter_spacing: Option<i8>,
    line_spacing: Option<i8>,
    text_align: Option<TextAlign>,
}

impl CascadeWinners {
    fn fill_from_patch(&mut self, patch: &StylePatch) {
        if self.bg_color.is_none() {
            self.bg_color = patch.bg_color;
        }
        if self.border_color.is_none() {
            self.border_color = patch.border_color;
        }
        if self.border_width.is_none() {
            self.border_width = patch.border_width;
        }
        if self.alpha.is_none() {
            self.alpha = patch.alpha;
        }
        if self.radius.is_none() {
            self.radius = patch.radius;
        }
        if self.text_color.is_none() {
            self.text_color = patch.text_color;
        }
        if self.font_id.is_none() {
            self.font_id = patch.font_id;
        }
        if self.letter_spacing.is_none() {
            self.letter_spacing = patch.letter_spacing;
        }
        if self.line_spacing.is_none() {
            self.line_spacing = patch.line_spacing;
        }
        if self.text_align.is_none() {
            self.text_align = patch.text_align;
        }
    }
}

/// Resolve visual [`Style`] and inheritable [`TextStyle`] for one query.
///
/// This is the LPAR-08 text-aware cascade entry point. It implements LPAR-07
/// precedence for the existing visual properties and extends the same
/// top-down inheritance model to text color, font id, spacing, and alignment.
pub fn resolve_styles(
    node_style: Option<&StyleState>,
    node_states: ObjectStates,
    part: Part,
    inherited: &InheritedContext,
) -> ResolvedStyles {
    // Walk the tiers in precedence order (transition override, MPY local,
    // native local, added, then theme), in reverse-registration order within
    // each tier. We collect the first `Some` for each property.
    let mut winners = CascadeWinners::default();

    if let Some(s) = node_style {
        // --- Tier 0: transition override (highest precedence) ---
        {
            let ov = s.transition_override.borrow();
            if ov.bg_color.is_some() {
                winners.bg_color = ov.bg_color;
            }
            if ov.border_color.is_some() {
                winners.border_color = ov.border_color;
            }
            if ov.border_width.is_some() {
                winners.border_width = ov.border_width;
            }
            if ov.alpha.is_some() {
                winners.alpha = ov.alpha;
            }
            if ov.radius.is_some() {
                winners.radius = ov.radius;
            }
        }

        // --- Tier 1: MPY-owned local entries ---
        for (sel, patch) in s.mpy_local.iter().rev() {
            if sel.matches(part, node_states) {
                winners.fill_from_patch(patch);
            }
        }

        // --- Tier 2: native local entries ---
        for (sel, patch) in s.local.iter().rev() {
            if sel.matches(part, node_states) {
                winners.fill_from_patch(patch);
            }
        }

        // --- Tier 3: added entries (reverse registration order) ---
        for (sel, patch) in s.added.iter().rev() {
            if sel.matches(part, node_states) {
                winners.fill_from_patch(patch);
            }
        }

        // --- Tier 4: default-theme entries (LPAR-07 §9.1, lowest style tier) ---
        // Consulted below local and added styles so widget/application styles
        // always win over the theme regardless of registration order.
        for (sel, patch) in s.theme.iter().rev() {
            if sel.matches(part, node_states) {
                winners.fill_from_patch(patch);
            }
        }
    }

    // --- Tier 5: take inheritable values from inherited context ---
    if winners.alpha.is_none() {
        winners.alpha = inherited.alpha;
    }
    if winners.text_color.is_none() {
        winners.text_color = inherited.text_color;
    }
    if winners.font_id.is_none() {
        winners.font_id = inherited.font_id;
    }
    if winners.letter_spacing.is_none() {
        winners.letter_spacing = inherited.letter_spacing;
    }
    if winners.line_spacing.is_none() {
        winners.line_spacing = inherited.line_spacing;
    }
    if winners.text_align.is_none() {
        winners.text_align = inherited.text_align;
    }

    // --- Tier 6: property defaults (§7.4) ---
    let defaults = Style::default();
    let text_defaults = TextStyle::default();
    let style = Style {
        bg_color: winners.bg_color.unwrap_or(defaults.bg_color),
        border_color: winners.border_color.unwrap_or(defaults.border_color),
        border_width: winners.border_width.unwrap_or(defaults.border_width),
        alpha: winners.alpha.unwrap_or(defaults.alpha),
        radius: winners.radius.unwrap_or(defaults.radius),
    };
    let text = TextStyle {
        text_color: winners.text_color.unwrap_or(text_defaults.text_color),
        font_id: winners.font_id.unwrap_or(text_defaults.font_id),
        letter_spacing: winners
            .letter_spacing
            .unwrap_or(text_defaults.letter_spacing),
        line_spacing: winners.line_spacing.unwrap_or(text_defaults.line_spacing),
        text_align: winners.text_align.unwrap_or(text_defaults.text_align),
    };

    let child_context = InheritedContext::with_resolved_styles(style.alpha, text);

    ResolvedStyles {
        style,
        text,
        child_context,
    }
}

/// Resolve the visual cascade for a given `(node_style, node_states, part)` query.
///
/// This compatibility wrapper preserves the LPAR-07 return shape for existing
/// callers. New text-aware callers should use [`resolve_styles`].
pub fn resolve(
    node_style: Option<&StyleState>,
    node_states: ObjectStates,
    part: Part,
    inherited: &InheritedContext,
) -> (Style, InheritedContext) {
    let resolved = resolve_styles(node_style, node_states, part, inherited);
    (resolved.style, resolved.child_context)
}

// ---------------------------------------------------------------------------
// start_transition / cancel_transition — LPAR-07 §8
// ---------------------------------------------------------------------------

/// Begin a style transition on `node` for property `prop`.
///
/// The transition interpolates `from → to` according to `desc` using
/// [`ObjectAnims::bind`]. While the animation runs, each tick writes the
/// interpolated value into the node's [`TransitionOverride`] (Tier-0 cascade
/// slot), so that subsequent calls to [`resolve`] return the in-flight value
/// at highest precedence.
///
/// When the tween finishes naturally, the property's override slot is cleared
/// so the cascade falls back to the layers below (local styles, added styles,
/// defaults). If you wish to stop an in-flight transition early, call
/// [`cancel_transition`] with the returned [`ObjectAnimId`].
///
/// # Arguments
///
/// - `node` — the target node. If it carries no style slot yet, one is
///   allocated lazily.
/// - `anims` — the `ObjectAnims` id allocator / walker for this tree.
/// - `prop` — which property to animate.
/// - `from` — starting value (must match the type expected for `prop`).
/// - `to` — ending value (must match the type expected for `prop`).
/// - `desc` — timing and easing parameters.
/// - `node_bounds` — the dirty rect returned by each apply tick so that
///   callers can route it into an invalidation planner.
///
/// # Type contract
///
/// `from` and `to` must both be [`AnimPropValue::Color`] when `prop` is
/// `BgColor` or `BorderColor`, and both [`AnimPropValue::Scalar`] when `prop`
/// is `BorderWidth`, `Alpha`, or `Radius`. Mismatched types produce a
/// `Some(node_bounds)` dirty rect every tick but leave the property at its
/// `from` value (the interpolation falls through to the scalar branch which
/// returns 0 for both `Color` variants — treat this as a programming error
/// to be caught in tests).
pub fn start_transition(
    node: &mut crate::object::ObjectNode,
    anims: &mut ObjectAnims,
    prop: AnimProp,
    from: AnimPropValue,
    to: AnimPropValue,
    desc: TransitionDesc,
    node_bounds: Rect,
) -> ObjectAnimId {
    // 1. Lazily allocate the style slot so the Rc handle is always present.
    node.style
        .get_or_insert_with(|| alloc::boxed::Box::new(StyleState::new()));

    // 2. Clone the Rc handle *before* borrowing node mutably through bind().
    let override_rc = Rc::clone(
        &node
            .style
            .as_ref()
            .expect("just inserted above")
            .transition_override,
    );

    // 3. Build the tween (0 → ANIM_SCALE over duration_ticks).
    let tween = Tween::new(0, ANIM_SCALE, desc.duration_ticks).with_easing(desc.easing);

    // 4. Build the apply closure.  Captures `override_rc`, `prop`, `from`, `to`.
    let apply_rc = Rc::clone(&override_rc);
    let apply: alloc::boxed::Box<dyn FnMut(i32) -> Option<Rect>> =
        alloc::boxed::Box::new(move |v: i32| {
            let interpolated = interpolate_prop(from, to, v);
            let mut ov = apply_rc.borrow_mut();
            write_override(&mut ov, prop, interpolated);
            Some(node_bounds)
        });

    // 5. Build the on_complete closure: clears the override for this property.
    let complete_rc = Rc::clone(&override_rc);
    let on_complete: alloc::boxed::Box<dyn FnOnce()> = alloc::boxed::Box::new(move || {
        let mut ov = complete_rc.borrow_mut();
        clear_override(&mut ov, prop);
    });

    anims.bind(node, tween, apply, desc.delay_ticks, Some(on_complete))
}

/// Cancel an in-flight style transition and clear the property's override slot.
///
/// Calls [`ObjectAnims::cancel`] (which removes the entry without firing
/// `on_complete`), then explicitly clears `prop` in the transition override so
/// that subsequent [`resolve`] calls fall through to the lower cascade tiers.
///
/// Returns `true` if the animation was found and removed, `false` if the id
/// was unknown or already completed.
pub fn cancel_transition(
    node: &mut crate::object::ObjectNode,
    anims: &mut ObjectAnims,
    old_id: ObjectAnimId,
    prop: AnimProp,
) -> bool {
    // Cancel the animation entry (no on_complete fired).
    let found = anims.cancel(node, old_id);

    // Always clear the override slot regardless of whether the id was still live
    // — if the animation completed naturally between the call to start_transition
    // and this call, the slot is already cleared, so borrow + clear is harmless.
    if let Some(style) = node.style.as_ref() {
        let mut ov = style.transition_override.borrow_mut();
        clear_override(&mut ov, prop);
    }

    found
}

// ---------------------------------------------------------------------------
// Interpolation helpers (private)
// ---------------------------------------------------------------------------

/// Interpolate between two [`AnimPropValue`]s at progress `v` (0..=ANIM_SCALE).
fn interpolate_prop(from: AnimPropValue, to: AnimPropValue, v: i32) -> AnimPropValue {
    match (from, to) {
        (AnimPropValue::Color(c_from), AnimPropValue::Color(c_to)) => {
            AnimPropValue::Color(c_from.lerp(c_to, v, ANIM_SCALE))
        }
        (AnimPropValue::Scalar(s_from), AnimPropValue::Scalar(s_to)) => {
            let interp = (s_from as i32 + (s_to as i32 - s_from as i32) * v / ANIM_SCALE)
                .clamp(0, 255) as u8;
            AnimPropValue::Scalar(interp)
        }
        // Type mismatch: fall through to the from value unchanged.
        (from, _) => from,
    }
}

/// Write an interpolated value into the appropriate override slot.
fn write_override(ov: &mut TransitionOverride, prop: AnimProp, value: AnimPropValue) {
    match (prop, value) {
        (AnimProp::BgColor, AnimPropValue::Color(c)) => ov.bg_color = Some(c),
        (AnimProp::BorderColor, AnimPropValue::Color(c)) => ov.border_color = Some(c),
        (AnimProp::BorderWidth, AnimPropValue::Scalar(s)) => ov.border_width = Some(s),
        (AnimProp::Alpha, AnimPropValue::Scalar(s)) => ov.alpha = Some(s),
        (AnimProp::Radius, AnimPropValue::Scalar(s)) => ov.radius = Some(s),
        _ => {}
    }
}

/// Clear the override slot for the given property.
fn clear_override(ov: &mut TransitionOverride, prop: AnimProp) {
    match prop {
        AnimProp::BgColor => ov.bg_color = None,
        AnimProp::BorderColor => ov.border_color = None,
        AnimProp::BorderWidth => ov.border_width = None,
        AnimProp::Alpha => ov.alpha = None,
        AnimProp::Radius => ov.radius = None,
    }
}

/// Convenience top-down traversal that resolves each node and calls `visitor`.
///
/// Descends `root → children` threading the [`InheritedContext`] through the
/// recursion. For each node, `visitor` is called with a reference to the
/// node's [`crate::object::ObjectNode`] and the resolved [`Style`] for
/// `Part::MAIN`. The child context produced for each node is passed to all of
/// its children.
///
/// This demonstrates the §7.3 top-down inheritance mechanism and provides
/// LPAR-08/draw with a ready entry point. For per-part draw calls you should
/// call [`resolve`] directly with the desired `Part`.
pub fn resolve_tree<F>(root: &crate::object::ObjectNode, visitor: &mut F)
where
    F: FnMut(&crate::object::ObjectNode, &Style),
{
    resolve_tree_inner(root, &InheritedContext::EMPTY, visitor);
}

fn resolve_tree_inner<F>(
    node: &crate::object::ObjectNode,
    inherited: &InheritedContext,
    visitor: &mut F,
) where
    F: FnMut(&crate::object::ObjectNode, &Style),
{
    let (style, child_ctx) = resolve(node.style.as_deref(), node.states(), Part::MAIN, inherited);
    visitor(node, &style);
    for child in node.children() {
        resolve_tree_inner(child, &child_ctx, visitor);
    }
}

/// Convenience top-down traversal that resolves visual and text styles.
///
/// This is the text-aware companion to [`resolve_tree`]. It threads the same
/// inherited context through the tree but exposes both [`Style`] and
/// [`TextStyle`] to the visitor.
pub fn resolve_tree_with_text<F>(root: &crate::object::ObjectNode, visitor: &mut F)
where
    F: FnMut(&crate::object::ObjectNode, &Style, &TextStyle),
{
    resolve_tree_with_text_inner(root, &InheritedContext::EMPTY, visitor);
}

fn resolve_tree_with_text_inner<F>(
    node: &crate::object::ObjectNode,
    inherited: &InheritedContext,
    visitor: &mut F,
) where
    F: FnMut(&crate::object::ObjectNode, &Style, &TextStyle),
{
    let resolved = resolve_styles(node.style.as_deref(), node.states(), Part::MAIN, inherited);
    visitor(node, &resolved.style, &resolved.text);
    for child in node.children() {
        resolve_tree_with_text_inner(child, &resolved.child_context, visitor);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use super::*;
    use crate::object::{ObjectNode, ObjectStates};
    use crate::widget::{Color, Rect, Widget};

    // -----------------------------------------------------------------------
    // Minimal test widget
    // -----------------------------------------------------------------------

    struct Dummy;

    impl Widget for Dummy {
        fn bounds(&self) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }
        }
        fn draw(&self, _r: &mut dyn crate::renderer::Renderer) {}
        fn handle_event(&mut self, _e: &crate::event::Event) -> bool {
            false
        }
    }

    fn make_node() -> ObjectNode {
        ObjectNode::new(Rc::new(RefCell::new(Dummy)))
    }

    fn red() -> Color {
        Color(255, 0, 0, 255)
    }
    fn blue() -> Color {
        Color(0, 0, 255, 255)
    }
    fn green() -> Color {
        Color(0, 255, 0, 255)
    }

    // -----------------------------------------------------------------------
    // Part / Selector matching
    // -----------------------------------------------------------------------

    #[test]
    fn selector_main_pressed_matches_pressed_state() {
        let sel = Selector::new(Part::MAIN, ObjectStates::PRESSED);
        // Matches when PRESSED is set.
        assert!(sel.matches(Part::MAIN, ObjectStates::PRESSED));
        // Does not match when PRESSED is absent.
        assert!(!sel.matches(Part::MAIN, ObjectStates::DEFAULT));
    }

    #[test]
    fn selector_part_matches_any_state() {
        let sel = Selector::part(Part::MAIN);
        assert!(sel.matches(Part::MAIN, ObjectStates::DEFAULT));
        assert!(sel.matches(Part::MAIN, ObjectStates::PRESSED));
        assert!(sel.matches(Part::MAIN, ObjectStates::FOCUSED));
    }

    #[test]
    fn selector_does_not_match_wrong_part() {
        let sel = Selector::part(Part::INDICATOR);
        assert!(!sel.matches(Part::MAIN, ObjectStates::DEFAULT));
    }

    #[test]
    fn selector_multi_state_requires_all_bits() {
        let mask = ObjectStates::from_bits_truncate(
            ObjectStates::PRESSED.bits() | ObjectStates::FOCUSED.bits(),
        );
        let sel = Selector::new(Part::MAIN, mask);
        // Both bits set → matches.
        assert!(sel.matches(Part::MAIN, mask));
        // Only one bit set → no match.
        assert!(!sel.matches(Part::MAIN, ObjectStates::PRESSED));
    }

    // -----------------------------------------------------------------------
    // Cascade precedence: local overrides added
    // -----------------------------------------------------------------------

    #[test]
    fn local_overrides_added_for_same_field() {
        let mut node = make_node();

        // Added (lower precedence) patch: bg = blue.
        static ADDED_PATCH: StylePatch = StylePatch {
            bg_color: Some(Color(0, 0, 255, 255)),
            border_color: None,
            border_width: None,
            alpha: None,
            radius: None,
            text_color: None,
            font_id: None,
            letter_spacing: None,
            line_spacing: None,
            text_align: None,
            padding_top: None,
            padding_bottom: None,
            padding_left: None,
            padding_right: None,
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            gap_row: None,
            gap_col: None,
        };
        node.add_style(&ADDED_PATCH, Selector::part(Part::MAIN));

        // Local (higher precedence) patch: bg = red.
        let local_patch = StylePatch {
            bg_color: Some(Color(255, 0, 0, 255)),
            ..StylePatch::new()
        };
        node.add_local_style(local_patch, Selector::part(Part::MAIN));

        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // Local patch wins: red.
        assert_eq!(style.bg_color, red());
    }

    // -----------------------------------------------------------------------
    // Last-added local wins among multiple matching locals
    // -----------------------------------------------------------------------

    #[test]
    fn last_added_local_wins_among_matching_locals() {
        let mut node = make_node();

        // First local: bg = red.
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        // Second local (added later): bg = blue.
        node.add_local_style(
            StylePatch {
                bg_color: Some(blue()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // Last-added wins: blue.
        assert_eq!(style.bg_color, blue());
    }

    // -----------------------------------------------------------------------
    // Per-field merge: only set fields override; others fall to default
    // -----------------------------------------------------------------------

    #[test]
    fn patch_only_bg_leaves_other_fields_at_default() {
        let mut node = make_node();
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        let defaults = Style::default();

        assert_eq!(style.bg_color, red(), "bg_color from patch");
        assert_eq!(
            style.border_color, defaults.border_color,
            "border_color is default"
        );
        assert_eq!(
            style.border_width, defaults.border_width,
            "border_width is default"
        );
        assert_eq!(style.alpha, defaults.alpha, "alpha is default");
        assert_eq!(style.radius, defaults.radius, "radius is default");
    }

    // -----------------------------------------------------------------------
    // State-driven: base + pressed override
    // -----------------------------------------------------------------------

    #[test]
    fn state_driven_base_and_pressed_override() {
        let mut node = make_node();

        // Base patch: bg = blue (matches any state via DEFAULT mask).
        node.add_local_style(
            StylePatch {
                bg_color: Some(blue()),
                ..StylePatch::new()
            },
            Selector::new(Part::MAIN, ObjectStates::DEFAULT),
        );
        // Pressed override: bg = red (only matches when PRESSED).
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::new(Part::MAIN, ObjectStates::PRESSED),
        );

        // Without PRESSED: base applies → blue.
        let (style_default, _) = resolve(
            node.style.as_deref(),
            ObjectStates::DEFAULT,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(
            style_default.bg_color,
            blue(),
            "default state should give blue"
        );

        // With PRESSED: DEFAULT still matches, but the pressed patch was added
        // AFTER the base patch, and also matches because PRESSED bits are all
        // set — so last-added-wins gives red.
        let (style_pressed, _) = resolve(
            node.style.as_deref(),
            ObjectStates::PRESSED,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(
            style_pressed.bg_color,
            red(),
            "pressed state should give red"
        );
    }

    // -----------------------------------------------------------------------
    // remove_local_styles
    // -----------------------------------------------------------------------

    #[test]
    fn remove_local_styles_removes_matching_entries() {
        let mut node = make_node();
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        node.add_local_style(
            StylePatch {
                bg_color: Some(green()),
                letter_spacing: Some(9),
                ..StylePatch::new()
            },
            Selector::part(Part::SCROLLBAR),
        );

        // Remove MAIN local styles.
        let removed = node.remove_local_styles(Part::MAIN, ObjectStates::DEFAULT);
        assert_eq!(removed, 1);

        // SCROLLBAR entry survives; resolving MAIN now gives default bg.
        let (style, _) = resolve(
            node.style.as_deref(),
            ObjectStates::DEFAULT,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(style.bg_color, Style::default().bg_color);
    }

    #[test]
    fn remove_all_local_styles_clears_everything() {
        let mut node = make_node();
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        node.add_local_style(
            StylePatch {
                bg_color: Some(blue()),
                ..StylePatch::new()
            },
            Selector::part(Part::INDICATOR),
        );

        let removed = node.remove_all_local_styles();
        assert_eq!(removed, 2);

        let (style, _) = resolve(
            node.style.as_deref(),
            ObjectStates::DEFAULT,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(style.bg_color, Style::default().bg_color);
    }

    // -----------------------------------------------------------------------
    // Top-down alpha inheritance
    // -----------------------------------------------------------------------

    #[test]
    fn alpha_inherits_from_parent_context() {
        // Parent resolves alpha = 128; child has no alpha patch → inherits.
        let parent_ctx = InheritedContext::with_resolved(128);
        let mut child = make_node();
        // Child has no alpha patch.
        child.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        let (child_style, _) = resolve(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &parent_ctx,
        );
        assert_eq!(child_style.alpha, 128, "child should inherit parent alpha");
    }

    #[test]
    fn alpha_own_patch_overrides_inheritance() {
        let parent_ctx = InheritedContext::with_resolved(128);
        let mut child = make_node();
        child.add_local_style(
            StylePatch {
                alpha: Some(200),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        let (child_style, grandchild_ctx) = resolve(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &parent_ctx,
        );
        assert_eq!(child_style.alpha, 200, "child's own patch overrides");
        // Grandchild gets child's resolved alpha.
        assert_eq!(grandchild_ctx.alpha, Some(200));
    }

    #[test]
    fn grandchild_inherits_overriding_ancestor_alpha() {
        // parent resolves alpha 100; child sets alpha 200; grandchild has none.
        let parent_ctx = InheritedContext::with_resolved(100);

        let mut child = make_node();
        child.add_local_style(
            StylePatch {
                alpha: Some(200),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        let (_, grandchild_ctx) = resolve(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &parent_ctx,
        );
        // Grandchild context carries child's alpha = 200.
        assert_eq!(grandchild_ctx.alpha, Some(200));

        let grandchild = make_node();
        let (gs, _) = resolve(
            grandchild.style.as_deref(),
            grandchild.states(),
            Part::MAIN,
            &grandchild_ctx,
        );
        assert_eq!(gs.alpha, 200);
    }

    #[test]
    fn no_alpha_anywhere_resolves_to_default_255() {
        let node = make_node();
        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(style.alpha, 255);
    }

    #[test]
    fn bg_color_does_not_inherit() {
        // Parent sets bg_color; child has no bg_color patch → child gets default.
        let mut parent = make_node();
        parent.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        let (_, child_ctx) = resolve(
            parent.style.as_deref(),
            parent.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );

        let child = make_node();
        let (child_style, _) = resolve(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &child_ctx,
        );
        // bg_color is not inheritable: child gets the default (white), not red.
        assert_eq!(child_style.bg_color, Style::default().bg_color);
    }

    // -----------------------------------------------------------------------
    // TextStyle cascade and inheritance
    // -----------------------------------------------------------------------

    #[test]
    fn text_style_defaults_resolve_without_patches() {
        let node = make_node();
        let resolved = resolve_styles(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(resolved.text, TextStyle::default());
        assert_eq!(resolved.child_context.text_color, Some(Color(0, 0, 0, 255)));
        assert_eq!(resolved.child_context.font_id, Some(FontId::DEFAULT));
    }

    #[test]
    fn text_style_inherits_from_parent_context() {
        let parent_text = TextStyle {
            text_color: green(),
            font_id: FontId(7),
            letter_spacing: 2,
            line_spacing: 3,
            text_align: TextAlign::Center,
        };
        let parent_ctx = InheritedContext::with_resolved_styles(128, parent_text);
        let child = make_node();

        let resolved = resolve_styles(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &parent_ctx,
        );

        assert_eq!(resolved.text, parent_text);
        assert_eq!(resolved.style.alpha, 128);
    }

    #[test]
    fn local_text_patch_overrides_inherited_text() {
        let parent_text = TextStyle {
            text_color: green(),
            font_id: FontId(7),
            letter_spacing: 2,
            line_spacing: 3,
            text_align: TextAlign::Right,
        };
        let parent_ctx = InheritedContext::with_resolved_styles(255, parent_text);
        let mut child = make_node();
        child.add_local_style(
            StylePatch::builder()
                .text_color(red())
                .font_id(FontId(9))
                .letter_spacing(4)
                .line_spacing(5)
                .text_align(TextAlign::Center)
                .build(),
            Selector::part(Part::MAIN),
        );

        let resolved = resolve_styles(
            child.style.as_deref(),
            child.states(),
            Part::MAIN,
            &parent_ctx,
        );

        assert_eq!(resolved.text.text_color, red());
        assert_eq!(resolved.text.font_id, FontId(9));
        assert_eq!(resolved.text.letter_spacing, 4);
        assert_eq!(resolved.text.line_spacing, 5);
        assert_eq!(resolved.text.text_align, TextAlign::Center);
    }

    // -----------------------------------------------------------------------
    // resolve_tree threads context through the tree
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_tree_visits_all_nodes_with_inherited_alpha() {
        use alloc::collections::BTreeMap;
        use alloc::string::String;

        // Build a two-level tree with a tagged root and child.
        let root_widget = Rc::new(RefCell::new(Dummy));
        let child_widget = Rc::new(RefCell::new(Dummy));

        let mut root = ObjectNode::new(root_widget).with_tag("root");
        let child = ObjectNode::new(child_widget).with_tag("child");

        // Root has alpha = 50.
        root.add_local_style(
            StylePatch {
                alpha: Some(50),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        root.append_child(child);

        let mut alpha_map: BTreeMap<String, u8> = BTreeMap::new();
        resolve_tree(&root, &mut |node, style| {
            if let Some(tag) = node.tag() {
                alpha_map.insert(tag.to_string(), style.alpha);
            }
        });

        assert_eq!(alpha_map.get("root"), Some(&50), "root has its own alpha");
        assert_eq!(
            alpha_map.get("child"),
            Some(&50),
            "child inherits root alpha"
        );
    }

    #[test]
    fn resolve_tree_with_text_threads_text_context() {
        use alloc::collections::BTreeMap;
        use alloc::string::String;

        let root_widget = Rc::new(RefCell::new(Dummy));
        let child_widget = Rc::new(RefCell::new(Dummy));

        let mut root = ObjectNode::new(root_widget).with_tag("root");
        let child = ObjectNode::new(child_widget).with_tag("child");

        root.add_local_style(
            StylePatch::builder()
                .text_color(blue())
                .font_id(FontId(11))
                .letter_spacing(1)
                .line_spacing(2)
                .text_align(TextAlign::Right)
                .build(),
            Selector::part(Part::MAIN),
        );
        root.append_child(child);

        let mut text_colors: BTreeMap<String, Color> = BTreeMap::new();
        let mut font_ids: BTreeMap<String, FontId> = BTreeMap::new();
        resolve_tree_with_text(&root, &mut |node, _style, text| {
            if let Some(tag) = node.tag() {
                text_colors.insert(tag.to_string(), text.text_color);
                font_ids.insert(tag.to_string(), text.font_id);
            }
        });

        assert_eq!(text_colors.get("root"), Some(&blue()));
        assert_eq!(text_colors.get("child"), Some(&blue()));
        assert_eq!(font_ids.get("root"), Some(&FontId(11)));
        assert_eq!(font_ids.get("child"), Some(&FontId(11)));
    }

    // -----------------------------------------------------------------------
    // Existing Style/StyleBuilder still work unchanged
    // -----------------------------------------------------------------------

    #[test]
    fn style_builder_still_works() {
        use crate::style::StyleBuilder;
        let s = StyleBuilder::new()
            .bg_color(red())
            .border_width(2)
            .alpha(128)
            .build();
        assert_eq!(s.bg_color, red());
        assert_eq!(s.border_width, 2);
        assert_eq!(s.alpha, 128);
    }

    // -----------------------------------------------------------------------
    // ObjectNode convenience methods compile and run
    // -----------------------------------------------------------------------

    #[test]
    fn object_node_methods_round_trip() {
        let mut node = make_node();
        assert!(node.style.is_none(), "initially no style slot");

        node.add_local_style(
            StylePatch {
                bg_color: Some(green()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        assert!(node.style.is_some(), "slot allocated after add");

        // Adding a shared patch.
        static SP: StylePatch = StylePatch {
            border_width: Some(3),
            bg_color: None,
            border_color: None,
            alpha: None,
            radius: None,
            text_color: None,
            font_id: None,
            letter_spacing: None,
            line_spacing: None,
            text_align: None,
            padding_top: None,
            padding_bottom: None,
            padding_left: None,
            padding_right: None,
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            gap_row: None,
            gap_col: None,
        };
        node.add_style(&SP, Selector::part(Part::MAIN));

        let (s, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(s.bg_color, green());
        assert_eq!(s.border_width, 3);

        let removed = node.remove_local_styles(Part::MAIN, ObjectStates::DEFAULT);
        assert_eq!(removed, 1);
    }

    // -----------------------------------------------------------------------
    // LPAR-07 Transition override (Tier-0) tests
    // -----------------------------------------------------------------------

    /// Helper: tick ObjectAnims N times and collect dirty rects.
    fn tick_n(oa: &mut crate::object_anim::ObjectAnims, root: &mut ObjectNode, n: u32) {
        for _ in 0..n {
            oa.tick(root, &mut |_| {});
        }
    }

    #[test]
    fn transition_override_wins_above_local_styles() {
        let mut node = make_node();
        // Local patch: bg = red.
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        // Manually write an override value directly into the Rc<RefCell<>>.
        let override_rc = node
            .style
            .get_or_insert_with(|| alloc::boxed::Box::new(StyleState::new()))
            .transition_override_handle();
        override_rc.borrow_mut().bg_color = Some(blue());

        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // Tier-0 override wins: blue.
        assert_eq!(
            style.bg_color,
            blue(),
            "transition override must beat local styles"
        );
    }

    #[test]
    fn resolve_without_override_identical_to_before() {
        let mut node = make_node();
        node.add_local_style(
            StylePatch {
                bg_color: Some(red()),
                border_width: Some(3),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        // No override set — transition_override is all None by default.
        let (style, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(style.bg_color, red(), "bg_color from local patch unchanged");
        assert_eq!(
            style.border_width, 3,
            "border_width from local patch unchanged"
        );
    }

    #[test]
    fn start_transition_animates_override() {
        let mut node = make_node();
        let mut oa = crate::object_anim::ObjectAnims::new();
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };

        // Transition bg_color from red → blue over 256 ticks (ANIM_SCALE duration
        // means progress == tick at each step, making interpolation trivial).
        let _id = start_transition(
            &mut node,
            &mut oa,
            AnimProp::BgColor,
            AnimPropValue::Color(red()),
            AnimPropValue::Color(blue()),
            TransitionDesc {
                duration_ticks: 256,
                delay_ticks: 0,
                easing: crate::anim::Easing::Linear,
            },
            bounds,
        );

        // After 128 ticks at linear interpolation over 256 ticks, progress v = 128.
        // lerp(red, blue, 128, 256):
        //   r: 255 + (0-255)*128/256 = 255-127 = 128
        //   b: 0 + 255*128/256 = 127
        // So the color is neither pure red nor pure blue.
        tick_n(&mut oa, &mut node, 128);

        let (style_mid, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // The override is now active: bg_color should be neither pure red nor pure blue.
        assert_ne!(
            style_mid.bg_color,
            red(),
            "should have started transitioning"
        );
        assert_ne!(style_mid.bg_color, blue(), "should not be at end yet");

        // Run remaining 128 ticks → tween completes → on_complete clears override.
        tick_n(&mut oa, &mut node, 128);

        let (style_after, _) = resolve(
            node.style.as_deref(),
            node.states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // Override cleared → falls back to Style::default().
        assert_eq!(
            style_after.bg_color,
            Style::default().bg_color,
            "override should be cleared after transition completes"
        );
    }

    #[test]
    fn transition_cancel_stops_without_completion() {
        let mut node = make_node();
        let mut oa = crate::object_anim::ObjectAnims::new();
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };

        let id = start_transition(
            &mut node,
            &mut oa,
            AnimProp::Alpha,
            AnimPropValue::Scalar(255),
            AnimPropValue::Scalar(0),
            TransitionDesc {
                duration_ticks: 100,
                delay_ticks: 0,
                easing: crate::anim::Easing::Linear,
            },
            bounds,
        );

        // Advance a few ticks so the override is non-None.
        tick_n(&mut oa, &mut node, 10);

        // The override should now be Some (intermediate alpha).
        {
            let ov_rc = node.style.as_ref().unwrap().transition_override_handle();
            assert!(
                ov_rc.borrow().alpha.is_some(),
                "override should be set after a few ticks"
            );
        }

        // Cancel: removes animation and clears the override.
        cancel_transition(&mut node, &mut oa, id, AnimProp::Alpha);

        let ov_rc = node.style.as_ref().unwrap().transition_override_handle();
        assert!(
            ov_rc.borrow().alpha.is_none(),
            "override should be cleared after cancel_transition"
        );
    }

    #[test]
    fn determinism_check() {
        use crate::anim::Easing;

        // Two independent runs of the same transition must produce the same
        // intermediate values at the same tick.
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let desc = TransitionDesc {
            duration_ticks: 50,
            delay_ticks: 0,
            easing: Easing::EaseOut,
        };

        let sample = || {
            let mut node = make_node();
            let mut oa = crate::object_anim::ObjectAnims::new();
            start_transition(
                &mut node,
                &mut oa,
                AnimProp::BgColor,
                AnimPropValue::Color(red()),
                AnimPropValue::Color(blue()),
                desc,
                bounds,
            );
            tick_n(&mut oa, &mut node, 25);
            let (style, _) = resolve(
                node.style.as_deref(),
                node.states(),
                Part::MAIN,
                &InheritedContext::EMPTY,
            );
            style.bg_color
        };

        assert_eq!(sample(), sample(), "transitions are deterministic");
    }

    #[test]
    fn mpy_local_updates_are_exact_sparse_and_reveal_lower_tiers() {
        static ADDED: StylePatch = StylePatch {
            bg_color: Some(Color(0, 0, 255, 255)),
            ..StylePatch::new()
        };
        let selector = Selector::part(Part::MAIN);
        let pressed = Selector::new(Part::MAIN, ObjectStates::PRESSED);
        let mut state = StyleState::new();
        push_theme(
            &mut state,
            StylePatch {
                bg_color: Some(green()),
                ..StylePatch::new()
            },
            selector,
        );
        push_added(&mut state, &ADDED, selector);
        push_local(
            &mut state,
            StylePatch {
                bg_color: Some(red()),
                border_width: Some(7),
                letter_spacing: Some(7),
                ..StylePatch::new()
            },
            selector,
        );

        let prepared = state
            .prepare_mpy_local_update(
                selector,
                StyleProperty::BgColor,
                MpyStyleUpdate::Set(StylePropertyValue::Color(Color(1, 2, 3, 4))),
                2,
            )
            .unwrap();
        let committed = state.commit_mpy_local_update(prepared).unwrap();
        assert!(committed.changed());
        state.release_mpy_local_update(committed);

        let resolved = resolve_styles(
            Some(&state),
            ObjectStates::PRESSED,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(resolved.style.bg_color, Color(1, 2, 3, 4));
        assert_eq!(resolved.style.border_width, 7);

        let pressed_update = state
            .prepare_mpy_local_update(
                pressed,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(0)),
                2,
            )
            .unwrap();
        let committed = state.commit_mpy_local_update(pressed_update).unwrap();
        state.release_mpy_local_update(committed);
        let text_zero = state
            .prepare_mpy_local_update(
                pressed,
                StyleProperty::LetterSpacing,
                MpyStyleUpdate::Set(StylePropertyValue::I32(0)),
                2,
            )
            .unwrap();
        let committed = state.commit_mpy_local_update(text_zero).unwrap();
        state.release_mpy_local_update(committed);
        assert_eq!(state.mpy_local_entries().len(), 2);

        let revision = state.mpy_revision();
        let unchanged = state
            .prepare_mpy_local_update(
                pressed,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(0)),
                2,
            )
            .unwrap();
        assert!(!unchanged.changed());
        let committed = state.commit_mpy_local_update(unchanged).unwrap();
        assert_eq!(committed.revision(), revision);
        state.release_mpy_local_update(committed);

        let remove = state
            .prepare_mpy_local_update(selector, StyleProperty::BgColor, MpyStyleUpdate::Remove, 2)
            .unwrap();
        let committed = state.commit_mpy_local_update(remove).unwrap();
        state.release_mpy_local_update(committed);
        assert_eq!(state.mpy_local_entries().len(), 1);
        assert_eq!(state.local_entries().len(), 1);
        assert_eq!(state.added_entries().len(), 1);
        assert_eq!(state.theme_entries().len(), 1);
        let resolved = resolve_styles(
            Some(&state),
            ObjectStates::PRESSED,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(resolved.style.bg_color, red());
        assert_eq!(resolved.style.alpha, 0);
        assert_eq!(resolved.text.letter_spacing, 0);

        let remove_text = state
            .prepare_mpy_local_update(
                pressed,
                StyleProperty::LetterSpacing,
                MpyStyleUpdate::Remove,
                2,
            )
            .unwrap();
        let committed = state.commit_mpy_local_update(remove_text).unwrap();
        state.release_mpy_local_update(committed);
        let resolved = resolve_styles(
            Some(&state),
            ObjectStates::PRESSED,
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(resolved.text.letter_spacing, 7);
    }

    #[test]
    fn mpy_local_prepare_rejects_type_range_capacity_and_stale_commit() {
        let selector = Selector::part(Part::MAIN);
        let mut state = StyleState::new();
        assert_eq!(
            state
                .prepare_mpy_local_update(
                    selector,
                    StyleProperty::BgColor,
                    MpyStyleUpdate::Set(StylePropertyValue::U32(1)),
                    1,
                )
                .unwrap_err(),
            MpyStyleStorageError::TypeMismatch
        );
        assert_eq!(
            state
                .prepare_mpy_local_update(
                    selector,
                    StyleProperty::Alpha,
                    MpyStyleUpdate::Set(StylePropertyValue::U32(256)),
                    1,
                )
                .unwrap_err(),
            MpyStyleStorageError::Range
        );

        let first = state
            .prepare_mpy_local_update(
                selector,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(1)),
                1,
            )
            .unwrap();
        let stale = state
            .prepare_mpy_local_update(
                selector,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(2)),
                1,
            )
            .unwrap();
        let committed = state.commit_mpy_local_update(first).unwrap();
        state.release_mpy_local_update(committed);
        let error = state.commit_mpy_local_update(stale).unwrap_err();
        assert_eq!(error.cause(), MpyStyleStorageError::Stale);
        drop(error.into_prepared());

        assert_eq!(
            state
                .prepare_mpy_local_update(
                    Selector::new(Part::MAIN, ObjectStates::PRESSED),
                    StyleProperty::Alpha,
                    MpyStyleUpdate::Set(StylePropertyValue::U32(3)),
                    1,
                )
                .unwrap_err(),
            MpyStyleStorageError::Capacity
        );

        let foreign = state
            .prepare_mpy_local_update(
                selector,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(4)),
                1,
            )
            .unwrap();
        let mut other = StyleState::new();
        let error = other.commit_mpy_local_update(foreign).unwrap_err();
        assert_eq!(error.cause(), MpyStyleStorageError::Stale);
        assert!(other.mpy_local_entries().is_empty());
        let committed = state
            .commit_mpy_local_update(error.into_prepared())
            .unwrap();
        state.release_mpy_local_update(committed);
    }

    #[test]
    fn all_twenty_style_properties_round_trip_through_one_sparse_patch() {
        let selector = Selector::part(Part::MAIN);
        let values = [
            (StyleProperty::BgColor, StylePropertyValue::Color(red())),
            (
                StyleProperty::BorderColor,
                StylePropertyValue::Color(blue()),
            ),
            (StyleProperty::BorderWidth, StylePropertyValue::U32(0)),
            (StyleProperty::Alpha, StylePropertyValue::U32(128)),
            (StyleProperty::Radius, StylePropertyValue::U32(255)),
            (StyleProperty::TextColor, StylePropertyValue::Color(green())),
            (StyleProperty::FontId, StylePropertyValue::U32(65_535)),
            (StyleProperty::LetterSpacing, StylePropertyValue::I32(-128)),
            (StyleProperty::LineSpacing, StylePropertyValue::I32(127)),
            (
                StyleProperty::TextAlign,
                StylePropertyValue::TextAlign(TextAlign::Auto),
            ),
            (StyleProperty::PaddingTop, StylePropertyValue::I32(1)),
            (StyleProperty::PaddingBottom, StylePropertyValue::I32(2)),
            (StyleProperty::PaddingLeft, StylePropertyValue::I32(3)),
            (StyleProperty::PaddingRight, StylePropertyValue::I32(4)),
            (StyleProperty::MarginTop, StylePropertyValue::I32(5)),
            (StyleProperty::MarginBottom, StylePropertyValue::I32(6)),
            (StyleProperty::MarginLeft, StylePropertyValue::I32(7)),
            (StyleProperty::MarginRight, StylePropertyValue::I32(8)),
            (StyleProperty::GapRow, StylePropertyValue::I32(9)),
            (StyleProperty::GapCol, StylePropertyValue::I32(10)),
        ];
        let mut state = StyleState::new();
        for (property, value) in values {
            let prepared = state
                .prepare_mpy_local_update(selector, property, MpyStyleUpdate::Set(value), 1)
                .unwrap();
            let committed = state.commit_mpy_local_update(prepared).unwrap();
            state.release_mpy_local_update(committed);
            assert_eq!(
                state.mpy_local_entries()[0].1.property(property),
                Some(value)
            );
        }
        assert_eq!(state.mpy_local_entries().len(), 1);

        for (property, _) in values {
            let prepared = state
                .prepare_mpy_local_update(selector, property, MpyStyleUpdate::Remove, 1)
                .unwrap();
            let committed = state.commit_mpy_local_update(prepared).unwrap();
            state.release_mpy_local_update(committed);
        }
        assert!(state.mpy_local_entries().is_empty());
    }
}
