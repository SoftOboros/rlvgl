//! Basic text label.
use alloc::string::String;
use rlvgl_core::actor::{
    ActorCapabilities, ActorFamily, ActorPreparation, ChildPolicy, ConstructedActor,
    ConstructorArgs, ConstructorFieldDescriptor, LayoutCapabilities, MPY_BASIC_STYLE_PARTS,
    MpyActor, MutationEffects, PropertyAccess, PropertyConstraint, PropertyDefault,
    PropertyDescriptor, RegistryError, ResourceCost, TargetSet, TypeDescriptor, TypeId, ValueTag,
    construct_native_actor,
};
use rlvgl_core::direction::{ActorDirection, OwnedValue};
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Simple text element.
pub struct Label {
    bounds: Rect,
    text: String,
    /// Visual style of the label background.
    pub style: Style,
    /// Color used to render the text.
    #[deprecated(note = "use the resolved TextStyle text_color cascade when drawing labels")]
    pub text_color: Color,
    /// Font assignment for this label (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Label {
    /// Create a new label with the provided text and bounds.
    #[allow(deprecated)]
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this label (FONT-00 §5).
    ///
    /// Pass any process-lifetime [`FontMetrics`] — e.g. a `PackedFont` for
    /// anti-aliased text. With no assignment the label renders with the
    /// built-in `FONT_6X10`.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Resolve this label's font handle — the assigned font, or `FONT_6X10`.
    ///
    /// Lets a containing widget (e.g. `ui::Input`/`Textarea`) draw extra text
    /// with the same font this label resolves, without duplicating the slot.
    pub fn resolved_font(&self) -> &'static dyn FontMetrics {
        self.font.resolve()
    }

    /// Update the text displayed by the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub(crate) fn replace_text(&mut self, text: String) -> String {
        core::mem::replace(&mut self.text, text)
    }

    /// Retrieve the current label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Update text color for this label.
    ///
    /// Prefer migrated `TextStyle` plumbing for future callers; this method
    /// is the compatibility path while upstream migration continues.
    #[allow(deprecated)]
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }

    /// Return text color blended by widget alpha.
    ///
    /// Prefer `TextStyle` plumbing for long-lived callers; this helper keeps
    /// existing widget implementations warning-free until migration completes.
    #[allow(deprecated)]
    pub fn text_color_with_alpha(&self, alpha: u8) -> Color {
        self.text_color.with_alpha(alpha)
    }

    /// Draw this label using an explicit font metrics backend.
    ///
    /// The shaped text path clips glyph coverage to the label bounds. The
    /// `Widget::draw` impl calls this with the [`set_font`](Self::set_font)
    /// assignment resolved via `WidgetFont` (FONT-00 §5); callers may also
    /// invoke it directly with any `&dyn FontMetrics`.
    #[allow(deprecated)]
    pub fn draw_with_font(&self, renderer: &mut dyn Renderer, font: &dyn FontMetrics) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        let metrics = font.line_metrics();
        let baseline = self.bounds.y + metrics.ascent as i32;
        let shaped = shape_text_ltr(font, &self.text, (self.bounds.x, baseline), 0);
        let mut clipped = ClipRenderer::new(renderer, self.bounds);
        clipped.draw_text_shaped(
            &shaped,
            (0, 0),
            self.text_color.with_alpha(self.style.alpha),
        );
    }
}

const MPY_BOUNDS_FIELD: u32 = 1;
const MPY_TEXT_FIELD: u32 = 2;
const MPY_TEXT_PROPERTY: u32 = 1;

const MPY_PROPERTIES: [PropertyDescriptor; 1] = [PropertyDescriptor {
    id: MPY_TEXT_PROPERTY,
    name: "text",
    value_tag: ValueTag::Text,
    access: PropertyAccess::ReadWrite,
    default: PropertyDefault::Text(""),
    constraint: PropertyConstraint::TextBytes { max: 4096 },
    required_capabilities: ActorCapabilities::TEXT,
    effects: MutationEffects::DRAW
        .union(MutationEffects::LAYOUT)
        .union(MutationEffects::SNAPSHOT),
}];

/// Stable MPY actor type identifier for [`Label`].
pub const MPY_TYPE_ID: TypeId = TypeId::registered(0x0001_0002);

/// Actor-local MPY descriptor for [`Label`].
pub const MPY_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: MPY_TYPE_ID,
    stable_name: "rlvgl_widgets::label::Label",
    schema_revision: 2,
    family: ActorFamily::Text,
    capabilities: ActorCapabilities::TEXT,
    targets: TargetSet::ALL,
    constructor_fields: &[
        ConstructorFieldDescriptor {
            id: MPY_BOUNDS_FIELD,
            name: "bounds",
            value_tag: ValueTag::Rect,
            required: true,
        },
        ConstructorFieldDescriptor {
            id: MPY_TEXT_FIELD,
            name: "text",
            value_tag: ValueTag::Text,
            required: true,
        },
    ],
    properties: &MPY_PROPERTIES,
    actions: &[],
    events: &[],
    styles: &MPY_BASIC_STYLE_PARTS,
    child_policy: ChildPolicy::None,
    layout: LayoutCapabilities::ITEM_HINTS.union(LayoutCapabilities::INTRINSIC_MEASUREMENT),
    resource_cost: ResourceCost {
        text_bytes: 0,
        resources: 0,
    },
    constructor: construct_mpy,
};

fn construct_mpy(args: ConstructorArgs<'_>) -> Result<ConstructedActor, RegistryError> {
    Ok(construct_native_actor(
        MPY_TYPE_ID,
        Label::new(
            args.required_text(MPY_TEXT_FIELD)?,
            args.required_rect(MPY_BOUNDS_FIELD)?,
        ),
    ))
}

impl Widget for Label {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.draw_with_font(renderer, self.font.resolve());
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

impl MpyActor for Label {
    type Prepared = String;

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        match id {
            MPY_TEXT_PROPERTY => Ok(OwnedValue::Text(String::from(self.text()))),
            _ => Err(RegistryError::UnknownProperty { property_id: id }),
        }
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<String>, RegistryError> {
        let mut text = String::from(self.text());
        for direction in directions {
            match direction {
                ActorDirection::SetProperty {
                    id: MPY_TEXT_PROPERTY,
                    value: OwnedValue::Text(value),
                } => text = value.clone(),
                ActorDirection::ResetProperty {
                    id: MPY_TEXT_PROPERTY,
                } => text.clear(),
                ActorDirection::SetProperty { id, .. } | ActorDirection::ResetProperty { id } => {
                    return Err(RegistryError::UnknownProperty { property_id: *id });
                }
                ActorDirection::InvokeAction { id, .. } => {
                    return Err(RegistryError::UnknownAction { action_id: *id });
                }
            }
        }
        let text_delta = i64::try_from(text.len()).map_err(|_| RegistryError::Internal)?
            - i64::try_from(self.text().len()).map_err(|_| RegistryError::Internal)?;
        Ok(ActorPreparation {
            prepared: text,
            text_delta,
        })
    }

    fn commit(&mut self, prepared: String) -> String {
        self.replace_text(prepared)
    }
}
