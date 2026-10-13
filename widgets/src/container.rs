//! Simple container grouping child widgets.
use rlvgl_core::actor::{
    ActorCapabilities, ActorFamily, ActorPreparation, ChildPolicy, ConstructedActor,
    ConstructorArgs, ConstructorFieldDescriptor, LayoutCapabilities, MPY_BASIC_STYLE_PARTS,
    MpyActor, RegistryError, ResourceCost, TargetSet, TypeDescriptor, TypeId, ValueTag,
    construct_native_actor,
};
use rlvgl_core::direction::{ActorDirection, OwnedValue};
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

/// Empty widget used to group child widgets and provide background styling.
pub struct Container {
    bounds: Rect,
    /// Visual style applied to the container background.
    pub style: Style,
}

impl Container {
    /// Create a new container with the specified bounds.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
        }
    }
}

const MPY_BOUNDS_FIELD: u32 = 1;

/// Stable MPY actor type identifier for [`Container`].
pub const MPY_TYPE_ID: TypeId = TypeId::registered(0x0001_0001);

/// Actor-local MPY descriptor for [`Container`].
pub const MPY_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: MPY_TYPE_ID,
    stable_name: "rlvgl_widgets::container::Container",
    schema_revision: 3,
    family: ActorFamily::Container,
    capabilities: ActorCapabilities::STAGE_ROOT.union(ActorCapabilities::CHILDREN),
    targets: TargetSet::ALL,
    constructor_fields: &[ConstructorFieldDescriptor {
        id: MPY_BOUNDS_FIELD,
        name: "bounds",
        value_tag: ValueTag::Rect,
        required: true,
    }],
    properties: &[],
    actions: &[],
    events: &[],
    styles: &MPY_BASIC_STYLE_PARTS,
    child_policy: ChildPolicy::AnyActor,
    layout: LayoutCapabilities::FLEX_CONTAINER
        .union(LayoutCapabilities::GRID_CONTAINER)
        .union(LayoutCapabilities::ITEM_HINTS),
    resource_cost: ResourceCost {
        text_bytes: 0,
        resources: 0,
    },
    constructor: construct_mpy,
};

fn construct_mpy(args: ConstructorArgs<'_>) -> Result<ConstructedActor, RegistryError> {
    Ok(construct_native_actor(
        MPY_TYPE_ID,
        Container::new(args.required_rect(MPY_BOUNDS_FIELD)?),
    ))
}

impl Widget for Container {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);
    }

    /// Containers are currently passive and do not react to events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

impl MpyActor for Container {
    type Prepared = ();

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        Err(RegistryError::UnknownProperty { property_id: id })
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<()>, RegistryError> {
        if directions.is_empty() {
            Ok(ActorPreparation {
                prepared: (),
                text_delta: 0,
            })
        } else {
            Err(RegistryError::BatchInvalid)
        }
    }

    fn commit(&mut self, _prepared: ()) {}
}
