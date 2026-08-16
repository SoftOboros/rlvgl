//! MPY-04 stage directions and owned introspection values.
//!
//! The wire codec continues to use borrowed [`rlvgl_api::protocol::ValueRef`]
//! values.  This module owns the corresponding runtime values so validation
//! and allocation can finish before a stage commit begins.

use alloc::{string::String, vec::Vec};

use rlvgl_api::protocol::{ValueRef, ValueTag};

use crate::{
    actor::{ObjectId, TypeId},
    layout::{FlexConfig, GridConfig, ItemHints, LayoutRole},
    widget::Rect,
};

/// Monotonic revision of one stage's director-visible state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct StageRevision(u64);

impl StageRevision {
    /// Construct a revision from its serialized value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the serialized value.
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) => Some(Self(next)),
            None => None,
        }
    }
}

/// Director-owned counterpart of the borrowed MPY value vocabulary.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum OwnedValue {
    /// Explicit absence.
    None,
    /// Boolean value.
    Bool(bool),
    /// Signed 32-bit integer.
    I32(i32),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Signed 64-bit integer.
    I64(i64),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed fixed-precision value.
    Precise(i32),
    /// ARGB8888 color.
    Color(u32),
    /// Logical point.
    Point {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Logical size.
    Size {
        /// Logical width.
        width: i32,
        /// Logical height.
        height: i32,
    },
    /// Logical rectangle.
    Rect {
        /// Horizontal origin.
        x: i32,
        /// Vertical origin.
        y: i32,
        /// Logical width.
        width: i32,
        /// Logical height.
        height: i32,
    },
    /// Domain-qualified enumeration.
    Enum {
        /// Stable enumeration domain.
        domain: u32,
        /// Value within the domain.
        value: u32,
    },
    /// UTF-8 text.
    Text(String),
    /// Opaque bytes.
    Bytes(Vec<u8>),
    /// Stable object identifier.
    Object(u64),
    /// Kind-qualified resource identifier.
    Resource {
        /// Registered resource kind.
        kind: u32,
        /// Stable resource identifier.
        id: u64,
    },
    /// Batch-local object reference.
    BatchObject(u16),
}

impl OwnedValue {
    /// Copy or allocate a borrowed protocol value into stage-owned storage.
    pub fn from_ref(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::None => Self::None,
            ValueRef::Bool(value) => Self::Bool(value),
            ValueRef::I32(value) => Self::I32(value),
            ValueRef::U32(value) => Self::U32(value),
            ValueRef::I64(value) => Self::I64(value),
            ValueRef::U64(value) => Self::U64(value),
            ValueRef::Precise(value) => Self::Precise(value),
            ValueRef::Color(value) => Self::Color(value),
            ValueRef::Point { x, y } => Self::Point { x, y },
            ValueRef::Size { width, height } => Self::Size { width, height },
            ValueRef::Rect {
                x,
                y,
                width,
                height,
            } => Self::Rect {
                x,
                y,
                width,
                height,
            },
            ValueRef::Enum { domain, value } => Self::Enum { domain, value },
            ValueRef::Text(value) => Self::Text(String::from(value)),
            ValueRef::Bytes(value) => Self::Bytes(Vec::from(value)),
            ValueRef::Object(value) => Self::Object(value),
            ValueRef::Resource { kind, id } => Self::Resource { kind, id },
            ValueRef::BatchObject(value) => Self::BatchObject(value),
        }
    }

    /// Return the stable wire tag for this value.
    pub const fn tag(&self) -> ValueTag {
        match self {
            Self::None => ValueTag::None,
            Self::Bool(_) => ValueTag::Bool,
            Self::I32(_) => ValueTag::I32,
            Self::U32(_) => ValueTag::U32,
            Self::I64(_) => ValueTag::I64,
            Self::U64(_) => ValueTag::U64,
            Self::Precise(_) => ValueTag::Precise,
            Self::Color(_) => ValueTag::Color,
            Self::Point { .. } => ValueTag::Point,
            Self::Size { .. } => ValueTag::Size,
            Self::Rect { .. } => ValueTag::Rect,
            Self::Enum { .. } => ValueTag::Enum,
            Self::Text(_) => ValueTag::Text,
            Self::Bytes(_) => ValueTag::Bytes,
            Self::Object(_) => ValueTag::Object,
            Self::Resource { .. } => ValueTag::Resource,
            Self::BatchObject(_) => ValueTag::BatchObject,
        }
    }

    pub(crate) fn text_bytes(&self) -> usize {
        match self {
            Self::Text(value) => value.len(),
            _ => 0,
        }
    }

    pub(crate) fn as_ref(&self) -> ValueRef<'_> {
        match self {
            Self::None => ValueRef::None,
            Self::Bool(value) => ValueRef::Bool(*value),
            Self::I32(value) => ValueRef::I32(*value),
            Self::U32(value) => ValueRef::U32(*value),
            Self::I64(value) => ValueRef::I64(*value),
            Self::U64(value) => ValueRef::U64(*value),
            Self::Precise(value) => ValueRef::Precise(*value),
            Self::Color(value) => ValueRef::Color(*value),
            Self::Point { x, y } => ValueRef::Point { x: *x, y: *y },
            Self::Size { width, height } => ValueRef::Size {
                width: *width,
                height: *height,
            },
            Self::Rect {
                x,
                y,
                width,
                height,
            } => ValueRef::Rect {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            },
            Self::Enum { domain, value } => ValueRef::Enum {
                domain: *domain,
                value: *value,
            },
            Self::Text(value) => ValueRef::Text(value.as_str()),
            Self::Bytes(value) => ValueRef::Bytes(value.as_slice()),
            Self::Object(value) => ValueRef::Object(*value),
            Self::Resource { kind, id } => ValueRef::Resource {
                kind: *kind,
                id: *id,
            },
            Self::BatchObject(value) => ValueRef::BatchObject(*value),
        }
    }
}

/// Stable or earlier batch-local actor reference used by an atomic Stage batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchObjectReference {
    /// Generation-checked actor already published in the Stage.
    Stable(ObjectId),
    /// Raw nonzero binding produced by an earlier Create in this batch.
    EarlierBatch(u16),
}

/// One owned constructor-only field supplied to an atomic Create.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateField {
    /// Descriptor-local constructor field identifier.
    pub id: u32,
    /// Owned neutral constructor value.
    pub value: OwnedValue,
}

/// Append-only destination of one atomic Create operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchCreateDestination {
    /// Append a uniquely named Stage root.
    Root {
        /// Stage-owned root name.
        name: String,
    },
    /// Append below a stable or earlier-created parent.
    Child {
        /// Parent reference resolved by the batch shadow.
        parent: BatchObjectReference,
    },
}

/// One owned generic Create request inside an atomic Stage batch.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateDirection {
    /// Raw nonzero binding unique within the submitted batch.
    pub batch_ref: u16,
    /// Registered actor type to construct.
    pub type_id: TypeId,
    /// Append-only root or child destination.
    pub destination: BatchCreateDestination,
    /// Constructor-only fields; initial mutable state uses later directions.
    pub fields: Vec<CreateField>,
}

/// One atomic Stage operation whose actor references may name earlier Creates.
///
/// Vector order is the submitted zero-based operation index. A successful
/// Create contributes one retained Object result at that index.
#[derive(Clone, Debug, PartialEq)]
pub enum BatchStageDirection {
    /// Construct and append one actor without publishing it before commit.
    Create(CreateDirection),
    /// Apply one or more actor-local property/action transitions collectively.
    MutateActor {
        /// Stable or earlier-created target actor.
        object: BatchObjectReference,
        /// Collectively validated actor-local directions.
        directions: Vec<ActorDirection>,
    },
    /// Set a descriptor-gated runtime flag.
    SetFlag {
        /// Stable or earlier-created target actor.
        object: BatchObjectReference,
        /// Runtime-owned flag.
        flag: RuntimeFlag,
        /// Requested semantic value.
        enabled: bool,
    },
    /// Replace requested layout state.
    SetRequestedLayout {
        /// Stable or earlier-created target actor.
        object: BatchObjectReference,
        /// Complete replacement layout request.
        layout: RequestedLayout,
    },
    /// Attempt to write computed geometry; always rejected as read-only.
    SetComputedGeometry {
        /// Stable or earlier-created target actor.
        object: BatchObjectReference,
        /// Rejected replacement geometry.
        bounds: Rect,
    },
    /// Reparent an actor subtree at one exact ordered index.
    Reparent {
        /// Stable or earlier-created subtree root.
        object: BatchObjectReference,
        /// Stable or earlier-created destination parent.
        new_parent: BatchObjectReference,
        /// Exact final child index.
        index: usize,
    },
    /// Promote or move an actor into the named-root order.
    PromoteRoot {
        /// Stable or earlier-created subtree root.
        object: BatchObjectReference,
        /// Unique Stage-root name.
        name: String,
        /// Exact final root index.
        index: usize,
    },
    /// Reorder an actor within its current parent or root order.
    Reorder {
        /// Stable or earlier-created actor.
        object: BatchObjectReference,
        /// Exact final sibling/root index.
        index: usize,
    },
    /// Delete an existing actor and its subtree.
    Delete {
        /// Stable or earlier-created subtree root.
        object: BatchObjectReference,
    },
    /// Local style addressing remains explicitly unsupported in this slice.
    SetLocalStyle {
        /// Stable or earlier-created target actor.
        object: BatchObjectReference,
        /// Independent part selector.
        part_id: u32,
        /// Independent native state mask.
        state_mask: u32,
        /// Stable style property identifier.
        property_id: u32,
        /// Replacement local value.
        value: OwnedValue,
    },
}

/// One descriptor-validated actor-local mutation.
#[derive(Clone, Debug, PartialEq)]
pub enum ActorDirection {
    /// Set a writable durable property.
    SetProperty {
        /// Descriptor property identifier.
        id: u32,
        /// Replacement value.
        value: OwnedValue,
    },
    /// Restore a property to its declared default or absence.
    ResetProperty {
        /// Descriptor property identifier.
        id: u32,
    },
    /// Invoke a typed actor action.
    InvokeAction {
        /// Descriptor action identifier.
        id: u32,
        /// Ordered typed arguments.
        arguments: Vec<OwnedValue>,
    },
}

/// Runtime-owned object metadata flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFlag {
    /// Visibility and target eligibility.
    Hidden,
    /// Inverse of the native disabled flag/state pair.
    Enabled,
    /// Pointer target eligibility.
    Clickable,
    /// Focus traversal eligibility.
    Focusable,
}

/// Requested, director-owned layout configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestedLayout {
    /// No requested layout role.
    None,
    /// Flex container configuration.
    Flex(FlexConfig),
    /// Grid container configuration.
    Grid(GridConfig),
    /// Child item hints.
    Item(ItemHints),
}

impl RequestedLayout {
    pub(crate) fn from_role(role: &LayoutRole) -> Self {
        use crate::layout::EngineConfig;
        match role {
            LayoutRole::None => Self::None,
            LayoutRole::Container(EngineConfig::Flex(config)) => Self::Flex(config.clone()),
            LayoutRole::Container(EngineConfig::Grid(config)) => Self::Grid(config.clone()),
            LayoutRole::Item(hints) => Self::Item(hints.clone()),
        }
    }
}

/// One atomic stage command. Unsupported local-style mutation is explicit.
#[derive(Clone, Debug, PartialEq)]
pub enum StageDirection {
    /// Apply one or more actor-local property/action transitions collectively.
    MutateActor {
        /// Target actor.
        object_id: ObjectId,
        /// Collectively validated actor-local directions.
        directions: Vec<ActorDirection>,
    },
    /// Set a descriptor-gated runtime flag.
    SetFlag {
        /// Target actor.
        object_id: ObjectId,
        /// Runtime-owned flag.
        flag: RuntimeFlag,
        /// Requested semantic value.
        enabled: bool,
    },
    /// Replace requested layout state.
    SetRequestedLayout {
        /// Target actor.
        object_id: ObjectId,
        /// Complete replacement layout request.
        layout: RequestedLayout,
    },
    /// Attempt to write computed geometry; always rejected as read-only.
    SetComputedGeometry {
        /// Target actor.
        object_id: ObjectId,
        /// Rejected replacement geometry.
        bounds: Rect,
    },
    /// Reparent an actor as a child at an exact ordered index.
    Reparent {
        /// Actor subtree root to move.
        object_id: ObjectId,
        /// Destination parent.
        new_parent: ObjectId,
        /// Exact final child index.
        index: usize,
    },
    /// Promote or move an actor into the named-root order.
    PromoteRoot {
        /// Actor subtree root to promote.
        object_id: ObjectId,
        /// Unique stage-root name.
        name: String,
        /// Exact final root index.
        index: usize,
    },
    /// Reorder an actor within its current parent or root order.
    Reorder {
        /// Actor to reorder.
        object_id: ObjectId,
        /// Exact final sibling/root index.
        index: usize,
    },
    /// Delete an actor and its subtree.
    Delete {
        /// Root of the subtree to delete.
        object_id: ObjectId,
    },
    /// Local style addressing is reserved but not implemented by this slice.
    SetLocalStyle {
        /// Target actor.
        object_id: ObjectId,
        /// Independent part selector.
        part_id: u32,
        /// Independent native state mask.
        state_mask: u32,
        /// Stable style property identifier.
        property_id: u32,
        /// Replacement local value.
        value: OwnedValue,
    },
}

/// Read-only geometry result, separate from requested layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryResult {
    /// Native widget bounds before object-managed placement.
    pub intrinsic_bounds: Rect,
    /// Canonical object-managed bounds.
    pub effective_bounds: Rect,
    /// Stage revision at which both values were read.
    pub revision: StageRevision,
    /// Current object-managed layout participation.
    pub layout_role: GeometryRole,
}

/// A node's current participation in object-managed layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryRole {
    /// No active layout role.
    None,
    /// Actor runs a layout engine over its children.
    Container,
    /// Actor carries parent-consumed item hints.
    Item,
}

/// Opaque handle for the minimum-profile single snapshot cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotToken(pub(crate) u32);

/// One property projected into a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotProperty {
    /// Descriptor property identifier.
    pub id: u32,
    /// Value when it fit in the bounded record budget.
    pub value: Option<OwnedValue>,
    /// Whether the value was explicitly omitted due to the bound.
    pub redacted: bool,
}

/// Deterministic snapshot actor record.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotRecord {
    /// Stable actor identifier.
    pub object_id: ObjectId,
    /// Descriptor actor type.
    pub type_id: TypeId,
    /// Stable descriptor name.
    pub stable_type_name: &'static str,
    /// Parent identifier or `None` for a root.
    pub parent: Option<ObjectId>,
    /// Ordered sibling or root position.
    pub position: usize,
    /// Ordered direct children.
    pub children: Vec<ObjectId>,
    /// Readable actor-owned properties.
    pub properties: Vec<SnapshotProperty>,
    /// Native object flags.
    pub flags: u32,
    /// Native object states.
    pub states: u32,
    /// Director-requested layout state.
    pub requested_layout: RequestedLayout,
    /// Read-only geometry result.
    pub geometry: GeometryResult,
    /// Whether any field was explicitly redacted.
    pub truncated: bool,
}

/// One bounded page from a snapshot cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotPage {
    /// Revision frozen at cursor begin.
    pub revision: StageRevision,
    /// Zero-based page sequence.
    pub sequence: u32,
    /// Bounded ordered records.
    pub records: Vec<SnapshotRecord>,
    /// Whether traversal completed and released the cursor.
    pub ended: bool,
}

/// Explicit snapshot cursor failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// A stage cursor is already active.
    Busy,
    /// The token is not the active cursor.
    InvalidCursor,
    /// Stage state changed after cursor begin.
    Stale {
        /// Revision at cursor begin.
        starting: StageRevision,
        /// Current revision that invalidated the cursor.
        current: StageRevision,
    },
    /// A requested page bound was zero or otherwise invalid.
    InvalidLimit,
    /// Underlying stage lookup or actor read failed.
    Registry(crate::actor::RegistryError),
}
