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
    object::ObjectNode,
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

/// Type-erased operations retained beside an [`ObjectNode`].
pub trait ActorOps {
    /// Return the descriptor-assigned actor type.
    fn type_id(&self) -> TypeId;
    /// Return the native widget's current intrinsic bounds.
    fn bounds(&self) -> Rect;
}

struct TypedActorOps<T> {
    actor: Rc<RefCell<T>>,
    type_id: TypeId,
}

impl<T: Widget> ActorOps for TypedActorOps<T> {
    fn type_id(&self) -> TypeId {
        self.type_id
    }

    fn bounds(&self) -> Rect {
        self.actor.borrow().bounds()
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
        self.ops.bounds()
    }
}

/// Build one native actor while retaining typed and erased handles to the same state.
pub fn construct_native_actor<T>(type_id: TypeId, actor: T) -> ConstructedActor
where
    T: Widget + 'static,
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
            Self::UnknownField { .. } | Self::MissingField { .. } | Self::DuplicateField { .. } => {
                ErrorClass::InvalidFrame
            }
            Self::TypeMismatch { .. } => ErrorClass::TypeMismatch,
            Self::Range { .. } => ErrorClass::Range,
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
    resources: u16,
    ops: Box<dyn ActorOps>,
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
        Ok(self.record(object_id)?.ops.bounds())
    }

    /// Validate, construct, attach, and publish one actor atomically.
    pub fn create(
        &mut self,
        type_id: TypeId,
        destination: CreateDestination<'_>,
        inputs: &[ConstructorInput<'_>],
    ) -> Result<ObjectId, RegistryError> {
        self.ensure_active()?;
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
                parent_node.append_child(constructed.node);
            }
        }

        let text_bytes = required_text_bytes(descriptor, inputs, root_name_bytes)?;
        let record = ActorRecord {
            descriptor,
            parent,
            depth,
            text_bytes,
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
        Ok(object_id)
    }

    /// Delete an actor subtree and invalidate every generation in it.
    pub fn delete(&mut self, object_id: ObjectId) -> Result<usize, RegistryError> {
        self.ensure_active()?;
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
                .detach_child(index)
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
        Ok(deleted)
    }

    fn ensure_active(&self) -> Result<(), RegistryError> {
        if self.active {
            Ok(())
        } else {
            Err(RegistryError::StageClosed)
        }
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
        if record.parent.is_none() {
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
