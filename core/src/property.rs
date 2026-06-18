//! Typed property accessor model for widget introspection.
//!
//! This module provides [`PropertyValue`] and the [`Queryable`] trait, giving
//! creator and playit tooling a uniform way to read and write named properties
//! on any widget — without requiring a global property registry or
//! object-identity infrastructure (LPAR-02/04 deferred).
//!
//! # Design
//!
//! Widgets opt in to property introspection by implementing [`Queryable`].
//! Callers hold the widget directly and query it by name:
//!
//! ```rust
//! use rlvgl_core::property::{Queryable, PropertyValue};
//!
//! struct MyWidget { label: String }
//!
//! impl Queryable for MyWidget {
//!     fn get_property(&self, key: &str) -> Option<PropertyValue> {
//!         match key {
//!             "text" => Some(PropertyValue::Text(self.label.clone())),
//!             _ => None,
//!         }
//!     }
//! }
//! ```
//!
//! Key strings follow LVGL's naming vocabulary (`"text"`, `"radius"`,
//! `"angle_start"`, `"frame_index"`, `"canvas_width"`, etc.).  A typed key
//! enum is deferred-Safe (Specification Required) and does not require changes
//! to this trait.
//!
//! # `no_std` notes
//!
//! `PropertyValue::Text` owns a heap-allocated [`alloc::string::String`], so
//! this module requires `alloc`.  All other variants are stack-resident.
//! Targets without a heap allocator MUST NOT use `PropertyValue::Text`
//! (they will fail to compile if they try to construct it).

extern crate alloc;

use alloc::string::String;

use crate::widget::Color;

/// A typed property value exchanged via [`Queryable`].
///
/// Variants mirror LVGL's `LV_PROPERTY_TYPE_*` set for the four types
/// available in v1.  The enum is `#[non_exhaustive]` so future additions
/// (e.g. `Float(f32)`, `TextRef(&'static str)`) are Specification Required
/// amendments that do not break existing `match` arms with `..` guards.
///
/// # Type mapping
///
/// | Variant | LVGL equivalent |
/// |---|---|
/// | `Int(i32)` | `LV_PROPERTY_TYPE_INT` |
/// | `Bool(bool)` | `LV_PROPERTY_TYPE_BOOL` |
/// | `Color(Color)` | `LV_PROPERTY_TYPE_COLOR` |
/// | `Text(String)` | `LV_PROPERTY_TYPE_TEXT` |
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// A 32-bit signed integer property (e.g. `"radius"`, `"frame_index"`).
    Int(i32),
    /// A boolean flag property (e.g. `"checked"`, `"hidden"`).
    Bool(bool),
    /// An RGBA color property (e.g. `"bg_color"`, `"text_color"`).
    Color(Color),
    /// A UTF-8 text property (e.g. `"text"`, `"placeholder"`).
    ///
    /// Requires `alloc`.  Use `PropertyValue::Text(s.into())` to convert from
    /// `&str`.
    Text(String),
}

impl PropertyValue {
    /// Extract the `Int` payload, returning `None` for other variants.
    ///
    /// ```rust
    /// use rlvgl_core::property::PropertyValue;
    /// assert_eq!(PropertyValue::Int(42).as_int(), Some(42));
    /// assert_eq!(PropertyValue::Bool(true).as_int(), None);
    /// ```
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Extract the `Bool` payload, returning `None` for other variants.
    ///
    /// ```rust
    /// use rlvgl_core::property::PropertyValue;
    /// assert_eq!(PropertyValue::Bool(true).as_bool(), Some(true));
    /// assert_eq!(PropertyValue::Int(1).as_bool(), None);
    /// ```
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Extract the `Color` payload, returning `None` for other variants.
    ///
    /// ```rust
    /// use rlvgl_core::property::PropertyValue;
    /// use rlvgl_core::widget::Color;
    /// let c = Color(255, 0, 0, 255);
    /// assert_eq!(PropertyValue::Color(c).as_color(), Some(c));
    /// assert_eq!(PropertyValue::Int(0).as_color(), None);
    /// ```
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(v) => Some(*v),
            _ => None,
        }
    }

    /// Extract a reference to the `Text` payload, returning `None` for other
    /// variants.
    ///
    /// ```rust
    /// use rlvgl_core::property::PropertyValue;
    /// assert_eq!(
    ///     PropertyValue::Text("hello".into()).as_text(),
    ///     Some("hello")
    /// );
    /// assert_eq!(PropertyValue::Bool(false).as_text(), None);
    /// ```
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

/// Per-widget, identity-free property accessor trait.
///
/// Widgets that want creator or playit introspection implement this trait.
/// The trait is deliberately decoupled from object identity (no
/// [`ObjectNode`](crate::object::ObjectNode) reference, no global registry) —
/// callers hold the widget directly and call `get_property` / `set_property`
/// on it.  This satisfies the LPAR-00 §9 mandate while deferring any
/// object-identity dependency to LPAR-02/04.
///
/// # Key strings
///
/// Key strings follow LVGL naming conventions.  Each widget defines its own
/// set; unknown keys return `None` / `false` without panicking.  A typed
/// key enum per widget is a Specification Required addition that does not
/// require changes to this trait.
///
/// # Default `set_property`
///
/// Read-only widgets need not override `set_property`; the default returns
/// `false` for every key.
pub trait Queryable {
    /// Read a named property value.
    ///
    /// Returns `None` if the key is unknown.  Implementations MUST NOT
    /// panic on unrecognised keys.
    fn get_property(&self, key: &str) -> Option<PropertyValue>;

    /// Write a named property value.
    ///
    /// Returns `true` if the property was recognised **and** the supplied
    /// value was of the correct type and applied successfully.  Returns
    /// `false` for unknown keys **and** for type mismatches — in the latter
    /// case the widget state MUST NOT be corrupted.
    ///
    /// The default implementation returns `false` for every key, making
    /// the trait safe to impl on read-only widgets without an override.
    fn set_property(&mut self, key: &str, value: PropertyValue) -> bool {
        let _ = (key, value);
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    // -----------------------------------------------------------------------
    // A minimal widget for round-trip tests
    // -----------------------------------------------------------------------

    struct Counter {
        count: i32,
        enabled: bool,
        label: String,
        tint: Color,
    }

    impl Counter {
        fn new() -> Self {
            Self {
                count: 0,
                enabled: true,
                label: "counter".into(),
                tint: Color(0, 128, 255, 255),
            }
        }
    }

    impl Queryable for Counter {
        fn get_property(&self, key: &str) -> Option<PropertyValue> {
            match key {
                "count" => Some(PropertyValue::Int(self.count)),
                "enabled" => Some(PropertyValue::Bool(self.enabled)),
                "label" => Some(PropertyValue::Text(self.label.clone())),
                "tint" => Some(PropertyValue::Color(self.tint)),
                _ => None,
            }
        }

        fn set_property(&mut self, key: &str, value: PropertyValue) -> bool {
            match (key, value) {
                ("count", PropertyValue::Int(v)) => {
                    self.count = v;
                    true
                }
                ("enabled", PropertyValue::Bool(v)) => {
                    self.enabled = v;
                    true
                }
                ("label", PropertyValue::Text(v)) => {
                    self.label = v;
                    true
                }
                ("tint", PropertyValue::Color(v)) => {
                    self.tint = v;
                    true
                }
                // Type mismatch or unknown key — no mutation
                _ => false,
            }
        }
    }

    // -----------------------------------------------------------------------
    // get_property round-trip tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_int_property_round_trip() {
        let c = Counter::new();
        assert_eq!(c.get_property("count"), Some(PropertyValue::Int(0)));
    }

    #[test]
    fn get_bool_property_round_trip() {
        let c = Counter::new();
        assert_eq!(c.get_property("enabled"), Some(PropertyValue::Bool(true)));
    }

    #[test]
    fn get_text_property_round_trip() {
        let c = Counter::new();
        assert_eq!(
            c.get_property("label"),
            Some(PropertyValue::Text("counter".into()))
        );
    }

    #[test]
    fn get_color_property_round_trip() {
        let c = Counter::new();
        assert_eq!(
            c.get_property("tint"),
            Some(PropertyValue::Color(Color(0, 128, 255, 255)))
        );
    }

    #[test]
    fn get_unknown_property_returns_none() {
        let c = Counter::new();
        assert_eq!(c.get_property("nonexistent"), None);
    }

    // -----------------------------------------------------------------------
    // set_property tests
    // -----------------------------------------------------------------------

    #[test]
    fn set_int_property_accepted_and_applied() {
        let mut c = Counter::new();
        let accepted = c.set_property("count", PropertyValue::Int(99));
        assert!(accepted);
        assert_eq!(c.count, 99);
    }

    #[test]
    fn set_bool_property_accepted_and_applied() {
        let mut c = Counter::new();
        let accepted = c.set_property("enabled", PropertyValue::Bool(false));
        assert!(accepted);
        assert!(!c.enabled);
    }

    #[test]
    fn set_text_property_accepted_and_applied() {
        let mut c = Counter::new();
        let accepted = c.set_property("label", PropertyValue::Text("new".into()));
        assert!(accepted);
        assert_eq!(c.label, "new");
    }

    #[test]
    fn set_color_property_accepted_and_applied() {
        let mut c = Counter::new();
        let new_color = Color(255, 0, 0, 255);
        let accepted = c.set_property("tint", PropertyValue::Color(new_color));
        assert!(accepted);
        assert_eq!(c.tint, new_color);
    }

    #[test]
    fn set_unknown_property_returns_false() {
        let mut c = Counter::new();
        let accepted = c.set_property("bogus", PropertyValue::Int(1));
        assert!(!accepted);
    }

    #[test]
    fn set_property_type_mismatch_returns_false_and_no_corruption() {
        let mut c = Counter::new();
        let original_count = c.count;
        // Pass a Bool where Int is expected
        let accepted = c.set_property("count", PropertyValue::Bool(true));
        assert!(!accepted, "type mismatch must be rejected");
        assert_eq!(c.count, original_count, "state must not be corrupted");
    }

    #[test]
    fn set_property_type_mismatch_text_for_int_no_corruption() {
        let mut c = Counter::new();
        let original = c.count;
        let accepted = c.set_property("count", PropertyValue::Text("five".into()));
        assert!(!accepted);
        assert_eq!(c.count, original);
    }

    // -----------------------------------------------------------------------
    // PropertyValue accessor helpers
    // -----------------------------------------------------------------------

    #[test]
    fn as_int_correct_variant() {
        assert_eq!(PropertyValue::Int(7).as_int(), Some(7));
    }

    #[test]
    fn as_int_wrong_variant() {
        assert_eq!(PropertyValue::Bool(true).as_int(), None);
    }

    #[test]
    fn as_bool_correct_variant() {
        assert_eq!(PropertyValue::Bool(false).as_bool(), Some(false));
    }

    #[test]
    fn as_bool_wrong_variant() {
        assert_eq!(PropertyValue::Int(0).as_bool(), None);
    }

    #[test]
    fn as_color_correct_variant() {
        let c = Color(1, 2, 3, 4);
        assert_eq!(PropertyValue::Color(c).as_color(), Some(c));
    }

    #[test]
    fn as_color_wrong_variant() {
        assert_eq!(PropertyValue::Text("x".into()).as_color(), None);
    }

    #[test]
    fn as_text_correct_variant() {
        assert_eq!(PropertyValue::Text("hello".into()).as_text(), Some("hello"));
    }

    #[test]
    fn as_text_wrong_variant() {
        assert_eq!(PropertyValue::Int(0).as_text(), None);
    }

    // -----------------------------------------------------------------------
    // Default set_property on a read-only widget
    // -----------------------------------------------------------------------

    struct ReadOnly;

    impl Queryable for ReadOnly {
        fn get_property(&self, key: &str) -> Option<PropertyValue> {
            match key {
                "version" => Some(PropertyValue::Int(1)),
                _ => None,
            }
        }
        // set_property not overridden → default returns false
    }

    #[test]
    fn default_set_property_returns_false() {
        let mut ro = ReadOnly;
        assert!(!ro.set_property("version", PropertyValue::Int(2)));
        // And state is trivially unchanged (ReadOnly has no mutable state)
    }

    #[test]
    fn property_value_clone_and_eq() {
        let v = PropertyValue::Text("abc".to_string());
        assert_eq!(v.clone(), v);
    }
}
