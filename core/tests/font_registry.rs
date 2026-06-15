//! FONT-05 font-registry + cascade→widget font-bridge conformance (FONT-05 §12).
//!
//! Exercises the public surface from a consumer's perspective:
//!   - `FontRegistry::resolve` hit / miss / `DEFAULT` (§5.A),
//!   - `apply_font_registry` pushing a cascade-resolved `font_id` into a
//!     widget's `WidgetFont` slot via the defaulted `Widget::widget_font_mut`
//!     sink (§5.B/§5.C),
//!   - precedence (registered overrides; `DEFAULT`/unmapped preserves an
//!     explicit `set_font`) + idempotency (§5.D),
//!   - inheritance: a child with no own `font_id` inherits the parent's
//!     registered font through the cascade (§5.C, reusing LPAR-07 inheritance).

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::event::Event;
use rlvgl_core::font::{FontId, FontMetrics, FontRegistry, WidgetFont, apply_font_registry};
use rlvgl_core::object::ObjectNode;
use rlvgl_core::packed_font::{GlyphMetric, PackedFont};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style_cascade::{Part, Selector, StylePatch};
use rlvgl_core::widget::{Rect, Widget};

// A synthetic AA font with a line height (10) distinct from FONT_6X10 (20),
// so "which font resolved" is observable via line metrics.
static GLYPHS: [GlyphMetric; 1] = [GlyphMetric {
    ch: 'A',
    width: 3,
    height: 3,
    advance_fp16: 64,
    offset: 0,
    ymin: 0,
}];
static DATA: [u8; 9] = [0; 9];
static PACKED: PackedFont = PackedFont {
    height: 10,
    ascent: 8,
    glyphs: &GLYPHS,
    data: &DATA,
};

// FontId(1) → PACKED. FontId(2) is intentionally unregistered; FontId::DEFAULT
// never resolves (§5.A). The app keeps the entry array in scope and builds the
// `Copy` registry from `&entries` (§5.A/§5.E) — `&dyn FontMetrics` is `!Sync`,
// so a named `static` table is rejected.
fn entries() -> [(FontId, &'static dyn FontMetrics); 1] {
    [(FontId(1), &PACKED)]
}

/// Minimal text-bearing widget: holds a `WidgetFont` and exposes it through the
/// FONT-05 §5.B sink, exactly as the real text widgets do.
struct FontProbe {
    font: WidgetFont,
}

impl Widget for FontProbe {
    fn bounds(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }
    }
    fn draw(&self, _r: &mut dyn Renderer) {}
    fn handle_event(&mut self, _e: &Event) -> bool {
        false
    }
    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }
}

fn probe() -> Rc<RefCell<FontProbe>> {
    Rc::new(RefCell::new(FontProbe {
        font: WidgetFont::new(),
    }))
}

/// Resolved line height of a probe's font: 20 = FONT_6X10 default, 10 = PACKED.
fn line_height(rc: &Rc<RefCell<FontProbe>>) -> u16 {
    rc.borrow().font.resolve().line_metrics().line_height
}

fn font_id_style(id: u16) -> StylePatch {
    StylePatch {
        font_id: Some(FontId(id)),
        ..StylePatch::new()
    }
}

// ---------------------------------------------------------------------------
// §5.A — FontRegistry::resolve
// ---------------------------------------------------------------------------

#[test]
fn registry_resolves_hit_miss_and_default() {
    let e = entries();
    let reg = FontRegistry::new(&e);
    assert!(reg.resolve(FontId(1)).is_some(), "registered id resolves");
    assert!(reg.resolve(FontId(2)).is_none(), "unregistered id is None");
    assert!(
        reg.resolve(FontId::DEFAULT).is_none(),
        "DEFAULT never overrides (§5.A)"
    );
    assert!(
        FontRegistry::EMPTY.resolve(FontId(1)).is_none(),
        "empty registry resolves nothing"
    );
}

// ---------------------------------------------------------------------------
// §5.B/§5.C — apply_font_registry pushes the cascade font_id into the widget
// ---------------------------------------------------------------------------

#[test]
fn apply_sets_widget_font_from_cascade_font_id() {
    let rc = probe();
    assert_eq!(line_height(&rc), 20, "starts at FONT_6X10 default");

    let mut node = ObjectNode::new(rc.clone());
    node.add_local_style(font_id_style(1), Selector::part(Part::MAIN));

    let e = entries();
    apply_font_registry(&node, &FontRegistry::new(&e));
    assert_eq!(
        line_height(&rc),
        10,
        "registry font_id=1 → PACKED reached the widget"
    );
}

// ---------------------------------------------------------------------------
// §5.D — precedence (DEFAULT/unmapped preserves explicit) + idempotency
// ---------------------------------------------------------------------------

#[test]
fn default_font_id_preserves_explicit_set_font() {
    let rc = probe();
    // Simulate an explicit set_font(&PACKED) by the application.
    rc.borrow_mut().font.set(&PACKED);
    assert_eq!(line_height(&rc), 10);

    // A node with no font_id resolves to DEFAULT → registry returns None →
    // the pass must NOT clear the explicit assignment (would drop to 20).
    let node = ObjectNode::new(rc.clone());
    let e = entries();
    apply_font_registry(&node, &FontRegistry::new(&e));
    assert_eq!(
        line_height(&rc),
        10,
        "DEFAULT font_id left the explicit set_font intact (§5.D)"
    );
}

#[test]
fn unmapped_font_id_preserves_explicit_set_font() {
    let rc = probe();
    rc.borrow_mut().font.set(&PACKED);

    // font_id=2 is unregistered → resolve None → preserve.
    let mut node = ObjectNode::new(rc.clone());
    node.add_local_style(font_id_style(2), Selector::part(Part::MAIN));
    let e = entries();
    apply_font_registry(&node, &FontRegistry::new(&e));
    assert_eq!(
        line_height(&rc),
        10,
        "unmapped font_id preserved explicit font"
    );
}

#[test]
fn apply_is_idempotent() {
    let rc = probe();
    let mut node = ObjectNode::new(rc.clone());
    node.add_local_style(font_id_style(1), Selector::part(Part::MAIN));
    let e = entries();
    let reg = FontRegistry::new(&e);

    apply_font_registry(&node, &reg);
    let first = line_height(&rc);
    apply_font_registry(&node, &reg);
    let second = line_height(&rc);
    assert_eq!(first, 10);
    assert_eq!(first, second, "re-running the pass is stable");
}

// ---------------------------------------------------------------------------
// §5.C — inheritance through the cascade
// ---------------------------------------------------------------------------

#[test]
fn child_inherits_registered_font_via_cascade() {
    let parent_rc = probe();
    let child_rc = probe();

    let mut parent = ObjectNode::new(parent_rc.clone());
    parent.add_local_style(font_id_style(1), Selector::part(Part::MAIN));
    // Child has no own font_id; it must inherit the parent's via the cascade.
    let child = ObjectNode::new(child_rc.clone());
    parent.append_child(child);

    let e = entries();
    apply_font_registry(&parent, &FontRegistry::new(&e));

    assert_eq!(
        line_height(&parent_rc),
        10,
        "parent resolved its registered font"
    );
    assert_eq!(
        line_height(&child_rc),
        10,
        "child inherited parent's font_id → registered font"
    );
}

// ---------------------------------------------------------------------------
// Backward-compat: a default-font_id tree leaves widgets at FONT_6X10.
// ---------------------------------------------------------------------------

#[test]
fn untouched_tree_keeps_default_font() {
    let rc = probe();
    let node = ObjectNode::new(rc.clone());
    let e = entries();
    apply_font_registry(&node, &FontRegistry::new(&e));
    assert_eq!(
        line_height(&rc),
        20,
        "no font_id → FONT_6X10 default unchanged"
    );
}
