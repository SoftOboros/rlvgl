//! Theme trait and basic implementations.
//!
//! This module provides two levels of theming:
//!
//! - [`Theme`] — a simple property-level hook on [`Style`]. Unchanged from
//!   earlier LPAR phases.
//! - [`LparTheme`] / [`WidgetClass`] / [`DefaultTheme`] — LPAR-07 §9 node-level
//!   theme chaining. [`LparTheme::apply_to_node`] calls
//!   [`ObjectNode::add_local_style`] with lowest-precedence defaults so that
//!   per-node overrides continue to win.

use crate::object::ObjectNode;
use crate::style::Style;
use crate::style_cascade::{Part, Selector, StylePatch};
use crate::widget::Color;

// ---------------------------------------------------------------------------
// Existing Theme trait + implementations (unchanged)
// ---------------------------------------------------------------------------

/// Global theme that can modify widget styles.
///
/// Themes provide a simple hook to set initial colors and other stylistic
/// properties for widgets. Applications can implement this trait to provide
/// bespoke looks across the UI.
pub trait Theme {
    /// Apply the theme to the provided [`Style`].
    fn apply(&self, style: &mut Style);
}

/// Simple light theme implementation.
pub struct LightTheme;

impl Theme for LightTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(255, 255, 255, 255);
        style.border_color = Color(0, 0, 0, 255);
    }
}

/// Simple dark theme implementation.
pub struct DarkTheme;

impl Theme for DarkTheme {
    fn apply(&self, style: &mut Style) {
        style.bg_color = Color(0, 0, 0, 255);
        style.border_color = Color(255, 255, 255, 255);
    }
}

// ---------------------------------------------------------------------------
// WidgetClass — LPAR-07 §9
// ---------------------------------------------------------------------------

/// Identifies the semantic widget type for theme application (LPAR-07 §9.1).
///
/// Passed to [`LparTheme::apply_to_node`] so the theme can select per-class
/// default patches without inspecting the node's widget instance.
///
/// Registration policy: **Specification Required** — adding a variant requires
/// a phase-doc amendment that updates the §9.1 table and cites the owning
/// widget phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetClass {
    /// Root screen / top-level container.
    Screen,
    /// Push-button widget.
    Button,
    /// Text label widget.
    Label,
    /// Generic container or panel.
    Container,
    /// Slider control.
    Slider,
    /// On/off toggle switch.
    Switch,
    /// Checkbox control.
    Checkbox,
    /// Scrollable list widget.
    List,
}

// ---------------------------------------------------------------------------
// LparTheme trait
// ---------------------------------------------------------------------------

/// Node-level theme chaining for the LPAR-07 cascade (§9).
///
/// Implementors call [`ObjectNode::add_local_style`] (or equivalent cascade
/// methods) to inject the lowest-precedence defaults for each [`WidgetClass`].
/// Because these are added as local styles they have Tier-1 precedence and
/// will be beaten by any explicit per-node style overrides added afterwards.
///
/// # Design note
///
/// The intentional split from [`Theme`] is that `Theme` modifies a bare
/// [`Style`] value (useful for resolving a standalone style outside a tree),
/// while `LparTheme` operates on a live [`ObjectNode`] and hooks into the full
/// cascade. Both coexist; they target different use cases.
pub trait LparTheme {
    /// Apply class-appropriate style defaults to `node` via the cascade.
    ///
    /// Implementors should call `node.add_local_style(patch, selector)` for
    /// each property they want to default. Per the LPAR-07 §9 contract, these
    /// calls MUST use [`Selector::part`]`(`[`Part::MAIN`]`)` for base defaults
    /// (state-specific overrides are allowed via [`Selector::new`]).
    fn apply_to_node(&self, node: &mut ObjectNode, class: WidgetClass);
}

// ---------------------------------------------------------------------------
// DefaultTheme — LPAR-07 §9.3 reference implementation
// ---------------------------------------------------------------------------

/// LPAR-07 default theme: Material-inspired light palette as lowest-precedence
/// baseline defaults (§9.3).
///
/// Applying this theme injects local style patches into the cascade so that
/// any later call to [`crate::style_cascade::resolve`] starts from a
/// principled visual foundation rather than the bare property defaults in
/// [`Style::default`].
///
/// Applications that want a custom look override on top of this baseline by
/// calling [`ObjectNode::add_local_style`] after `DefaultTheme::apply_to_node`.
pub struct DefaultTheme;

/// Primary brand color used by [`DefaultTheme`] for interactive elements.
const DEFAULT_PRIMARY: Color = Color(98, 0, 238, 255);
/// Surface background color used by [`DefaultTheme`].
const DEFAULT_SURFACE: Color = Color(255, 255, 255, 255);
/// Transparent color constant for container backgrounds.
const TRANSPARENT: Color = Color(0, 0, 0, 0);
/// Default text label foreground.
const DEFAULT_ON_SURFACE: Color = Color(33, 33, 33, 255);
/// Default corner radius for buttons.
const BUTTON_RADIUS: u8 = 6;
/// Default border width for buttons.
const BUTTON_BORDER_WIDTH: u8 = 1;

impl LparTheme for DefaultTheme {
    fn apply_to_node(&self, node: &mut ObjectNode, class: WidgetClass) {
        let sel = Selector::part(Part::MAIN);
        // Theme defaults go in the lowest-precedence theme tier (LPAR-07 §9.1),
        // so widget/application styles always win over the theme regardless of
        // registration order. Clear first so a re-apply replaces cleanly.
        node.clear_theme_styles();
        match class {
            WidgetClass::Button => {
                // Buttons: primary bg, rounded corners, thin border.
                node.add_theme_style(
                    StylePatch {
                        bg_color: Some(DEFAULT_PRIMARY),
                        border_color: Some(DEFAULT_PRIMARY),
                        border_width: Some(BUTTON_BORDER_WIDTH),
                        radius: Some(BUTTON_RADIUS),
                        alpha: None,
                    },
                    sel,
                );
            }
            WidgetClass::Label => {
                // Labels: transparent background, visible text by default.
                // (Text color is not yet in StylePatch; set bg/alpha).
                node.add_theme_style(
                    StylePatch {
                        bg_color: Some(TRANSPARENT),
                        alpha: Some(255),
                        border_color: None,
                        border_width: Some(0),
                        radius: Some(0),
                    },
                    sel,
                );
            }
            WidgetClass::Container | WidgetClass::Screen => {
                // Containers and screens: white surface, no border.
                node.add_theme_style(
                    StylePatch {
                        bg_color: Some(DEFAULT_SURFACE),
                        border_width: Some(0),
                        border_color: None,
                        radius: Some(0),
                        alpha: Some(255),
                    },
                    sel,
                );
            }
            WidgetClass::Slider | WidgetClass::Switch | WidgetClass::Checkbox => {
                // Interactive controls: primary accent color.
                node.add_theme_style(
                    StylePatch {
                        bg_color: Some(DEFAULT_PRIMARY),
                        alpha: Some(255),
                        border_color: None,
                        border_width: Some(0),
                        radius: Some(4),
                    },
                    sel,
                );
            }
            WidgetClass::List => {
                // Lists: surface background, slight border.
                node.add_theme_style(
                    StylePatch {
                        bg_color: Some(DEFAULT_SURFACE),
                        border_color: Some(DEFAULT_ON_SURFACE),
                        border_width: Some(1),
                        radius: Some(2),
                        alpha: Some(255),
                    },
                    sel,
                );
            }
        }
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
    use crate::object::ObjectNode;
    use crate::style_cascade::{InheritedContext, Part, resolve};
    use crate::widget::{Rect, Widget};

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

    // -----------------------------------------------------------------------
    // Theme (original)
    // -----------------------------------------------------------------------

    #[test]
    fn light_theme_sets_bg_white() {
        let mut style = Style::default();
        LightTheme.apply(&mut style);
        assert_eq!(style.bg_color, Color(255, 255, 255, 255));
    }

    #[test]
    fn dark_theme_sets_bg_black() {
        let mut style = Style::default();
        DarkTheme.apply(&mut style);
        assert_eq!(style.bg_color, Color(0, 0, 0, 255));
    }

    // -----------------------------------------------------------------------
    // LPAR-07 §9 DefaultTheme / LparTheme
    // -----------------------------------------------------------------------

    #[test]
    fn default_theme_applies_to_button() {
        let mut node = make_node();
        DefaultTheme.apply_to_node(&mut node, WidgetClass::Button);
        let (style, _) = resolve(
            node.style.as_deref(),
            node.meta().states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(
            style.bg_color, DEFAULT_PRIMARY,
            "button bg should be primary color"
        );
        assert_eq!(style.radius, BUTTON_RADIUS, "button radius from theme");
        assert_eq!(
            style.border_width, BUTTON_BORDER_WIDTH,
            "button border width from theme"
        );
    }

    #[test]
    fn local_override_wins_over_theme() {
        let mut node = make_node();
        // Apply theme first.
        DefaultTheme.apply_to_node(&mut node, WidgetClass::Button);

        // Then add a node-local override (higher precedence, added later).
        node.add_local_style(
            StylePatch {
                bg_color: Some(Color(255, 0, 0, 255)),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );

        let (style, _) = resolve(
            node.style.as_deref(),
            node.meta().states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        // The per-node red override wins over the theme's primary color.
        assert_eq!(
            style.bg_color,
            Color(255, 0, 0, 255),
            "local override must win over DefaultTheme"
        );
    }

    #[test]
    fn local_override_wins_regardless_of_apply_order() {
        // Order-independence: apply the local override FIRST, then the theme.
        // The theme lives in the lowest-precedence theme tier (§9.1), so the
        // local override still wins — this would fail if the theme used the
        // local tier and relied on last-added-wins ordering.
        let mut node = make_node();
        node.add_local_style(
            StylePatch {
                bg_color: Some(Color(255, 0, 0, 255)),
                ..StylePatch::new()
            },
            Selector::part(Part::MAIN),
        );
        DefaultTheme.apply_to_node(&mut node, WidgetClass::Button);

        let (style, _) = resolve(
            node.style.as_deref(),
            node.meta().states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(
            style.bg_color,
            Color(255, 0, 0, 255),
            "local override must win even when the theme is applied afterward"
        );
    }

    #[test]
    fn default_theme_container_sets_surface_bg() {
        let mut node = make_node();
        DefaultTheme.apply_to_node(&mut node, WidgetClass::Container);
        let (style, _) = resolve(
            node.style.as_deref(),
            node.meta().states(),
            Part::MAIN,
            &InheritedContext::EMPTY,
        );
        assert_eq!(
            style.bg_color, DEFAULT_SURFACE,
            "container bg should be surface white"
        );
    }
}
