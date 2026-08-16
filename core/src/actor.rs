//! MPY actor descriptors and compatibility-first Stage Registry.
//!
//! This module owns the language-neutral runtime substrate selected by MPY-03.
//! Actor declarations remain beside their native widget implementations; a
//! registry consumes a static catalog of those declarations without storing
//! raw pointers to nodes in its slot table.

use alloc::{boxed::Box, rc::Rc, string::String, vec::Vec};
use core::cell::RefCell;

use rlvgl_api::protocol::{CodecError, decode_value, encode_value};
pub use rlvgl_api::protocol::{ErrorClass, ValueRef, ValueTag};

use crate::{
    direction::{
        ActorDirection, GeometryResult, GeometryRole, OwnedValue, RequestedLayout, RuntimeFlag,
        SnapshotError, SnapshotPage, SnapshotProperty, SnapshotRecord, SnapshotToken,
        StageDirection, StageRevision,
    },
    layout::{EngineConfig, GridTrack, LayoutRole, LayoutState},
    object::{
        CompletedObjectDispatch, DispatchInput, DispatchPhase, NativeEventObserver,
        ObjectDispatchError, ObjectDispatchObservationPlans, ObjectEvent, ObjectFlags, ObjectNode,
        ObjectStates, ResolvedObjectDispatch, resolve_object_dispatch,
    },
    widget::{Rect, Widget},
};

/// Stable nonzero Stage identifier within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct StageId(u32);

impl StageId {
    /// Construct a Stage identifier, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone)]
struct ShadowActor {
    object_id: ObjectId,
    parent: Option<ObjectId>,
    children: Vec<ObjectId>,
    max_children: usize,
    alive: bool,
}

struct TreeShadow {
    actors: Vec<ShadowActor>,
    roots: Vec<(String, ObjectId)>,
    deleted_object_ids: Vec<ObjectId>,
    delete_groups: Vec<Vec<ObjectId>>,
    max_roots: usize,
    initial_root_text: i64,
    root_text_delta: i64,
}

impl TreeShadow {
    fn capture(registry: &StageRegistry) -> Result<Self, RegistryError> {
        let mut actors = Vec::with_capacity(registry.usage.actors);
        for (index, slot) in registry.slots.iter().enumerate() {
            let Some(record) = slot.record.as_ref() else {
                continue;
            };
            let object_id = ObjectId::from_parts(slot.generation, index as u32);
            let children = registry.children(object_id)?;
            let max_children = children.len();
            actors.push(ShadowActor {
                object_id,
                parent: record.parent,
                children,
                max_children,
                alive: true,
            });
        }
        let mut roots = Vec::with_capacity(registry.roots.len());
        for root in &registry.roots {
            let object_id = root
                .node
                .actor_identity()
                .ok_or(RegistryError::Internal)?
                .object_id;
            roots.push((root.name.clone(), object_id));
        }
        let initial_root_text = roots.iter().try_fold(0i64, |total, (name, _)| {
            total
                .checked_add(i64::try_from(name.len()).map_err(|_| RegistryError::Internal)?)
                .ok_or(RegistryError::Internal)
        })?;
        Ok(Self {
            actors,
            roots,
            deleted_object_ids: Vec::new(),
            delete_groups: Vec::new(),
            max_roots: registry.roots.len(),
            initial_root_text,
            root_text_delta: 0,
        })
    }

    fn actor(&self, object_id: ObjectId) -> Result<&ShadowActor, RegistryError> {
        self.actors
            .iter()
            .find(|actor| actor.object_id == object_id && actor.alive)
            .ok_or(RegistryError::StaleObject { object_id })
    }

    fn actor_mut(&mut self, object_id: ObjectId) -> Result<&mut ShadowActor, RegistryError> {
        self.actors
            .iter_mut()
            .find(|actor| actor.object_id == object_id && actor.alive)
            .ok_or(RegistryError::StaleObject { object_id })
    }

    fn remove_from_owner(&mut self, object_id: ObjectId) -> Result<(), RegistryError> {
        let parent = self.actor(object_id)?.parent;
        if let Some(parent) = parent {
            let children = &mut self.actor_mut(parent)?.children;
            let index = children
                .iter()
                .position(|child| *child == object_id)
                .ok_or(RegistryError::Internal)?;
            children.remove(index);
        } else {
            let index = self
                .roots
                .iter()
                .position(|(_, root)| *root == object_id)
                .ok_or(RegistryError::Internal)?;
            self.roots.remove(index);
        }
        Ok(())
    }

    fn apply(
        &mut self,
        registry: &StageRegistry,
        direction: &StageDirection,
    ) -> Result<(), RegistryError> {
        match direction {
            StageDirection::Reparent {
                object_id,
                new_parent,
                index,
            } => {
                self.actor(*object_id)?;
                self.actor(*new_parent)?;
                if object_id == new_parent || self.is_descendant(*object_id, *new_parent)? {
                    return Err(RegistryError::InvalidParent);
                }
                let child_descriptor = registry.record(*object_id)?.descriptor;
                let parent_descriptor = registry.record(*new_parent)?.descriptor;
                if !parent_descriptor.child_policy.allows(child_descriptor) {
                    return Err(RegistryError::InvalidParent);
                }
                self.remove_from_owner(*object_id)?;
                if *index > self.actor(*new_parent)?.children.len() {
                    return Err(RegistryError::Range { field_id: 0 });
                }
                self.actor_mut(*new_parent)?
                    .children
                    .insert(*index, *object_id);
                self.actor_mut(*object_id)?.parent = Some(*new_parent);
            }
            StageDirection::PromoteRoot {
                object_id,
                name,
                index,
            } => {
                let descriptor = registry.record(*object_id)?.descriptor;
                self.actor(*object_id)?;
                if name.is_empty()
                    || !descriptor
                        .capabilities
                        .contains(ActorCapabilities::STAGE_ROOT)
                    || self
                        .roots
                        .iter()
                        .any(|(candidate, root)| candidate == name && root != object_id)
                {
                    return Err(RegistryError::InvalidParent);
                }
                self.remove_from_owner(*object_id)?;
                if *index > self.roots.len() {
                    return Err(RegistryError::Range { field_id: 0 });
                }
                self.roots.insert(*index, (name.clone(), *object_id));
                self.actor_mut(*object_id)?.parent = None;
            }
            StageDirection::Reorder { object_id, index } => {
                let parent = self.actor(*object_id)?.parent;
                if let Some(parent) = parent {
                    let children = &mut self.actor_mut(parent)?.children;
                    let old = children
                        .iter()
                        .position(|child| child == object_id)
                        .ok_or(RegistryError::Internal)?;
                    let child = children.remove(old);
                    if *index > children.len() {
                        return Err(RegistryError::Range { field_id: 0 });
                    }
                    children.insert(*index, child);
                } else {
                    let old = self
                        .roots
                        .iter()
                        .position(|(_, root)| root == object_id)
                        .ok_or(RegistryError::Internal)?;
                    let root = self.roots.remove(old);
                    if *index > self.roots.len() {
                        return Err(RegistryError::Range { field_id: 0 });
                    }
                    self.roots.insert(*index, root);
                }
            }
            StageDirection::Delete { object_id } => {
                self.remove_from_owner(*object_id)?;
                let first_deleted = self.deleted_object_ids.len();
                self.mark_deleted(*object_id)?;
                self.delete_groups
                    .push(self.deleted_object_ids[first_deleted..].to_vec());
            }
            _ => {}
        }
        self.max_roots = self.max_roots.max(self.roots.len());
        for actor in &mut self.actors {
            actor.max_children = actor.max_children.max(actor.children.len());
        }
        Ok(())
    }

    fn is_descendant(
        &self,
        ancestor: ObjectId,
        candidate: ObjectId,
    ) -> Result<bool, RegistryError> {
        if self.actor(ancestor)?.children.contains(&candidate) {
            return Ok(true);
        }
        for child in &self.actor(ancestor)?.children {
            if self.is_descendant(*child, candidate)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn mark_deleted(&mut self, object_id: ObjectId) -> Result<(), RegistryError> {
        let children = self.actor(object_id)?.children.clone();
        for child in children {
            self.mark_deleted(child)?;
        }
        self.actor_mut(object_id)?.alive = false;
        push_unique(&mut self.deleted_object_ids, object_id);
        Ok(())
    }

    fn validate_final(&mut self, registry: &StageRegistry) -> Result<(), RegistryError> {
        if self.roots.len() > registry.limits.max_roots {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::Roots,
            });
        }
        for (index, (name, root)) in self.roots.iter().enumerate() {
            if self.roots[..index].iter().any(|(prior, _)| prior == name)
                || self.actor(*root)?.parent.is_some()
            {
                return Err(RegistryError::InvalidParent);
            }
            self.validate_depth(registry, *root, 1)?;
        }
        for actor in self.actors.iter().filter(|actor| actor.alive) {
            if actor.children.len() > registry.limits.max_children_per_actor {
                return Err(RegistryError::Capacity {
                    kind: CapacityKind::Children,
                });
            }
        }
        let final_root_text = self.roots.iter().try_fold(0i64, |total, (name, _)| {
            total
                .checked_add(i64::try_from(name.len()).map_err(|_| RegistryError::Internal)?)
                .ok_or(RegistryError::Internal)
        })?;
        self.root_text_delta = final_root_text
            .checked_sub(self.initial_root_text)
            .ok_or(RegistryError::Internal)?;
        for actor in self.actors.iter().filter(|actor| !actor.alive) {
            let record = registry.record(actor.object_id)?;
            let durable = record
                .text_bytes
                .checked_sub(record.root_name_bytes)
                .ok_or(RegistryError::Internal)?;
            self.root_text_delta = self
                .root_text_delta
                .checked_sub(i64::from(durable))
                .ok_or(RegistryError::Internal)?;
        }
        Ok(())
    }

    fn validate_depth(
        &self,
        registry: &StageRegistry,
        object_id: ObjectId,
        depth: usize,
    ) -> Result<(), RegistryError> {
        if depth > registry.limits.max_tree_depth {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::TreeDepth,
            });
        }
        for child in &self.actor(object_id)?.children {
            self.validate_depth(registry, *child, depth + 1)?;
        }
        Ok(())
    }

    fn final_depths(&self) -> Result<Vec<(ObjectId, usize)>, RegistryError> {
        let live_count = self.actors.iter().filter(|actor| actor.alive).count();
        let mut depths = Vec::with_capacity(live_count);
        for (_, root) in &self.roots {
            self.collect_depths(*root, 1, &mut depths)?;
        }
        if depths.len() != live_count {
            return Err(RegistryError::Internal);
        }
        Ok(depths)
    }

    fn collect_depths(
        &self,
        object_id: ObjectId,
        depth: usize,
        output: &mut Vec<(ObjectId, usize)>,
    ) -> Result<(), RegistryError> {
        let actor = self.actor(object_id)?;
        output.push((object_id, depth));
        for child in &actor.children {
            self.collect_depths(*child, depth + 1, output)?;
        }
        Ok(())
    }
}

fn validate_actor_directions(
    descriptor: &TypeDescriptor,
    directions: &[ActorDirection],
) -> Result<(), RegistryError> {
    for direction in directions {
        match direction {
            ActorDirection::SetProperty { id, value } => {
                let property = descriptor
                    .properties
                    .iter()
                    .find(|property| property.id == *id)
                    .ok_or(RegistryError::UnknownProperty { property_id: *id })?;
                if property.access != PropertyAccess::ReadWrite {
                    return Err(RegistryError::ReadOnly);
                }
                if value.tag() != property.value_tag {
                    return Err(RegistryError::TypeMismatch {
                        field_id: *id,
                        expected: property.value_tag,
                        actual: value.tag(),
                    });
                }
                validate_property_constraint(property, value)?;
            }
            ActorDirection::ResetProperty { id } => {
                let property = descriptor
                    .properties
                    .iter()
                    .find(|property| property.id == *id)
                    .ok_or(RegistryError::UnknownProperty { property_id: *id })?;
                if property.access != PropertyAccess::ReadWrite {
                    return Err(RegistryError::ReadOnly);
                }
                let _ = property.default.owned();
            }
            ActorDirection::InvokeAction { id, arguments } => {
                let action = descriptor
                    .actions
                    .iter()
                    .find(|action| action.id == *id)
                    .ok_or(RegistryError::UnknownAction { action_id: *id })?;
                if action.transaction == ActionTransaction::BatchForbidden {
                    return Err(RegistryError::BatchInvalid);
                }
                if action.arguments.len() != arguments.len() {
                    return Err(RegistryError::BatchInvalid);
                }
                for (argument, expected) in arguments.iter().zip(action.arguments) {
                    if argument.tag() != *expected {
                        return Err(RegistryError::TypeMismatch {
                            field_id: *id,
                            expected: *expected,
                            actual: argument.tag(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn actor_direction_effects(
    descriptor: &TypeDescriptor,
    directions: &[ActorDirection],
) -> Result<MutationEffects, RegistryError> {
    let mut effects = MutationEffects::NONE;
    for direction in directions {
        let next = match direction {
            ActorDirection::SetProperty { id, .. } | ActorDirection::ResetProperty { id } => {
                descriptor
                    .properties
                    .iter()
                    .find(|property| property.id == *id)
                    .ok_or(RegistryError::UnknownProperty { property_id: *id })?
                    .effects
            }
            ActorDirection::InvokeAction { id, .. } => {
                descriptor
                    .actions
                    .iter()
                    .find(|action| action.id == *id)
                    .ok_or(RegistryError::UnknownAction { action_id: *id })?
                    .effects
            }
        };
        effects = effects.union(next);
    }
    Ok(effects)
}

fn push_unique(values: &mut Vec<ObjectId>, value: ObjectId) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn push_rect_unique(values: &mut Vec<Rect>, value: Rect) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn prepared_layout_state(
    requested: &RequestedLayout,
    current: (bool, Option<Rect>),
) -> Option<Box<LayoutState>> {
    let role = match requested {
        RequestedLayout::None => return None,
        RequestedLayout::Flex(config) => LayoutRole::Container(EngineConfig::Flex(config.clone())),
        RequestedLayout::Grid(config) => LayoutRole::Container(EngineConfig::Grid(config.clone())),
        RequestedLayout::Item(hints) => LayoutRole::Item(hints.clone()),
    };
    Some(Box::new(LayoutState {
        role,
        computed: current.0.then_some(current.1).flatten(),
        layout_dirty: true,
    }))
}

fn validate_property_constraint(
    descriptor: &PropertyDescriptor,
    value: &OwnedValue,
) -> Result<(), RegistryError> {
    match (descriptor.constraint, value) {
        (PropertyConstraint::None, _) => Ok(()),
        (PropertyConstraint::I32 { min, max }, OwnedValue::I32(value))
            if *value >= min && *value <= max =>
        {
            Ok(())
        }
        (PropertyConstraint::TextBytes { max }, OwnedValue::Text(value))
            if value.len() <= max as usize =>
        {
            Ok(())
        }
        _ => Err(RegistryError::Range {
            field_id: descriptor.id,
        }),
    }
}

fn apply_text_delta(current: u32, delta: i64) -> Result<u32, RegistryError> {
    let final_value = i64::from(current)
        .checked_add(delta)
        .filter(|value| *value >= 0 && *value <= i64::from(u32::MAX))
        .ok_or(RegistryError::Internal)?;
    u32::try_from(final_value).map_err(|_| RegistryError::Internal)
}

/// Stable actor type identifier assigned by the descriptor catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    /// Construct a registered Type identifier.
    ///
    /// This function is intended for actor-local constants and panics during
    /// constant evaluation if `raw` is the reserved zero value.
    pub const fn registered(raw: u32) -> Self {
        assert!(raw != 0, "registered TypeId must be nonzero");
        Self(raw)
    }

    /// Convert a runtime value into a Type identifier.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Generation-checked actor identifier scoped to one [`StageId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ObjectId(u64);

impl ObjectId {
    fn from_parts(generation: u32, slot_index: u32) -> Self {
        debug_assert!(generation != 0);
        let slot = slot_index
            .checked_add(1)
            .expect("live ObjectId slot index is bounded below u32::MAX");
        Self((u64::from(generation) << 32) | u64::from(slot))
    }

    /// Convert a serialized value with nonzero generation and slot words.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw >> 32 == 0 || raw as u32 == 0 {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// Return the serialized `u64` representation.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Return the upper generation word.
    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Return the lower slot word.
    pub const fn slot(self) -> u32 {
        self.0 as u32
    }

    const fn slot_index(self) -> usize {
        (self.slot() - 1) as usize
    }
}

/// Runtime identity associated with one [`ObjectNode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorIdentity {
    /// Generation-checked actor identifier.
    pub object_id: ObjectId,
    /// Registered actor type identifier.
    pub type_id: TypeId,
}

/// Coarse actor family used for descriptor discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorFamily {
    /// Tree/container actor.
    Container,
    /// Text-bearing actor.
    Text,
    /// Interactive control actor.
    Control,
    /// Composite or collection actor.
    Composite,
}

/// Actor capability bitset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct ActorCapabilities(u32);

impl ActorCapabilities {
    /// No declared capabilities.
    pub const EMPTY: Self = Self(0);
    /// Actor may occupy a named stage-root slot.
    pub const STAGE_ROOT: Self = Self(1 << 0);
    /// Actor may contain child actors in the object tree.
    pub const CHILDREN: Self = Self(1 << 1);
    /// Actor contains or renders text.
    pub const TEXT: Self = Self(1 << 2);
    /// Actor accepts native input as a control.
    pub const CONTROL: Self = Self(1 << 3);
    /// Actor owns a collection-like native model.
    pub const COLLECTION: Self = Self(1 << 4);

    /// Return the union of two capability sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return whether every bit in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return the stable bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// Layout capability bitset advertised by an actor descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct LayoutCapabilities(u32);

impl LayoutCapabilities {
    /// No layout-specific capability.
    pub const EMPTY: Self = Self(0);
    /// Actor can host flex layout.
    pub const FLEX_CONTAINER: Self = Self(1 << 0);
    /// Actor can host grid layout.
    pub const GRID_CONTAINER: Self = Self(1 << 1);
    /// Actor accepts item-level layout hints.
    pub const ITEM_HINTS: Self = Self(1 << 2);
    /// Actor exposes intrinsic measurement.
    pub const INTRINSIC_MEASUREMENT: Self = Self(1 << 3);

    /// Return the union of two layout capability sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return whether every bit in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return the stable bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// Target-profile availability bitset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct TargetSet(u8);

impl TargetSet {
    /// Host/simulator target.
    pub const HOST: Self = Self(1 << 0);
    /// Cortex-M4 runtime target.
    pub const CM4: Self = Self(1 << 1);
    /// Cortex-M7 same-core target.
    pub const CM7: Self = Self(1 << 2);
    /// All initial MPY targets.
    pub const ALL: Self = Self(Self::HOST.0 | Self::CM4.0 | Self::CM7.0);

    /// Return whether every bit in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return the stable bit representation.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

/// Parent/child policy declared by an actor type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildPolicy {
    /// Actor cannot contain script-visible child actors.
    None,
    /// Actor accepts any registered actor type.
    AnyActor,
    /// Actor accepts children carrying the specified capability set.
    Capability(ActorCapabilities),
    /// Actor accepts only the listed type identifiers.
    Explicit(&'static [TypeId]),
}

impl ChildPolicy {
    fn allows(self, child: &TypeDescriptor) -> bool {
        match self {
            Self::None => false,
            Self::AnyActor => true,
            Self::Capability(capability) => child.capabilities.contains(capability),
            Self::Explicit(types) => types.contains(&child.type_id),
        }
    }
}

/// One constructor input field declared by an actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructorFieldDescriptor {
    /// Descriptor-local stable field identifier.
    pub id: u32,
    /// Stable source-level field name.
    pub name: &'static str,
    /// Required MPY value tag.
    pub value_tag: ValueTag,
    /// Whether Create must supply the field.
    pub required: bool,
}

/// Read/write classification reserved for MPY-04 property descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyAccess {
    /// Property can only be read.
    ReadOnly,
    /// Property can be read and written.
    ReadWrite,
}

/// Static default value that can be embedded in a property descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyDefault {
    /// Reset removes the durable value.
    Absent,
    /// Boolean default.
    Bool(bool),
    /// Signed integer default.
    I32(i32),
    /// Unsigned integer default.
    U32(u32),
    /// Process-lifetime UTF-8 default.
    Text(&'static str),
}

impl PropertyDefault {
    fn owned(self) -> Option<OwnedValue> {
        match self {
            Self::Absent => None,
            Self::Bool(value) => Some(OwnedValue::Bool(value)),
            Self::I32(value) => Some(OwnedValue::I32(value)),
            Self::U32(value) => Some(OwnedValue::U32(value)),
            Self::Text(value) => Some(OwnedValue::Text(String::from(value))),
        }
    }

    const fn tag(self) -> Option<ValueTag> {
        match self {
            Self::Absent => None,
            Self::Bool(_) => Some(ValueTag::Bool),
            Self::I32(_) => Some(ValueTag::I32),
            Self::U32(_) => Some(ValueTag::U32),
            Self::Text(_) => Some(ValueTag::Text),
        }
    }
}

/// Descriptor-level constraint applied before an actor-specific collective check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyConstraint {
    /// No additional scalar constraint.
    None,
    /// Inclusive signed integer range.
    I32 {
        /// Inclusive lower bound.
        min: i32,
        /// Inclusive upper bound.
        max: i32,
    },
    /// Maximum UTF-8 byte length.
    TextBytes {
        /// Maximum encoded UTF-8 bytes.
        max: u32,
    },
}

/// Mutation side effects used to derive commit invalidation and revision work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct MutationEffects(u16);

impl MutationEffects {
    /// No visible effect.
    pub const NONE: Self = Self(0);
    /// Drawing output may change.
    pub const DRAW: Self = Self(1 << 0);
    /// Requested or computed layout may change.
    pub const LAYOUT: Self = Self(1 << 1);
    /// Tree ownership or ordering may change.
    pub const TREE: Self = Self(1 << 2);
    /// Focus eligibility or state may change.
    pub const FOCUS: Self = Self(1 << 3);
    /// Snapshot-visible state changes.
    pub const SNAPSHOT: Self = Self(1 << 4);

    /// Union two effect sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return the stable bit representation.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Return whether every requested effect bit is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Discoverable property schema entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyDescriptor {
    /// Stable property identifier.
    pub id: u32,
    /// Stable source-level property name.
    pub name: &'static str,
    /// Property value tag.
    pub value_tag: ValueTag,
    /// Property access mode.
    pub access: PropertyAccess,
    /// Reset behavior.
    pub default: PropertyDefault,
    /// Scalar constraint checked before native preparation.
    pub constraint: PropertyConstraint,
    /// Capabilities required on the owning actor.
    pub required_capabilities: ActorCapabilities,
    /// Commit effects.
    pub effects: MutationEffects,
}

/// Whether an action may participate in an atomic batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionTransaction {
    /// Prepared and committed with the surrounding batch.
    Transactional,
    /// Explicitly unsupported in an atomic batch.
    BatchForbidden,
}

/// Discoverable action schema entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionDescriptor {
    /// Stable action identifier.
    pub id: u32,
    /// Stable source-level action name.
    pub name: &'static str,
    /// Ordered argument value tags.
    pub arguments: &'static [ValueTag],
    /// Ordered result value tags.
    pub results: &'static [ValueTag],
    /// Batch participation class.
    pub transaction: ActionTransaction,
    /// Capabilities required on the owning actor.
    pub required_capabilities: ActorCapabilities,
    /// Commit effects.
    pub effects: MutationEffects,
}

/// Cue-delivery classification reserved for MPY-05 event descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventDelivery {
    /// Required cue that may not be coalesced or silently lost.
    Critical,
    /// Every queued cue must be preserved in order.
    Ordered,
    /// Exact-key queue-tail replacement retains the latest event payload.
    LatestValueCoalescible,
}

/// Native object-event source matched by an MPY event descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEventKind {
    /// [`ObjectEvent::Clicked`] pointer semantic event.
    Clicked,
}

impl NativeEventKind {
    /// Return whether a native object event has this stable source kind.
    pub fn matches(self, event: &ObjectEvent) -> bool {
        match self {
            Self::Clicked => matches!(event, ObjectEvent::Clicked { .. }),
        }
    }
}

/// Allowed native propagation phases for one event descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct EventPhaseSet(u8);

impl EventPhaseSet {
    /// No phase is allowed.
    pub const NONE: Self = Self(0);
    /// Ancestor trickle observation is allowed.
    pub const TRICKLE: Self = Self(1 << 0);
    /// Target observation is allowed.
    pub const TARGET: Self = Self(1 << 1);
    /// Bubble observation is allowed.
    pub const BUBBLE: Self = Self(1 << 2);
    /// Every native propagation phase is allowed.
    pub const ALL: Self = Self(Self::TRICKLE.0 | Self::TARGET.0 | Self::BUBBLE.0);

    /// Return the union of two allowed phase sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return whether a native dispatch phase is allowed.
    pub const fn allows(self, phase: DispatchPhase) -> bool {
        let required = match phase {
            DispatchPhase::Trickle => Self::TRICKLE.0,
            DispatchPhase::Target => Self::TARGET.0,
            DispatchPhase::Bubble => Self::BUBBLE.0,
        };
        self.0 & required != 0
    }

    /// Return whether no phase is allowed.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Filter forms a subscription may request for one event descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct EventFilterSet(u8);

impl EventFilterSet {
    /// Only unfiltered observation is allowed.
    pub const ANY: Self = Self(1 << 0);
    /// Pointer coordinates may be constrained to a logical rectangle.
    pub const POINTER_REGION: Self = Self(1 << 1);

    /// Return the union of two filter sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return whether every bit in `other` is supported.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Return whether no filter form is supported.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Discoverable event schema entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventDescriptor {
    /// Stable event identifier.
    pub id: u32,
    /// Stable source-level event name.
    pub name: &'static str,
    /// Ordered payload value tags.
    pub payload: &'static [ValueTag],
    /// Maximum encoded event-payload bytes before the MPY-05 metadata envelope.
    pub max_payload_bytes: u32,
    /// Native object-semantic source event.
    pub native_event: NativeEventKind,
    /// Native propagation phases eligible for subscription.
    pub phases: EventPhaseSet,
    /// Supported predeclared filter forms.
    pub filters: EventFilterSet,
    /// Whether the target widget semantic adapter must have run.
    pub requires_widget_invocation: bool,
    /// Whether the target widget semantic adapter must have consumed the event.
    pub requires_native_consumed: bool,
    /// Whether `ConsumeAtTarget` may be installed before dispatch.
    pub allow_consume_at_target: bool,
    /// Whether `StopAfterPhase` may be installed before dispatch.
    pub allow_stop_after_phase: bool,
    /// Director-visible effects applied once for an actual native emission.
    pub native_effects: MutationEffects,
    /// Cue-delivery classification.
    pub delivery: EventDelivery,
    /// Descriptor-owned key for latest-value queue-tail replacement.
    pub coalescing_key: Option<u64>,
}

/// One descriptor-first native semantic publication prepared during dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMutationPublication {
    /// Actor whose native semantic adapter emitted.
    pub object_id: ObjectId,
    /// Descriptor-declared director-visible effects.
    pub effects: MutationEffects,
}

/// One actor and its geometry captured before a possible native observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePublicationTarget {
    /// Exact actor generation and type expected during dispatch.
    pub actor_identity: ActorIdentity,
    /// Canonical effective bounds captured while resolving the route.
    pub pre_dispatch_bounds: Rect,
}

/// Sealed descriptor-first publication produced by native observation.
///
/// Construction is crate-private so an armed Stage commit only receives
/// publications derived from its preflighted subscription workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeDispatchPublication {
    actor_identity: ActorIdentity,
    effects: MutationEffects,
    invalidation_bounds: Rect,
}

impl NativeDispatchPublication {
    pub(crate) const fn new(
        actor_identity: ActorIdentity,
        effects: MutationEffects,
        invalidation_bounds: Rect,
    ) -> Self {
        Self {
            actor_identity,
            effects,
            invalidation_bounds,
        }
    }

    /// Return the exact actor that emitted this publication.
    pub const fn actor_identity(&self) -> ActorIdentity {
        self.actor_identity
    }

    /// Return the descriptor-declared director-visible effects.
    pub const fn effects(&self) -> MutationEffects {
        self.effects
    }

    /// Return the union of pre-dispatch and observed effective geometry.
    pub const fn invalidation_bounds(&self) -> Rect {
        self.invalidation_bounds
    }
}

/// Allocation-owned Stage publication preparation made before native dispatch.
#[must_use = "native publication preparation must be armed or explicitly released"]
pub struct PreparedNativePublications {
    stage_id: StageId,
    starting_revision: StageRevision,
    next_revision: Option<StageRevision>,
    maximum_publications: usize,
    possible_effects: MutationEffects,
    targets: Vec<NativePublicationTarget>,
}

impl PreparedNativePublications {
    /// Return the Stage that owns this preparation.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the revision captured before dispatch.
    pub const fn starting_revision(&self) -> StageRevision {
        self.starting_revision
    }

    /// Return the maximum descriptor publications reserved for the traversal.
    pub const fn maximum_publications(&self) -> usize {
        self.maximum_publications
    }
}

/// Fully refreshed native publication transaction ready for infallible commit.
#[must_use = "armed native publications must be committed or explicitly released"]
pub struct NativePublicationCommit {
    prepared: PreparedNativePublications,
}

/// Committed native publication scratch retained through cue enqueue.
#[must_use = "committed native publication scratch must be explicitly released"]
pub struct CompletedNativePublications {
    prepared: PreparedNativePublications,
}

impl CompletedNativePublications {
    /// Return the Stage that accepted the publication.
    pub const fn stage_id(&self) -> StageId {
        self.prepared.stage_id
    }
}

/// One Stage-selected, allocation-owned object route.
///
/// Pointer and focused resolution is explicitly scoped to `root_id`. Direct
/// actor resolution derives that actor's owning root and structural path.
pub struct ResolvedStageDispatch {
    stage_id: StageId,
    stage_revision: StageRevision,
    root_id: ObjectId,
    resolved: ResolvedObjectDispatch,
}

impl ResolvedStageDispatch {
    /// Return the owning Stage.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the selected Stage root.
    pub const fn root_id(&self) -> ObjectId {
        self.root_id
    }

    /// Borrow the exact object-semantic event.
    pub const fn event(&self) -> &ObjectEvent {
        self.resolved.event()
    }

    /// Return the target actor when the resolved target belongs to this Stage.
    pub const fn target_identity(&self) -> Option<ActorIdentity> {
        self.resolved.target_view().actor_identity
    }

    /// Iterate the conservative native phase/node reservation plan.
    pub fn possible_observations(&self) -> ObjectDispatchObservationPlans<'_> {
        self.resolved.possible_observations()
    }
}

/// Stage-scoped route resolution or final object-dispatch failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageDispatchError {
    /// Stage identity, actor identity, or lifecycle validation failed.
    Registry(RegistryError),
    /// Object route resolution or final freshness validation failed.
    Object(ObjectDispatchError),
    /// The Stage changed after route resolution.
    StaleStage,
    /// The supplied resolved route belongs to another Stage.
    StageMismatch,
}

impl From<RegistryError> for StageDispatchError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

impl From<ObjectDispatchError> for StageDispatchError {
    fn from(value: ObjectDispatchError) -> Self {
        Self::Object(value)
    }
}

/// Encode a descriptor-owned event value sequence into caller-owned storage.
///
/// Widget adapters use this helper so every event payload has the canonical
/// MPY tagged-value representation without allocating.
pub fn encode_event_values(
    values: &[ValueRef<'_>],
    output: &mut [u8],
) -> Result<usize, RegistryError> {
    let mut position = 0usize;
    for value in values {
        let encoded =
            encode_value(*value, &mut output[position..]).map_err(|error| match error {
                CodecError::BufferTooSmall => RegistryError::Capacity {
                    kind: CapacityKind::EventPayloadBytes,
                },
                _ => RegistryError::Internal,
            })?;
        position = position
            .checked_add(encoded)
            .ok_or(RegistryError::Capacity {
                kind: CapacityKind::EventPayloadBytes,
            })?;
    }
    Ok(position)
}

impl EventDescriptor {
    /// Return whether this descriptor applies to one completed native phase.
    pub fn matches_native(
        self,
        phase: DispatchPhase,
        event: &ObjectEvent,
        widget_invoked: bool,
        native_consumed: bool,
    ) -> bool {
        self.phases.allows(phase)
            && self.native_event.matches(event)
            && (!self.requires_widget_invocation || widget_invoked)
            && (!self.requires_native_consumed || native_consumed)
    }
}

/// Conservative fixed resource cost advertised by a descriptor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceCost {
    /// Text bytes reserved in addition to actual constructor text.
    pub text_bytes: u32,
    /// Non-text resource handles reserved for the actor.
    pub resources: u16,
}

/// One borrowed constructor field supplied to generic Create.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructorInput<'a> {
    /// Descriptor-local field identifier.
    pub id: u32,
    /// Neutral MPY value.
    pub value: ValueRef<'a>,
}

/// Validated borrowed constructor input view passed to actor-local constructors.
#[derive(Clone, Copy)]
pub struct ConstructorArgs<'a> {
    inputs: &'a [ConstructorInput<'a>],
}

impl<'a> ConstructorArgs<'a> {
    fn new(inputs: &'a [ConstructorInput<'a>]) -> Self {
        Self { inputs }
    }

    /// Return a field value by descriptor-local identifier.
    pub fn get(self, id: u32) -> Option<ValueRef<'a>> {
        self.inputs
            .iter()
            .find(|input| input.id == id)
            .map(|input| input.value)
    }

    /// Read a required rectangle field.
    pub fn required_rect(self, id: u32) -> Result<Rect, RegistryError> {
        match self.get(id) {
            Some(ValueRef::Rect {
                x,
                y,
                width,
                height,
            }) if width >= 0 && height >= 0 => Ok(Rect {
                x,
                y,
                width,
                height,
            }),
            Some(ValueRef::Rect { .. }) => Err(RegistryError::Range { field_id: id }),
            Some(value) => Err(RegistryError::TypeMismatch {
                field_id: id,
                expected: ValueTag::Rect,
                actual: value_tag(value),
            }),
            None => Err(RegistryError::MissingField { field_id: id }),
        }
    }

    /// Read a required signed 32-bit field.
    pub fn required_i32(self, id: u32) -> Result<i32, RegistryError> {
        match self.get(id) {
            Some(ValueRef::I32(value)) => Ok(value),
            Some(value) => Err(RegistryError::TypeMismatch {
                field_id: id,
                expected: ValueTag::I32,
                actual: value_tag(value),
            }),
            None => Err(RegistryError::MissingField { field_id: id }),
        }
    }

    /// Read an optional signed 32-bit field.
    pub fn optional_i32(self, id: u32) -> Result<Option<i32>, RegistryError> {
        match self.get(id) {
            Some(ValueRef::I32(value)) => Ok(Some(value)),
            Some(value) => Err(RegistryError::TypeMismatch {
                field_id: id,
                expected: ValueTag::I32,
                actual: value_tag(value),
            }),
            None => Ok(None),
        }
    }

    /// Read a required UTF-8 text field.
    pub fn required_text(self, id: u32) -> Result<&'a str, RegistryError> {
        match self.get(id) {
            Some(ValueRef::Text(value)) => Ok(value),
            Some(value) => Err(RegistryError::TypeMismatch {
                field_id: id,
                expected: ValueTag::Text,
                actual: value_tag(value),
            }),
            None => Err(RegistryError::MissingField { field_id: id }),
        }
    }
}

/// Actor-local MPY preparation implemented beside each native widget.
///
/// `prepare` may validate and allocate. `commit` must only swap prepared state
/// into the actor and return the retired state without allocating, dropping its
/// owned storage, or failing. The Stage transaction retains that retired value
/// until its explicit post-commit release phase.
pub trait MpyActor: Widget {
    /// Fully allocated actor-local state for one mutation group.
    type Prepared: 'static;

    /// Read a descriptor-owned durable property.
    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError>;

    /// Encode one descriptor-owned post-widget event payload when emitted.
    ///
    /// Implementations write tagged MPY values into caller-owned storage and
    /// must not allocate or invoke a language runtime.
    fn event_payload(
        &self,
        event_id: u32,
        _event: &ObjectEvent,
        _output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError> {
        Err(RegistryError::UnknownEvent { event_id })
    }

    /// Validate a collective property/action group without mutation.
    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<Self::Prepared>, RegistryError>;

    /// Infallibly swap in prepared state and return the retired state.
    fn commit(&mut self, prepared: Self::Prepared) -> Self::Prepared;
}

/// Prepared actor-local state plus its exact change in stage-owned text bytes.
pub struct ActorPreparation<T> {
    /// Fully allocated state to publish.
    pub prepared: T,
    /// Signed delta relative to the actor's current durable text.
    pub text_delta: i64,
}

/// Type-erased, fully prepared actor mutation used by [`StageRegistry`].
pub trait PreparedActorMutation {
    /// Exact change in stage-owned text bytes.
    fn text_delta(&self) -> i64;
    /// Confirm the actor can be borrowed for a callback-free commit window.
    fn ready(&self) -> bool;
    /// Infallibly swap the prepared and retired native states in place.
    fn commit(&mut self);
}

/// Type-erased operations retained beside an [`ObjectNode`].
pub trait ActorOps {
    /// Return the descriptor-assigned actor type.
    fn type_id(&self) -> TypeId;
    /// Return the native widget's current intrinsic bounds.
    fn bounds(&self) -> Result<Rect, RegistryError>;
    /// Read one actor-owned property.
    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError>;
    /// Encode one post-widget event payload into caller-owned storage.
    ///
    /// `None` means the semantic source ran but did not change the durable
    /// value represented by a transition event.
    fn event_payload(
        &self,
        event_id: u32,
        event: &ObjectEvent,
        output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError>;
    /// Prepare an actor-local group without native mutation.
    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<Box<dyn PreparedActorMutation>, RegistryError>;
}

/// Cloneable event adapter usable while the Stage's object tree is borrowed.
#[derive(Clone)]
pub struct ActorEventHandle {
    object_id: ObjectId,
    type_id: TypeId,
    ops: Rc<dyn ActorOps>,
}

impl ActorEventHandle {
    /// Return the exact actor identity captured before native dispatch.
    pub const fn actor_identity(&self) -> ActorIdentity {
        ActorIdentity {
            object_id: self.object_id,
            type_id: self.type_id,
        }
    }

    /// Run one actor adapter into caller-reserved storage.
    pub fn event_payload(
        &self,
        descriptor: &EventDescriptor,
        event: &ObjectEvent,
        output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError> {
        let Some(length) = self.ops.event_payload(descriptor.id, event, output)? else {
            return Ok(None);
        };
        if length > output.len() || length > descriptor.max_payload_bytes as usize {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::EventPayloadBytes,
            });
        }
        validate_event_payload(descriptor, &output[..length])?;
        Ok(Some(length))
    }
}

struct TypedActorOps<T> {
    actor: Rc<RefCell<T>>,
    type_id: TypeId,
}

struct TypedPrepared<T: MpyActor> {
    actor: Rc<RefCell<T>>,
    prepared: Option<T::Prepared>,
    text_delta: i64,
}

impl<T: MpyActor + 'static> PreparedActorMutation for TypedPrepared<T> {
    fn text_delta(&self) -> i64 {
        self.text_delta
    }

    fn ready(&self) -> bool {
        self.actor.try_borrow_mut().is_ok()
    }

    fn commit(&mut self) {
        let prepared = self
            .prepared
            .take()
            .expect("prepared mutation consumed once");
        let retired = self
            .actor
            .try_borrow_mut()
            .expect("atomic commit follows exclusive borrow preflight without callbacks")
            .commit(prepared);
        self.prepared = Some(retired);
    }
}

impl<T: MpyActor + 'static> ActorOps for TypedActorOps<T> {
    fn type_id(&self) -> TypeId {
        self.type_id
    }

    fn bounds(&self) -> Result<Rect, RegistryError> {
        self.actor
            .try_borrow()
            .map_err(|_| RegistryError::DispatchBusy)
            .map(|actor| actor.bounds())
    }

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        self.actor
            .try_borrow()
            .map_err(|_| RegistryError::DispatchBusy)?
            .property(id)
    }

    fn event_payload(
        &self,
        event_id: u32,
        event: &ObjectEvent,
        output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError> {
        self.actor
            .try_borrow()
            .map_err(|_| RegistryError::DispatchBusy)?
            .event_payload(event_id, event, output)
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<Box<dyn PreparedActorMutation>, RegistryError> {
        let actor = self
            .actor
            .try_borrow()
            .map_err(|_| RegistryError::DispatchBusy)?;
        let ActorPreparation {
            prepared,
            text_delta,
        } = actor.prepare(directions)?;
        Ok(Box::new(TypedPrepared {
            actor: self.actor.clone(),
            prepared: Some(prepared),
            text_delta,
        }))
    }
}

/// Native node and parallel typed adapter returned by an actor constructor.
pub struct ConstructedActor {
    node: ObjectNode,
    ops: Rc<dyn ActorOps>,
}

impl ConstructedActor {
    /// Borrow the compatible native node before registry publication.
    pub const fn node(&self) -> &ObjectNode {
        &self.node
    }

    /// Return the descriptor-assigned type retained by the typed adapter.
    pub fn type_id(&self) -> TypeId {
        self.ops.type_id()
    }

    /// Read intrinsic bounds through the typed adapter.
    pub fn actor_bounds(&self) -> Rect {
        self.ops
            .bounds()
            .expect("constructed actor has no externally retained widget borrow")
    }
}

/// Build one native actor while retaining typed and erased handles to the same state.
pub fn construct_native_actor<T>(type_id: TypeId, actor: T) -> ConstructedActor
where
    T: MpyActor + 'static,
{
    let typed = Rc::new(RefCell::new(actor));
    let erased: Rc<RefCell<dyn Widget>> = typed.clone();
    ConstructedActor {
        node: ObjectNode::new(erased),
        ops: Rc::new(TypedActorOps {
            actor: typed,
            type_id,
        }),
    }
}

/// Actor-local constructor function stored in a [`TypeDescriptor`].
pub type ActorConstructor =
    for<'a> fn(ConstructorArgs<'a>) -> Result<ConstructedActor, RegistryError>;

/// Canonical actor descriptor consumed by the Stage Registry and bindings.
#[derive(Clone, Copy, Debug)]
pub struct TypeDescriptor {
    /// Stable actor type identifier.
    pub type_id: TypeId,
    /// Stable source-level fully qualified actor name.
    pub stable_name: &'static str,
    /// Actor-local schema revision.
    pub schema_revision: u16,
    /// Coarse actor family.
    pub family: ActorFamily,
    /// Declared actor capabilities.
    pub capabilities: ActorCapabilities,
    /// Supported target profiles.
    pub targets: TargetSet,
    /// Constructor field schema.
    pub constructor_fields: &'static [ConstructorFieldDescriptor],
    /// Property schema owned by MPY-04.
    pub properties: &'static [PropertyDescriptor],
    /// Action schema owned by MPY-04.
    pub actions: &'static [ActionDescriptor],
    /// Event schema owned by MPY-05.
    pub events: &'static [EventDescriptor],
    /// Allowed script-visible child actors.
    pub child_policy: ChildPolicy,
    /// Layout capabilities exposed for later directions.
    pub layout: LayoutCapabilities,
    /// Conservative fixed resource cost.
    pub resource_cost: ResourceCost,
    /// Actor-local native constructor.
    pub constructor: ActorConstructor,
}

/// Destination for one generic actor Create operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateDestination<'a> {
    /// Create a named stage root.
    Root {
        /// Name unique within the stage.
        name: &'a str,
    },
    /// Attach the actor under an existing parent.
    Child {
        /// Generation-checked parent identifier.
        parent: ObjectId,
    },
}

/// Negotiated limits enforced before actor publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryLimits {
    /// Maximum simultaneous named roots.
    pub max_roots: usize,
    /// Maximum simultaneous live actors.
    pub max_actors: usize,
    /// Maximum root-inclusive tree depth.
    pub max_tree_depth: usize,
    /// Maximum direct children under one actor.
    pub max_children_per_actor: usize,
    /// Maximum stage-owned text bytes.
    pub max_text_bytes: u32,
    /// Maximum stage-owned non-text resources.
    pub max_resources: u16,
}

/// Current Stage Registry resource usage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegistryUsage {
    /// Live named roots.
    pub roots: usize,
    /// Live actors including roots.
    pub actors: usize,
    /// Reserved text bytes.
    pub text_bytes: u32,
    /// Reserved non-text resources.
    pub resources: u16,
}

/// Capacity dimension that rejected a Create operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapacityKind {
    /// Named root limit.
    Roots,
    /// Live actor limit.
    Actors,
    /// Tree-depth limit.
    TreeDepth,
    /// Direct child limit.
    Children,
    /// Stage text budget.
    TextBytes,
    /// Stage non-text resource budget.
    Resources,
    /// Encoded native event payload budget.
    EventPayloadBytes,
    /// Pre-dispatch native publication invalidation reservation.
    NativeEventPublications,
}

/// Stage Registry or actor-construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Stage identifier or configured limits are invalid.
    InvalidStage,
    /// Static catalog contains a duplicate or malformed declaration.
    InvalidCatalog,
    /// Requested actor type is not present in the catalog.
    UnknownType {
        /// Requested type identifier.
        type_id: TypeId,
    },
    /// Constructor field is not declared by the actor.
    UnknownField {
        /// Unrecognized field identifier.
        field_id: u32,
    },
    /// Requested property is not declared by the actor.
    UnknownProperty {
        /// Unrecognized property identifier.
        property_id: u32,
    },
    /// Requested action is not declared by the actor.
    UnknownAction {
        /// Unrecognized action identifier.
        action_id: u32,
    },
    /// Requested event is not declared by the actor.
    UnknownEvent {
        /// Unrecognized event identifier.
        event_id: u32,
    },
    /// Required constructor field is absent.
    MissingField {
        /// Missing field identifier.
        field_id: u32,
    },
    /// Constructor field appears more than once.
    DuplicateField {
        /// Repeated field identifier.
        field_id: u32,
    },
    /// Constructor field carries the wrong value tag.
    TypeMismatch {
        /// Field with the mismatch.
        field_id: u32,
        /// Descriptor-required tag.
        expected: ValueTag,
        /// Supplied tag.
        actual: ValueTag,
    },
    /// Constructor field is outside its accepted range.
    Range {
        /// Field outside its range.
        field_id: u32,
    },
    /// Attempted mutation targets read-only state.
    ReadOnly,
    /// A described direction is not implemented by this runtime profile.
    Unsupported,
    /// A command group is structurally valid but cannot be committed atomically.
    BatchInvalid,
    /// A public native widget borrow prevents entering an atomic director turn.
    DispatchBusy,
    /// Root or parent policy rejects the requested relationship.
    InvalidParent,
    /// Named root already exists.
    DuplicateRoot,
    /// Object handle is deleted, unknown, or generation-stale.
    StaleObject {
        /// Rejected handle.
        object_id: ObjectId,
    },
    /// Negotiated capacity would be exceeded.
    Capacity {
        /// Exhausted capacity dimension.
        kind: CapacityKind,
    },
    /// Stage has been torn down and accepts no further operations.
    StageClosed,
    /// Internal registry metadata and object tree disagree.
    Internal,
}

impl RegistryError {
    /// Return the stable MPY error class for this failure.
    pub const fn error_class(self) -> ErrorClass {
        match self {
            Self::InvalidStage | Self::InvalidCatalog | Self::Internal => ErrorClass::Internal,
            Self::UnknownType { .. } => ErrorClass::UnknownType,
            Self::UnknownProperty { .. } => ErrorClass::UnknownProperty,
            Self::UnknownAction { .. } => ErrorClass::UnknownAction,
            Self::UnknownEvent { .. } => ErrorClass::UnknownEvent,
            Self::UnknownField { .. } | Self::MissingField { .. } | Self::DuplicateField { .. } => {
                ErrorClass::InvalidFrame
            }
            Self::TypeMismatch { .. } => ErrorClass::TypeMismatch,
            Self::Range { .. } => ErrorClass::Range,
            Self::ReadOnly => ErrorClass::ReadOnly,
            Self::Unsupported => ErrorClass::Unsupported,
            Self::BatchInvalid => ErrorClass::BatchInvalid,
            Self::DispatchBusy => ErrorClass::DispatchBusy,
            Self::InvalidParent | Self::DuplicateRoot => ErrorClass::InvalidParent,
            Self::StaleObject { .. } => ErrorClass::StaleObject,
            Self::Capacity { .. } => ErrorClass::Capacity,
            Self::StageClosed => ErrorClass::StageNotFound,
        }
    }
}

/// Public metadata associated with one live actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorInfo {
    /// Stable generation-checked identifier.
    pub object_id: ObjectId,
    /// Registered actor type.
    pub type_id: TypeId,
    /// Parent identifier, or `None` for a named root.
    pub parent: Option<ObjectId>,
    /// Root-inclusive tree depth.
    pub depth: usize,
}

struct ActorRecord {
    descriptor: &'static TypeDescriptor,
    parent: Option<ObjectId>,
    depth: usize,
    text_bytes: u32,
    root_name_bytes: u32,
    resources: u16,
    ops: Rc<dyn ActorOps>,
}

#[derive(Clone, Copy)]
struct ActiveSnapshot {
    token: SnapshotToken,
    revision: StageRevision,
    position: usize,
    sequence: u32,
}

struct PreparedActorGroup {
    object_id: ObjectId,
    mutation: Box<dyn PreparedActorMutation>,
    final_text_bytes: u32,
}

struct PreparedLayoutMutation {
    object_id: ObjectId,
    next: Option<Box<LayoutState>>,
}

/// Fully validated and allocation-reserved Stage transaction.
///
/// This value is tied to the Stage Revision observed by
/// [`StageRegistry::prepare_batch`]. It owns every direction, actor-local
/// preparation, tree scratch buffer, layout replacement, deletion identity,
/// and lifecycle slot required by the callback-free commit window.
pub struct PreparedStageBatch {
    stage_id: StageId,
    starting_revision: StageRevision,
    next_revision: StageRevision,
    directions: Vec<StageDirection>,
    actor_groups: Vec<PreparedActorGroup>,
    layout_mutations: Vec<PreparedLayoutMutation>,
    final_usage: RegistryUsage,
    before_geometry: Vec<(ObjectId, Rect)>,
    geometry_scratch: Vec<(ObjectId, Rect)>,
    invalidations: Vec<Rect>,
    effects: MutationEffects,
    touched: Vec<ObjectId>,
    deleted_object_ids: Vec<ObjectId>,
    delete_groups: Vec<Vec<ObjectId>>,
    depth_updates: Vec<(ObjectId, usize)>,
    child_capacities: Vec<(ObjectId, usize)>,
    max_roots: usize,
    lifecycle: Vec<PendingLifecycle>,
    retired_records: Vec<ActorRecord>,
    retired_root_names: Vec<String>,
}

impl PreparedStageBatch {
    /// Return the Stage that owns this transaction.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the Stage Revision against which preparation was validated.
    pub const fn starting_revision(&self) -> StageRevision {
        self.starting_revision
    }

    /// Return the single revision that a successful commit will publish.
    pub const fn next_revision(&self) -> StageRevision {
        self.next_revision
    }

    /// Borrow exact unique deletion identities in child-first order.
    pub fn deleted_object_ids(&self) -> &[ObjectId] {
        &self.deleted_object_ids
    }
}

impl core::fmt::Debug for PreparedStageBatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedStageBatch")
            .field("stage_id", &self.stage_id)
            .field("starting_revision", &self.starting_revision)
            .field("next_revision", &self.next_revision)
            .field("direction_count", &self.directions.len())
            .field("deleted_object_ids", &self.deleted_object_ids)
            .finish_non_exhaustive()
    }
}

/// Successful Stage commit whose retained resources await explicit release.
pub struct CommittedStageBatch {
    revision: StageRevision,
    prepared: Box<PreparedStageBatch>,
}

impl CommittedStageBatch {
    /// Return the revision published by the commit.
    pub const fn revision(&self) -> StageRevision {
        self.revision
    }

    /// Borrow exact child-first identities retired by the commit.
    pub fn deleted_object_ids(&self) -> &[ObjectId] {
        &self.prepared.deleted_object_ids
    }
}

impl core::fmt::Debug for CommittedStageBatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CommittedStageBatch")
            .field("revision", &self.revision)
            .field("prepared", &self.prepared)
            .finish()
    }
}

/// Pre-mutation rejection of an owned prepared transaction.
///
/// The transaction remains owned by the error so returning a stale or busy
/// result does not deallocate its prepared storage inside the commit window.
pub struct PreparedBatchCommitError {
    cause: RegistryError,
    prepared: Box<PreparedStageBatch>,
}

impl PreparedBatchCommitError {
    /// Return why the pre-mutation type-state guard rejected the commit.
    pub const fn cause(&self) -> RegistryError {
        self.cause
    }

    /// Recover the still-owned prepared transaction.
    pub fn into_prepared(self) -> Box<PreparedStageBatch> {
        self.prepared
    }
}

impl core::fmt::Debug for PreparedBatchCommitError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedBatchCommitError")
            .field("cause", &self.cause)
            .field("prepared", &self.prepared)
            .finish()
    }
}

/// Fully prepared transaction that retires and closes one complete Stage.
pub struct PreparedStageTeardown {
    prepared: Box<PreparedStageBatch>,
}

impl PreparedStageTeardown {
    /// Return the Stage that will be closed.
    pub const fn stage_id(&self) -> StageId {
        self.prepared.stage_id
    }

    /// Return the Stage Revision against which teardown was prepared.
    pub const fn starting_revision(&self) -> StageRevision {
        self.prepared.starting_revision
    }

    /// Return the single final Stage Revision published by commit.
    pub const fn next_revision(&self) -> StageRevision {
        self.prepared.next_revision
    }

    /// Borrow every live object identity in exact child-first retirement order.
    pub fn deleted_object_ids(&self) -> &[ObjectId] {
        &self.prepared.deleted_object_ids
    }

    /// Return the exact number of objects retired by this teardown.
    pub fn deletion_count(&self) -> usize {
        self.prepared.deleted_object_ids.len()
    }
}

impl core::fmt::Debug for PreparedStageTeardown {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedStageTeardown")
            .field("prepared", &self.prepared)
            .finish()
    }
}

/// Successful full-Stage teardown whose retired storage awaits release.
pub struct CommittedStageTeardown {
    committed: CommittedStageBatch,
}

impl CommittedStageTeardown {
    /// Return the final revision published while closing the Stage.
    pub const fn revision(&self) -> StageRevision {
        self.committed.revision
    }

    /// Borrow exact child-first identities retired by the teardown.
    pub fn deleted_object_ids(&self) -> &[ObjectId] {
        self.committed.deleted_object_ids()
    }

    /// Return the exact number of retired objects awaiting release.
    pub fn deletion_count(&self) -> usize {
        self.committed.deleted_object_ids().len()
    }
}

impl core::fmt::Debug for CommittedStageTeardown {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CommittedStageTeardown")
            .field("committed", &self.committed)
            .finish()
    }
}

/// Pre-mutation rejection of an owned full-Stage teardown transaction.
pub struct PreparedStageTeardownCommitError {
    cause: RegistryError,
    prepared: PreparedStageTeardown,
}

impl PreparedStageTeardownCommitError {
    /// Return why the final pre-mutation guard rejected teardown.
    pub const fn cause(&self) -> RegistryError {
        self.cause
    }

    /// Recover the still-owned prepared teardown transaction.
    pub fn into_prepared(self) -> PreparedStageTeardown {
        self.prepared
    }
}

impl core::fmt::Debug for PreparedStageTeardownCommitError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PreparedStageTeardownCommitError")
            .field("cause", &self.cause)
            .field("prepared", &self.prepared)
            .finish()
    }
}

enum PendingLifecycle {
    Detached(ObjectNode),
    Attached(ObjectId),
    ChildChanged(ObjectId),
}

struct ActorSlot {
    generation: u32,
    record: Option<ActorRecord>,
    retired: bool,
}

struct RootRecord {
    name: String,
    node: ObjectNode,
}

#[derive(Clone, Copy)]
struct SlotReservation {
    index: usize,
    generation: u32,
    append: bool,
}

/// Read-only deletion work discovered by a validated batch or Stage teardown.
///
/// Identifiers are unique and ordered exactly as native subtree retirement:
/// children precede their parent, while sibling and root order is stable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageDeletionPreflight {
    stage_id: StageId,
    starting_revision: StageRevision,
    deleted_object_ids: Vec<ObjectId>,
}

impl StageDeletionPreflight {
    /// Return the Stage that produced this report.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the Stage Revision observed during validation.
    pub const fn starting_revision(&self) -> StageRevision {
        self.starting_revision
    }

    /// Borrow exact unique deletion identities in child-first order.
    pub fn deleted_object_ids(&self) -> &[ObjectId] {
        &self.deleted_object_ids
    }

    /// Return the exact number of deleted actor identities in the report.
    pub fn deletion_count(&self) -> usize {
        self.deleted_object_ids.len()
    }
}

/// Compatibility-first owner of one stage's roots, actors, and generations.
pub struct StageRegistry {
    stage_id: StageId,
    catalog: &'static [TypeDescriptor],
    limits: RegistryLimits,
    usage: RegistryUsage,
    roots: Vec<RootRecord>,
    slots: Vec<ActorSlot>,
    active: bool,
    revision: StageRevision,
    snapshot: Option<ActiveSnapshot>,
    next_snapshot_token: u32,
    last_effects: MutationEffects,
    last_invalidations: Vec<Rect>,
}

impl StageRegistry {
    /// Construct an empty Stage Registry over a static descriptor catalog.
    pub fn new(
        stage_id: StageId,
        catalog: &'static [TypeDescriptor],
        limits: RegistryLimits,
    ) -> Result<Self, RegistryError> {
        if limits.max_roots == 0
            || limits.max_actors == 0
            || limits.max_tree_depth == 0
            || limits.max_children_per_actor == 0
            || limits.max_actors > u32::MAX as usize
        {
            return Err(RegistryError::InvalidStage);
        }
        validate_catalog(catalog)?;
        Ok(Self {
            stage_id,
            catalog,
            limits,
            usage: RegistryUsage::default(),
            roots: Vec::new(),
            slots: Vec::new(),
            active: true,
            revision: StageRevision::default(),
            snapshot: None,
            next_snapshot_token: 1,
            last_effects: MutationEffects::NONE,
            last_invalidations: Vec::new(),
        })
    }

    /// Return this registry's Stage identifier.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the canonical descriptor catalog.
    pub const fn catalog(&self) -> &'static [TypeDescriptor] {
        self.catalog
    }

    /// Return the configured limits.
    pub const fn limits(&self) -> RegistryLimits {
        self.limits
    }

    /// Return current resource usage.
    pub const fn usage(&self) -> RegistryUsage {
        self.usage
    }

    /// Return the current director-visible Stage Revision.
    pub const fn revision(&self) -> StageRevision {
        self.revision
    }

    /// Return the union of declared effects from the most recent commit.
    pub const fn last_commit_effects(&self) -> MutationEffects {
        self.last_effects
    }

    /// Return deterministic dirty rectangles derived for the most recent commit.
    pub fn last_invalidations(&self) -> &[Rect] {
        &self.last_invalidations
    }

    /// Find an actor descriptor by its stable identifier.
    pub fn descriptor(&self, type_id: TypeId) -> Option<&'static TypeDescriptor> {
        self.catalog
            .iter()
            .find(|descriptor| descriptor.type_id == type_id)
    }

    /// Resolve a named root to its actor identifier.
    pub fn root_id(&self, name: &str) -> Option<ObjectId> {
        self.roots
            .iter()
            .find(|root| root.name == name)
            .and_then(|root| root.node.actor_identity())
            .map(|identity| identity.object_id)
    }

    /// Resolve live actor metadata, rejecting stale generations.
    pub fn actor_info(&self, object_id: ObjectId) -> Result<ActorInfo, RegistryError> {
        let record = self.record(object_id)?;
        Ok(ActorInfo {
            object_id,
            type_id: record.descriptor.type_id,
            parent: record.parent,
            depth: record.depth,
        })
    }

    /// Clone an event adapter before borrowing the Stage object tree for dispatch.
    pub fn actor_event_handle(
        &self,
        object_id: ObjectId,
    ) -> Result<ActorEventHandle, RegistryError> {
        let record = self.record(object_id)?;
        Ok(ActorEventHandle {
            object_id,
            type_id: record.descriptor.type_id,
            ops: record.ops.clone(),
        })
    }

    /// Resolve one pointer, focused, or explicit-path input below a selected root.
    ///
    /// No-target and route errors occur before the endpoint assigns a native
    /// event sequence or reserves subscription/cue storage.
    pub fn resolve_root_dispatch(
        &self,
        root_id: ObjectId,
        input: DispatchInput,
    ) -> Result<ResolvedStageDispatch, StageDispatchError> {
        self.record(root_id)?;
        let root = self
            .roots
            .iter()
            .find(|root| {
                root.node
                    .actor_identity()
                    .is_some_and(|identity| identity.object_id == root_id)
            })
            .ok_or(RegistryError::InvalidParent)?;
        Ok(ResolvedStageDispatch {
            stage_id: self.stage_id,
            stage_revision: self.revision,
            root_id,
            resolved: resolve_object_dispatch(&root.node, input)?,
        })
    }

    /// Resolve a direct object event by actor identity and derive its owning root/path.
    pub fn resolve_actor_dispatch(
        &self,
        target: ActorIdentity,
        event: ObjectEvent,
    ) -> Result<ResolvedStageDispatch, StageDispatchError> {
        let actor = self.actor_info(target.object_id)?;
        if actor.type_id != target.type_id {
            return Err(StageDispatchError::Registry(RegistryError::StaleObject {
                object_id: target.object_id,
            }));
        }
        let mut path = Vec::new();
        path.try_reserve_exact(self.limits.max_tree_depth.saturating_sub(1))
            .map_err(|_| StageDispatchError::Object(ObjectDispatchError::AllocationFailed))?;
        for root in &self.roots {
            path.clear();
            if find_path_to_object(&root.node, target.object_id, &mut path) {
                let root_id = root
                    .node
                    .actor_identity()
                    .ok_or(RegistryError::Internal)?
                    .object_id;
                return Ok(ResolvedStageDispatch {
                    stage_id: self.stage_id,
                    stage_revision: self.revision,
                    root_id,
                    resolved: resolve_object_dispatch(
                        &root.node,
                        DispatchInput::Container { path, event },
                    )?,
                });
            }
        }
        Err(RegistryError::Internal.into())
    }

    /// Run a previously resolved route without exposing mutable Stage roots.
    ///
    /// This is the final fallible route freshness/borrow check. Success runs
    /// the allocation-free native traversal and returns all route storage for
    /// explicit release after publication.
    pub fn dispatch_resolved_native<O: NativeEventObserver + ?Sized>(
        &mut self,
        route: ResolvedStageDispatch,
        observer: &mut O,
    ) -> Result<CompletedObjectDispatch, StageDispatchError> {
        if route.stage_id != self.stage_id {
            return Err(StageDispatchError::StageMismatch);
        }
        self.ensure_active()?;
        if route.stage_revision != self.revision {
            return Err(StageDispatchError::StaleStage);
        }
        let root = self
            .roots
            .iter_mut()
            .find(|root| {
                root.node
                    .actor_identity()
                    .is_some_and(|identity| identity.object_id == route.root_id)
            })
            .ok_or(StageDispatchError::StaleStage)?;
        let prepared = route.resolved.prepare(&mut root.node)?;
        Ok(prepared.dispatch_observed(observer))
    }

    /// Allocate and validate Stage-side storage for one complete native traversal.
    pub fn prepare_native_publications(
        &mut self,
        targets: &[NativePublicationTarget],
        maximum_publications: usize,
        possible_effects: MutationEffects,
    ) -> Result<PreparedNativePublications, RegistryError> {
        self.ensure_active()?;
        for target in targets {
            let actor = self.actor_info(target.actor_identity.object_id)?;
            if actor.type_id != target.actor_identity.type_id {
                return Err(RegistryError::StaleObject {
                    object_id: target.actor_identity.object_id,
                });
            }
            self.node(target.actor_identity.object_id)?
                .try_effective_bounds()
                .ok_or(RegistryError::DispatchBusy)?;
        }
        self.last_invalidations
            .try_reserve_exact(maximum_publications)
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::NativeEventPublications,
            })?;
        let mut owned_targets = Vec::new();
        owned_targets
            .try_reserve_exact(targets.len())
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::NativeEventPublications,
            })?;
        owned_targets.extend_from_slice(targets);
        Ok(PreparedNativePublications {
            stage_id: self.stage_id,
            starting_revision: self.revision,
            next_revision: if possible_effects == MutationEffects::NONE {
                None
            } else {
                Some(self.next_revision()?)
            },
            maximum_publications,
            possible_effects,
            targets: owned_targets,
        })
    }

    /// Perform the final allocation-free Stage freshness and actor-borrow guard.
    pub fn arm_native_publications(
        &self,
        prepared: PreparedNativePublications,
    ) -> Result<NativePublicationCommit, RegistryError> {
        self.ensure_active()?;
        if prepared.stage_id != self.stage_id || prepared.starting_revision != self.revision {
            return Err(RegistryError::Internal);
        }
        if prepared.maximum_publications > self.last_invalidations.capacity() {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::NativeEventPublications,
            });
        }
        for target in &prepared.targets {
            let actor = self.actor_info(target.actor_identity.object_id)?;
            if actor.type_id != target.actor_identity.type_id {
                return Err(RegistryError::StaleObject {
                    object_id: target.actor_identity.object_id,
                });
            }
            let bounds = self
                .node(target.actor_identity.object_id)?
                .try_effective_bounds()
                .ok_or(RegistryError::DispatchBusy)?;
            if bounds != target.pre_dispatch_bounds {
                return Err(RegistryError::Internal);
            }
        }
        Ok(NativePublicationCommit { prepared })
    }

    /// Infallibly publish one armed traversal and retain all preparation scratch.
    ///
    /// A descriptor publication set with nonempty aggregate Stage effects
    /// advances the revision once. Event-only publications and unchanged
    /// transitions retain the current revision; native sequencing still
    /// distinguishes their causality.
    pub fn commit_native_publications(
        &mut self,
        commit: NativePublicationCommit,
        publications: &[NativeDispatchPublication],
    ) -> (StageRevision, CompletedNativePublications) {
        let prepared = commit.prepared;
        assert!(self.active, "armed native publication Stage remains active");
        assert_eq!(prepared.stage_id, self.stage_id);
        assert_eq!(prepared.starting_revision, self.revision);
        assert!(publications.len() <= prepared.maximum_publications);
        assert!(
            publications
                .iter()
                .all(|publication| prepared.possible_effects.contains(publication.effects))
        );
        assert!(publications.iter().all(|publication| {
            prepared
                .targets
                .iter()
                .any(|target| target.actor_identity == publication.actor_identity)
        }));

        let effects = publications
            .iter()
            .fold(MutationEffects::NONE, |effects, publication| {
                effects.union(publication.effects)
            });
        if effects != MutationEffects::NONE {
            self.snapshot = None;
            self.last_effects = effects;
            self.last_invalidations.clear();
            for publication in publications {
                if publication.effects.contains(MutationEffects::DRAW)
                    && !self
                        .last_invalidations
                        .contains(&publication.invalidation_bounds)
                {
                    debug_assert!(
                        self.last_invalidations.len() < self.last_invalidations.capacity()
                    );
                    self.last_invalidations
                        .push(publication.invalidation_bounds);
                }
            }
            self.revision = prepared
                .next_revision
                .expect("a native mutation preflighted its next revision");
        }
        (self.revision, CompletedNativePublications { prepared })
    }

    /// Release an uncommitted native publication preparation outside dispatch.
    pub fn release_prepared_native_publications(&self, prepared: PreparedNativePublications) {
        drop(prepared);
    }

    /// Release an armed publication transaction outside dispatch.
    pub fn release_native_publication_commit(&self, commit: NativePublicationCommit) {
        drop(commit);
    }

    /// Release committed publication scratch after cue enqueue completes.
    pub fn release_completed_native_publications(&self, completed: CompletedNativePublications) {
        drop(completed);
    }

    /// Reserve invalidation storage for post-dispatch native publications.
    pub fn reserve_native_event_publications(
        &mut self,
        maximum: usize,
    ) -> Result<(), RegistryError> {
        self.ensure_active()?;
        self.last_invalidations
            .try_reserve_exact(maximum)
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::NativeEventPublications,
            })
    }

    /// Publish descriptor-first native mutations without allocating.
    ///
    /// The endpoint calls this after object-tree dispatch releases its mutable
    /// root borrow. One Stage Revision is committed for the traversal's
    /// aggregate nonempty native effects, snapshots are invalidated, and current actor bounds become draw
    /// invalidations for descriptors that declare [`MutationEffects::DRAW`].
    pub fn publish_native_mutations(
        &mut self,
        starting_revision: StageRevision,
        publications: &[NativeMutationPublication],
    ) -> Result<StageRevision, RegistryError> {
        self.ensure_active()?;
        if self.revision != starting_revision {
            return Err(RegistryError::Internal);
        }
        let required_invalidations = publications
            .iter()
            .filter(|publication| publication.effects.contains(MutationEffects::DRAW))
            .count();
        if required_invalidations > self.last_invalidations.capacity() {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::NativeEventPublications,
            });
        }
        for publication in publications {
            self.record(publication.object_id)?;
        }

        let effects = publications
            .iter()
            .fold(MutationEffects::NONE, |effects, publication| {
                effects.union(publication.effects)
            });
        if effects == MutationEffects::NONE {
            return Ok(self.revision);
        }

        self.snapshot = None;
        self.last_effects = effects;
        self.last_invalidations.clear();
        for publication in publications {
            if publication.effects.contains(MutationEffects::DRAW) {
                let bounds = self.record(publication.object_id)?.ops.bounds()?;
                if !self.last_invalidations.contains(&bounds) {
                    self.last_invalidations.push(bounds);
                }
            }
        }
        self.revision = self.revision.next().ok_or(RegistryError::Internal)?;
        Ok(self.revision)
    }

    /// Resolve the compatible native node by traversing stage roots.
    pub fn node(&self, object_id: ObjectId) -> Result<&ObjectNode, RegistryError> {
        self.record(object_id)?;
        self.roots
            .iter()
            .find_map(|root| find_node(&root.node, object_id))
            .ok_or(RegistryError::Internal)
    }

    /// Read native bounds through the parallel typed ActorOps adapter.
    pub fn actor_bounds(&self, object_id: ObjectId) -> Result<Rect, RegistryError> {
        self.record(object_id)?.ops.bounds()
    }

    /// Read one descriptor-owned actor property.
    pub fn property(
        &self,
        object_id: ObjectId,
        property_id: u32,
    ) -> Result<OwnedValue, RegistryError> {
        let record = self.record(object_id)?;
        let descriptor = record
            .descriptor
            .properties
            .iter()
            .find(|property| property.id == property_id)
            .ok_or(RegistryError::UnknownProperty { property_id })?;
        let value = record.ops.property(property_id)?;
        if value.tag() != descriptor.value_tag {
            return Err(RegistryError::Internal);
        }
        Ok(value)
    }

    /// Find one event descriptor declared by a live actor.
    pub fn event_descriptor(
        &self,
        object_id: ObjectId,
        event_id: u32,
    ) -> Result<&'static EventDescriptor, RegistryError> {
        self.record(object_id)?
            .descriptor
            .events
            .iter()
            .find(|event| event.id == event_id)
            .ok_or(RegistryError::UnknownEvent { event_id })
    }

    /// Return requested layout independently of computed geometry.
    pub fn requested_layout(&self, object_id: ObjectId) -> Result<RequestedLayout, RegistryError> {
        Ok(RequestedLayout::from_role(
            &self.node(object_id)?.requested_layout_role(),
        ))
    }

    /// Return read-only intrinsic and computed geometry.
    pub fn geometry(&self, object_id: ObjectId) -> Result<GeometryResult, RegistryError> {
        let node = self.node(object_id)?;
        let layout_role = match node.requested_layout_role() {
            LayoutRole::None => GeometryRole::None,
            LayoutRole::Container(_) => GeometryRole::Container,
            LayoutRole::Item(_) => GeometryRole::Item,
        };
        Ok(GeometryResult {
            intrinsic_bounds: self.actor_bounds(object_id)?,
            effective_bounds: node.effective_bounds(),
            revision: self.revision,
            layout_role,
        })
    }

    /// Return ordered direct child identifiers.
    pub fn children(&self, object_id: ObjectId) -> Result<Vec<ObjectId>, RegistryError> {
        self.node(object_id)?
            .children()
            .iter()
            .map(|child| {
                child
                    .actor_identity()
                    .map(|identity| identity.object_id)
                    .ok_or(RegistryError::Internal)
            })
            .collect()
    }

    /// Return the actor's ordered position within its parent or the root list.
    pub fn position(&self, object_id: ObjectId) -> Result<usize, RegistryError> {
        let record = self.record(object_id)?;
        if let Some(parent) = record.parent {
            return self
                .children(parent)?
                .iter()
                .position(|child| *child == object_id)
                .ok_or(RegistryError::Internal);
        }
        self.roots
            .iter()
            .position(|root| {
                root.node
                    .actor_identity()
                    .is_some_and(|identity| identity.object_id == object_id)
            })
            .ok_or(RegistryError::Internal)
    }

    /// Validate a batch and report its exact actor deletions without mutation.
    ///
    /// This runs the same tree, descriptor, actor-state, resource, borrow, and
    /// geometry preparation used by [`Self::apply_batch`]. The endpoint can
    /// reserve child-first subscription-release cues from the returned report
    /// before it permits the batch to mutate the Stage.
    pub fn preflight_batch(
        &self,
        directions: &[StageDirection],
    ) -> Result<StageDeletionPreflight, RegistryError> {
        let prepared = self.build_prepared_batch(directions.to_vec())?;
        Ok(StageDeletionPreflight {
            stage_id: self.stage_id,
            starting_revision: self.revision,
            deleted_object_ids: prepared.deleted_object_ids,
        })
    }

    /// Enumerate every live Stage actor in deterministic child-first order.
    ///
    /// This is a read-only teardown preflight: it neither publishes lifecycle
    /// events nor deletes actors, advances the revision, or closes the Stage.
    pub fn preflight_teardown(&self) -> Result<StageDeletionPreflight, RegistryError> {
        self.ensure_active()?;
        let mut deleted_object_ids = Vec::with_capacity(self.usage.actors);
        for root in &self.roots {
            collect_postorder_ids(&root.node, &mut deleted_object_ids)?;
        }
        if deleted_object_ids.len() != self.usage.actors {
            return Err(RegistryError::Internal);
        }
        for (index, object_id) in deleted_object_ids.iter().enumerate() {
            if deleted_object_ids[..index].contains(object_id) {
                return Err(RegistryError::Internal);
            }
            self.record(*object_id)?;
        }
        Ok(StageDeletionPreflight {
            stage_id: self.stage_id,
            starting_revision: self.revision,
            deleted_object_ids,
        })
    }

    /// Prepare one allocation-owned transaction that closes the complete Stage.
    ///
    /// Preparation is semantically read-only: it validates every current
    /// actor/tree/geometry borrow, owns root-order deletion directions and
    /// exact child-first identities, and reserves all commit and release
    /// storage without changing revision, usage, tree order, or lifecycle.
    pub fn prepare_stage_teardown(&self) -> Result<PreparedStageTeardown, RegistryError> {
        self.ensure_active()?;
        let mut directions = Vec::new();
        directions
            .try_reserve_exact(self.roots.len())
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Roots,
            })?;
        for root in &self.roots {
            let object_id = root
                .node
                .actor_identity()
                .ok_or(RegistryError::Internal)?
                .object_id;
            directions.push(StageDirection::Delete { object_id });
        }
        let prepared = if directions.is_empty() {
            self.build_empty_stage_teardown()?
        } else {
            self.build_prepared_batch(directions)?
        };
        if prepared.deleted_object_ids.len() != self.usage.actors {
            return Err(RegistryError::Internal);
        }
        Ok(PreparedStageTeardown {
            prepared: Box::new(prepared),
        })
    }

    /// Validate and publish an atomic group under one Stage Revision.
    pub fn apply_batch(
        &mut self,
        directions: &[StageDirection],
    ) -> Result<StageRevision, RegistryError> {
        let prepared = self.prepare_batch(directions.to_vec())?;
        let committed = self
            .commit_prepared_batch(prepared)
            .map_err(|error| error.cause())?;
        let revision = committed.revision();
        self.release_committed_batch(committed)?;
        Ok(revision)
    }

    /// Validate an owned batch and reserve every commit-window buffer.
    ///
    /// Capacity-only reservations may grow native vectors, but preparation
    /// does not change actor state, tree order, lifecycle, usage, or revision.
    pub fn prepare_batch(
        &mut self,
        directions: Vec<StageDirection>,
    ) -> Result<Box<PreparedStageBatch>, RegistryError> {
        let mut prepared = self.build_prepared_batch(directions)?;
        self.reserve_prepared_capacity(&mut prepared)?;
        Ok(Box::new(prepared))
    }

    /// Publish a prepared batch under one callback-free Stage Revision.
    ///
    /// A rejected transaction is returned intact by
    /// [`PreparedBatchCommitError`]. Once the pre-mutation revision, geometry,
    /// and actor-borrow guard succeeds, the remaining path only moves or swaps
    /// storage reserved by [`Self::prepare_batch`] and cannot return an error.
    pub fn commit_prepared_batch(
        &mut self,
        mut prepared: Box<PreparedStageBatch>,
    ) -> Result<CommittedStageBatch, PreparedBatchCommitError> {
        let reject = |cause, prepared| PreparedBatchCommitError { cause, prepared };
        if prepared.stage_id != self.stage_id {
            return Err(reject(RegistryError::InvalidStage, prepared));
        }
        if !self.active {
            return Err(reject(RegistryError::StageClosed, prepared));
        }
        if prepared.starting_revision != self.revision {
            return Err(reject(RegistryError::BatchInvalid, prepared));
        }
        if prepared
            .actor_groups
            .iter()
            .any(|group| !group.mutation.ready())
        {
            return Err(reject(RegistryError::DispatchBusy, prepared));
        }
        if let Err(cause) = self.capture_geometry_into(&mut prepared.geometry_scratch) {
            return Err(reject(cause, prepared));
        }
        if prepared.geometry_scratch != prepared.before_geometry {
            return Err(reject(RegistryError::BatchInvalid, prepared));
        }
        if let Err(cause) = self.preflight_after_geometry_borrows(&prepared) {
            return Err(reject(cause, prepared));
        }
        prepared.geometry_scratch.clear();

        for group in &mut prepared.actor_groups {
            group.mutation.commit();
            let slot = &mut self.slots[group.object_id.slot_index()];
            debug_assert_eq!(slot.generation, group.object_id.generation());
            slot.record
                .as_mut()
                .expect("prepared actor remains live until its tree direction")
                .text_bytes = group.final_text_bytes;
        }

        let mut layout_index = 0usize;
        let mut delete_index = 0usize;
        let mut directions = core::mem::take(&mut prepared.directions);
        for direction in &mut directions {
            match direction {
                StageDirection::SetFlag {
                    object_id,
                    flag,
                    enabled,
                } => self.commit_runtime_flag(*object_id, *flag, *enabled).expect(
                    "prepared runtime flag remains descriptor-valid through callback-free commit",
                ),
                StageDirection::SetRequestedLayout { object_id, .. } => {
                    let replacement = &mut prepared.layout_mutations[layout_index];
                    debug_assert_eq!(replacement.object_id, *object_id);
                    let node = self
                        .node_mut(*object_id)
                        .expect("prepared layout target remains live");
                    core::mem::swap(&mut node.layout, &mut replacement.next);
                    layout_index += 1;
                }
                StageDirection::Reparent {
                    object_id,
                    new_parent,
                    index,
                } => self.commit_prepared_reparent(
                    *object_id,
                    *new_parent,
                    *index,
                    &mut prepared,
                ),
                StageDirection::PromoteRoot {
                    object_id,
                    name,
                    index,
                } => {
                    let owned_name = core::mem::take(name);
                    self.commit_prepared_promote(
                        *object_id,
                        owned_name,
                        *index,
                        &mut prepared,
                    );
                }
                StageDirection::Reorder { object_id, index } => {
                    self.commit_prepared_reorder(*object_id, *index, &mut prepared)
                }
                StageDirection::Delete { object_id } => {
                    let ids = core::mem::take(&mut prepared.delete_groups[delete_index]);
                    self.commit_prepared_delete(*object_id, &ids, &mut prepared);
                    prepared.delete_groups[delete_index] = ids;
                    delete_index += 1;
                }
                StageDirection::MutateActor { .. } => {}
                StageDirection::SetComputedGeometry { .. }
                | StageDirection::SetLocalStyle { .. } => {
                    unreachable!("unsupported directions cannot survive preparation")
                }
            }
        }
        prepared.directions = directions;
        debug_assert_eq!(layout_index, prepared.layout_mutations.len());
        debug_assert_eq!(delete_index, prepared.delete_groups.len());

        for (object_id, depth) in &prepared.depth_updates {
            let slot = &mut self.slots[object_id.slot_index()];
            debug_assert_eq!(slot.generation, object_id.generation());
            slot.record
                .as_mut()
                .expect("final shadow depth names a live actor")
                .depth = *depth;
        }
        self.usage = prepared.final_usage;
        self.revision = prepared.next_revision;
        self.last_effects = prepared.effects;
        self.capture_geometry_into(&mut prepared.geometry_scratch)
            .expect("actor borrows were validated before callback-free commit");
        self.derive_invalidations_into(
            &prepared.before_geometry,
            &prepared.geometry_scratch,
            &prepared.touched,
            prepared.effects.contains(MutationEffects::TREE)
                || prepared.effects.contains(MutationEffects::LAYOUT),
            &mut prepared.invalidations,
        );
        core::mem::swap(&mut self.last_invalidations, &mut prepared.invalidations);

        Ok(CommittedStageBatch {
            revision: self.revision,
            prepared,
        })
    }

    /// Publish retained lifecycle events and release transaction scratch.
    ///
    /// This explicit post-commit phase may invoke native lifecycle handlers
    /// and deallocate the old actor/layout/tree state retained by the
    /// [`CommittedStageBatch`].
    pub fn release_committed_batch(
        &mut self,
        mut committed: CommittedStageBatch,
    ) -> Result<(), RegistryError> {
        if committed.prepared.stage_id != self.stage_id {
            return Err(RegistryError::InvalidStage);
        }
        for event in committed.prepared.lifecycle.drain(..) {
            match event {
                PendingLifecycle::Detached(mut node) => node.emit_detached(),
                PendingLifecycle::Attached(object_id) => {
                    if let Ok(node) = self.node_mut(object_id) {
                        node.emit_attached();
                    }
                }
                PendingLifecycle::ChildChanged(object_id) => {
                    if let Ok(node) = self.node_mut(object_id) {
                        node.emit_child_changed();
                    }
                }
            }
        }
        Ok(())
    }

    /// Atomically retire every live object and close the prepared Stage.
    ///
    /// All fallible validation occurs before the first mutation through the
    /// same Stage/revision/geometry/borrow guard as ordinary prepared batches.
    /// The successful path only moves or swaps prepared storage, then marks the
    /// empty Stage closed.
    pub fn commit_prepared_teardown(
        &mut self,
        prepared: PreparedStageTeardown,
    ) -> Result<CommittedStageTeardown, PreparedStageTeardownCommitError> {
        let PreparedStageTeardown { prepared } = prepared;
        match self.commit_prepared_batch(prepared) {
            Ok(committed) => {
                debug_assert_eq!(self.usage, RegistryUsage::default());
                debug_assert!(self.roots.is_empty());
                self.active = false;
                self.snapshot = None;
                Ok(CommittedStageTeardown { committed })
            }
            Err(error) => {
                let cause = error.cause();
                Err(PreparedStageTeardownCommitError {
                    cause,
                    prepared: PreparedStageTeardown {
                        prepared: error.into_prepared(),
                    },
                })
            }
        }
    }

    /// Publish detached-root lifecycle and release all retired Stage storage.
    pub fn release_committed_teardown(
        &mut self,
        committed: CommittedStageTeardown,
    ) -> Result<(), RegistryError> {
        if self.active {
            return Err(RegistryError::BatchInvalid);
        }
        self.release_committed_batch(committed.committed)
    }

    fn build_prepared_batch(
        &self,
        directions: Vec<StageDirection>,
    ) -> Result<PreparedStageBatch, RegistryError> {
        self.ensure_active()?;
        if directions.is_empty() {
            return Err(RegistryError::BatchInvalid);
        }
        let next_revision = self.next_revision()?;
        let mut actor_groups: Vec<(ObjectId, Vec<ActorDirection>)> = Vec::new();
        let mut tree_shadow = TreeShadow::capture(self)?;
        let mut effects = MutationEffects::NONE;
        let mut touched = Vec::new();
        let mut layout_mutations = Vec::new();
        let mut layout_presence: Vec<(ObjectId, bool, Option<Rect>)> = Vec::new();

        for direction in &directions {
            match direction {
                StageDirection::MutateActor {
                    object_id,
                    directions,
                } => {
                    tree_shadow.actor(*object_id)?;
                    if directions.is_empty() {
                        return Err(RegistryError::BatchInvalid);
                    }
                    let record = self.record(*object_id)?;
                    validate_actor_directions(record.descriptor, directions)?;
                    effects =
                        effects.union(actor_direction_effects(record.descriptor, directions)?);
                    push_unique(&mut touched, *object_id);
                    if let Some((_, group)) = actor_groups
                        .iter_mut()
                        .find(|(candidate, _)| candidate == object_id)
                    {
                        group.extend(directions.iter().cloned());
                    } else {
                        actor_groups.push((*object_id, directions.clone()));
                    }
                }
                StageDirection::SetFlag {
                    object_id, flag, ..
                } => {
                    tree_shadow.actor(*object_id)?;
                    self.validate_runtime_flag(*object_id, *flag)?;
                    effects = effects
                        .union(MutationEffects::DRAW)
                        .union(MutationEffects::SNAPSHOT);
                    if matches!(flag, RuntimeFlag::Enabled | RuntimeFlag::Focusable) {
                        effects = effects.union(MutationEffects::FOCUS);
                    }
                    push_unique(&mut touched, *object_id);
                }
                StageDirection::SetRequestedLayout { object_id, layout } => {
                    tree_shadow.actor(*object_id)?;
                    self.validate_requested_layout(*object_id, layout)?;
                    let state = if let Some((_, present, computed)) = layout_presence
                        .iter()
                        .find(|(candidate, _, _)| candidate == object_id)
                    {
                        (*present, *computed)
                    } else {
                        self.node(*object_id)?
                            .layout
                            .as_deref()
                            .map_or((false, None), |state| (true, state.computed))
                    };
                    let next = prepared_layout_state(layout, state);
                    if let Some((_, present, computed)) = layout_presence
                        .iter_mut()
                        .find(|(candidate, _, _)| candidate == object_id)
                    {
                        *present = next.is_some();
                        *computed = next.as_deref().and_then(|state| state.computed);
                    } else {
                        layout_presence.push((
                            *object_id,
                            next.is_some(),
                            next.as_deref().and_then(|state| state.computed),
                        ));
                    }
                    layout_mutations.push(PreparedLayoutMutation {
                        object_id: *object_id,
                        next,
                    });
                    effects = effects
                        .union(MutationEffects::DRAW)
                        .union(MutationEffects::LAYOUT)
                        .union(MutationEffects::SNAPSHOT);
                    push_unique(&mut touched, *object_id);
                }
                StageDirection::SetComputedGeometry { object_id, .. } => {
                    tree_shadow.actor(*object_id)?;
                    self.record(*object_id)?;
                    return Err(RegistryError::ReadOnly);
                }
                StageDirection::SetLocalStyle { object_id, .. } => {
                    tree_shadow.actor(*object_id)?;
                    self.record(*object_id)?;
                    return Err(RegistryError::Unsupported);
                }
                _ => {
                    tree_shadow.apply(self, direction)?;
                    effects = effects
                        .union(MutationEffects::DRAW)
                        .union(MutationEffects::TREE)
                        .union(MutationEffects::LAYOUT)
                        .union(MutationEffects::SNAPSHOT);
                }
            }
        }
        tree_shadow.validate_final(self)?;
        let depth_updates = tree_shadow.final_depths()?;

        let mut prepared = Vec::with_capacity(actor_groups.len());
        let mut total_text_delta = tree_shadow.root_text_delta;
        for (object_id, group) in actor_groups {
            let mutation = self.record(object_id)?.ops.prepare(&group)?;
            let final_text_bytes =
                apply_text_delta(self.record(object_id)?.text_bytes, mutation.text_delta())?;
            if !tree_shadow.deleted_object_ids.contains(&object_id) {
                total_text_delta = total_text_delta.checked_add(mutation.text_delta()).ok_or(
                    RegistryError::Capacity {
                        kind: CapacityKind::TextBytes,
                    },
                )?;
            }
            prepared.push(PreparedActorGroup {
                object_id,
                mutation,
                final_text_bytes,
            });
        }
        let final_text = i64::from(self.usage.text_bytes)
            .checked_add(total_text_delta)
            .filter(|total| *total >= 0 && *total <= i64::from(self.limits.max_text_bytes))
            .ok_or(RegistryError::Capacity {
                kind: CapacityKind::TextBytes,
            })?;
        let before_geometry = self.capture_geometry()?;
        let deleted_resources =
            tree_shadow
                .deleted_object_ids
                .iter()
                .try_fold(0u16, |total, object_id| {
                    total
                        .checked_add(self.record(*object_id)?.resources)
                        .ok_or(RegistryError::Internal)
                })?;
        let final_usage = RegistryUsage {
            roots: tree_shadow.roots.len(),
            actors: self
                .usage
                .actors
                .checked_sub(tree_shadow.deleted_object_ids.len())
                .ok_or(RegistryError::Internal)?,
            text_bytes: u32::try_from(final_text).map_err(|_| RegistryError::Internal)?,
            resources: self
                .usage
                .resources
                .checked_sub(deleted_resources)
                .ok_or(RegistryError::Internal)?,
        };
        let child_capacities = tree_shadow
            .actors
            .iter()
            .map(|actor| (actor.object_id, actor.max_children))
            .collect();
        let mut geometry_scratch = Vec::new();
        geometry_scratch
            .try_reserve_exact(self.usage.actors)
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Actors,
            })?;
        let mut invalidations = Vec::new();
        invalidations
            .try_reserve_exact(
                self.usage
                    .actors
                    .checked_mul(2)
                    .ok_or(RegistryError::Internal)?,
            )
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Actors,
            })?;
        let lifecycle_capacity = directions
            .len()
            .checked_mul(3)
            .ok_or(RegistryError::Internal)?;
        let mut lifecycle = Vec::new();
        lifecycle
            .try_reserve_exact(lifecycle_capacity)
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Actors,
            })?;
        let mut retired_records = Vec::new();
        retired_records
            .try_reserve_exact(tree_shadow.deleted_object_ids.len())
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Actors,
            })?;
        let mut retired_root_names = Vec::new();
        retired_root_names
            .try_reserve_exact(directions.len())
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Roots,
            })?;
        Ok(PreparedStageBatch {
            stage_id: self.stage_id,
            starting_revision: self.revision,
            next_revision,
            directions,
            actor_groups: prepared,
            layout_mutations,
            final_usage,
            before_geometry,
            geometry_scratch,
            invalidations,
            effects,
            touched,
            deleted_object_ids: tree_shadow.deleted_object_ids,
            delete_groups: tree_shadow.delete_groups,
            depth_updates,
            child_capacities,
            max_roots: tree_shadow.max_roots,
            lifecycle,
            retired_records,
            retired_root_names,
        })
    }

    fn build_empty_stage_teardown(&self) -> Result<PreparedStageBatch, RegistryError> {
        if self.usage != RegistryUsage::default()
            || !self.roots.is_empty()
            || self.slots.iter().any(|slot| slot.record.is_some())
        {
            return Err(RegistryError::Internal);
        }
        Ok(PreparedStageBatch {
            stage_id: self.stage_id,
            starting_revision: self.revision,
            next_revision: self.next_revision()?,
            directions: Vec::new(),
            actor_groups: Vec::new(),
            layout_mutations: Vec::new(),
            final_usage: RegistryUsage::default(),
            before_geometry: Vec::new(),
            geometry_scratch: Vec::new(),
            invalidations: Vec::new(),
            effects: MutationEffects::DRAW
                .union(MutationEffects::TREE)
                .union(MutationEffects::LAYOUT)
                .union(MutationEffects::SNAPSHOT),
            touched: Vec::new(),
            deleted_object_ids: Vec::new(),
            delete_groups: Vec::new(),
            depth_updates: Vec::new(),
            child_capacities: Vec::new(),
            max_roots: 0,
            lifecycle: Vec::new(),
            retired_records: Vec::new(),
            retired_root_names: Vec::new(),
        })
    }

    /// Begin the minimum-profile single snapshot cursor.
    pub fn snapshot_begin(&mut self) -> Result<SnapshotToken, SnapshotError> {
        self.ensure_active().map_err(SnapshotError::Registry)?;
        if self.snapshot.is_some() {
            return Err(SnapshotError::Busy);
        }
        let raw = self.next_snapshot_token.max(1);
        self.next_snapshot_token = raw.wrapping_add(1).max(1);
        let token = SnapshotToken(raw);
        self.snapshot = Some(ActiveSnapshot {
            token,
            revision: self.revision,
            position: 0,
            sequence: 0,
        });
        Ok(token)
    }

    /// Read a bounded deterministic root-order/pre-order snapshot page.
    pub fn snapshot_read(
        &mut self,
        token: SnapshotToken,
        max_records: usize,
        max_text_bytes_per_record: usize,
    ) -> Result<SnapshotPage, SnapshotError> {
        if max_records == 0 {
            return Err(SnapshotError::InvalidLimit);
        }
        let cursor = self.snapshot.ok_or(SnapshotError::InvalidCursor)?;
        if cursor.token != token {
            return Err(SnapshotError::InvalidCursor);
        }
        if cursor.revision != self.revision {
            self.snapshot = None;
            return Err(SnapshotError::Stale {
                starting: cursor.revision,
                current: self.revision,
            });
        }
        let mut traversal = Vec::with_capacity(self.usage.actors);
        for root in &self.roots {
            collect_preorder_ids(&root.node, &mut traversal).map_err(SnapshotError::Registry)?;
        }
        let end = cursor
            .position
            .saturating_add(max_records)
            .min(traversal.len());
        let mut records = Vec::with_capacity(end.saturating_sub(cursor.position));
        for object_id in &traversal[cursor.position..end] {
            records.push(
                self.snapshot_record(*object_id, max_text_bytes_per_record)
                    .map_err(SnapshotError::Registry)?,
            );
        }
        let ended = end == traversal.len();
        let page = SnapshotPage {
            revision: cursor.revision,
            sequence: cursor.sequence,
            records,
            ended,
        };
        if ended {
            self.snapshot = None;
        } else if let Some(active) = self.snapshot.as_mut() {
            active.position = end;
            active.sequence = active.sequence.saturating_add(1);
        }
        Ok(page)
    }

    /// End a live snapshot cursor without reading its remaining records.
    pub fn snapshot_end(&mut self, token: SnapshotToken) -> Result<(), SnapshotError> {
        match self.snapshot {
            Some(cursor) if cursor.token == token => {
                self.snapshot = None;
                Ok(())
            }
            _ => Err(SnapshotError::InvalidCursor),
        }
    }

    /// Validate, construct, attach, and publish one actor atomically.
    pub fn create(
        &mut self,
        type_id: TypeId,
        destination: CreateDestination<'_>,
        inputs: &[ConstructorInput<'_>],
    ) -> Result<ObjectId, RegistryError> {
        self.ensure_active()?;
        let next_revision = self.next_revision()?;
        let descriptor = self
            .descriptor(type_id)
            .ok_or(RegistryError::UnknownType { type_id })?;
        validate_constructor_inputs(descriptor, inputs)?;

        let (parent, depth, root_name_bytes) =
            self.validate_destination(descriptor, destination)?;
        self.validate_capacity(descriptor, inputs, root_name_bytes, depth)?;
        let reservation = self.reserve_slot()?;
        let object_id = ObjectId::from_parts(reservation.generation, reservation.index as u32);

        let mut constructed = (descriptor.constructor)(ConstructorArgs::new(inputs))?;
        if constructed.ops.type_id() != descriptor.type_id {
            return Err(RegistryError::Internal);
        }
        constructed
            .node
            .meta_mut()
            .set_actor_identity(ActorIdentity { object_id, type_id });

        match destination {
            CreateDestination::Root { name } => self.roots.push(RootRecord {
                name: String::from(name),
                node: constructed.node,
            }),
            CreateDestination::Child { parent } => {
                let parent_node = self.node_mut(parent)?;
                parent_node.append_child_quiet(constructed.node);
            }
        };

        let text_bytes = required_text_bytes(descriptor, inputs, root_name_bytes)?;
        let record = ActorRecord {
            descriptor,
            parent,
            depth,
            text_bytes,
            root_name_bytes: u32::try_from(root_name_bytes).map_err(|_| {
                RegistryError::Capacity {
                    kind: CapacityKind::TextBytes,
                }
            })?,
            resources: descriptor.resource_cost.resources,
            ops: constructed.ops,
        };
        self.publish_slot(reservation, record);
        self.usage.actors += 1;
        if parent.is_none() {
            self.usage.roots += 1;
        }
        self.usage.text_bytes += text_bytes;
        self.usage.resources += descriptor.resource_cost.resources;
        self.revision = next_revision;
        self.last_effects = MutationEffects::DRAW
            .union(MutationEffects::TREE)
            .union(MutationEffects::SNAPSHOT);
        self.last_invalidations.clear();
        self.last_invalidations.push(
            self.node(object_id)?
                .try_effective_bounds()
                .ok_or(RegistryError::DispatchBusy)?,
        );
        if let CreateDestination::Child { parent } = destination {
            self.node_mut(object_id)?.emit_attached();
            self.node_mut(parent)?.emit_child_changed();
        }
        Ok(object_id)
    }

    /// Delete an actor subtree and invalidate every generation in it.
    pub fn delete(&mut self, object_id: ObjectId) -> Result<usize, RegistryError> {
        self.ensure_active()?;
        let next_revision = self.next_revision()?;
        let old_bounds = self
            .node(object_id)?
            .try_effective_bounds()
            .ok_or(RegistryError::DispatchBusy)?;
        let parent = self.record(object_id)?.parent;
        let mut removed = if let Some(parent_id) = parent {
            let parent_node = self.node_mut(parent_id)?;
            let index = parent_node
                .children()
                .iter()
                .position(|child| {
                    child
                        .actor_identity()
                        .is_some_and(|identity| identity.object_id == object_id)
                })
                .ok_or(RegistryError::Internal)?;
            parent_node
                .detach_child_quiet(index)
                .ok_or(RegistryError::Internal)?
        } else {
            let index = self
                .roots
                .iter()
                .position(|root| {
                    root.node
                        .actor_identity()
                        .is_some_and(|identity| identity.object_id == object_id)
                })
                .ok_or(RegistryError::Internal)?;
            let mut root = self.roots.remove(index).node;
            root.set_detached_recursive(true);
            root
        };

        removed.set_detached_recursive(true);
        let mut ids = Vec::new();
        collect_postorder_ids(&removed, &mut ids)?;
        for id in &ids {
            self.retire_slot(*id)?;
        }
        self.revision = next_revision;
        self.last_effects = MutationEffects::DRAW
            .union(MutationEffects::TREE)
            .union(MutationEffects::SNAPSHOT);
        self.last_invalidations.clear();
        self.last_invalidations.push(old_bounds);
        removed.emit_detached();
        if let Some(parent) = parent {
            self.node_mut(parent)?.emit_child_changed();
        }
        Ok(ids.len())
    }

    /// Tear down every root and permanently close this Stage Registry.
    pub fn teardown(&mut self) -> Result<usize, RegistryError> {
        let prepared = self.prepare_stage_teardown()?;
        let deleted = prepared.deletion_count();
        let committed = self
            .commit_prepared_teardown(prepared)
            .map_err(|error| error.cause())?;
        self.release_committed_teardown(committed)?;
        Ok(deleted)
    }

    fn ensure_active(&self) -> Result<(), RegistryError> {
        if self.active {
            Ok(())
        } else {
            Err(RegistryError::StageClosed)
        }
    }

    fn next_revision(&self) -> Result<StageRevision, RegistryError> {
        self.revision.next().ok_or(RegistryError::Internal)
    }

    fn validate_destination(
        &self,
        descriptor: &TypeDescriptor,
        destination: CreateDestination<'_>,
    ) -> Result<(Option<ObjectId>, usize, usize), RegistryError> {
        match destination {
            CreateDestination::Root { name } => {
                if name.is_empty()
                    || !descriptor
                        .capabilities
                        .contains(ActorCapabilities::STAGE_ROOT)
                {
                    return Err(RegistryError::InvalidParent);
                }
                if self.root_id(name).is_some() {
                    return Err(RegistryError::DuplicateRoot);
                }
                if self.usage.roots >= self.limits.max_roots {
                    return Err(RegistryError::Capacity {
                        kind: CapacityKind::Roots,
                    });
                }
                Ok((None, 1, name.len()))
            }
            CreateDestination::Child { parent } => {
                let parent_record = self.record(parent)?;
                if !parent_record.descriptor.child_policy.allows(descriptor) {
                    return Err(RegistryError::InvalidParent);
                }
                let parent_node = self.node(parent)?;
                if parent_node.children().len() >= self.limits.max_children_per_actor {
                    return Err(RegistryError::Capacity {
                        kind: CapacityKind::Children,
                    });
                }
                let depth = parent_record.depth + 1;
                if depth > self.limits.max_tree_depth {
                    return Err(RegistryError::Capacity {
                        kind: CapacityKind::TreeDepth,
                    });
                }
                Ok((Some(parent), depth, 0))
            }
        }
    }

    fn validate_capacity(
        &self,
        descriptor: &TypeDescriptor,
        inputs: &[ConstructorInput<'_>],
        root_name_bytes: usize,
        _depth: usize,
    ) -> Result<(), RegistryError> {
        if self.usage.actors >= self.limits.max_actors {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::Actors,
            });
        }
        let text_bytes = required_text_bytes(descriptor, inputs, root_name_bytes)?;
        if self
            .usage
            .text_bytes
            .checked_add(text_bytes)
            .is_none_or(|total| total > self.limits.max_text_bytes)
        {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::TextBytes,
            });
        }
        if self
            .usage
            .resources
            .checked_add(descriptor.resource_cost.resources)
            .is_none_or(|total| total > self.limits.max_resources)
        {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::Resources,
            });
        }
        Ok(())
    }

    fn reserve_slot(&self) -> Result<SlotReservation, RegistryError> {
        if let Some((index, slot)) = self
            .slots
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.record.is_none() && !slot.retired)
        {
            return Ok(SlotReservation {
                index,
                generation: slot.generation,
                append: false,
            });
        }
        if self.slots.len() >= self.limits.max_actors {
            return Err(RegistryError::Capacity {
                kind: CapacityKind::Actors,
            });
        }
        Ok(SlotReservation {
            index: self.slots.len(),
            generation: 1,
            append: true,
        })
    }

    fn publish_slot(&mut self, reservation: SlotReservation, record: ActorRecord) {
        if reservation.append {
            self.slots.push(ActorSlot {
                generation: reservation.generation,
                record: Some(record),
                retired: false,
            });
        } else {
            self.slots[reservation.index].record = Some(record);
        }
    }

    fn record(&self, object_id: ObjectId) -> Result<&ActorRecord, RegistryError> {
        self.ensure_active()?;
        let Some(slot) = self.slots.get(object_id.slot_index()) else {
            return Err(RegistryError::StaleObject { object_id });
        };
        if slot.generation != object_id.generation() {
            return Err(RegistryError::StaleObject { object_id });
        }
        slot.record
            .as_ref()
            .ok_or(RegistryError::StaleObject { object_id })
    }

    fn record_mut(&mut self, object_id: ObjectId) -> Result<&mut ActorRecord, RegistryError> {
        self.ensure_active()?;
        let Some(slot) = self.slots.get_mut(object_id.slot_index()) else {
            return Err(RegistryError::StaleObject { object_id });
        };
        if slot.generation != object_id.generation() {
            return Err(RegistryError::StaleObject { object_id });
        }
        slot.record
            .as_mut()
            .ok_or(RegistryError::StaleObject { object_id })
    }

    fn validate_runtime_flag(
        &self,
        object_id: ObjectId,
        flag: RuntimeFlag,
    ) -> Result<(), RegistryError> {
        let record = self.record(object_id)?;
        match flag {
            RuntimeFlag::Hidden | RuntimeFlag::Enabled => Ok(()),
            RuntimeFlag::Clickable | RuntimeFlag::Focusable
                if record
                    .descriptor
                    .capabilities
                    .contains(ActorCapabilities::CONTROL) =>
            {
                Ok(())
            }
            RuntimeFlag::Clickable | RuntimeFlag::Focusable => Err(RegistryError::Unsupported),
        }
    }

    fn commit_runtime_flag(
        &mut self,
        object_id: ObjectId,
        flag: RuntimeFlag,
        enabled: bool,
    ) -> Result<(), RegistryError> {
        let node = self.node_mut(object_id)?;
        match flag {
            RuntimeFlag::Hidden => node.set_flag(ObjectFlags::HIDDEN, enabled),
            RuntimeFlag::Enabled => {
                node.set_flag(ObjectFlags::DISABLED, !enabled);
                node.set_state(ObjectStates::DISABLED, !enabled);
                if !enabled {
                    node.set_state(ObjectStates::FOCUSED, false);
                    node.set_state(ObjectStates::PRESSED, false);
                    node.set_state(ObjectStates::EDITED, false);
                }
            }
            RuntimeFlag::Clickable => node.set_flag(ObjectFlags::CLICKABLE, enabled),
            RuntimeFlag::Focusable => {
                node.set_flag(ObjectFlags::FOCUSABLE, enabled);
                if !enabled {
                    node.set_state(ObjectStates::FOCUSED, false);
                    node.set_state(ObjectStates::EDITED, false);
                }
            }
        }
        Ok(())
    }

    fn validate_requested_layout(
        &self,
        object_id: ObjectId,
        layout: &RequestedLayout,
    ) -> Result<(), RegistryError> {
        let descriptor = self.record(object_id)?.descriptor;
        let required = match layout {
            RequestedLayout::None => return Ok(()),
            RequestedLayout::Flex(_) => LayoutCapabilities::FLEX_CONTAINER,
            RequestedLayout::Grid(_) => LayoutCapabilities::GRID_CONTAINER,
            RequestedLayout::Item(_) => LayoutCapabilities::ITEM_HINTS,
        };
        if !descriptor.layout.contains(required) {
            return Err(RegistryError::Unsupported);
        }
        if let RequestedLayout::Item(hints) = layout
            && (hints.col_span == 0
                || hints.row_span == 0
                || hints
                    .min_width
                    .zip(hints.max_width)
                    .is_some_and(|(min, max)| min > max)
                || hints
                    .min_height
                    .zip(hints.max_height)
                    .is_some_and(|(min, max)| min > max))
        {
            return Err(RegistryError::Range { field_id: 0 });
        }
        if let RequestedLayout::Grid(config) = layout
            && (config.col_tracks.is_empty()
                || config.row_tracks.is_empty()
                || config
                    .col_tracks
                    .iter()
                    .chain(config.row_tracks.iter())
                    .any(|track| {
                        matches!(track, GridTrack::Px(value) if *value < 0)
                            || matches!(track, GridTrack::Fr(0))
                    }))
        {
            return Err(RegistryError::Range { field_id: 0 });
        }
        Ok(())
    }

    fn reserve_prepared_capacity(
        &mut self,
        prepared: &mut PreparedStageBatch,
    ) -> Result<(), RegistryError> {
        let additional_roots = prepared.max_roots.saturating_sub(self.roots.len());
        self.roots
            .try_reserve_exact(additional_roots)
            .map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::Roots,
            })?;
        for (object_id, maximum) in &prepared.child_capacities {
            let children = self.node_mut(*object_id)?.children_mut();
            let additional = maximum.saturating_sub(children.len());
            children
                .try_reserve_exact(additional)
                .map_err(|_| RegistryError::Capacity {
                    kind: CapacityKind::Children,
                })?;
        }
        Ok(())
    }

    fn detach_prepared(
        &mut self,
        object_id: ObjectId,
        prepared: &mut PreparedStageBatch,
    ) -> ObjectNode {
        let parent = self
            .record(object_id)
            .expect("prepared tree object remains live")
            .parent;
        if let Some(parent_id) = parent {
            let node = {
                let parent_node = self
                    .node_mut(parent_id)
                    .expect("prepared parent remains live");
                let index = parent_node
                    .children()
                    .iter()
                    .position(|child| {
                        child
                            .actor_identity()
                            .is_some_and(|identity| identity.object_id == object_id)
                    })
                    .expect("prepared shadow preserves native child membership");
                parent_node
                    .detach_child_quiet(index)
                    .expect("prepared child index is in range")
            };
            debug_assert!(prepared.lifecycle.len() < prepared.lifecycle.capacity());
            prepared
                .lifecycle
                .push(PendingLifecycle::ChildChanged(parent_id));
            node
        } else {
            let index = self
                .roots
                .iter()
                .position(|root| {
                    root.node
                        .actor_identity()
                        .is_some_and(|identity| identity.object_id == object_id)
                })
                .expect("prepared shadow preserves native root membership");
            let RootRecord { name, node } = self.roots.remove(index);
            debug_assert!(
                prepared.retired_root_names.len() < prepared.retired_root_names.capacity()
            );
            prepared.retired_root_names.push(name);
            let record = self
                .record_mut(object_id)
                .expect("prepared root record remains live");
            record.text_bytes -= record.root_name_bytes;
            record.root_name_bytes = 0;
            node
        }
    }

    fn commit_prepared_reparent(
        &mut self,
        object_id: ObjectId,
        new_parent: ObjectId,
        index: usize,
        prepared: &mut PreparedStageBatch,
    ) {
        let node = self.detach_prepared(object_id, prepared);
        let inserted = self
            .node_mut(new_parent)
            .expect("prepared destination remains live")
            .insert_child_quiet(index, node);
        debug_assert!(inserted);
        self.record_mut(object_id)
            .expect("prepared actor record remains live")
            .parent = Some(new_parent);
        debug_assert!(prepared.lifecycle.len() + 2 <= prepared.lifecycle.capacity());
        prepared
            .lifecycle
            .push(PendingLifecycle::Attached(object_id));
        prepared
            .lifecycle
            .push(PendingLifecycle::ChildChanged(new_parent));
    }

    fn commit_prepared_promote(
        &mut self,
        object_id: ObjectId,
        name: String,
        index: usize,
        prepared: &mut PreparedStageBatch,
    ) {
        let node = self.detach_prepared(object_id, prepared);
        debug_assert!(self.roots.len() < self.roots.capacity());
        let name_bytes = u32::try_from(name.len()).expect("prepared root name fits text budget");
        self.roots.insert(index, RootRecord { name, node });
        let record = self
            .record_mut(object_id)
            .expect("prepared promoted actor remains live");
        record.parent = None;
        record.root_name_bytes = name_bytes;
        record.text_bytes = record
            .text_bytes
            .checked_add(name_bytes)
            .expect("prepared root text budget was validated");
        debug_assert!(prepared.lifecycle.len() < prepared.lifecycle.capacity());
        prepared
            .lifecycle
            .push(PendingLifecycle::Attached(object_id));
    }

    fn commit_prepared_reorder(
        &mut self,
        object_id: ObjectId,
        index: usize,
        prepared: &mut PreparedStageBatch,
    ) {
        let parent = self
            .record(object_id)
            .expect("prepared reordered actor remains live")
            .parent;
        if let Some(parent) = parent {
            let parent_node = self
                .node_mut(parent)
                .expect("prepared reorder parent remains live");
            let old = parent_node
                .children()
                .iter()
                .position(|child| {
                    child
                        .actor_identity()
                        .is_some_and(|identity| identity.object_id == object_id)
                })
                .expect("prepared shadow preserves reorder membership");
            let node = parent_node
                .detach_child_quiet(old)
                .expect("prepared reorder index is in range");
            let inserted = parent_node.insert_child_quiet(index, node);
            debug_assert!(inserted);
            debug_assert!(prepared.lifecycle.len() < prepared.lifecycle.capacity());
            prepared
                .lifecycle
                .push(PendingLifecycle::ChildChanged(parent));
        } else {
            let old = self
                .roots
                .iter()
                .position(|root| {
                    root.node
                        .actor_identity()
                        .is_some_and(|identity| identity.object_id == object_id)
                })
                .expect("prepared shadow preserves root reorder membership");
            let root = self.roots.remove(old);
            self.roots.insert(index, root);
        }
    }

    fn commit_prepared_delete(
        &mut self,
        object_id: ObjectId,
        ids: &[ObjectId],
        prepared: &mut PreparedStageBatch,
    ) {
        let mut removed = self.detach_prepared(object_id, prepared);
        removed.set_detached_recursive(true);
        for id in ids {
            let slot = &mut self.slots[id.slot_index()];
            debug_assert_eq!(slot.generation, id.generation());
            let record = slot
                .record
                .take()
                .expect("prepared deletion identity remains live");
            if slot.generation == u32::MAX {
                slot.retired = true;
            } else {
                slot.generation += 1;
            }
            debug_assert!(prepared.retired_records.len() < prepared.retired_records.capacity());
            prepared.retired_records.push(record);
        }
        debug_assert!(prepared.lifecycle.len() < prepared.lifecycle.capacity());
        prepared.lifecycle.push(PendingLifecycle::Detached(removed));
    }

    fn snapshot_record(
        &self,
        object_id: ObjectId,
        max_text_bytes: usize,
    ) -> Result<SnapshotRecord, RegistryError> {
        let record = self.record(object_id)?;
        let node = self.node(object_id)?;
        let mut text_bytes = 0usize;
        let mut truncated = false;
        let mut properties = Vec::with_capacity(record.descriptor.properties.len());
        for descriptor in record.descriptor.properties {
            let value = record.ops.property(descriptor.id)?;
            let next = text_bytes.saturating_add(value.text_bytes());
            if next > max_text_bytes {
                truncated = true;
                properties.push(SnapshotProperty {
                    id: descriptor.id,
                    value: None,
                    redacted: true,
                });
            } else {
                text_bytes = next;
                properties.push(SnapshotProperty {
                    id: descriptor.id,
                    value: Some(value),
                    redacted: false,
                });
            }
        }
        Ok(SnapshotRecord {
            object_id,
            type_id: record.descriptor.type_id,
            stable_type_name: record.descriptor.stable_name,
            parent: record.parent,
            position: self.position(object_id)?,
            children: self.children(object_id)?,
            properties,
            flags: node.flags().bits(),
            states: node.states().bits(),
            requested_layout: RequestedLayout::from_role(&node.requested_layout_role()),
            geometry: self.geometry(object_id)?,
            truncated,
        })
    }

    fn capture_geometry(&self) -> Result<Vec<(ObjectId, Rect)>, RegistryError> {
        let mut geometry = Vec::with_capacity(self.usage.actors);
        self.capture_geometry_into(&mut geometry)?;
        Ok(geometry)
    }

    fn capture_geometry_into(
        &self,
        geometry: &mut Vec<(ObjectId, Rect)>,
    ) -> Result<(), RegistryError> {
        geometry.clear();
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.record.is_none() {
                continue;
            }
            let object_id = ObjectId::from_parts(slot.generation, index as u32);
            let bounds = self
                .node(object_id)?
                .try_effective_bounds()
                .ok_or(RegistryError::DispatchBusy)?;
            debug_assert!(geometry.len() < geometry.capacity());
            geometry.push((object_id, bounds));
        }
        Ok(())
    }

    fn preflight_after_geometry_borrows(
        &self,
        prepared: &PreparedStageBatch,
    ) -> Result<(), RegistryError> {
        for (index, slot) in self.slots.iter().enumerate() {
            let Some(record) = slot.record.as_ref() else {
                continue;
            };
            let object_id = ObjectId::from_parts(slot.generation, index as u32);
            if prepared.deleted_object_ids.contains(&object_id) {
                continue;
            }
            let final_layout = if let Some(mutation) = prepared
                .layout_mutations
                .iter()
                .rev()
                .find(|mutation| mutation.object_id == object_id)
            {
                mutation.next.as_deref()
            } else {
                self.node(object_id)?.layout.as_deref()
            };
            if final_layout.and_then(|layout| layout.computed).is_none() {
                record.ops.bounds()?;
            }
        }
        Ok(())
    }

    fn derive_invalidations_into(
        &self,
        before: &[(ObjectId, Rect)],
        after: &[(ObjectId, Rect)],
        touched: &[ObjectId],
        whole_stage: bool,
        invalidations: &mut Vec<Rect>,
    ) {
        invalidations.clear();
        for (object_id, old) in before {
            let new = after
                .iter()
                .find(|(candidate, _)| candidate == object_id)
                .map(|(_, bounds)| *bounds);
            if whole_stage || touched.contains(object_id) || new.is_none() || new != Some(*old) {
                push_rect_unique(
                    invalidations,
                    new.map_or(*old, |new_bounds| old.union(new_bounds)),
                );
            }
        }
        for (object_id, bounds) in after.iter().copied() {
            if before.iter().all(|(candidate, _)| *candidate != object_id)
                && (whole_stage || touched.contains(&object_id))
            {
                push_rect_unique(invalidations, bounds);
            }
        }
    }

    fn node_mut(&mut self, object_id: ObjectId) -> Result<&mut ObjectNode, RegistryError> {
        self.record(object_id)?;
        self.roots
            .iter_mut()
            .find_map(|root| find_node_mut(&mut root.node, object_id))
            .ok_or(RegistryError::Internal)
    }

    fn retire_slot(&mut self, object_id: ObjectId) -> Result<(), RegistryError> {
        let slot = self
            .slots
            .get_mut(object_id.slot_index())
            .ok_or(RegistryError::Internal)?;
        if slot.generation != object_id.generation() {
            return Err(RegistryError::Internal);
        }
        let record = slot.record.take().ok_or(RegistryError::Internal)?;
        self.usage.actors -= 1;
        if record.root_name_bytes > 0 {
            self.usage.roots -= 1;
        }
        self.usage.text_bytes -= record.text_bytes;
        self.usage.resources -= record.resources;
        if slot.generation == u32::MAX {
            slot.retired = true;
        } else {
            slot.generation += 1;
        }
        Ok(())
    }
}

fn validate_catalog(catalog: &[TypeDescriptor]) -> Result<(), RegistryError> {
    if catalog.is_empty() {
        return Err(RegistryError::InvalidCatalog);
    }
    for (index, descriptor) in catalog.iter().enumerate() {
        if descriptor.stable_name.is_empty() || descriptor.schema_revision == 0 {
            return Err(RegistryError::InvalidCatalog);
        }
        if catalog[..index].iter().any(|prior| {
            prior.type_id == descriptor.type_id || prior.stable_name == descriptor.stable_name
        }) {
            return Err(RegistryError::InvalidCatalog);
        }
        for (field_index, field) in descriptor.constructor_fields.iter().enumerate() {
            if field.id == 0
                || field.name.is_empty()
                || descriptor.constructor_fields[..field_index]
                    .iter()
                    .any(|prior| prior.id == field.id || prior.name == field.name)
            {
                return Err(RegistryError::InvalidCatalog);
            }
        }
        for (property_index, property) in descriptor.properties.iter().enumerate() {
            if property.id == 0
                || property.name.is_empty()
                || descriptor.properties[..property_index]
                    .iter()
                    .any(|prior| prior.id == property.id || prior.name == property.name)
                || !descriptor
                    .capabilities
                    .contains(property.required_capabilities)
                || property
                    .default
                    .tag()
                    .is_some_and(|tag| tag != property.value_tag)
                || match property.constraint {
                    PropertyConstraint::None => false,
                    PropertyConstraint::I32 { min, max } => {
                        property.value_tag != ValueTag::I32 || min > max
                    }
                    PropertyConstraint::TextBytes { .. } => property.value_tag != ValueTag::Text,
                }
            {
                return Err(RegistryError::InvalidCatalog);
            }
        }
        for (action_index, action) in descriptor.actions.iter().enumerate() {
            if action.id == 0
                || action.name.is_empty()
                || descriptor.actions[..action_index]
                    .iter()
                    .any(|prior| prior.id == action.id || prior.name == action.name)
                || !descriptor
                    .capabilities
                    .contains(action.required_capabilities)
            {
                return Err(RegistryError::InvalidCatalog);
            }
        }
        for (event_index, event) in descriptor.events.iter().enumerate() {
            let coalescing_is_valid = match event.delivery {
                EventDelivery::Critical | EventDelivery::Ordered => event.coalescing_key.is_none(),
                EventDelivery::LatestValueCoalescible => event.coalescing_key.is_some(),
            };
            if event.id == 0
                || event.name.is_empty()
                || event.phases.is_empty()
                || event.filters.is_empty()
                || !event.filters.contains(EventFilterSet::ANY)
                || (event.requires_native_consumed && !event.requires_widget_invocation)
                || (event.allow_consume_at_target && !event.phases.allows(DispatchPhase::Target))
                || !coalescing_is_valid
                || minimum_event_payload_bytes(event.payload)
                    .is_none_or(|minimum| minimum > event.max_payload_bytes)
                || descriptor.events[..event_index]
                    .iter()
                    .any(|prior| prior.id == event.id || prior.name == event.name)
                || catalog[..index]
                    .iter()
                    .flat_map(|prior| prior.events)
                    .any(|prior| prior.id == event.id)
            {
                return Err(RegistryError::InvalidCatalog);
            }
        }
    }
    Ok(())
}

fn minimum_event_payload_bytes(tags: &[ValueTag]) -> Option<u32> {
    tags.iter().try_fold(0u32, |total, tag| {
        let bytes = match tag {
            ValueTag::None => 1,
            ValueTag::Bool => 2,
            ValueTag::I32 | ValueTag::U32 | ValueTag::Precise | ValueTag::Color => 5,
            ValueTag::I64 | ValueTag::U64 => 9,
            ValueTag::Point | ValueTag::Size | ValueTag::Enum => 9,
            ValueTag::Rect => 17,
            ValueTag::Text | ValueTag::Bytes => 5,
            ValueTag::Object => 9,
            ValueTag::Resource => 13,
            ValueTag::BatchObject => 3,
        };
        total.checked_add(bytes)
    })
}

fn validate_event_payload(
    descriptor: &EventDescriptor,
    payload: &[u8],
) -> Result<(), RegistryError> {
    let mut position = 0usize;
    for expected in descriptor.payload {
        let (value, consumed) =
            decode_value(&payload[position..]).map_err(|_| RegistryError::Internal)?;
        if value_tag(value) != *expected {
            return Err(RegistryError::Internal);
        }
        position = position
            .checked_add(consumed)
            .ok_or(RegistryError::Internal)?;
    }
    if position != payload.len() {
        return Err(RegistryError::Internal);
    }
    Ok(())
}

fn validate_constructor_inputs(
    descriptor: &TypeDescriptor,
    inputs: &[ConstructorInput<'_>],
) -> Result<(), RegistryError> {
    for (index, input) in inputs.iter().enumerate() {
        if inputs[..index].iter().any(|prior| prior.id == input.id) {
            return Err(RegistryError::DuplicateField { field_id: input.id });
        }
        let field = descriptor
            .constructor_fields
            .iter()
            .find(|field| field.id == input.id)
            .ok_or(RegistryError::UnknownField { field_id: input.id })?;
        let actual = value_tag(input.value);
        if actual != field.value_tag {
            return Err(RegistryError::TypeMismatch {
                field_id: input.id,
                expected: field.value_tag,
                actual,
            });
        }
    }
    for field in descriptor.constructor_fields {
        if field.required && !inputs.iter().any(|input| input.id == field.id) {
            return Err(RegistryError::MissingField { field_id: field.id });
        }
    }
    Ok(())
}

fn required_text_bytes(
    descriptor: &TypeDescriptor,
    inputs: &[ConstructorInput<'_>],
    root_name_bytes: usize,
) -> Result<u32, RegistryError> {
    let mut total = descriptor.resource_cost.text_bytes;
    total = total
        .checked_add(
            u32::try_from(root_name_bytes).map_err(|_| RegistryError::Capacity {
                kind: CapacityKind::TextBytes,
            })?,
        )
        .ok_or(RegistryError::Capacity {
            kind: CapacityKind::TextBytes,
        })?;
    for input in inputs {
        if let ValueRef::Text(text) = input.value {
            total = total
                .checked_add(
                    u32::try_from(text.len()).map_err(|_| RegistryError::Capacity {
                        kind: CapacityKind::TextBytes,
                    })?,
                )
                .ok_or(RegistryError::Capacity {
                    kind: CapacityKind::TextBytes,
                })?;
        }
    }
    Ok(total)
}

fn value_tag(value: ValueRef<'_>) -> ValueTag {
    match value {
        ValueRef::None => ValueTag::None,
        ValueRef::Bool(_) => ValueTag::Bool,
        ValueRef::I32(_) => ValueTag::I32,
        ValueRef::U32(_) => ValueTag::U32,
        ValueRef::I64(_) => ValueTag::I64,
        ValueRef::U64(_) => ValueTag::U64,
        ValueRef::Precise(_) => ValueTag::Precise,
        ValueRef::Color(_) => ValueTag::Color,
        ValueRef::Point { .. } => ValueTag::Point,
        ValueRef::Size { .. } => ValueTag::Size,
        ValueRef::Rect { .. } => ValueTag::Rect,
        ValueRef::Enum { .. } => ValueTag::Enum,
        ValueRef::Text(_) => ValueTag::Text,
        ValueRef::Bytes(_) => ValueTag::Bytes,
        ValueRef::Object(_) => ValueTag::Object,
        ValueRef::Resource { .. } => ValueTag::Resource,
        ValueRef::BatchObject(_) => ValueTag::BatchObject,
    }
}

fn find_node(node: &ObjectNode, object_id: ObjectId) -> Option<&ObjectNode> {
    if node
        .actor_identity()
        .is_some_and(|identity| identity.object_id == object_id)
    {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| find_node(child, object_id))
}

fn find_node_mut(node: &mut ObjectNode, object_id: ObjectId) -> Option<&mut ObjectNode> {
    if node
        .actor_identity()
        .is_some_and(|identity| identity.object_id == object_id)
    {
        return Some(node);
    }
    node.children_mut()
        .iter_mut()
        .find_map(|child| find_node_mut(child, object_id))
}

fn find_path_to_object(node: &ObjectNode, object_id: ObjectId, path: &mut Vec<usize>) -> bool {
    if node
        .actor_identity()
        .is_some_and(|identity| identity.object_id == object_id)
    {
        return true;
    }
    for (index, child) in node.children().iter().enumerate() {
        path.push(index);
        if find_path_to_object(child, object_id, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn collect_postorder_ids(node: &ObjectNode, ids: &mut Vec<ObjectId>) -> Result<(), RegistryError> {
    for child in node.children() {
        collect_postorder_ids(child, ids)?;
    }
    let identity = node.actor_identity().ok_or(RegistryError::Internal)?;
    ids.push(identity.object_id);
    Ok(())
}

fn collect_preorder_ids(node: &ObjectNode, ids: &mut Vec<ObjectId>) -> Result<(), RegistryError> {
    let identity = node.actor_identity().ok_or(RegistryError::Internal)?;
    ids.push(identity.object_id);
    for child in node.children() {
        collect_preorder_ids(child, ids)?;
    }
    Ok(())
}

#[cfg(test)]
mod prepared_geometry_tests {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    use super::*;
    use crate::event::Event;
    use crate::layout::FlexConfig;
    use crate::renderer::Renderer;

    struct TrackingAllocator;

    thread_local! {
        static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
        static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
        static DEALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    }

    // SAFETY: every operation delegates unchanged layouts and pointers to the
    // process System allocator; thread-local bookkeeping only observes calls.
    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
                let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
            }
            // SAFETY: `layout` is forwarded unchanged to the System allocator.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
                let _ = DEALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
            }
            // SAFETY: both values came from the matching System allocation.
            unsafe { System.dealloc(pointer, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
                let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
            }
            // SAFETY: `layout` is forwarded unchanged to the System allocator.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
                let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
            }
            // SAFETY: the allocation and layout belong to System; `size` is
            // the requested replacement size under GlobalAlloc's contract.
            unsafe { System.realloc(pointer, layout, size) }
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

    const TEST_TYPE: TypeId = TypeId::registered(0x0001_ff01);
    const TEST_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
        type_id: TEST_TYPE,
        stable_name: "rlvgl_core::actor::tests::PreparedGeometryActor",
        schema_revision: 1,
        family: ActorFamily::Container,
        capabilities: ActorCapabilities::STAGE_ROOT.union(ActorCapabilities::CHILDREN),
        targets: TargetSet::ALL,
        constructor_fields: &[ConstructorFieldDescriptor {
            id: 1,
            name: "bounds",
            value_tag: ValueTag::Rect,
            required: true,
        }],
        properties: &[],
        actions: &[],
        events: &[],
        child_policy: ChildPolicy::AnyActor,
        layout: LayoutCapabilities::FLEX_CONTAINER,
        resource_cost: ResourceCost {
            text_bytes: 0,
            resources: 0,
        },
        constructor: construct_test_actor,
    };
    static TEST_CATALOG: [TypeDescriptor; 1] = [TEST_DESCRIPTOR];

    struct TestActor {
        bounds: Rect,
    }

    impl Widget for TestActor {
        fn bounds(&self) -> Rect {
            self.bounds
        }

        fn draw(&self, _renderer: &mut dyn Renderer) {}

        fn handle_event(&mut self, _event: &Event) -> bool {
            false
        }
    }

    impl MpyActor for TestActor {
        type Prepared = ();

        fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
            Err(RegistryError::UnknownProperty { property_id: id })
        }

        fn prepare(
            &self,
            directions: &[ActorDirection],
        ) -> Result<ActorPreparation<Self::Prepared>, RegistryError> {
            if directions.is_empty() {
                Ok(ActorPreparation {
                    prepared: (),
                    text_delta: 0,
                })
            } else {
                Err(RegistryError::BatchInvalid)
            }
        }

        fn commit(&mut self, (): Self::Prepared) {}
    }

    fn construct_test_actor(
        arguments: ConstructorArgs<'_>,
    ) -> Result<ConstructedActor, RegistryError> {
        Ok(construct_native_actor(
            TEST_TYPE,
            TestActor {
                bounds: arguments.required_rect(1)?,
            },
        ))
    }

    fn count_allocator_operations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
        struct TrackingGuard;

        impl Drop for TrackingGuard {
            fn drop(&mut self) {
                TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
            }
        }

        ALLOCATION_COUNT.with(|count| count.set(0));
        DEALLOCATION_COUNT.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
        let guard = TrackingGuard;
        let result = operation();
        drop(guard);
        let allocations = ALLOCATION_COUNT.with(Cell::get);
        let deallocations = DEALLOCATION_COUNT.with(Cell::get);
        (result, allocations, deallocations)
    }

    fn registry() -> StageRegistry {
        StageRegistry::new(
            StageId::new(13).unwrap(),
            &TEST_CATALOG,
            RegistryLimits {
                max_roots: 2,
                max_actors: 4,
                max_tree_depth: 4,
                max_children_per_actor: 4,
                max_text_bytes: 64,
                max_resources: 4,
            },
        )
        .unwrap()
    }

    fn create_container(registry: &mut StageRegistry) -> ObjectId {
        registry
            .create(
                TEST_TYPE,
                CreateDestination::Root { name: "main" },
                &[ConstructorInput {
                    id: 1,
                    value: ValueRef::Rect {
                        x: 0,
                        y: 0,
                        width: 20,
                        height: 10,
                    },
                }],
            )
            .unwrap()
    }

    #[test]
    fn computed_to_none_with_retained_widget_borrow_rejects_before_mutation() {
        let mut registry = registry();
        let root = create_container(&mut registry);
        registry
            .apply_batch(&[StageDirection::SetRequestedLayout {
                object_id: root,
                layout: RequestedLayout::Flex(FlexConfig::default()),
            }])
            .unwrap();
        let computed = Rect {
            x: 3,
            y: 4,
            width: 30,
            height: 40,
        };
        registry
            .node_mut(root)
            .unwrap()
            .layout
            .as_deref_mut()
            .unwrap()
            .computed = Some(computed);
        let starting_revision = registry.revision();
        let starting_usage = registry.usage();
        let starting_effects = registry.last_commit_effects();
        let starting_invalidations = registry.last_invalidations().to_vec();
        let prepared = registry
            .prepare_batch(vec![StageDirection::SetRequestedLayout {
                object_id: root,
                layout: RequestedLayout::None,
            }])
            .unwrap();
        let widget = registry.node(root).unwrap().widget().clone();
        let retained_borrow = widget.borrow_mut();

        let (error, allocations, deallocations) =
            count_allocator_operations(|| registry.commit_prepared_batch(prepared).unwrap_err());

        assert_eq!(allocations, 0);
        assert_eq!(deallocations, 0);
        assert_eq!(error.cause(), RegistryError::DispatchBusy);
        assert_eq!(registry.revision(), starting_revision);
        assert_eq!(registry.usage(), starting_usage);
        assert_eq!(registry.last_commit_effects(), starting_effects);
        assert_eq!(registry.last_invalidations(), starting_invalidations);
        assert_eq!(
            registry.requested_layout(root).unwrap(),
            RequestedLayout::Flex(FlexConfig::default())
        );
        assert_eq!(registry.node(root).unwrap().effective_bounds(), computed);

        drop(retained_borrow);
        drop(error.into_prepared());
    }
}
