//! Horizontal slider widget.
use rlvgl_core::actor::{
    ActorCapabilities, ActorFamily, ActorPreparation, ChildPolicy, ConstructedActor,
    ConstructorArgs, ConstructorFieldDescriptor, EventDelivery, EventDescriptor, EventFilterSet,
    EventPhaseSet, LayoutCapabilities, MPY_CONTROL_STYLE_PARTS, MpyActor, MutationEffects,
    NativeEventKind, PropertyAccess, PropertyConstraint, PropertyDefault, PropertyDescriptor,
    RegistryError, ResourceCost, TargetSet, TypeDescriptor, TypeId, ValueRef, ValueTag,
    construct_native_actor, encode_event_values,
};
use rlvgl_core::direction::{ActorDirection, OwnedValue};
use rlvgl_core::draw::{draw_widget_bg, fill_rounded_rect};
use rlvgl_core::event::Event;
use rlvgl_core::object::ObjectEvent;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Horizontal slider allowing selection of a value within a range.
pub struct Slider {
    bounds: Rect,
    /// Style for the track and background.
    pub style: Style,
    /// Color of the draggable knob.
    pub knob_color: Color,
    min: i32,
    max: i32,
    value: i32,
    last_native_value_changed: bool,
}

impl Slider {
    /// Create a new slider.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            bounds,
            style: Style::default(),
            knob_color: Color(0, 0, 0, 255),
            min,
            max,
            value: min,
            last_native_value_changed: false,
        }
    }

    /// Current slider value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the slider value, clamped to the valid range.
    pub fn set_value(&mut self, val: i32) {
        self.value = val.clamp(self.min, self.max);
    }

    /// Convert the current value into a pixel position for the knob.
    fn position_from_value(&self) -> i32 {
        let range = self.max - self.min;
        if range == 0 {
            return self.bounds.x;
        }
        let ratio = (self.value - self.min) as f32 / range as f32;
        self.bounds.x + (ratio * self.bounds.width as f32) as i32
    }
}

const MPY_BOUNDS_FIELD: u32 = 1;
const MPY_MIN_FIELD: u32 = 2;
const MPY_MAX_FIELD: u32 = 3;
const MPY_VALUE_FIELD: u32 = 4;
const MPY_MIN_PROPERTY: u32 = 1;
const MPY_MAX_PROPERTY: u32 = 2;
const MPY_VALUE_PROPERTY: u32 = 3;

/// Stable MPY event identifier for a completed native slider update.
pub const MPY_VALUE_CHANGED_EVENT_ID: u32 = 0x0001_0002;

const SLIDER_EFFECTS: MutationEffects = MutationEffects::DRAW.union(MutationEffects::SNAPSHOT);
const MPY_PROPERTIES: [PropertyDescriptor; 3] = [
    PropertyDescriptor {
        id: MPY_MIN_PROPERTY,
        name: "min",
        value_tag: ValueTag::I32,
        access: PropertyAccess::ReadWrite,
        default: PropertyDefault::I32(0),
        constraint: PropertyConstraint::I32 {
            min: i32::MIN,
            max: i32::MAX,
        },
        required_capabilities: ActorCapabilities::CONTROL,
        effects: SLIDER_EFFECTS,
    },
    PropertyDescriptor {
        id: MPY_MAX_PROPERTY,
        name: "max",
        value_tag: ValueTag::I32,
        access: PropertyAccess::ReadWrite,
        default: PropertyDefault::I32(100),
        constraint: PropertyConstraint::I32 {
            min: i32::MIN,
            max: i32::MAX,
        },
        required_capabilities: ActorCapabilities::CONTROL,
        effects: SLIDER_EFFECTS,
    },
    PropertyDescriptor {
        id: MPY_VALUE_PROPERTY,
        name: "value",
        value_tag: ValueTag::I32,
        access: PropertyAccess::ReadWrite,
        default: PropertyDefault::I32(0),
        constraint: PropertyConstraint::I32 {
            min: i32::MIN,
            max: i32::MAX,
        },
        required_capabilities: ActorCapabilities::CONTROL,
        effects: SLIDER_EFFECTS,
    },
];

const MPY_EVENTS: [EventDescriptor; 1] = [EventDescriptor {
    id: MPY_VALUE_CHANGED_EVENT_ID,
    name: "value_changed",
    payload: &[ValueTag::I32],
    max_payload_bytes: 5,
    native_event: NativeEventKind::Clicked,
    phases: EventPhaseSet::TARGET,
    filters: EventFilterSet::ANY,
    requires_widget_invocation: true,
    requires_native_consumed: true,
    allow_consume_at_target: true,
    allow_stop_after_phase: false,
    native_effects: MutationEffects::DRAW.union(MutationEffects::SNAPSHOT),
    delivery: EventDelivery::Ordered,
    coalescing_key: None,
}];

/// Stable MPY actor type identifier for [`Slider`].
pub const MPY_TYPE_ID: TypeId = TypeId::registered(0x0001_0004);

/// Actor-local MPY descriptor for [`Slider`].
pub const MPY_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: MPY_TYPE_ID,
    stable_name: "rlvgl_widgets::slider::Slider",
    schema_revision: 3,
    family: ActorFamily::Control,
    capabilities: ActorCapabilities::CONTROL,
    targets: TargetSet::ALL,
    constructor_fields: &[
        ConstructorFieldDescriptor {
            id: MPY_BOUNDS_FIELD,
            name: "bounds",
            value_tag: ValueTag::Rect,
            required: true,
        },
        ConstructorFieldDescriptor {
            id: MPY_MIN_FIELD,
            name: "min",
            value_tag: ValueTag::I32,
            required: true,
        },
        ConstructorFieldDescriptor {
            id: MPY_MAX_FIELD,
            name: "max",
            value_tag: ValueTag::I32,
            required: true,
        },
        ConstructorFieldDescriptor {
            id: MPY_VALUE_FIELD,
            name: "value",
            value_tag: ValueTag::I32,
            required: false,
        },
    ],
    properties: &MPY_PROPERTIES,
    actions: &[],
    events: &MPY_EVENTS,
    styles: &MPY_CONTROL_STYLE_PARTS,
    child_policy: ChildPolicy::None,
    layout: LayoutCapabilities::ITEM_HINTS,
    resource_cost: ResourceCost {
        text_bytes: 0,
        resources: 0,
    },
    constructor: construct_mpy,
};

fn construct_mpy(args: ConstructorArgs<'_>) -> Result<ConstructedActor, RegistryError> {
    let min = args.required_i32(MPY_MIN_FIELD)?;
    let max = args.required_i32(MPY_MAX_FIELD)?;
    if min > max {
        return Err(RegistryError::Range {
            field_id: MPY_MAX_FIELD,
        });
    }
    let mut slider = Slider::new(args.required_rect(MPY_BOUNDS_FIELD)?, min, max);
    if let Some(value) = args.optional_i32(MPY_VALUE_FIELD)? {
        slider.set_value(value);
    }
    Ok(construct_native_actor(MPY_TYPE_ID, slider))
}

impl Widget for Slider {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let a = self.style.alpha;
        let r = self.style.radius;
        draw_widget_bg(renderer, self.bounds, &self.style);

        // Draw track (pill-shaped when radius > 0)
        let track_height = 4;
        let track_y = self.bounds.y + (self.bounds.height - track_height) / 2;
        let track_rect = Rect {
            x: self.bounds.x,
            y: track_y,
            width: self.bounds.width,
            height: track_height,
        };
        let track_r = if r > 0 { (track_height / 2) as u8 } else { 0 };
        fill_rounded_rect(
            renderer,
            track_rect,
            self.style.border_color.with_alpha(a),
            track_r,
        );

        // Draw knob (rounded when radius > 0)
        let knob_x = self.position_from_value();
        let knob_size = 10;
        let knob_rect = Rect {
            x: knob_x - knob_size / 2,
            y: self.bounds.y + (self.bounds.height - knob_size) / 2,
            width: knob_size,
            height: knob_size,
        };
        let knob_r = if r > 0 { (knob_size / 2) as u8 } else { 0 };
        fill_rounded_rect(renderer, knob_rect, self.knob_color.with_alpha(a), knob_r);
    }

    /// Update the slider value based on pointer release position.
    fn handle_event(&mut self, event: &Event) -> bool {
        let Event::PressRelease { x, y } = event else {
            return false;
        };

        if *y < self.bounds.y
            || *y >= self.bounds.y + self.bounds.height
            || *x < self.bounds.x
            || *x >= self.bounds.x + self.bounds.width
        {
            return false;
        }

        let relative = *x - self.bounds.x;
        let ratio = relative as f32 / self.bounds.width as f32;
        let new_value = self.min + ((self.max - self.min) as f32 * ratio) as i32;
        let previous = self.value;
        self.set_value(new_value);
        self.last_native_value_changed = self.value != previous;
        true
    }
}

impl MpyActor for Slider {
    type Prepared = (i32, i32, i32);

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        match id {
            MPY_MIN_PROPERTY => Ok(OwnedValue::I32(self.min)),
            MPY_MAX_PROPERTY => Ok(OwnedValue::I32(self.max)),
            MPY_VALUE_PROPERTY => Ok(OwnedValue::I32(self.value)),
            _ => Err(RegistryError::UnknownProperty { property_id: id }),
        }
    }

    fn event_payload(
        &self,
        event_id: u32,
        event: &ObjectEvent,
        output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError> {
        match (event_id, event) {
            (MPY_VALUE_CHANGED_EVENT_ID, ObjectEvent::Clicked { .. }) => {
                if self.last_native_value_changed {
                    encode_event_values(&[ValueRef::I32(self.value())], output).map(Some)
                } else {
                    Ok(None)
                }
            }
            (MPY_VALUE_CHANGED_EVENT_ID, _) => Err(RegistryError::Internal),
            _ => Err(RegistryError::UnknownEvent { event_id }),
        }
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<Self::Prepared>, RegistryError> {
        let (mut min, mut max, mut value) = (self.min, self.max, self.value);
        for direction in directions {
            match direction {
                ActorDirection::SetProperty { id, value: next } => {
                    let OwnedValue::I32(next) = next else {
                        return Err(RegistryError::TypeMismatch {
                            field_id: *id,
                            expected: ValueTag::I32,
                            actual: next.tag(),
                        });
                    };
                    match *id {
                        MPY_MIN_PROPERTY => min = *next,
                        MPY_MAX_PROPERTY => max = *next,
                        MPY_VALUE_PROPERTY => value = *next,
                        _ => return Err(RegistryError::UnknownProperty { property_id: *id }),
                    }
                }
                ActorDirection::ResetProperty { id } => match *id {
                    MPY_MIN_PROPERTY => min = 0,
                    MPY_MAX_PROPERTY => max = 100,
                    MPY_VALUE_PROPERTY => value = 0,
                    _ => return Err(RegistryError::UnknownProperty { property_id: *id }),
                },
                ActorDirection::InvokeAction { id, .. } => {
                    return Err(RegistryError::UnknownAction { action_id: *id });
                }
            }
        }
        if min > max {
            return Err(RegistryError::Range {
                field_id: MPY_MAX_PROPERTY,
            });
        }
        if value < min || value > max {
            return Err(RegistryError::Range {
                field_id: MPY_VALUE_PROPERTY,
            });
        }
        Ok(ActorPreparation {
            prepared: (min, max, value),
            text_delta: 0,
        })
    }

    fn commit(&mut self, prepared: Self::Prepared) -> Self::Prepared {
        let retired = (self.min, self.max, self.value);
        (self.min, self.max, self.value) = prepared;
        retired
    }
}
