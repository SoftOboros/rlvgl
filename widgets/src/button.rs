//! Interactive button widget with callback support.
use alloc::{boxed::Box, string::String};
use rlvgl_core::actor::{
    ActorCapabilities, ActorFamily, ChildPolicy, ConstructedActor, ConstructorArgs,
    ConstructorFieldDescriptor, LayoutCapabilities, RegistryError, ResourceCost, TargetSet,
    TypeDescriptor, TypeId, ValueTag, construct_native_actor,
};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::label::Label;
use rlvgl_core::style::Style;

type ClickHandler = Box<dyn FnMut(&mut Button)>;

/// Clickable button widget.
pub struct Button {
    /// Bounding rectangle defining the clickable area.
    bounds: Rect,
    label: Label,
    on_click: Option<ClickHandler>,
}

impl Button {
    /// Create a new button with the provided label text.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            label: Label::new(text, bounds),
            on_click: None,
        }
    }

    /// Immutable access to the button's style.
    pub fn style(&self) -> &Style {
        &self.label.style
    }

    /// Mutable access to the button's style.
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.label.style
    }

    /// Update the label displayed on the button.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.label.set_text(text);
    }

    /// Retrieve the current button label.
    pub fn text(&self) -> &str {
        self.label.text()
    }

    /// Register a callback invoked when the button is released.
    pub fn set_on_click<F: FnMut(&mut Self) + 'static>(&mut self, handler: F) {
        self.on_click = Some(Box::new(handler));
    }

    /// Check if the given coordinates are inside the button's bounds.
    fn inside_bounds(&self, x: i32, y: i32) -> bool {
        let b = self.bounds;
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    }
}

const MPY_BOUNDS_FIELD: u32 = 1;
const MPY_TEXT_FIELD: u32 = 2;

/// Stable MPY actor type identifier for [`Button`].
pub const MPY_TYPE_ID: TypeId = TypeId::registered(0x0001_0003);

/// Actor-local MPY descriptor for [`Button`].
pub const MPY_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: MPY_TYPE_ID,
    stable_name: "rlvgl_widgets::button::Button",
    schema_revision: 1,
    family: ActorFamily::Control,
    capabilities: ActorCapabilities::TEXT.union(ActorCapabilities::CONTROL),
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
    properties: &[],
    actions: &[],
    events: &[],
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
        Button::new(
            args.required_text(MPY_TEXT_FIELD)?,
            args.required_rect(MPY_BOUNDS_FIELD)?,
        ),
    ))
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.label.draw(renderer);
    }

    /// Delegate pointer events and invoke the click handler when released.
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressRelease { x, y } if self.inside_bounds(*x, *y) => {
                if let Some(mut cb) = self.on_click.take() {
                    cb(self);
                    self.on_click = Some(cb);
                }
                return true;
            }
            _ => {}
        }
        false
    }
}
