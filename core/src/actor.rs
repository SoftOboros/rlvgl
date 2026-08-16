//! MPY actor descriptors and compatibility-first Stage Registry.
//!
//! This module owns the language-neutral runtime substrate selected by MPY-03.
//! Actor declarations remain beside their native widget implementations; a
//! registry consumes a static catalog of those declarations without storing
//! raw pointers to nodes in its slot table.

use alloc::{boxed::Box, rc::Rc, string::String, vec::Vec};
use core::cell::RefCell;

pub use rlvgl_api::protocol::{ErrorClass, ValueRef, ValueTag};

use crate::{
    direction::{
        ActorDirection, GeometryResult, GeometryRole, OwnedValue, RequestedLayout, RuntimeFlag,
        SnapshotError, SnapshotPage, SnapshotProperty, SnapshotRecord, SnapshotToken,
        StageDirection, StageRevision,
    },
    layout::{GridTrack, LayoutRole},
    object::{ObjectFlags, ObjectNode, ObjectStates},
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
    alive: bool,
}

struct TreeShadow {
    actors: Vec<ShadowActor>,
    roots: Vec<(String, ObjectId)>,
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
            actors.push(ShadowActor {
                object_id,
                parent: record.parent,
                children: registry.children(object_id)?,
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
                self.mark_deleted(*object_id)?;
            }
            _ => {}
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
    fn from_parts(generation: u32, slot: u32) -> Self {
        debug_assert!(generation != 0);
        Self((u64::from(generation) << 32) | u64::from(slot))
    }

    /// Convert a nonzero serialized value into an Object identifier.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
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
    /// Every queued cue must be preserved in order.
    Ordered,
    /// Runtime may coalesce superseded cues according to the event policy.
    Coalescible,
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
    /// Cue-delivery classification.
    pub delivery: EventDelivery,
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
/// into the actor and therefore cannot fail.
pub trait MpyActor: Widget {
    /// Fully allocated actor-local state for one mutation group.
    type Prepared: 'static;

    /// Read a descriptor-owned durable property.
    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError>;

    /// Validate a collective property/action group without mutation.
    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<Self::Prepared>, RegistryError>;

    /// Infallibly publish state returned by [`prepare`](Self::prepare).
    fn commit(&mut self, prepared: Self::Prepared);
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
    /// Infallibly publish the prepared native state.
    fn commit(self: Box<Self>);
}

/// Type-erased operations retained beside an [`ObjectNode`].
pub trait ActorOps {
    /// Return the descriptor-assigned actor type.
    fn type_id(&self) -> TypeId;
    /// Return the native widget's current intrinsic bounds.
    fn bounds(&self) -> Result<Rect, RegistryError>;
    /// Read one actor-owned property.
    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError>;
    /// Prepare an actor-local group without native mutation.
    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<Box<dyn PreparedActorMutation>, RegistryError>;
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

    fn commit(mut self: Box<Self>) {
        let prepared = self
            .prepared
            .take()
            .expect("prepared mutation consumed once");
        self.actor
            .try_borrow_mut()
            .expect("atomic commit follows exclusive borrow preflight without callbacks")
            .commit(prepared);
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
    ops: Box<dyn ActorOps>,
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
        ops: Box::new(TypedActorOps {
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
    ops: Box<dyn ActorOps>,
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
    pending_lifecycle: Vec<PendingLifecycle>,
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
            pending_lifecycle: Vec::new(),
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

    /// Validate and publish an atomic group under one Stage Revision.
    pub fn apply_batch(
        &mut self,
        directions: &[StageDirection],
    ) -> Result<StageRevision, RegistryError> {
        self.ensure_active()?;
        if directions.is_empty() {
            return Err(RegistryError::BatchInvalid);
        }
        let next_revision = self.next_revision()?;
        let mut actor_groups: Vec<(ObjectId, Vec<ActorDirection>)> = Vec::new();
        let mut tree_shadow = TreeShadow::capture(self)?;
        let mut effects = MutationEffects::NONE;
        let mut touched = Vec::new();

        for direction in directions {
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

        let mut prepared = Vec::with_capacity(actor_groups.len());
        let mut total_text_delta = tree_shadow.root_text_delta;
        for (object_id, group) in actor_groups {
            let mutation = self.record(object_id)?.ops.prepare(&group)?;
            total_text_delta = total_text_delta.checked_add(mutation.text_delta()).ok_or(
                RegistryError::Capacity {
                    kind: CapacityKind::TextBytes,
                },
            )?;
            prepared.push(PreparedActorGroup {
                object_id,
                mutation,
            });
        }
        let final_text = i64::from(self.usage.text_bytes)
            .checked_add(total_text_delta)
            .filter(|total| *total >= 0 && *total <= i64::from(self.limits.max_text_bytes))
            .ok_or(RegistryError::Capacity {
                kind: CapacityKind::TextBytes,
            })?;
        let before_geometry = self.capture_geometry()?;

        for group in prepared {
            let delta = group.mutation.text_delta();
            group.mutation.commit();
            let record = self.record_mut(group.object_id)?;
            record.text_bytes = apply_text_delta(record.text_bytes, delta)?;
            self.usage.text_bytes = apply_text_delta(self.usage.text_bytes, delta)?;
        }
        for direction in directions {
            match direction {
                StageDirection::SetFlag {
                    object_id,
                    flag,
                    enabled,
                } => self.commit_runtime_flag(*object_id, *flag, *enabled)?,
                StageDirection::SetRequestedLayout { object_id, layout } => {
                    self.commit_requested_layout(*object_id, layout.clone())?
                }
                StageDirection::Reparent { .. }
                | StageDirection::PromoteRoot { .. }
                | StageDirection::Reorder { .. }
                | StageDirection::Delete { .. } => self.commit_tree_direction(direction)?,
                _ => {}
            }
        }
        debug_assert_eq!(u32::try_from(final_text).ok(), Some(self.usage.text_bytes));
        self.revision = next_revision;
        self.last_effects = effects;
        self.last_invalidations = self.derive_invalidations(
            &before_geometry,
            &touched,
            effects.contains(MutationEffects::TREE) || effects.contains(MutationEffects::LAYOUT),
        )?;
        self.publish_pending_lifecycle();
        Ok(self.revision)
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
        self.ensure_active()?;
        let mut deleted = 0;
        while let Some(object_id) = self
            .roots
            .first()
            .and_then(|root| root.node.actor_identity())
            .map(|identity| identity.object_id)
        {
            deleted += self.delete(object_id)?;
        }
        self.active = false;
        self.snapshot = None;
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
        let Some(slot) = self.slots.get(object_id.slot() as usize) else {
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
        let Some(slot) = self.slots.get_mut(object_id.slot() as usize) else {
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

    fn commit_requested_layout(
        &mut self,
        object_id: ObjectId,
        layout: RequestedLayout,
    ) -> Result<(), RegistryError> {
        let node = self.node_mut(object_id)?;
        match layout {
            RequestedLayout::None => node.clear_requested_layout(),
            RequestedLayout::Flex(config) => node.set_layout_flex(config),
            RequestedLayout::Grid(config) => node.set_layout_grid(config),
            RequestedLayout::Item(hints) => node.set_item_hints(hints),
        }
        Ok(())
    }

    fn detach_for_tree(&mut self, object_id: ObjectId) -> Result<ObjectNode, RegistryError> {
        let parent = self.record(object_id)?.parent;
        if let Some(parent_id) = parent {
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
            let node = parent_node
                .detach_child_quiet(index)
                .ok_or(RegistryError::Internal)?;
            self.pending_lifecycle
                .push(PendingLifecycle::ChildChanged(parent_id));
            Ok(node)
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
            let root = self.roots.remove(index);
            self.usage.roots -= 1;
            self.usage.text_bytes -= self.record(object_id)?.root_name_bytes;
            let record = self.record_mut(object_id)?;
            record.text_bytes -= record.root_name_bytes;
            record.root_name_bytes = 0;
            Ok(root.node)
        }
    }

    fn commit_tree_direction(&mut self, direction: &StageDirection) -> Result<(), RegistryError> {
        match direction {
            StageDirection::Reparent {
                object_id,
                new_parent,
                index,
            } => {
                let node = self.detach_for_tree(*object_id)?;
                if !self.node_mut(*new_parent)?.insert_child_quiet(*index, node) {
                    return Err(RegistryError::Internal);
                }
                self.record_mut(*object_id)?.parent = Some(*new_parent);
                self.refresh_depths(*object_id, self.record(*new_parent)?.depth + 1)?;
                self.pending_lifecycle
                    .push(PendingLifecycle::Attached(*object_id));
                self.pending_lifecycle
                    .push(PendingLifecycle::ChildChanged(*new_parent));
            }
            StageDirection::PromoteRoot {
                object_id,
                name,
                index,
            } => {
                let node = self.detach_for_tree(*object_id)?;
                self.roots.insert(
                    *index,
                    RootRecord {
                        name: name.clone(),
                        node,
                    },
                );
                let name_bytes = u32::try_from(name.len()).map_err(|_| RegistryError::Internal)?;
                let record = self.record_mut(*object_id)?;
                record.parent = None;
                record.root_name_bytes = name_bytes;
                record.text_bytes = record
                    .text_bytes
                    .checked_add(name_bytes)
                    .ok_or(RegistryError::Internal)?;
                self.usage.roots += 1;
                self.usage.text_bytes = self
                    .usage
                    .text_bytes
                    .checked_add(name_bytes)
                    .ok_or(RegistryError::Internal)?;
                self.refresh_depths(*object_id, 1)?;
                self.pending_lifecycle
                    .push(PendingLifecycle::Attached(*object_id));
            }
            StageDirection::Reorder { object_id, index } => {
                let parent = self.record(*object_id)?.parent;
                if let Some(parent) = parent {
                    let parent_node = self.node_mut(parent)?;
                    let old = parent_node
                        .children()
                        .iter()
                        .position(|child| {
                            child
                                .actor_identity()
                                .is_some_and(|identity| identity.object_id == *object_id)
                        })
                        .ok_or(RegistryError::Internal)?;
                    let node = parent_node
                        .detach_child_quiet(old)
                        .ok_or(RegistryError::Internal)?;
                    if !parent_node.insert_child_quiet(*index, node) {
                        return Err(RegistryError::Internal);
                    }
                    self.pending_lifecycle
                        .push(PendingLifecycle::ChildChanged(parent));
                } else {
                    let old = self.position(*object_id)?;
                    let root = self.roots.remove(old);
                    self.roots.insert(*index, root);
                }
            }
            StageDirection::Delete { object_id } => {
                let mut removed = self.detach_for_tree(*object_id)?;
                let mut ids = Vec::new();
                collect_postorder_ids(&removed, &mut ids)?;
                for id in &ids {
                    self.retire_slot(*id)?;
                }
                removed.set_detached_recursive(true);
                self.pending_lifecycle
                    .push(PendingLifecycle::Detached(removed));
            }
            _ => {}
        }
        Ok(())
    }

    fn refresh_depths(&mut self, root: ObjectId, depth: usize) -> Result<(), RegistryError> {
        self.record_mut(root)?.depth = depth;
        let children = self.children(root)?;
        for child in children {
            self.refresh_depths(child, depth + 1)?;
        }
        Ok(())
    }

    fn publish_pending_lifecycle(&mut self) {
        let pending = core::mem::take(&mut self.pending_lifecycle);
        for event in pending {
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
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.record.is_none() {
                continue;
            }
            let object_id = ObjectId::from_parts(slot.generation, index as u32);
            let bounds = self
                .node(object_id)?
                .try_effective_bounds()
                .ok_or(RegistryError::DispatchBusy)?;
            geometry.push((object_id, bounds));
        }
        Ok(geometry)
    }

    fn derive_invalidations(
        &self,
        before: &[(ObjectId, Rect)],
        touched: &[ObjectId],
        whole_stage: bool,
    ) -> Result<Vec<Rect>, RegistryError> {
        let after = self.capture_geometry()?;
        let mut invalidations = Vec::new();
        for (object_id, old) in before {
            let new = after
                .iter()
                .find(|(candidate, _)| candidate == object_id)
                .map(|(_, bounds)| *bounds);
            if whole_stage || touched.contains(object_id) || new.is_none() || new != Some(*old) {
                push_rect_unique(
                    &mut invalidations,
                    new.map_or(*old, |new_bounds| old.union(new_bounds)),
                );
            }
        }
        for (object_id, bounds) in after {
            if before.iter().all(|(candidate, _)| *candidate != object_id)
                && (whole_stage || touched.contains(&object_id))
            {
                push_rect_unique(&mut invalidations, bounds);
            }
        }
        Ok(invalidations)
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
            .get_mut(object_id.slot() as usize)
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
