//! RLVGL-owned portable cell widgets over unmodified Ratatui core primitives.
//!
//! This module is a bounded compatibility surface, not a replacement for the
//! upstream widgets facade. It does not depend on `ratatui-widgets`.

#[cfg(feature = "portable-atomic")]
use alloc::boxed::Box;
#[cfg(not(feature = "portable-atomic"))]
use alloc::sync::Arc;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::BitOr;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;
#[cfg(feature = "portable-atomic")]
use portable_atomic_util::Arc;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Offset, Position, Rect};
use ratatui_core::style::{Style, Styled};
use ratatui_core::text::{Line, Text};
use ratatui_core::widgets::Widget;

#[cfg(all(not(target_has_atomic = "ptr"), not(feature = "portable-atomic")))]
compile_error!(
    "ratatui-rlvgl requires its portable-atomic feature on targets without pointer atomics"
);

/// A shared cell effect with the same thread and panic bounds as native shadows.
pub trait CellEffect: fmt::Debug + Send + Sync + UnwindSafe + RefUnwindSafe {
    /// Apply an effect after shadow style, using the clipped shadow and base areas.
    fn apply(&self, shadow_area: Rect, base_area: Rect, buffer: &mut Buffer);
}

#[derive(Debug, Clone)]
enum Effect {
    Overlay,
    Symbol(&'static str),
    Custom(Arc<dyn CellEffect>),
}

impl PartialEq for Effect {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Overlay, Self::Overlay) => true,
            (Self::Symbol(a), Self::Symbol(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
impl Eq for Effect {}
impl Hash for Effect {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Overlay => 0_u8.hash(state),
            Self::Symbol(symbol) => {
                1_u8.hash(state);
                symbol.hash(state);
            }
            Self::Custom(effect) => {
                2_u8.hash(state);
                ptr::hash(Arc::as_ptr(effect), state);
            }
        }
    }
}

/// An owned shadow rendered with native core geometry and buffer semantics.
///
/// Custom effects share identity when cloned. Their ownership uses portable Arc
/// when the `portable-atomic` feature is enabled; no upstream widget is loaded.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Shadow {
    effect: Effect,
    style: Style,
    offset: Offset,
}

impl Shadow {
    /// Apply style without replacing existing symbols.
    pub fn overlay() -> Self {
        Self {
            effect: Effect::Overlay,
            style: Style::default(),
            offset: Offset::new(1, 1),
        }
    }
    /// Fill the exposed shadow cells with full block symbols.
    pub fn block() -> Self {
        Self::symbol("█")
    }
    /// Fill the exposed shadow cells with light shade symbols.
    pub fn light_shade() -> Self {
        Self::symbol("░")
    }
    /// Fill the exposed shadow cells with medium shade symbols.
    pub fn medium_shade() -> Self {
        Self::symbol("▒")
    }
    /// Fill the exposed shadow cells with dark shade symbols.
    pub fn dark_shade() -> Self {
        Self::symbol("▓")
    }
    /// Fill exposed cells with a supplied terminal symbol.
    pub fn symbol(symbol: &'static str) -> Self {
        Self {
            effect: Effect::Symbol(symbol),
            ..Self::overlay()
        }
    }
    /// Share a custom effect without weakening its thread or panic bounds.
    pub fn custom<F: CellEffect + 'static>(effect: F) -> Self {
        #[cfg(feature = "portable-atomic")]
        let effect = Arc::from(Box::new(effect) as Box<dyn CellEffect>);
        #[cfg(not(feature = "portable-atomic"))]
        let effect = Arc::new(effect);
        Self {
            effect: Effect::Custom(effect),
            ..Self::overlay()
        }
    }
    /// Set the style applied before the effect.
    #[must_use]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
    /// Set the signed offset interpreted by native `Rect::offset`.
    #[must_use]
    pub const fn offset(mut self, offset: Offset) -> Self {
        self.offset = offset;
        self
    }
}
impl Default for Shadow {
    fn default() -> Self {
        Self::overlay()
    }
}
impl Styled for Shadow {
    type Item = Self;
    fn style(&self) -> Style {
        self.style
    }
    fn set_style<S: Into<Style>>(self, style: S) -> Self {
        self.style(style)
    }
}
impl Widget for &Shadow {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let shadow_area = area.offset(self.offset).intersection(buffer.area);
        shadow_cells(shadow_area, area, buffer, |x, y, buffer| {
            buffer[(x, y)].set_style(self.style);
        });
        match &self.effect {
            Effect::Overlay => {}
            Effect::Symbol(symbol) => shadow_cells(shadow_area, area, buffer, |x, y, buffer| {
                buffer[(x, y)].set_symbol(symbol);
            }),
            Effect::Custom(effect) => effect.apply(shadow_area, area, buffer),
        }
    }
}
impl Widget for Shadow {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self).render(area, buffer);
    }
}
fn shadow_cells(
    area: Rect,
    base: Rect,
    buffer: &mut Buffer,
    mut visit: impl FnMut(u16, u16, &mut Buffer),
) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if !base.contains(Position { x, y }) {
                visit(x, y, buffer);
            }
        }
    }
}

/// Compose an owned shadow with any native core widget, including move-only widgets.
#[derive(Debug, Clone)]
pub struct WithShadow<W> {
    widget: W,
    shadow: Shadow,
}
impl<W> WithShadow<W> {
    /// Render `shadow` first, then move `widget` into its native widget renderer.
    pub const fn new(widget: W, shadow: Shadow) -> Self {
        Self { widget, shadow }
    }
}
impl<W: Widget> Widget for WithShadow<W> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self.shadow).render(area, buffer);
        self.widget.render(area, buffer);
    }
}

/// Border sides supported by the owned panel subset.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct Borders(u8);
impl Borders {
    /// No borders.
    pub const NONE: Self = Self(0);
    /// Top border.
    const TOP: Self = Self(1);
    /// Right border.
    const RIGHT: Self = Self(2);
    /// Bottom border.
    const BOTTOM: Self = Self(4);
    /// Left border.
    const LEFT: Self = Self(8);
    /// All four borders.
    pub const ALL: Self = Self(15);
    /// Whether every supplied side is selected.
    pub const fn contains(self, sides: Self) -> bool {
        self.0 & sides.0 == sides.0
    }
}
impl BitOr for Borders {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// A bounded owned background, plain-border and left-title panel.
///
/// This covers the battery dashboard's calls rather than re-exporting the
/// upstream Block API. Existing symbols are retained except borders and title.
#[derive(Debug, Clone, Default)]
pub struct Panel<'a> {
    borders: Borders,
    title: Option<Line<'a>>,
    style: Style,
    border_style: Style,
}
impl<'a> Panel<'a> {
    /// Select border sides.
    #[must_use]
    pub const fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }
    /// Set a top-left title inside horizontal borders.
    #[must_use]
    pub fn title<T: Into<Line<'a>>>(mut self, title: T) -> Self {
        self.title = Some(title.into());
        self
    }
    /// Set the background and base cell style without erasing existing symbols.
    #[must_use]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
    /// Set the border and default title style over the panel style.
    #[must_use]
    pub fn border_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.border_style = style.into();
        self
    }
    /// Return content geometry, reserving borders and a title row with saturating extents.
    pub fn inner(&self, area: Rect) -> Rect {
        let left = u16::from(self.borders.contains(Borders::LEFT));
        let right = u16::from(self.borders.contains(Borders::RIGHT));
        let top = u16::from(self.borders.contains(Borders::TOP) || self.title.is_some());
        let bottom = u16::from(self.borders.contains(Borders::BOTTOM));
        Rect::new(
            area.x.saturating_add(left),
            area.y.saturating_add(top),
            area.width.saturating_sub(left + right),
            area.height.saturating_sub(top + bottom),
        )
    }
}
impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let visible = area.intersection(buffer.area);
        if visible.is_empty() {
            return;
        }
        buffer.set_style(visible, self.style);
        let right = area.right().saturating_sub(1);
        let bottom = area.bottom().saturating_sub(1);
        let mut put = |x, y, symbol| {
            if visible.contains(Position { x, y }) {
                buffer[(x, y)]
                    .set_symbol(symbol)
                    .set_style(self.border_style);
            }
        };
        if self.borders.contains(Borders::TOP) {
            for x in area.left()..area.right() {
                put(x, area.y, "─");
            }
        }
        if self.borders.contains(Borders::BOTTOM) {
            for x in area.left()..area.right() {
                put(x, bottom, "─");
            }
        }
        if self.borders.contains(Borders::LEFT) {
            for y in area.top()..area.bottom() {
                put(area.x, y, "│");
            }
        }
        if self.borders.contains(Borders::RIGHT) {
            for y in area.top()..area.bottom() {
                put(right, y, "│");
            }
        }
        if self.borders.contains(Borders::TOP | Borders::LEFT) {
            put(area.x, area.y, "┌");
        }
        if self.borders.contains(Borders::TOP | Borders::RIGHT) {
            put(right, area.y, "┐");
        }
        if self.borders.contains(Borders::BOTTOM | Borders::LEFT) {
            put(area.x, bottom, "└");
        }
        if self.borders.contains(Borders::BOTTOM | Borders::RIGHT) {
            put(right, bottom, "┘");
        }
        if let Some(title) = &self.title {
            let left = u16::from(self.borders.contains(Borders::LEFT));
            let right = u16::from(self.borders.contains(Borders::RIGHT));
            let title_area = Rect::new(
                area.x.saturating_add(left),
                area.y,
                area.width.saturating_sub(left + right),
                1,
            );
            buffer.set_style(title_area.intersection(buffer.area), self.border_style);
            let mut title = title.clone();
            title.alignment = Some(Alignment::Left);
            title.render(title_area, buffer);
        }
    }
}
impl Widget for Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self).render(area, buffer);
    }
}

/// Left-aligned, hard-line text with native styles and logical-origin clipping.
///
/// This subset intentionally has no wrapping or scrolling API. It does not load
/// the upstream Paragraph/widget facade on no-pointer-atomic targets.
#[derive(Debug, Clone)]
pub struct TextPanel<'a> {
    text: Text<'a>,
    style: Style,
}
impl<'a> TextPanel<'a> {
    /// Accept native Text, Line, borrowed text or an owned String.
    pub fn new<T: Into<Text<'a>>>(text: T) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
        }
    }
    /// Set the base style, underneath explicit native Text/Line/Span styles.
    #[must_use]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}
impl Widget for &TextPanel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let visible = area.intersection(buffer.area);
        if visible.is_empty() {
            return;
        }
        buffer.set_style(visible, self.style.patch(self.text.style));
        let skipped = usize::from(visible.y.saturating_sub(area.y));
        for (line, row) in self.text.iter().skip(skipped).zip(visible.rows()) {
            let mut line = line.clone();
            line.alignment = Some(Alignment::Left);
            line.render(Rect::new(area.x, row.y, area.width, 1), buffer);
        }
    }
}
impl Widget for TextPanel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self).render(area, buffer);
    }
}

/// An explicit display-width-centered value label over a caller-owned bar buffer.
///
/// It styles the bottom row without drawing a bar body. Label placement is in
/// display columns, not UTF-8 bytes; existing curated-font policy is unchanged.
/// This does not modify or substitute the native `BarChart` API.
#[derive(Debug, Clone)]
pub struct BarValueLabel<'a> {
    text: Line<'a>,
    value_style: Style,
    bar_style: Style,
}
impl<'a> BarValueLabel<'a> {
    /// Accept a native Line, borrowed label or owned String.
    pub fn new<T: Into<Line<'a>>>(text: T) -> Self {
        Self {
            text: text.into(),
            value_style: Style::default(),
            bar_style: Style::default(),
        }
    }
    /// Set glyph style over the bar row; explicit native Line/Span styles take priority.
    #[must_use]
    pub fn value_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.value_style = style.into();
        self
    }
    /// Set the base style of the visible label row without replacing its symbols.
    #[must_use]
    pub fn bar_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.bar_style = style.into();
        self
    }
}
impl Widget for &BarValueLabel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let row = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        let visible = row.intersection(buffer.area);
        if visible.is_empty() {
            return;
        }
        buffer.set_style(visible, self.bar_style);
        let width = self.text.width().min(usize::from(u16::MAX)) as u16;
        let padding = area.width.saturating_sub(width) / 2;
        let mut text = self.text.clone();
        text.style = self.value_style.patch(text.style);
        text.alignment = Some(Alignment::Left);
        text.render(
            Rect::new(
                area.x.saturating_add(padding),
                row.y,
                area.width.saturating_sub(padding),
                1,
            ),
            buffer,
        );
    }
}
impl Widget for BarValueLabel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self).render(area, buffer);
    }
}
