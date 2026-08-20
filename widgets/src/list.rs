//! Vertical scrolling list of selectable strings.
use alloc::{string::String, vec::Vec};
use rlvgl_core::actor::{
    ActionDescriptor, ActionTransaction, ActorCapabilities, ActorFamily, ActorPreparation,
    ChildPolicy, ConstructedActor, ConstructorArgs, ConstructorFieldDescriptor, EventDelivery,
    EventDescriptor, EventFilterSet, EventPhaseSet, LayoutCapabilities, MPY_CONTROL_STYLE_PARTS,
    MpyActor, MutationEffects, NativeEventKind, PropertyAccess, PropertyConstraint,
    PropertyDefault, PropertyDescriptor, RegistryError, ResourceCost, TargetSet, TypeDescriptor,
    TypeId, ValueRef, ValueTag, construct_native_actor, encode_event_values,
};
use rlvgl_core::direction::{ActorDirection, OwnedValue};
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
use rlvgl_core::object::ObjectEvent;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Scrollable list of selectable text items.
pub struct List {
    bounds: Rect,
    /// Style used for list items.
    pub style: Style,
    /// Color for item text.
    pub text_color: Color,
    items: Vec<String>,
    selected: Option<usize>,
    last_native_selection_changed: bool,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl List {
    /// Create an empty list widget.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            items: Vec::new(),
            selected: None,
            last_native_selection_changed: false,
            font: WidgetFont::new(),
        }
    }

    /// Append an item to the end of the list.
    pub fn add_item(&mut self, text: impl Into<String>) {
        self.items.push(text.into());
    }

    /// Replace all list items and clear the current selection.
    pub fn set_items(&mut self, items: &[impl AsRef<str>]) {
        self.items.clear();
        self.items
            .extend(items.iter().map(|item| String::from(item.as_ref())));
        self.selected = None;
    }

    /// Return a slice of all list items.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Index of the currently selected item, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Remove one item, maintaining a valid selection.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        self.selected = match self.selected {
            Some(selected) if selected == index => None,
            Some(selected) if selected > index => Some(selected - 1),
            selected => selected,
        };
        true
    }

    /// Remove every item and clear selection.
    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected = None;
    }

    /// Select one existing item.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.selected = Some(index);
        true
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Translate a y coordinate into a list index.
    fn index_at(&self, y: i32) -> Option<usize> {
        let row_height = 16;
        if y < self.bounds.y || y >= self.bounds.y + self.bounds.height {
            return None;
        }
        let idx = (y - self.bounds.y) / row_height;
        if idx < 0 {
            return None;
        }
        let idx = idx as usize;
        if idx < self.items.len() {
            Some(idx)
        } else {
            None
        }
    }
}

const MPY_BOUNDS_FIELD: u32 = 1;
const MPY_ITEM_COUNT_PROPERTY: u32 = 1;
const MPY_APPEND_ACTION: u32 = 1;
const MPY_REMOVE_ACTION: u32 = 2;
const MPY_CLEAR_ACTION: u32 = 3;
const MPY_SELECT_ACTION: u32 = 4;
const MPY_CLEAR_SELECTION_ACTION: u32 = 5;

/// Stable MPY event identifier for a completed native list selection.
pub const MPY_SELECTION_CHANGED_EVENT_ID: u32 = 0x0001_0003;

const MPY_PROPERTIES: [PropertyDescriptor; 1] = [PropertyDescriptor {
    id: MPY_ITEM_COUNT_PROPERTY,
    name: "item_count",
    value_tag: ValueTag::U32,
    access: PropertyAccess::ReadOnly,
    default: PropertyDefault::U32(0),
    constraint: PropertyConstraint::None,
    required_capabilities: ActorCapabilities::COLLECTION,
    effects: MutationEffects::NONE,
}];

const LIST_EFFECTS: MutationEffects = MutationEffects::DRAW.union(MutationEffects::SNAPSHOT);
const MPY_ACTIONS: [ActionDescriptor; 5] = [
    ActionDescriptor {
        id: MPY_APPEND_ACTION,
        name: "append",
        arguments: &[ValueTag::Text],
        results: &[],
        transaction: ActionTransaction::Transactional,
        required_capabilities: ActorCapabilities::COLLECTION,
        effects: LIST_EFFECTS,
    },
    ActionDescriptor {
        id: MPY_REMOVE_ACTION,
        name: "remove",
        arguments: &[ValueTag::U32],
        results: &[],
        transaction: ActionTransaction::Transactional,
        required_capabilities: ActorCapabilities::COLLECTION,
        effects: LIST_EFFECTS,
    },
    ActionDescriptor {
        id: MPY_CLEAR_ACTION,
        name: "clear",
        arguments: &[],
        results: &[],
        transaction: ActionTransaction::Transactional,
        required_capabilities: ActorCapabilities::COLLECTION,
        effects: LIST_EFFECTS,
    },
    ActionDescriptor {
        id: MPY_SELECT_ACTION,
        name: "select",
        arguments: &[ValueTag::U32],
        results: &[],
        transaction: ActionTransaction::Transactional,
        required_capabilities: ActorCapabilities::COLLECTION,
        effects: LIST_EFFECTS,
    },
    ActionDescriptor {
        id: MPY_CLEAR_SELECTION_ACTION,
        name: "clear_selection",
        arguments: &[],
        results: &[],
        transaction: ActionTransaction::Transactional,
        required_capabilities: ActorCapabilities::COLLECTION,
        effects: LIST_EFFECTS,
    },
];

const MPY_EVENTS: [EventDescriptor; 1] = [EventDescriptor {
    id: MPY_SELECTION_CHANGED_EVENT_ID,
    name: "selection_changed",
    payload: &[ValueTag::U32],
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

/// Stable MPY actor type identifier for [`List`].
pub const MPY_TYPE_ID: TypeId = TypeId::registered(0x0001_0005);

/// Actor-local MPY descriptor for [`List`].
pub const MPY_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: MPY_TYPE_ID,
    stable_name: "rlvgl_widgets::list::List",
    schema_revision: 4,
    family: ActorFamily::Composite,
    capabilities: ActorCapabilities::TEXT
        .union(ActorCapabilities::CONTROL)
        .union(ActorCapabilities::COLLECTION),
    targets: TargetSet::ALL,
    constructor_fields: &[ConstructorFieldDescriptor {
        id: MPY_BOUNDS_FIELD,
        name: "bounds",
        value_tag: ValueTag::Rect,
        required: true,
    }],
    properties: &MPY_PROPERTIES,
    actions: &MPY_ACTIONS,
    events: &MPY_EVENTS,
    styles: &MPY_CONTROL_STYLE_PARTS,
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
        List::new(args.required_rect(MPY_BOUNDS_FIELD)?),
    ))
}

impl Widget for List {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        Some(&mut self.font)
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let a = self.style.alpha;
        draw_widget_bg(renderer, self.bounds, &self.style);
        let font = self.font.resolve();
        let row_height = 16;
        for (i, item) in self.items.iter().enumerate() {
            let y = self.bounds.y + (i as i32 * row_height);
            let pos = (self.bounds.x + 2, y + row_height);
            let color = if self.selected == Some(i) {
                self.style.border_color
            } else {
                self.text_color
            };
            let shaped = shape_text_ltr(font, item, pos, 0);
            renderer.draw_text_shaped(&shaped, (0, 0), color.with_alpha(a));
        }
    }

    /// Select an item when the pointer is released over it.
    fn handle_event(&mut self, event: &Event) -> bool {
        let Event::PressRelease { x, y } = event else {
            return false;
        };

        if *x < self.bounds.x || *x >= self.bounds.x + self.bounds.width {
            return false;
        }

        let Some(idx) = self.index_at(*y) else {
            return false;
        };

        let previous = self.selected;
        self.selected = Some(idx);
        self.last_native_selection_changed = self.selected != previous;
        true
    }
}

impl MpyActor for List {
    type Prepared = (Vec<String>, Option<usize>);

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        match id {
            MPY_ITEM_COUNT_PROPERTY => Ok(OwnedValue::U32(
                u32::try_from(self.items.len()).map_err(|_| RegistryError::Internal)?,
            )),
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
            (MPY_SELECTION_CHANGED_EVENT_ID, ObjectEvent::Clicked { .. }) => {
                if !self.last_native_selection_changed {
                    return Ok(None);
                }
                let selected = self.selected().ok_or(RegistryError::Internal)?;
                let selected = u32::try_from(selected).map_err(|_| RegistryError::Capacity {
                    kind: rlvgl_core::actor::CapacityKind::EventPayloadBytes,
                })?;
                encode_event_values(&[ValueRef::U32(selected)], output).map(Some)
            }
            (MPY_SELECTION_CHANGED_EVENT_ID, _) => Err(RegistryError::Internal),
            _ => Err(RegistryError::UnknownEvent { event_id }),
        }
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<Self::Prepared>, RegistryError> {
        let mut items = self.items.clone();
        let mut selected = self.selected;
        for direction in directions {
            match direction {
                ActorDirection::SetProperty { id, .. } | ActorDirection::ResetProperty { id } => {
                    return if *id == MPY_ITEM_COUNT_PROPERTY {
                        Err(RegistryError::ReadOnly)
                    } else {
                        Err(RegistryError::UnknownProperty { property_id: *id })
                    };
                }
                ActorDirection::InvokeAction { id, arguments } => match *id {
                    MPY_APPEND_ACTION => {
                        let [OwnedValue::Text(text)] = arguments.as_slice() else {
                            return Err(RegistryError::BatchInvalid);
                        };
                        items.push(text.clone());
                    }
                    MPY_REMOVE_ACTION => {
                        let [OwnedValue::U32(index)] = arguments.as_slice() else {
                            return Err(RegistryError::BatchInvalid);
                        };
                        let index = *index as usize;
                        if index >= items.len() {
                            return Err(RegistryError::Range {
                                field_id: MPY_REMOVE_ACTION,
                            });
                        }
                        items.remove(index);
                        selected = match selected {
                            Some(current) if current == index => None,
                            Some(current) if current > index => Some(current - 1),
                            current => current,
                        };
                    }
                    MPY_CLEAR_ACTION => {
                        items.clear();
                        selected = None;
                    }
                    MPY_SELECT_ACTION => {
                        let [OwnedValue::U32(index)] = arguments.as_slice() else {
                            return Err(RegistryError::BatchInvalid);
                        };
                        let index = *index as usize;
                        if index >= items.len() {
                            return Err(RegistryError::Range {
                                field_id: MPY_SELECT_ACTION,
                            });
                        }
                        selected = Some(index);
                    }
                    MPY_CLEAR_SELECTION_ACTION => selected = None,
                    _ => return Err(RegistryError::UnknownAction { action_id: *id }),
                },
            }
        }
        let before = self.items.iter().try_fold(0i64, |total, item| {
            total
                .checked_add(i64::try_from(item.len()).map_err(|_| RegistryError::Internal)?)
                .ok_or(RegistryError::Internal)
        })?;
        let after = items.iter().try_fold(0i64, |total, item| {
            total
                .checked_add(i64::try_from(item.len()).map_err(|_| RegistryError::Internal)?)
                .ok_or(RegistryError::Internal)
        })?;
        Ok(ActorPreparation {
            prepared: (items, selected),
            text_delta: after - before,
        })
    }

    fn commit(&mut self, (items, selected): Self::Prepared) -> Self::Prepared {
        let items = core::mem::replace(&mut self.items, items);
        let selected = core::mem::replace(&mut self.selected, selected);
        (items, selected)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn set_items_replaces_values_and_clears_selection() {
        let mut list = List::new(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 48,
        });
        list.add_item("A");
        list.add_item("B");
        assert!(list.handle_event(&Event::PressRelease { x: 5, y: 20 }));
        assert_eq!(list.selected(), Some(1));

        list.set_items(&["C"]);

        assert_eq!(list.items(), &["C"]);
        assert_eq!(list.selected(), None);
    }

    #[test]
    fn mpy_actions_prepare_all_list_transitions_in_declared_order() {
        let mut list = List::new(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 48,
        });
        let prepared = list
            .prepare(&[
                ActorDirection::InvokeAction {
                    id: MPY_APPEND_ACTION,
                    arguments: vec![OwnedValue::Text(String::from("A"))],
                },
                ActorDirection::InvokeAction {
                    id: MPY_APPEND_ACTION,
                    arguments: vec![OwnedValue::Text(String::from("B"))],
                },
                ActorDirection::InvokeAction {
                    id: MPY_SELECT_ACTION,
                    arguments: vec![OwnedValue::U32(1)],
                },
                ActorDirection::InvokeAction {
                    id: MPY_REMOVE_ACTION,
                    arguments: vec![OwnedValue::U32(0)],
                },
                ActorDirection::InvokeAction {
                    id: MPY_CLEAR_SELECTION_ACTION,
                    arguments: vec![],
                },
            ])
            .unwrap();
        assert!(list.items().is_empty());
        assert_eq!(list.selected(), None);
        assert_eq!(prepared.text_delta, 1);

        list.commit(prepared.prepared);
        assert_eq!(list.items(), &[String::from("B")]);
        assert_eq!(list.selected(), None);

        let prepared = list
            .prepare(&[ActorDirection::InvokeAction {
                id: MPY_CLEAR_ACTION,
                arguments: vec![],
            }])
            .unwrap();
        assert_eq!(prepared.text_delta, -1);
        list.commit(prepared.prepared);
        assert!(list.items().is_empty());
        assert_eq!(list.selected(), None);
    }
}
