// protocol.rs - Allocation-free canonical MPY v1 logical-frame and value codec.

//! Canonical, allocation-free encoding for MPY logical frames and tagged values.
//!
//! The codec writes to caller-provided buffers, reads borrowed payloads, and
//! never depends on Rust object layout. Multi-byte integers are little-endian.

use core::{fmt, str};

/// Bytes in every canonical logical-frame header.
pub const FRAME_HEADER_LEN: usize = 8;

/// MPY v1 minimum complete logical-frame capacity.
pub const MIN_FRAME_BYTES: u32 = 256;

/// MPY v1 minimum UTF-8 text capacity.
pub const MIN_TEXT_BYTES: u32 = 128;

/// MPY v1 minimum command fields or Batch operations per logical frame.
pub const MIN_ITEMS_PER_COMMAND: u16 = 8;

/// Stable MPY command and Batch-operation opcode registry.
pub mod opcode {
    /// Invalid opcode sentinel.
    pub const INVALID: u32 = 0;
    /// Create one actor and bind its nonzero Batch reference.
    pub const CREATE: u32 = 0x0000_0001;
    /// Set one or more actor properties collectively.
    pub const SET_PROPERTIES: u32 = 0x0000_0002;
    /// Reset one or more actor properties to descriptor defaults or absence.
    pub const RESET_PROPERTIES: u32 = 0x0000_0003;
    /// Invoke one descriptor-owned actor action.
    pub const INVOKE_ACTION: u32 = 0x0000_0004;
    /// Set one descriptor-gated runtime flag.
    pub const SET_FLAG: u32 = 0x0000_0005;
    /// Replace one actor's complete requested-layout state.
    pub const SET_REQUESTED_LAYOUT: u32 = 0x0000_0006;
    /// Reparent an actor subtree at one exact child index.
    pub const REPARENT: u32 = 0x0000_0007;
    /// Promote or move an actor into the named-root order.
    pub const PROMOTE_ROOT: u32 = 0x0000_0008;
    /// Reorder an actor within its current parent or root order.
    pub const REORDER: u32 = 0x0000_0009;
    /// Delete an actor and its complete subtree.
    pub const DELETE: u32 = 0x0000_000a;
    /// Set one actor-local style value at an explicit selector.
    pub const SET_LOCAL_STYLE: u32 = 0x0000_000b;
    /// First opcode available only through explicit experimental/private negotiation.
    pub const EXPERIMENTAL_FIRST: u32 = 0x8000_0000;
    /// Last opcode available only through explicit experimental/private negotiation.
    pub const EXPERIMENTAL_LAST: u32 = 0xffff_fffe;
    /// Permanently reserved opcode sentinel.
    pub const RESERVED: u32 = 0xffff_ffff;
}

/// MPY protocol version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Major compatibility component.
    pub major: u8,
    /// Minor capability component.
    pub minor: u8,
    /// Patch component.
    pub patch: u8,
}

/// Canonical MPY v1 protocol version.
pub const MPY_V1: ProtocolVersion = ProtocolVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

/// Logical frame discriminants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    /// Endpoint greeting and initial limits.
    Hello = 0x01,
    /// Detailed supported capability declaration.
    Capabilities = 0x02,
    /// One runtime command.
    Command = 0x03,
    /// One atomic sequence of operations.
    Batch = 0x04,
    /// One correlated completion.
    Result = 0x05,
    /// One subscribed callback cue.
    Cue = 0x06,
    /// One unsolicited runtime diagnostic notice.
    RuntimeNotice = 0x07,
}

impl FrameKind {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::Capabilities),
            0x03 => Ok(Self::Command),
            0x04 => Ok(Self::Batch),
            0x05 => Ok(Self::Result),
            0x06 => Ok(Self::Cue),
            0x07 => Ok(Self::RuntimeNotice),
            value => Err(CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::FrameKind,
                value,
            }),
        }
    }
}

/// Tagged-value discriminants in table order from MPY-02.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueTag {
    /// Explicit absence.
    None = 0x00,
    /// Boolean encoded as exactly zero or one.
    Bool = 0x01,
    /// Signed 32-bit integer.
    I32 = 0x02,
    /// Unsigned 32-bit integer.
    U32 = 0x03,
    /// Signed 64-bit integer.
    I64 = 0x04,
    /// Unsigned 64-bit integer.
    U64 = 0x05,
    /// Signed fixed-precision value.
    Precise = 0x06,
    /// ARGB8888 color.
    Color = 0x07,
    /// Logical point.
    Point = 0x08,
    /// Logical size.
    Size = 0x09,
    /// Logical rectangle.
    Rect = 0x0a,
    /// Domain-qualified enumeration value.
    Enum = 0x0b,
    /// Length-delimited UTF-8 text.
    Text = 0x0c,
    /// Length-delimited opaque bytes.
    Bytes = 0x0d,
    /// Stable object identifier.
    Object = 0x0e,
    /// Kind-qualified resource identifier.
    Resource = 0x0f,
    /// Batch-local object reference.
    BatchObject = 0x10,
}

impl ValueTag {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Bool),
            0x02 => Ok(Self::I32),
            0x03 => Ok(Self::U32),
            0x04 => Ok(Self::I64),
            0x05 => Ok(Self::U64),
            0x06 => Ok(Self::Precise),
            0x07 => Ok(Self::Color),
            0x08 => Ok(Self::Point),
            0x09 => Ok(Self::Size),
            0x0a => Ok(Self::Rect),
            0x0b => Ok(Self::Enum),
            0x0c => Ok(Self::Text),
            0x0d => Ok(Self::Bytes),
            0x0e => Ok(Self::Object),
            0x0f => Ok(Self::Resource),
            0x10 => Ok(Self::BatchObject),
            value => Err(CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::ValueTag,
                value,
            }),
        }
    }
}

/// Stable MPY error classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorClass {
    /// Structurally invalid frame or value.
    InvalidFrame = 0x01,
    /// Incompatible protocol version.
    VersionMismatch = 0x02,
    /// Unknown stage identifier.
    StageNotFound = 0x03,
    /// Stale object generation or deleted object.
    StaleObject = 0x04,
    /// Unknown actor type.
    UnknownType = 0x05,
    /// Unknown actor property.
    UnknownProperty = 0x06,
    /// Unknown actor action.
    UnknownAction = 0x07,
    /// Unknown actor event.
    UnknownEvent = 0x08,
    /// Value has the wrong tag.
    TypeMismatch = 0x09,
    /// Value is outside the accepted range.
    Range = 0x0a,
    /// Attempted write to read-only state.
    ReadOnly = 0x0b,
    /// Parent or child relationship is invalid.
    InvalidParent = 0x0c,
    /// Capability is not supported.
    Unsupported = 0x0d,
    /// Negotiated capacity would be exceeded.
    Capacity = 0x0e,
    /// Transport or cue queue is full.
    QueueFull = 0x0f,
    /// Runtime cannot enter a safe dispatch turn.
    DispatchBusy = 0x10,
    /// Atomic batch validation failed.
    BatchInvalid = 0x11,
    /// Internal runtime failure.
    Internal = 0x12,
}

impl ErrorClass {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0x01 => Ok(Self::InvalidFrame),
            0x02 => Ok(Self::VersionMismatch),
            0x03 => Ok(Self::StageNotFound),
            0x04 => Ok(Self::StaleObject),
            0x05 => Ok(Self::UnknownType),
            0x06 => Ok(Self::UnknownProperty),
            0x07 => Ok(Self::UnknownAction),
            0x08 => Ok(Self::UnknownEvent),
            0x09 => Ok(Self::TypeMismatch),
            0x0a => Ok(Self::Range),
            0x0b => Ok(Self::ReadOnly),
            0x0c => Ok(Self::InvalidParent),
            0x0d => Ok(Self::Unsupported),
            0x0e => Ok(Self::Capacity),
            0x0f => Ok(Self::QueueFull),
            0x10 => Ok(Self::DispatchBusy),
            0x11 => Ok(Self::BatchInvalid),
            0x12 => Ok(Self::Internal),
            value => Err(CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::ErrorClass,
                value,
            }),
        }
    }
}

/// Discriminant table associated with an unsupported byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscriminantDomain {
    /// Logical frame kind.
    FrameKind,
    /// Tagged value kind.
    ValueTag,
    /// Stable error class.
    ErrorClass,
    /// Create destination kind.
    CreateDestination,
}

/// Canonical codec failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    /// The caller-provided destination cannot hold the encoding.
    BufferTooSmall,
    /// The source ended before a declared field completed.
    Truncated,
    /// A field has a noncanonical or structurally invalid representation.
    InvalidFrame,
    /// A structurally valid frame exceeds an active negotiated capacity.
    LimitExceeded,
    /// A discriminant is not part of MPY v1.
    UnsupportedDiscriminant {
        /// Table in which the value was looked up.
        domain: DiscriminantDomain,
        /// Unsupported byte value.
        value: u8,
    },
}

/// Negotiated logical-frame limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum complete logical-frame bytes.
    pub max_frame_bytes: u32,
    /// Maximum bytes in one UTF-8 text value.
    pub max_text_bytes: u32,
    /// Maximum bytes in one opaque byte value.
    pub max_byte_payload: u32,
    /// Maximum typed fields or Batch operations in one command envelope.
    pub max_items_per_command: u16,
    /// Maximum typed values in one result.
    pub max_values_per_result: u16,
}

impl Limits {
    /// Return the v1 wire slot's former fields-only projection.
    ///
    /// PCDN-MPY-02-005 generalized this unchanged `u16` wire position to
    /// [`Self::max_items_per_command`]. This accessor preserves source-level
    /// reads for adapters that still use the earlier fields-only terminology.
    pub const fn max_fields_per_command(self) -> u16 {
        self.max_items_per_command
    }
}

/// Borrowed MPY tagged value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueRef<'a> {
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
    /// Domain-qualified enumeration value.
    Enum {
        /// Stable enumeration domain.
        domain: u32,
        /// Value within that domain.
        value: u32,
    },
    /// Valid UTF-8 text.
    Text(&'a str),
    /// Opaque bytes.
    Bytes(&'a [u8]),
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

impl ValueRef<'_> {
    /// Return the stable tag carried by this value.
    pub const fn tag(self) -> ValueTag {
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
}

/// Contextual object reference carried by an existing MPY value tag.
///
/// This is a semantic view over [`ValueRef::Object`] and
/// [`ValueRef::BatchObject`], not a new wire discriminant. Opcode-owned codecs
/// use it only in fields whose schema declares an object reference. Resolving
/// one batch-local reference to an earlier unique Create remains a semantic
/// Batch-validation step rather than part of this structural codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectReference {
    /// Stable generation-checked object identity.
    Object(u64),
    /// Nonzero reference bound by an earlier Create in the same Batch.
    BatchObject(u16),
}

/// Failure to decode or semantically classify an object-reference field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectReferenceError {
    /// The underlying tagged value was malformed or unsupported.
    Codec(CodecError),
    /// A canonical value carried a tag not permitted by an object-reference field.
    TypeMismatch {
        /// Canonical tag supplied by the field.
        actual: ValueTag,
    },
}

impl From<CodecError> for ObjectReferenceError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl ObjectReference {
    /// Return the existing tagged value used to encode this reference.
    pub const fn as_value(self) -> ValueRef<'static> {
        match self {
            Self::Object(value) => ValueRef::Object(value),
            Self::BatchObject(value) => ValueRef::BatchObject(value),
        }
    }
}

impl TryFrom<ValueRef<'_>> for ObjectReference {
    type Error = ObjectReferenceError;

    fn try_from(value: ValueRef<'_>) -> Result<Self, Self::Error> {
        match value {
            ValueRef::Object(value) => Ok(Self::Object(value)),
            ValueRef::BatchObject(value) => Ok(Self::BatchObject(value)),
            value => Err(ObjectReferenceError::TypeMismatch {
                actual: value.tag(),
            }),
        }
    }
}

/// One nonzero keyed field in a canonical typed field list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldRef<'a> {
    /// Stable descriptor- or opcode-owned field identifier.
    pub id: u32,
    /// Canonical tagged field value.
    pub value: ValueRef<'a>,
}

#[derive(Clone, Copy)]
enum ValueSource<'a> {
    Native(&'a [ValueRef<'a>]),
    Wire { count: usize, values: &'a [u8] },
}

/// Borrowed counted list of canonical tagged values.
#[derive(Clone, Copy)]
pub struct ValueList<'a> {
    source: ValueSource<'a>,
}

impl<'a> ValueList<'a> {
    /// Build a value list from native borrowed values for encoding.
    pub const fn from_slice(values: &'a [ValueRef<'a>]) -> Self {
        Self {
            source: ValueSource::Native(values),
        }
    }

    fn from_wire(count: usize, values: &'a [u8]) -> Self {
        Self {
            source: ValueSource::Wire { count, values },
        }
    }

    /// Return the exact value count.
    pub fn len(self) -> usize {
        match self.source {
            ValueSource::Native(values) => values.len(),
            ValueSource::Wire { count, .. } => count,
        }
    }

    /// Return whether the list contains no values.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Iterate values in their canonical positional order.
    pub fn iter(self) -> ValueIter<'a> {
        let source = match self.source {
            ValueSource::Native(values) => ValueIterSource::Native(values.iter()),
            ValueSource::Wire { count, values } => ValueIterSource::Wire {
                remaining: count,
                reader: Reader::new(values),
            },
        };
        ValueIter { source }
    }
}

impl fmt::Debug for ValueList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for ValueList<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

enum ValueIterSource<'a> {
    Native(core::slice::Iter<'a, ValueRef<'a>>),
    Wire {
        remaining: usize,
        reader: Reader<'a>,
    },
}

/// Iterator over a native or zero-copy wire-backed value list.
pub struct ValueIter<'a> {
    source: ValueIterSource<'a>,
}

impl<'a> Iterator for ValueIter<'a> {
    type Item = ValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.source {
            ValueIterSource::Native(values) => values.next().copied(),
            ValueIterSource::Wire { remaining, reader } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                Some(decode_value_from_reader(reader).expect("wire value list was validated"))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.source {
            ValueIterSource::Native(values) => values.len(),
            ValueIterSource::Wire { remaining, .. } => *remaining,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ValueIter<'_> {}

#[derive(Clone, Copy)]
enum FieldSource<'a> {
    Native(&'a [FieldRef<'a>]),
    Wire { count: usize, fields: &'a [u8] },
}

/// Borrowed keyed list of strictly increasing canonical typed fields.
#[derive(Clone, Copy)]
pub struct FieldList<'a> {
    source: FieldSource<'a>,
}

impl<'a> FieldList<'a> {
    /// Build a field list from native borrowed fields for encoding.
    pub const fn from_slice(fields: &'a [FieldRef<'a>]) -> Self {
        Self {
            source: FieldSource::Native(fields),
        }
    }

    fn from_wire(count: usize, fields: &'a [u8]) -> Self {
        Self {
            source: FieldSource::Wire { count, fields },
        }
    }

    /// Return the exact field count.
    pub fn len(self) -> usize {
        match self.source {
            FieldSource::Native(fields) => fields.len(),
            FieldSource::Wire { count, .. } => count,
        }
    }

    /// Return whether the list contains no fields.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Iterate fields in strictly increasing identifier order.
    pub fn iter(self) -> FieldIter<'a> {
        let source = match self.source {
            FieldSource::Native(fields) => FieldIterSource::Native(fields.iter()),
            FieldSource::Wire { count, fields } => FieldIterSource::Wire {
                remaining: count,
                reader: Reader::new(fields),
            },
        };
        FieldIter { source }
    }
}

impl fmt::Debug for FieldList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for FieldList<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

enum FieldIterSource<'a> {
    Native(core::slice::Iter<'a, FieldRef<'a>>),
    Wire {
        remaining: usize,
        reader: Reader<'a>,
    },
}

/// Iterator over a native or zero-copy wire-backed field list.
pub struct FieldIter<'a> {
    source: FieldIterSource<'a>,
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = FieldRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.source {
            FieldIterSource::Native(fields) => fields.next().copied(),
            FieldIterSource::Wire { remaining, reader } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                Some(FieldRef {
                    id: reader.u32().expect("wire field list was validated"),
                    value: decode_value_from_reader(reader).expect("wire field list was validated"),
                })
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.source {
            FieldIterSource::Native(fields) => fields.len(),
            FieldIterSource::Wire { remaining, .. } => *remaining,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for FieldIter<'_> {}

/// One output-bearing operation in a successful Batch result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperationResultRef<'a> {
    /// Zero-based index of the submitted operation.
    pub operation_index: u16,
    /// Nonempty ordered values declared by that operation's schema.
    pub values: ValueList<'a>,
}

#[derive(Clone, Copy)]
enum OperationResultSource<'a> {
    Native(&'a [OperationResultRef<'a>]),
    Wire { count: usize, records: &'a [u8] },
}

/// Borrowed list of strictly increasing successful Batch operation results.
#[derive(Clone, Copy)]
pub struct OperationResultList<'a> {
    source: OperationResultSource<'a>,
}

impl<'a> OperationResultList<'a> {
    /// Build a result list from native borrowed records for encoding.
    pub const fn from_slice(results: &'a [OperationResultRef<'a>]) -> Self {
        Self {
            source: OperationResultSource::Native(results),
        }
    }

    fn from_wire(count: usize, records: &'a [u8]) -> Self {
        Self {
            source: OperationResultSource::Wire { count, records },
        }
    }

    /// Return the exact output-bearing operation count.
    pub fn len(self) -> usize {
        match self.source {
            OperationResultSource::Native(results) => results.len(),
            OperationResultSource::Wire { count, .. } => count,
        }
    }

    /// Return whether the accepted Batch produced no operation outputs.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Iterate result records in strictly increasing operation-index order.
    pub fn iter(self) -> OperationResultIter<'a> {
        let source = match self.source {
            OperationResultSource::Native(results) => {
                OperationResultIterSource::Native(results.iter())
            }
            OperationResultSource::Wire { count, records } => OperationResultIterSource::Wire {
                remaining: count,
                reader: Reader::new(records),
            },
        };
        OperationResultIter { source }
    }
}

impl fmt::Debug for OperationResultList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for OperationResultList<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

enum OperationResultIterSource<'a> {
    Native(core::slice::Iter<'a, OperationResultRef<'a>>),
    Wire {
        remaining: usize,
        reader: Reader<'a>,
    },
}

/// Iterator over native or zero-copy wire-backed Batch operation results.
pub struct OperationResultIter<'a> {
    source: OperationResultIterSource<'a>,
}

impl<'a> Iterator for OperationResultIter<'a> {
    type Item = OperationResultRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.source {
            OperationResultIterSource::Native(results) => results.next().copied(),
            OperationResultIterSource::Wire { remaining, reader } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                let operation_index = reader
                    .u16()
                    .expect("wire operation result list was validated");
                let value_count = reader
                    .u16()
                    .expect("wire operation result list was validated")
                    as usize;
                let values_start = reader.position;
                for _ in 0..value_count {
                    let _ = decode_value_from_reader(reader)
                        .expect("wire operation result list was validated");
                }
                let values = &reader.input[values_start..reader.position];
                Some(OperationResultRef {
                    operation_index,
                    values: ValueList::from_wire(value_count, values),
                })
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.source {
            OperationResultIterSource::Native(results) => results.len(),
            OperationResultIterSource::Wire { remaining, .. } => *remaining,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for OperationResultIter<'_> {}

/// Canonical payload of one successful Batch Result frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BatchSuccess<'a> {
    /// Stage revision visible after the accepted Batch.
    pub result_revision: u64,
    /// Output-bearing operations in submitted operation order.
    pub results: OperationResultList<'a>,
}

/// Stable Create destination discriminants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CreateDestinationTag {
    /// Append a named Stage root.
    Root = 0x01,
    /// Append below an existing or earlier-created parent.
    Child = 0x02,
}

impl CreateDestinationTag {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0x00 => Err(CodecError::InvalidFrame),
            0x01 => Ok(Self::Root),
            0x02 => Ok(Self::Child),
            value => Err(CodecError::UnsupportedDiscriminant {
                domain: DiscriminantDomain::CreateDestination,
                value,
            }),
        }
    }
}

/// Destination carried by one Batch-only Create operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateDestinationRef<'a> {
    /// Append a root under one UTF-8 name.
    ///
    /// The codec preserves an empty name so runtime graph validation can
    /// report the existing `InvalidParent` semantic error.
    Root(&'a str),
    /// Append below one contextual stable or batch-local parent reference.
    Child(ObjectReference),
}

/// Borrowed canonical payload for the Batch-only Create opcode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreatePayload<'a> {
    /// Nonzero batch-local binding declared by this Create.
    pub batch_ref: u16,
    /// Nonzero registered actor type identifier.
    pub type_id: u32,
    /// Append-only root or child destination.
    pub destination: CreateDestinationRef<'a>,
    /// Constructor-only descriptor fields in strictly increasing ID order.
    pub constructor_fields: FieldList<'a>,
}

/// Structural or contextual failure while decoding a Create payload.
///
/// This reuses [`ObjectReferenceError`] so a canonical non-object child value
/// remains distinguishable from malformed bytes.
pub type CreatePayloadError = ObjectReferenceError;

/// Common Batch-only target prefix for MPY v1 mutation operations.
///
/// The target is one contextual object reference. The remainder belongs to the
/// selected opcode and is intentionally opaque to this common codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MutationTargetEnvelope<'a> {
    /// Stable or earlier-created actor targeted by the operation.
    pub target: ObjectReference,
    /// Complete borrowed bytes after the target prefix.
    pub remainder: &'a [u8],
}

/// Structural or contextual failure while decoding a mutation target.
pub type MutationTargetError = ObjectReferenceError;

/// Structural or contextual failure while decoding a Delete payload.
pub type DeletePayloadError = ObjectReferenceError;

/// Complete Batch-only Reorder payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReorderPayload {
    /// Stable or earlier-created actor moved within its current owner.
    pub target: ObjectReference,
    /// Final zero-based position after removing the target from its owner.
    pub index: u32,
}

/// Structural or contextual failure while decoding a Reorder payload.
pub type ReorderPayloadError = ObjectReferenceError;

/// Complete Batch-only Reparent payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReparentPayload {
    /// Stable or earlier-created subtree root being moved.
    pub target: ObjectReference,
    /// Stable or earlier-created destination parent.
    pub new_parent: ObjectReference,
    /// Final zero-based child position after detaching the target.
    pub index: u32,
}

/// Structural or contextual failure while decoding a Reparent payload.
pub type ReparentPayloadError = ObjectReferenceError;

/// Complete Batch-only PromoteRoot payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromoteRootPayload<'a> {
    /// Stable or earlier-created actor becoming or moving as a named root.
    pub target: ObjectReference,
    /// Exact UTF-8 root name assigned after detaching the actor.
    pub name: &'a str,
    /// Final zero-based root position after detaching the actor.
    pub index: u32,
}

/// Structural or contextual failure while decoding a PromoteRoot payload.
pub type PromoteRootPayloadError = ObjectReferenceError;

/// Completion status carried by a Result frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionStatus {
    /// Successful completion.
    Success,
    /// Failed completion with a stable error class.
    Error(ErrorClass),
}

/// Borrowed list of supported opcode IDs.
#[derive(Clone, Copy)]
pub struct OpcodeList<'a> {
    source: OpcodeSource<'a>,
}

#[derive(Clone, Copy)]
enum OpcodeSource<'a> {
    Native(&'a [u32]),
    Wire(&'a [u8]),
}

impl<'a> OpcodeList<'a> {
    /// Builds a list from native opcode words for encoding.
    pub const fn from_slice(opcodes: &'a [u32]) -> Self {
        Self {
            source: OpcodeSource::Native(opcodes),
        }
    }

    fn from_wire(bytes: &'a [u8]) -> Self {
        Self {
            source: OpcodeSource::Wire(bytes),
        }
    }

    /// Number of opcodes.
    pub fn len(self) -> usize {
        match self.source {
            OpcodeSource::Native(values) => values.len(),
            OpcodeSource::Wire(bytes) => bytes.len() / 4,
        }
    }

    /// Returns true when no opcodes are declared.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Iterates decoded opcode IDs.
    pub fn iter(self) -> OpcodeIter<'a> {
        match self.source {
            OpcodeSource::Native(values) => OpcodeIter::Native(values.iter()),
            OpcodeSource::Wire(bytes) => OpcodeIter::Wire(bytes.chunks_exact(4)),
        }
    }
}

impl fmt::Debug for OpcodeList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for OpcodeList<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for OpcodeList<'_> {}

/// Iterator over native or wire-backed opcode IDs.
pub enum OpcodeIter<'a> {
    /// Native-word iterator.
    Native(core::slice::Iter<'a, u32>),
    /// Canonical little-endian byte iterator.
    Wire(core::slice::ChunksExact<'a, u8>),
}

impl Iterator for OpcodeIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Native(values) => values.next().copied(),
            Self::Wire(bytes) => bytes
                .next()
                .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]])),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = match self {
            Self::Native(values) => values.len(),
            Self::Wire(bytes) => bytes.len(),
        };
        (length, Some(length))
    }
}

impl ExactSizeIterator for OpcodeIter<'_> {}

/// Canonical fixed-envelope Batch operation borrowing its opcode-owned payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationRef<'a> {
    /// Explicit globally registered operation opcode.
    pub opcode: u32,
    /// Opcode-declared flags; all initial MPY v1 operations require zero.
    pub flags: u32,
    /// Opcode-owned payload bytes.
    pub payload: &'a [u8],
}

#[derive(Clone, Copy)]
enum OperationSource<'a> {
    Native(&'a [OperationRef<'a>]),
    Wire { count: usize, records: &'a [u8] },
}

/// Borrowed counted list of canonical Batch operations.
#[derive(Clone, Copy)]
pub struct OperationList<'a> {
    source: OperationSource<'a>,
}

impl<'a> OperationList<'a> {
    /// Build an operation list from native borrowed records for encoding.
    pub const fn from_slice(operations: &'a [OperationRef<'a>]) -> Self {
        Self {
            source: OperationSource::Native(operations),
        }
    }

    fn from_wire(count: usize, records: &'a [u8]) -> Self {
        Self {
            source: OperationSource::Wire { count, records },
        }
    }

    /// Return the exact operation count.
    pub fn len(self) -> usize {
        match self.source {
            OperationSource::Native(operations) => operations.len(),
            OperationSource::Wire { count, .. } => count,
        }
    }

    /// Return whether the Batch contains no operations.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Iterate operations in their implicit zero-based operation-index order.
    pub fn iter(self) -> OperationIter<'a> {
        let source = match self.source {
            OperationSource::Native(operations) => OperationIterSource::Native(operations.iter()),
            OperationSource::Wire { count, records } => OperationIterSource::Wire {
                remaining: count,
                reader: Reader::new(records),
            },
        };
        OperationIter { source }
    }
}

impl fmt::Debug for OperationList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl PartialEq for OperationList<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for OperationList<'_> {}

enum OperationIterSource<'a> {
    Native(core::slice::Iter<'a, OperationRef<'a>>),
    Wire {
        remaining: usize,
        reader: Reader<'a>,
    },
}

/// Iterator over native or wire-backed Batch operation records.
pub struct OperationIter<'a> {
    source: OperationIterSource<'a>,
}

impl<'a> Iterator for OperationIter<'a> {
    type Item = OperationRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.source {
            OperationIterSource::Native(operations) => operations.next().copied(),
            OperationIterSource::Wire { remaining, reader } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                Some(OperationRef {
                    opcode: reader.u32().expect("wire operation list was validated"),
                    flags: reader.u32().expect("wire operation list was validated"),
                    payload: reader
                        .bytes_with_length()
                        .expect("wire operation list was validated"),
                })
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.source {
            OperationIterSource::Native(operations) => operations.len(),
            OperationIterSource::Wire { remaining, .. } => *remaining,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for OperationIter<'_> {}

/// Hello-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hello {
    /// Actor-schema version.
    pub schema_version: ProtocolVersion,
    /// Endpoint limits.
    pub limits: Limits,
    /// Supported feature bitset.
    pub features: u64,
}

/// Capabilities-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities<'a> {
    /// Actor-schema version.
    pub schema_version: ProtocolVersion,
    /// Endpoint limits.
    pub limits: Limits,
    /// Supported feature bitset.
    pub features: u64,
    /// Supported `ValueTag` bitset, indexed by its discriminant.
    pub value_tags: u32,
    /// Supported registered opcode IDs.
    pub opcodes: OpcodeList<'a>,
}

/// Command-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command<'a> {
    /// Nonzero stage identifier.
    pub stage_id: u32,
    /// Nonzero request identifier.
    pub request_id: u32,
    /// Registered command opcode.
    pub opcode: u32,
    /// Command flags.
    pub flags: u32,
    /// Opcode-owned encoded payload.
    pub payload: &'a [u8],
}

/// Declared reservation budget for an atomic Batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchBudget {
    /// Actor slots reserved by the batch.
    pub actors: u16,
    /// Text bytes reserved by the batch.
    pub text_bytes: u32,
    /// Resource slots reserved by the batch.
    pub resources: u16,
    /// Complete success-result bytes reserved by the batch.
    pub result_bytes: u32,
}

/// Batch-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch<'a> {
    /// Nonzero stage identifier.
    pub stage_id: u32,
    /// Nonzero request identifier.
    pub request_id: u32,
    /// Batch flags.
    pub flags: u32,
    /// Preflight resource budget.
    pub budget: BatchBudget,
    /// Counted ordered operation records using the canonical fixed envelope.
    pub operations: OperationList<'a>,
}

/// Result-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion<'a> {
    /// Request being completed.
    pub request_id: u32,
    /// Success or stable failure class.
    pub status: CompletionStatus,
    /// First failing or result-bearing operation index when applicable.
    pub operation_index: Option<u16>,
    /// Descriptor or field ID when applicable.
    pub field_id: Option<u32>,
    /// Bounded diagnostic text.
    pub diagnostic: &'a str,
    /// Status-owned typed payload.
    pub payload: &'a [u8],
}

/// Cue-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue<'a> {
    /// Monotonic cue sequence.
    pub sequence: u32,
    /// Nonzero stage identifier.
    pub stage_id: u32,
    /// Nonzero stable object identifier.
    pub object_id: u64,
    /// Nonzero runtime subscription identifier.
    pub subscription_id: u32,
    /// Nonzero adapter callback identifier.
    pub callback_id: u32,
    /// Registered event identifier.
    pub event_id: u32,
    /// Cue flags.
    pub flags: u32,
    /// Event-schema-owned typed payload.
    pub payload: &'a [u8],
}

/// RuntimeNotice-frame payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeNotice<'a> {
    /// Monotonic notice sequence.
    pub sequence: u32,
    /// Registered notice kind.
    pub kind: u32,
    /// Bounded diagnostic text.
    pub diagnostic: &'a str,
    /// Notice-kind-owned payload.
    pub payload: &'a [u8],
}

/// Borrowed logical frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRef<'a> {
    /// Endpoint greeting.
    Hello(Hello),
    /// Detailed capability declaration.
    Capabilities(Capabilities<'a>),
    /// One command.
    Command(Command<'a>),
    /// One atomic batch.
    Batch(Batch<'a>),
    /// One completion result.
    Result(Completion<'a>),
    /// One callback cue.
    Cue(Cue<'a>),
    /// One runtime notice.
    RuntimeNotice(RuntimeNotice<'a>),
}

impl FrameRef<'_> {
    /// Logical frame kind.
    pub const fn kind(self) -> FrameKind {
        match self {
            Self::Hello(_) => FrameKind::Hello,
            Self::Capabilities(_) => FrameKind::Capabilities,
            Self::Command(_) => FrameKind::Command,
            Self::Batch(_) => FrameKind::Batch,
            Self::Result(_) => FrameKind::Result,
            Self::Cue(_) => FrameKind::Cue,
            Self::RuntimeNotice(_) => FrameKind::RuntimeNotice,
        }
    }
}

/// Decoded logical frame and its header protocol version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodedFrame<'a> {
    /// Protocol version from the common header.
    pub version: ProtocolVersion,
    /// Decoded class-specific frame.
    pub frame: FrameRef<'a>,
}

/// Encodes one tagged value into a caller-provided buffer.
pub fn encode_value(value: ValueRef<'_>, output: &mut [u8]) -> Result<usize, CodecError> {
    let mut writer = Writer::new(output);
    encode_value_into(value, &mut writer)?;
    Ok(writer.position)
}

fn encode_value_into(value: ValueRef<'_>, writer: &mut Writer<'_>) -> Result<(), CodecError> {
    match value {
        ValueRef::None => writer.u8(ValueTag::None as u8)?,
        ValueRef::Bool(value) => {
            writer.u8(ValueTag::Bool as u8)?;
            writer.u8(u8::from(value))?;
        }
        ValueRef::I32(value) => tagged_i32(writer, ValueTag::I32, value)?,
        ValueRef::U32(value) => tagged_u32(writer, ValueTag::U32, value)?,
        ValueRef::I64(value) => {
            writer.u8(ValueTag::I64 as u8)?;
            writer.i64(value)?;
        }
        ValueRef::U64(value) => {
            writer.u8(ValueTag::U64 as u8)?;
            writer.u64(value)?;
        }
        ValueRef::Precise(value) => tagged_i32(writer, ValueTag::Precise, value)?,
        ValueRef::Color(value) => tagged_u32(writer, ValueTag::Color, value)?,
        ValueRef::Point { x, y } => {
            writer.u8(ValueTag::Point as u8)?;
            writer.i32(x)?;
            writer.i32(y)?;
        }
        ValueRef::Size { width, height } => {
            writer.u8(ValueTag::Size as u8)?;
            writer.i32(width)?;
            writer.i32(height)?;
        }
        ValueRef::Rect {
            x,
            y,
            width,
            height,
        } => {
            writer.u8(ValueTag::Rect as u8)?;
            writer.i32(x)?;
            writer.i32(y)?;
            writer.i32(width)?;
            writer.i32(height)?;
        }
        ValueRef::Enum { domain, value } => {
            require_nonzero(domain)?;
            writer.u8(ValueTag::Enum as u8)?;
            writer.u32(domain)?;
            writer.u32(value)?;
        }
        ValueRef::Text(value) => {
            writer.u8(ValueTag::Text as u8)?;
            writer.bytes_with_length(value.as_bytes())?;
        }
        ValueRef::Bytes(value) => {
            writer.u8(ValueTag::Bytes as u8)?;
            writer.bytes_with_length(value)?;
        }
        ValueRef::Object(value) => {
            require_object_id(value)?;
            writer.u8(ValueTag::Object as u8)?;
            writer.u64(value)?;
        }
        ValueRef::Resource { kind, id } => {
            require_nonzero(kind)?;
            require_nonzero_u64(id)?;
            writer.u8(ValueTag::Resource as u8)?;
            writer.u32(kind)?;
            writer.u64(id)?;
        }
        ValueRef::BatchObject(value) => {
            if value == 0 {
                return Err(CodecError::InvalidFrame);
            }
            writer.u8(ValueTag::BatchObject as u8)?;
            writer.u16(value)?;
        }
    }
    Ok(())
}

/// Decodes one tagged value and returns the number of consumed bytes.
pub fn decode_value(input: &[u8]) -> Result<(ValueRef<'_>, usize), CodecError> {
    let mut reader = Reader::new(input);
    let value = decode_value_from_reader(&mut reader)?;
    Ok((value, reader.position))
}

fn decode_value_from_reader<'a>(reader: &mut Reader<'a>) -> Result<ValueRef<'a>, CodecError> {
    let tag = ValueTag::decode(reader.u8()?)?;
    let value = match tag {
        ValueTag::None => ValueRef::None,
        ValueTag::Bool => match reader.u8()? {
            0 => ValueRef::Bool(false),
            1 => ValueRef::Bool(true),
            _ => return Err(CodecError::InvalidFrame),
        },
        ValueTag::I32 => ValueRef::I32(reader.i32()?),
        ValueTag::U32 => ValueRef::U32(reader.u32()?),
        ValueTag::I64 => ValueRef::I64(reader.i64()?),
        ValueTag::U64 => ValueRef::U64(reader.u64()?),
        ValueTag::Precise => ValueRef::Precise(reader.i32()?),
        ValueTag::Color => ValueRef::Color(reader.u32()?),
        ValueTag::Point => ValueRef::Point {
            x: reader.i32()?,
            y: reader.i32()?,
        },
        ValueTag::Size => ValueRef::Size {
            width: reader.i32()?,
            height: reader.i32()?,
        },
        ValueTag::Rect => ValueRef::Rect {
            x: reader.i32()?,
            y: reader.i32()?,
            width: reader.i32()?,
            height: reader.i32()?,
        },
        ValueTag::Enum => {
            let domain = reader.u32()?;
            require_nonzero(domain)?;
            ValueRef::Enum {
                domain,
                value: reader.u32()?,
            }
        }
        ValueTag::Text => ValueRef::Text(
            str::from_utf8(reader.bytes_with_length()?).map_err(|_| CodecError::InvalidFrame)?,
        ),
        ValueTag::Bytes => ValueRef::Bytes(reader.bytes_with_length()?),
        ValueTag::Object => {
            let value = reader.u64()?;
            require_object_id(value)?;
            ValueRef::Object(value)
        }
        ValueTag::Resource => {
            let kind = reader.u32()?;
            let id = reader.u64()?;
            require_nonzero(kind)?;
            require_nonzero_u64(id)?;
            ValueRef::Resource { kind, id }
        }
        ValueTag::BatchObject => {
            let value = reader.u16()?;
            if value == 0 {
                return Err(CodecError::InvalidFrame);
            }
            ValueRef::BatchObject(value)
        }
    };
    Ok(value)
}

/// Encode one contextual object reference using an existing MPY value tag.
pub fn encode_object_reference(
    reference: ObjectReference,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    encode_value(reference.as_value(), output)
}

/// Decode one contextual object reference and return the consumed byte count.
///
/// A canonical value carrying any tag other than [`ValueTag::Object`] or
/// [`ValueTag::BatchObject`] reports [`ObjectReferenceError::TypeMismatch`].
/// Batch ordering and uniqueness are validated by the owning opcode.
pub fn decode_object_reference(
    input: &[u8],
) -> Result<(ObjectReference, usize), ObjectReferenceError> {
    let (value, consumed) = decode_value(input)?;
    Ok((ObjectReference::try_from(value)?, consumed))
}

/// Encode a complete counted canonical value list.
pub fn encode_value_list(values: ValueList<'_>, output: &mut [u8]) -> Result<usize, CodecError> {
    validate_value_list_structure(values)?;
    let mut writer = Writer::new(output);
    encode_value_list_into(values, &mut writer)?;
    Ok(writer.position)
}

/// Encode a value list under one explicit count limit.
///
/// This count-only helper is useful to opcode tooling. Post-negotiation request
/// adapters use [`encode_value_list_with_limits`] to enforce value payload
/// bounds as well as `max_items_per_command`.
pub fn encode_value_list_with_limit(
    values: ValueList<'_>,
    maximum_values: u16,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_value_list_structure(values)?;
    validate_value_count(values, maximum_values)?;
    encode_value_list(values, output)
}

/// Encode a request argument list under active negotiated payload limits.
pub fn encode_value_list_with_limits(
    values: ValueList<'_>,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_value_list_structure(values)?;
    validate_value_count(values, limits.max_items_per_command)?;
    validate_value_list_payload_limits(values, limits)?;
    encode_value_list(values, output)
}

/// Decode and structurally validate one complete counted value list.
pub fn decode_value_list(input: &[u8]) -> Result<ValueList<'_>, CodecError> {
    decode_value_list_inner(input).map_err(nested_frame_error)
}

/// Decode a value list under one explicit count limit.
///
/// Post-negotiation request adapters use [`decode_value_list_with_limits`].
pub fn decode_value_list_with_limit(
    input: &[u8],
    maximum_values: u16,
) -> Result<ValueList<'_>, CodecError> {
    let values = decode_value_list(input)?;
    validate_value_count(values, maximum_values)?;
    Ok(values)
}

/// Decode a request argument list under active negotiated payload limits.
pub fn decode_value_list_with_limits(
    input: &[u8],
    limits: Limits,
) -> Result<ValueList<'_>, CodecError> {
    let values = decode_value_list(input)?;
    validate_value_count(values, limits.max_items_per_command)?;
    validate_value_list_payload_limits(values, limits)?;
    Ok(values)
}

fn decode_value_list_inner(input: &[u8]) -> Result<ValueList<'_>, CodecError> {
    let mut reader = Reader::new(input);
    let count = reader.u16()? as usize;
    let values_start = reader.position;
    for _ in 0..count {
        let _ = decode_value_from_reader(&mut reader)?;
    }
    if reader.position != input.len() {
        return Err(CodecError::InvalidFrame);
    }
    Ok(ValueList::from_wire(count, &input[values_start..]))
}

/// Encode a complete counted canonical typed field list.
pub fn encode_field_list(fields: FieldList<'_>, output: &mut [u8]) -> Result<usize, CodecError> {
    validate_field_list_structure(fields)?;
    let mut writer = Writer::new(output);
    encode_field_list_into(fields, &mut writer)?;
    Ok(writer.position)
}

/// Encode a typed field list under one explicit count limit.
///
/// Post-negotiation opcode adapters use [`encode_field_list_with_limits`] to
/// enforce value payload bounds as well as `max_items_per_command`.
pub fn encode_field_list_with_limit(
    fields: FieldList<'_>,
    maximum_items: u16,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_field_list_structure(fields)?;
    validate_field_count(fields, maximum_items)?;
    encode_field_list(fields, output)
}

/// Encode a typed field list under active negotiated payload limits.
pub fn encode_field_list_with_limits(
    fields: FieldList<'_>,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_field_list_structure(fields)?;
    validate_field_count(fields, limits.max_items_per_command)?;
    validate_field_list_payload_limits(fields, limits)?;
    encode_field_list(fields, output)
}

/// Decode and structurally validate one complete canonical typed field list.
pub fn decode_field_list(input: &[u8]) -> Result<FieldList<'_>, CodecError> {
    decode_field_list_inner(input).map_err(nested_frame_error)
}

/// Decode a typed field list under one explicit count limit.
///
/// Post-negotiation opcode adapters use [`decode_field_list_with_limits`].
pub fn decode_field_list_with_limit(
    input: &[u8],
    maximum_items: u16,
) -> Result<FieldList<'_>, CodecError> {
    let fields = decode_field_list(input)?;
    validate_field_count(fields, maximum_items)?;
    Ok(fields)
}

/// Decode a typed field list under active negotiated payload limits.
pub fn decode_field_list_with_limits(
    input: &[u8],
    limits: Limits,
) -> Result<FieldList<'_>, CodecError> {
    let fields = decode_field_list(input)?;
    validate_field_count(fields, limits.max_items_per_command)?;
    validate_field_list_payload_limits(fields, limits)?;
    Ok(fields)
}

fn decode_field_list_inner(input: &[u8]) -> Result<FieldList<'_>, CodecError> {
    let mut reader = Reader::new(input);
    let count = reader.u16()? as usize;
    let fields_start = reader.position;
    let mut previous = None;
    for _ in 0..count {
        let id = reader.u32()?;
        require_increasing_id(previous, id)?;
        previous = Some(id);
        let _ = decode_value_from_reader(&mut reader)?;
    }
    if reader.position != input.len() {
        return Err(CodecError::InvalidFrame);
    }
    Ok(FieldList::from_wire(count, &input[fields_start..]))
}

/// Encode one complete canonical Batch-only Create payload.
pub fn encode_create_payload(
    payload: CreatePayload<'_>,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_create_payload_structure(payload)?;
    let mut writer = Writer::new(output);
    encode_create_payload_into(payload, &mut writer)?;
    Ok(writer.position)
}

/// Encode a Create payload under active negotiated payload limits.
///
/// This enforces the root-name, constructor-field count, and contained
/// Text/Bytes limits. The caller still applies the complete-frame limit when it
/// wraps the payload in an [`OperationRef`] and [`Batch`].
pub fn encode_create_payload_with_limits(
    payload: CreatePayload<'_>,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_create_payload_structure(payload)?;
    validate_create_payload_limits(payload, limits)?;
    encode_create_payload(payload, output)
}

/// Decode one complete canonical Batch-only Create payload.
pub fn decode_create_payload(input: &[u8]) -> Result<CreatePayload<'_>, CreatePayloadError> {
    decode_create_payload_inner(input).map_err(nested_create_payload_error)
}

/// Decode a Create payload under active negotiated payload limits.
pub fn decode_create_payload_with_limits(
    input: &[u8],
    limits: Limits,
) -> Result<CreatePayload<'_>, CreatePayloadError> {
    let payload = decode_create_payload(input)?;
    validate_create_payload_limits(payload, limits)?;
    Ok(payload)
}

/// Decode a zero-flag Create operation from the counted Batch operation list.
///
/// There is deliberately no Command counterpart: MPY v1 Create is Batch-only.
pub fn decode_create_operation_with_limits(
    operation: OperationRef<'_>,
    limits: Limits,
) -> Result<CreatePayload<'_>, CreatePayloadError> {
    if operation.opcode != opcode::CREATE || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_create_payload_with_limits(operation.payload, limits)
}

/// Validate the exact one-Object output schema of one successful Create record.
///
/// The containing [`OperationResultRef::operation_index`] correlates this
/// stable object to the Create operation and therefore to its input BatchRef.
pub fn create_result_object(values: ValueList<'_>) -> Result<u64, CreatePayloadError> {
    validate_value_list_structure(values)?;
    if values.len() != 1 {
        return Err(CodecError::InvalidFrame.into());
    }
    match values.iter().next().expect("one value was validated") {
        ValueRef::Object(object_id) => Ok(object_id),
        value => Err(ObjectReferenceError::TypeMismatch {
            actual: value.tag(),
        }),
    }
}

/// Return whether an opcode uses the common MPY v1 mutation-target envelope.
pub const fn is_batch_mutation_opcode(value: u32) -> bool {
    matches!(
        value,
        opcode::SET_PROPERTIES
            | opcode::RESET_PROPERTIES
            | opcode::INVOKE_ACTION
            | opcode::SET_FLAG
            | opcode::SET_REQUESTED_LAYOUT
            | opcode::REPARENT
            | opcode::PROMOTE_ROOT
            | opcode::REORDER
            | opcode::DELETE
            | opcode::SET_LOCAL_STYLE
    )
}

/// Encode one common mutation target followed by its opaque opcode remainder.
///
/// No negotiated count or variable-value limit belongs to this prefix. The
/// opcode-owned remainder codec applies its own full [`Limits`], and the
/// enclosing frame codec applies `max_frame_bytes`.
pub fn encode_mutation_target_envelope(
    envelope: MutationTargetEnvelope<'_>,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_value_structure(envelope.target.as_value())?;
    let mut writer = Writer::new(output);
    encode_value_into(envelope.target.as_value(), &mut writer)?;
    writer.bytes(envelope.remainder)?;
    Ok(writer.position)
}

/// Decode one common mutation target and borrow all remaining opcode bytes.
pub fn decode_mutation_target_envelope(
    input: &[u8],
) -> Result<MutationTargetEnvelope<'_>, MutationTargetError> {
    let (target, consumed) =
        decode_object_reference(input).map_err(nested_mutation_target_error)?;
    Ok(MutationTargetEnvelope {
        target,
        remainder: &input[consumed..],
    })
}

/// Decode only the common target prefix of a registered Batch mutation.
///
/// Success validates the opcode, flags, and contextual target prefix. The
/// returned remainder is still opaque and MUST be validated by the selected
/// opcode's complete payload codec before request acceptance.
pub fn decode_mutation_operation_target(
    operation: OperationRef<'_>,
) -> Result<MutationTargetEnvelope<'_>, MutationTargetError> {
    require_mutation_operation(operation.opcode, operation.flags)?;
    decode_mutation_target_envelope(operation.payload)
}

/// Encode one complete Delete payload containing only its contextual target.
///
/// No negotiated variable-size limit applies to this fixed-size payload. The
/// enclosing Batch remains responsible for operation-count and frame limits.
pub fn encode_delete_payload(
    target: ObjectReference,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    encode_mutation_target_envelope(
        MutationTargetEnvelope {
            target,
            remainder: &[],
        },
        output,
    )
}

/// Decode one complete Delete payload and reject every trailing byte.
pub fn decode_delete_payload(input: &[u8]) -> Result<ObjectReference, DeletePayloadError> {
    let envelope = decode_mutation_target_envelope(input)?;
    if !envelope.remainder.is_empty() {
        return Err(CodecError::InvalidFrame.into());
    }
    Ok(envelope.target)
}

/// Decode one zero-flag Delete operation from a counted Batch operation list.
///
/// There is deliberately no Command counterpart: MPY v1 Delete is Batch-only.
pub fn decode_delete_operation(
    operation: OperationRef<'_>,
) -> Result<ObjectReference, DeletePayloadError> {
    if operation.opcode != opcode::DELETE || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_delete_payload(operation.payload)
}

/// Encode one complete Reorder payload.
///
/// No negotiated variable-size limit applies to this fixed-size payload. The
/// enclosing Batch remains responsible for operation-count and frame limits.
pub fn encode_reorder_payload(
    payload: ReorderPayload,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    let index = payload.index.to_le_bytes();
    encode_mutation_target_envelope(
        MutationTargetEnvelope {
            target: payload.target,
            remainder: &index,
        },
        output,
    )
}

/// Decode one complete Reorder payload and reject truncation or trailing bytes.
pub fn decode_reorder_payload(input: &[u8]) -> Result<ReorderPayload, ReorderPayloadError> {
    let envelope = decode_mutation_target_envelope(input)?;
    let index = <[u8; 4]>::try_from(envelope.remainder).map_err(|_| CodecError::InvalidFrame)?;
    Ok(ReorderPayload {
        target: envelope.target,
        index: u32::from_le_bytes(index),
    })
}

/// Decode one zero-flag Reorder operation from a counted Batch operation list.
///
/// There is deliberately no Command counterpart: MPY v1 Reorder is Batch-only.
pub fn decode_reorder_operation(
    operation: OperationRef<'_>,
) -> Result<ReorderPayload, ReorderPayloadError> {
    if operation.opcode != opcode::REORDER || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_reorder_payload(operation.payload)
}

/// Encode one complete Reparent payload.
///
/// No negotiated variable-size limit applies to this fixed-size payload. The
/// enclosing Batch remains responsible for operation-count and frame limits.
pub fn encode_reparent_payload(
    payload: ReparentPayload,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_value_structure(payload.target.as_value())?;
    validate_value_structure(payload.new_parent.as_value())?;
    let mut writer = Writer::new(output);
    encode_value_into(payload.target.as_value(), &mut writer)?;
    encode_value_into(payload.new_parent.as_value(), &mut writer)?;
    writer.u32(payload.index)?;
    Ok(writer.position)
}

/// Decode one complete Reparent payload in target, parent, index order.
///
/// Truncation or trailing bytes are rejected without changing contextual
/// target-before-parent error precedence.
pub fn decode_reparent_payload(input: &[u8]) -> Result<ReparentPayload, ReparentPayloadError> {
    let (target, target_consumed) =
        decode_object_reference(input).map_err(nested_mutation_target_error)?;
    let (new_parent, parent_consumed) =
        decode_object_reference(&input[target_consumed..]).map_err(nested_mutation_target_error)?;
    let index_start = target_consumed
        .checked_add(parent_consumed)
        .ok_or(CodecError::InvalidFrame)?;
    let index = <[u8; 4]>::try_from(&input[index_start..]).map_err(|_| CodecError::InvalidFrame)?;
    Ok(ReparentPayload {
        target,
        new_parent,
        index: u32::from_le_bytes(index),
    })
}

/// Decode one zero-flag Reparent operation from a counted Batch operation list.
///
/// There is deliberately no Command counterpart: MPY v1 Reparent is Batch-only.
pub fn decode_reparent_operation(
    operation: OperationRef<'_>,
) -> Result<ReparentPayload, ReparentPayloadError> {
    if operation.opcode != opcode::REPARENT || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_reparent_payload(operation.payload)
}

/// Encode one complete PromoteRoot payload without negotiated-limit checks.
///
/// This allocation-free structural helper is appropriate before negotiation
/// or inside a caller that separately enforces [`Limits`]. Protocol request
/// acceptance should use [`encode_promote_root_payload_with_limits`]. An empty
/// name is structurally canonical and is rejected later by Stage semantics.
pub fn encode_promote_root_payload(
    payload: PromoteRootPayload<'_>,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_promote_root_payload_structure(payload)?;
    let mut writer = Writer::new(output);
    encode_value_into(payload.target.as_value(), &mut writer)?;
    writer.bytes_with_length(payload.name.as_bytes())?;
    writer.u32(payload.index)?;
    Ok(writer.position)
}

/// Encode one complete PromoteRoot payload under negotiated text limits.
///
/// Structural validation precedes the `max_text_bytes` check. No other
/// negotiated value or item limit applies to this payload. The caller still
/// applies complete-frame `max_frame_bytes` when wrapping the payload in its
/// Operation and Batch envelopes.
pub fn encode_promote_root_payload_with_limits(
    payload: PromoteRootPayload<'_>,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_promote_root_payload_structure(payload)?;
    if payload.name.len() > limits.max_text_bytes as usize {
        return Err(CodecError::LimitExceeded);
    }
    encode_promote_root_payload(payload, output)
}

/// Decode one complete PromoteRoot payload without negotiated-limit checks.
///
/// The decoder classifies the contextual target first, then validates the
/// length-delimited UTF-8 name, raw index, and absence of trailing bytes. Use
/// [`decode_promote_root_payload_with_limits`] at the negotiated request
/// boundary. An empty name is structurally preserved for semantic rejection.
pub fn decode_promote_root_payload(
    input: &[u8],
) -> Result<PromoteRootPayload<'_>, PromoteRootPayloadError> {
    let (target, target_consumed) =
        decode_object_reference(input).map_err(nested_mutation_target_error)?;
    let mut reader = Reader::new(&input[target_consumed..]);
    let name = str::from_utf8(
        reader
            .bytes_with_length()
            .map_err(|error| nested_mutation_target_error(error.into()))?,
    )
    .map_err(|_| CodecError::InvalidFrame)?;
    let index = reader
        .u32()
        .map_err(|error| nested_mutation_target_error(error.into()))?;
    if reader.position != reader.input.len() {
        return Err(CodecError::InvalidFrame.into());
    }
    Ok(PromoteRootPayload {
        target,
        name,
        index,
    })
}

/// Decode one complete PromoteRoot payload under negotiated text limits.
///
/// Complete structural decoding precedes the `max_text_bytes` check.
pub fn decode_promote_root_payload_with_limits(
    input: &[u8],
    limits: Limits,
) -> Result<PromoteRootPayload<'_>, PromoteRootPayloadError> {
    let payload = decode_promote_root_payload(input)?;
    if payload.name.len() > limits.max_text_bytes as usize {
        return Err(CodecError::LimitExceeded.into());
    }
    Ok(payload)
}

/// Decode one zero-flag PromoteRoot operation without negotiated-limit checks.
///
/// There is deliberately no Command counterpart: MPY v1 PromoteRoot is
/// Batch-only. Request acceptance should use
/// [`decode_promote_root_operation_with_limits`].
pub fn decode_promote_root_operation(
    operation: OperationRef<'_>,
) -> Result<PromoteRootPayload<'_>, PromoteRootPayloadError> {
    if operation.opcode != opcode::PROMOTE_ROOT || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_promote_root_payload(operation.payload)
}

/// Decode one zero-flag PromoteRoot operation under negotiated text limits.
///
/// This is the acceptance-facing operation decoder after limits negotiation.
pub fn decode_promote_root_operation_with_limits(
    operation: OperationRef<'_>,
    limits: Limits,
) -> Result<PromoteRootPayload<'_>, PromoteRootPayloadError> {
    if operation.opcode != opcode::PROMOTE_ROOT || operation.flags != 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    decode_promote_root_payload_with_limits(operation.payload, limits)
}

/// Validate that one correlated Delete emitted no operation-result record.
///
/// The caller MUST already have correlated `delete_operation_index` to a
/// submitted opcode [`opcode::DELETE`]. This helper validates the structural
/// [`BatchSuccess`] shape and the absence of that one index only. It does not
/// validate other opcode result schemas, negotiated Limits, or the complete
/// success envelope. Other output-bearing operations may still contribute
/// records.
pub fn validate_delete_result_absent(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    delete_operation_index: u16,
) -> Result<(), CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    if delete_operation_index >= submitted_operation_count
        || success
            .results
            .iter()
            .any(|result| result.operation_index == delete_operation_index)
    {
        return Err(CodecError::InvalidFrame);
    }
    Ok(())
}

/// Validate that one correlated Reorder emitted no operation-result record.
///
/// The caller MUST already have correlated `reorder_operation_index` to a
/// submitted opcode [`opcode::REORDER`]. This helper validates the structural
/// [`BatchSuccess`] shape and the absence of that one index only. It does not
/// validate other opcode result schemas, negotiated Limits, or the complete
/// success envelope. Other output-bearing operations may still contribute
/// records.
pub fn validate_reorder_result_absent(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    reorder_operation_index: u16,
) -> Result<(), CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    if reorder_operation_index >= submitted_operation_count
        || success
            .results
            .iter()
            .any(|result| result.operation_index == reorder_operation_index)
    {
        return Err(CodecError::InvalidFrame);
    }
    Ok(())
}

/// Validate that one correlated Reparent emitted no operation-result record.
///
/// The caller MUST already have correlated `reparent_operation_index` to a
/// submitted opcode [`opcode::REPARENT`]. This helper validates the structural
/// [`BatchSuccess`] shape and the absence of that one index only. It does not
/// validate other opcode result schemas, negotiated Limits, or the complete
/// success envelope. Other output-bearing operations may still contribute
/// records.
pub fn validate_reparent_result_absent(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    reparent_operation_index: u16,
) -> Result<(), CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    if reparent_operation_index >= submitted_operation_count
        || success
            .results
            .iter()
            .any(|result| result.operation_index == reparent_operation_index)
    {
        return Err(CodecError::InvalidFrame);
    }
    Ok(())
}

/// Validate that one correlated PromoteRoot emitted no operation-result record.
///
/// The caller MUST already have correlated `promote_root_operation_index` to a
/// submitted opcode [`opcode::PROMOTE_ROOT`]. This helper validates the
/// structural [`BatchSuccess`] shape and the absence of that one index only. It
/// does not validate other opcode result schemas, negotiated Limits, or the
/// complete success envelope. Other output-bearing operations may still
/// contribute records.
pub fn validate_promote_root_result_absent(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    promote_root_operation_index: u16,
) -> Result<(), CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    if promote_root_operation_index >= submitted_operation_count
        || success
            .results
            .iter()
            .any(|result| result.operation_index == promote_root_operation_index)
    {
        return Err(CodecError::InvalidFrame);
    }
    Ok(())
}

fn validate_promote_root_payload_structure(
    payload: PromoteRootPayload<'_>,
) -> Result<(), CodecError> {
    validate_value_structure(payload.target.as_value())?;
    let _ = u32::try_from(payload.name.len()).map_err(|_| CodecError::InvalidFrame)?;
    Ok(())
}

fn decode_create_payload_inner(input: &[u8]) -> Result<CreatePayload<'_>, CreatePayloadError> {
    let mut reader = Reader::new(input);
    let batch_ref = reader.u16()?;
    if batch_ref == 0 {
        return Err(CodecError::InvalidFrame.into());
    }
    let type_id = reader.u32()?;
    require_nonzero(type_id)?;
    let destination = match CreateDestinationTag::decode(reader.u8()?)? {
        CreateDestinationTag::Root => {
            let name = str::from_utf8(reader.bytes_with_length()?)
                .map_err(|_| CodecError::InvalidFrame)?;
            CreateDestinationRef::Root(name)
        }
        CreateDestinationTag::Child => {
            let value = decode_value_from_reader(&mut reader)?;
            CreateDestinationRef::Child(ObjectReference::try_from(value)?)
        }
    };
    let constructor_fields = decode_field_list(&input[reader.position..])?;
    Ok(CreatePayload {
        batch_ref,
        type_id,
        destination,
        constructor_fields,
    })
}

/// Encode one canonical successful Batch payload.
///
/// `submitted_operation_count` correlates every output record to the submitted
/// operation list. Whether a correlated operation's opcode actually declares
/// output is an opcode-schema validation performed by the caller.
pub fn encode_batch_success(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    let mut writer = Writer::new(output);
    encode_batch_success_into(success, submitted_operation_count, &mut writer)?;
    Ok(writer.position)
}

/// Encode a successful Batch payload under one explicit aggregate value limit.
///
/// Post-negotiation endpoint adapters use [`encode_batch_success_with_limits`]
/// to enforce result value payload bounds as well.
pub fn encode_batch_success_with_limit(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    maximum_values: u16,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    validate_batch_success_value_count(success, maximum_values)?;
    encode_batch_success(success, submitted_operation_count, output)
}

/// Encode a successful Batch payload under active negotiated payload limits.
pub fn encode_batch_success_with_limits(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_batch_success_structure(success, submitted_operation_count)?;
    validate_batch_success_value_count(success, limits.max_values_per_result)?;
    validate_batch_success_payload_limits(success, limits)?;
    encode_batch_success(success, submitted_operation_count, output)
}

/// Decode and structurally validate one complete successful Batch payload.
///
/// Output records are correlated to `submitted_operation_count`. The caller
/// separately checks that each referenced opcode declares the decoded output
/// schema before publishing the Result.
pub fn decode_batch_success(
    input: &[u8],
    submitted_operation_count: u16,
) -> Result<BatchSuccess<'_>, CodecError> {
    decode_batch_success_inner(input, submitted_operation_count).map_err(nested_frame_error)
}

/// Decode a successful Batch payload under one explicit aggregate value limit.
///
/// Post-negotiation endpoint adapters use [`decode_batch_success_with_limits`].
pub fn decode_batch_success_with_limit(
    input: &[u8],
    submitted_operation_count: u16,
    maximum_values: u16,
) -> Result<BatchSuccess<'_>, CodecError> {
    let success = decode_batch_success(input, submitted_operation_count)?;
    validate_batch_success_value_count(success, maximum_values)?;
    Ok(success)
}

/// Decode a successful Batch payload under active negotiated payload limits.
pub fn decode_batch_success_with_limits(
    input: &[u8],
    submitted_operation_count: u16,
    limits: Limits,
) -> Result<BatchSuccess<'_>, CodecError> {
    let success = decode_batch_success(input, submitted_operation_count)?;
    validate_batch_success_value_count(success, limits.max_values_per_result)?;
    validate_batch_success_payload_limits(success, limits)?;
    Ok(success)
}

fn decode_batch_success_inner(
    input: &[u8],
    submitted_operation_count: u16,
) -> Result<BatchSuccess<'_>, CodecError> {
    let mut reader = Reader::new(input);
    let result_revision = reader.u64()?;
    let count = reader.u16()? as usize;
    let records_start = reader.position;
    let mut previous = None;
    for _ in 0..count {
        let operation_index = reader.u16()?;
        require_operation_result_index(previous, operation_index, submitted_operation_count)?;
        previous = Some(operation_index);
        let value_count = reader.u16()? as usize;
        if value_count == 0 {
            return Err(CodecError::InvalidFrame);
        }
        for _ in 0..value_count {
            let _ = decode_value_from_reader(&mut reader)?;
        }
    }
    if reader.position != input.len() {
        return Err(CodecError::InvalidFrame);
    }
    Ok(BatchSuccess {
        result_revision,
        results: OperationResultList::from_wire(count, &input[records_start..]),
    })
}

/// Encode a counted canonical Batch operation list into caller-provided storage.
///
/// This structural codec does not enforce a negotiated item limit. Use
/// [`encode_operation_list_with_limit`] after negotiation; direct use is for
/// pre-negotiation tooling, golden vectors, and other callers that enforce
/// capacity separately.
pub fn encode_operation_list(
    operations: OperationList<'_>,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    let mut writer = Writer::new(output);
    encode_operation_list_into(operations, &mut writer)?;
    Ok(writer.position)
}

/// Encode a Batch operation list under the active negotiated item limit.
pub fn encode_operation_list_with_limit(
    operations: OperationList<'_>,
    maximum_items: u16,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_operation_count(operations, maximum_items)?;
    encode_operation_list(operations, output)
}

/// Decode and structurally validate one complete canonical Batch operation list.
///
/// This structural codec does not enforce a negotiated item limit. Use
/// [`decode_operation_list_with_limit`] after negotiation; direct use is for
/// pre-negotiation tooling, golden vectors, and other callers that enforce
/// capacity separately.
pub fn decode_operation_list(input: &[u8]) -> Result<OperationList<'_>, CodecError> {
    decode_operation_list_inner(input).map_err(nested_frame_error)
}

/// Decode a Batch operation list under the active negotiated item limit.
pub fn decode_operation_list_with_limit(
    input: &[u8],
    maximum_items: u16,
) -> Result<OperationList<'_>, CodecError> {
    let operations = decode_operation_list(input)?;
    validate_operation_count(operations, maximum_items)?;
    Ok(operations)
}

fn decode_operation_list_inner(input: &[u8]) -> Result<OperationList<'_>, CodecError> {
    let mut reader = Reader::new(input);
    let count = reader.u16()? as usize;
    let records_start = reader.position;
    for _ in 0..count {
        require_opcode(reader.u32()?)?;
        require_operation_flags(reader.u32()?)?;
        let _ = reader.bytes_with_length()?;
    }
    if reader.position != input.len() {
        return Err(CodecError::InvalidFrame);
    }
    Ok(OperationList::from_wire(count, &input[records_start..]))
}

/// Encodes one complete canonical logical frame.
///
/// This structural codec does not enforce negotiated frame or item limits. Use
/// [`encode_frame_with_limits`] after negotiation; direct use is for Hello and
/// Capabilities exchange, golden vectors, and callers that enforce capacity
/// separately.
pub fn encode_frame(
    version: ProtocolVersion,
    frame: FrameRef<'_>,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    if version.major == 0 || output.len() < FRAME_HEADER_LEN {
        return Err(if output.len() < FRAME_HEADER_LEN {
            CodecError::BufferTooSmall
        } else {
            CodecError::InvalidFrame
        });
    }

    let kind = frame.kind();
    let (header, payload) = output.split_at_mut(FRAME_HEADER_LEN);
    let mut writer = Writer::new(payload);
    encode_frame_payload(frame, &mut writer)?;
    let payload_len = u32::try_from(writer.position).map_err(|_| CodecError::InvalidFrame)?;

    header[0] = kind as u8;
    header[1] = version.major;
    header[2] = version.minor;
    header[3] = version.patch;
    header[4..8].copy_from_slice(&payload_len.to_le_bytes());
    Ok(FRAME_HEADER_LEN + writer.position)
}

/// Encode a post-negotiation frame under its negotiated envelope limits.
///
/// Hello and Capabilities negotiation itself uses [`encode_frame`]. After
/// negotiation, endpoint adapters use this function so Batch operation count
/// and complete-frame size are rejected before transport submission. Opcode
/// payload codecs remain responsible for text, byte-payload, Command-field,
/// and Result-value limits.
pub fn encode_frame_with_limits(
    version: ProtocolVersion,
    frame: FrameRef<'_>,
    limits: Limits,
    output: &mut [u8],
) -> Result<usize, CodecError> {
    validate_frame_item_limit(frame, limits.max_items_per_command)?;
    let length = encode_frame(version, frame, output)?;
    if length > limits.max_frame_bytes as usize {
        return Err(CodecError::LimitExceeded);
    }
    Ok(length)
}

/// Decodes one complete canonical logical frame.
///
/// This structural codec does not enforce negotiated frame or item limits. Use
/// [`decode_frame_with_limits`] after negotiation; direct use is for Hello and
/// Capabilities exchange, golden vectors, and callers that enforce capacity
/// separately.
pub fn decode_frame(input: &[u8]) -> Result<DecodedFrame<'_>, CodecError> {
    if input.len() < FRAME_HEADER_LEN {
        return Err(CodecError::Truncated);
    }
    let kind = FrameKind::decode(input[0])?;
    let version = ProtocolVersion {
        major: input[1],
        minor: input[2],
        patch: input[3],
    };
    if version.major == 0 {
        return Err(CodecError::InvalidFrame);
    }
    let payload_len = u32::from_le_bytes([input[4], input[5], input[6], input[7]]) as usize;
    let expected = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(CodecError::InvalidFrame)?;
    if input.len() < expected {
        return Err(CodecError::Truncated);
    }
    if input.len() != expected {
        return Err(CodecError::InvalidFrame);
    }
    let mut reader = Reader::new(&input[FRAME_HEADER_LEN..]);
    let frame = decode_frame_payload(kind, &mut reader)?;
    if reader.position != payload_len {
        return Err(CodecError::InvalidFrame);
    }
    Ok(DecodedFrame { version, frame })
}

/// Decode a post-negotiation frame under its negotiated envelope limits.
///
/// Hello and Capabilities negotiation itself uses [`decode_frame`]. After
/// negotiation, endpoint adapters use this function to enforce complete-frame
/// size and Batch operation count before accepting work. Opcode payload codecs
/// remain responsible for text, byte-payload, Command-field, and Result-value
/// limits.
pub fn decode_frame_with_limits(
    input: &[u8],
    limits: Limits,
) -> Result<DecodedFrame<'_>, CodecError> {
    if input.len() > limits.max_frame_bytes as usize {
        return Err(CodecError::LimitExceeded);
    }
    let decoded = decode_frame(input)?;
    validate_frame_item_limit(decoded.frame, limits.max_items_per_command)?;
    Ok(decoded)
}

fn encode_frame_payload(frame: FrameRef<'_>, writer: &mut Writer<'_>) -> Result<(), CodecError> {
    match frame {
        FrameRef::Hello(frame) => {
            writer.version(frame.schema_version)?;
            writer.limits(frame.limits)?;
            writer.u64(frame.features)?;
        }
        FrameRef::Capabilities(frame) => {
            writer.version(frame.schema_version)?;
            writer.limits(frame.limits)?;
            writer.u64(frame.features)?;
            writer.u32(frame.value_tags)?;
            let count = u16::try_from(frame.opcodes.len()).map_err(|_| CodecError::InvalidFrame)?;
            writer.u16(count)?;
            for opcode in frame.opcodes.iter() {
                require_opcode(opcode)?;
                writer.u32(opcode)?;
            }
        }
        FrameRef::Command(frame) => {
            require_stage_request(frame.stage_id, frame.request_id)?;
            require_opcode(frame.opcode)?;
            writer.u32(frame.stage_id)?;
            writer.u32(frame.request_id)?;
            writer.u32(frame.opcode)?;
            writer.u32(frame.flags)?;
            writer.bytes_with_length(frame.payload)?;
        }
        FrameRef::Batch(frame) => {
            require_stage_request(frame.stage_id, frame.request_id)?;
            writer.u32(frame.stage_id)?;
            writer.u32(frame.request_id)?;
            writer.u32(frame.flags)?;
            writer.u16(frame.budget.actors)?;
            writer.u32(frame.budget.text_bytes)?;
            writer.u16(frame.budget.resources)?;
            writer.u32(frame.budget.result_bytes)?;
            let encoded_length = encoded_operation_list_length(frame.operations)?;
            writer.u32(encoded_length)?;
            encode_operation_list_into(frame.operations, writer)?;
        }
        FrameRef::Result(frame) => {
            require_nonzero(frame.request_id)?;
            if let Some(field_id) = frame.field_id {
                require_nonzero(field_id)?;
            }
            if frame.status == CompletionStatus::Success
                && (frame.operation_index.is_some()
                    || frame.field_id.is_some()
                    || !frame.diagnostic.is_empty())
            {
                return Err(CodecError::InvalidFrame);
            }
            writer.u32(frame.request_id)?;
            writer.u8(match frame.status {
                CompletionStatus::Success => 0,
                CompletionStatus::Error(error) => error as u8,
            })?;
            writer.optional_u16(frame.operation_index)?;
            writer.optional_u32(frame.field_id)?;
            writer.bytes_with_length(frame.diagnostic.as_bytes())?;
            writer.bytes_with_length(frame.payload)?;
        }
        FrameRef::Cue(frame) => {
            require_nonzero(frame.stage_id)?;
            require_object_id(frame.object_id)?;
            require_nonzero(frame.subscription_id)?;
            require_nonzero(frame.callback_id)?;
            require_nonzero(frame.event_id)?;
            writer.u32(frame.sequence)?;
            writer.u32(frame.stage_id)?;
            writer.u64(frame.object_id)?;
            writer.u32(frame.subscription_id)?;
            writer.u32(frame.callback_id)?;
            writer.u32(frame.event_id)?;
            writer.u32(frame.flags)?;
            writer.bytes_with_length(frame.payload)?;
        }
        FrameRef::RuntimeNotice(frame) => {
            require_nonzero(frame.kind)?;
            writer.u32(frame.sequence)?;
            writer.u32(frame.kind)?;
            writer.bytes_with_length(frame.diagnostic.as_bytes())?;
            writer.bytes_with_length(frame.payload)?;
        }
    }
    Ok(())
}

fn decode_frame_payload<'a>(
    kind: FrameKind,
    reader: &mut Reader<'a>,
) -> Result<FrameRef<'a>, CodecError> {
    Ok(match kind {
        FrameKind::Hello => FrameRef::Hello(Hello {
            schema_version: reader.version()?,
            limits: reader.limits()?,
            features: reader.u64()?,
        }),
        FrameKind::Capabilities => {
            let schema_version = reader.version()?;
            let limits = reader.limits()?;
            let features = reader.u64()?;
            let value_tags = reader.u32()?;
            let opcode_count = reader.u16()? as usize;
            let opcode_bytes = opcode_count
                .checked_mul(4)
                .ok_or(CodecError::InvalidFrame)?;
            let opcodes = OpcodeList::from_wire(reader.bytes(opcode_bytes)?);
            for opcode in opcodes.iter() {
                require_opcode(opcode)?;
            }
            FrameRef::Capabilities(Capabilities {
                schema_version,
                limits,
                features,
                value_tags,
                opcodes,
            })
        }
        FrameKind::Command => {
            let stage_id = reader.u32()?;
            let request_id = reader.u32()?;
            require_stage_request(stage_id, request_id)?;
            let opcode = reader.u32()?;
            require_opcode(opcode)?;
            FrameRef::Command(Command {
                stage_id,
                request_id,
                opcode,
                flags: reader.u32()?,
                payload: reader.bytes_with_length()?,
            })
        }
        FrameKind::Batch => {
            let stage_id = reader.u32()?;
            let request_id = reader.u32()?;
            require_stage_request(stage_id, request_id)?;
            let flags = reader.u32()?;
            let budget = BatchBudget {
                actors: reader.u16()?,
                text_bytes: reader.u32()?,
                resources: reader.u16()?,
                result_bytes: reader.u32()?,
            };
            let operation_bytes = reader.bytes_with_length().map_err(nested_frame_error)?;
            let operations = decode_operation_list(operation_bytes)?;
            FrameRef::Batch(Batch {
                stage_id,
                request_id,
                flags,
                budget,
                operations,
            })
        }
        FrameKind::Result => {
            let request_id = reader.u32()?;
            require_nonzero(request_id)?;
            let status = match reader.u8()? {
                0 => CompletionStatus::Success,
                value => CompletionStatus::Error(ErrorClass::decode(value)?),
            };
            let operation_index = reader.optional_u16()?;
            let field_id = reader.optional_u32()?;
            if let Some(field_id) = field_id {
                require_nonzero(field_id)?;
            }
            let diagnostic = str::from_utf8(reader.bytes_with_length()?)
                .map_err(|_| CodecError::InvalidFrame)?;
            let payload = reader.bytes_with_length()?;
            if status == CompletionStatus::Success
                && (operation_index.is_some() || field_id.is_some() || !diagnostic.is_empty())
            {
                return Err(CodecError::InvalidFrame);
            }
            FrameRef::Result(Completion {
                request_id,
                status,
                operation_index,
                field_id,
                diagnostic,
                payload,
            })
        }
        FrameKind::Cue => {
            let frame = Cue {
                sequence: reader.u32()?,
                stage_id: reader.u32()?,
                object_id: reader.u64()?,
                subscription_id: reader.u32()?,
                callback_id: reader.u32()?,
                event_id: reader.u32()?,
                flags: reader.u32()?,
                payload: reader.bytes_with_length()?,
            };
            require_nonzero(frame.stage_id)?;
            require_object_id(frame.object_id)?;
            require_nonzero(frame.subscription_id)?;
            require_nonzero(frame.callback_id)?;
            require_nonzero(frame.event_id)?;
            FrameRef::Cue(frame)
        }
        FrameKind::RuntimeNotice => {
            let frame = RuntimeNotice {
                sequence: reader.u32()?,
                kind: reader.u32()?,
                diagnostic: str::from_utf8(reader.bytes_with_length()?)
                    .map_err(|_| CodecError::InvalidFrame)?,
                payload: reader.bytes_with_length()?,
            };
            require_nonzero(frame.kind)?;
            FrameRef::RuntimeNotice(frame)
        }
    })
}

fn tagged_i32(writer: &mut Writer<'_>, tag: ValueTag, value: i32) -> Result<(), CodecError> {
    writer.u8(tag as u8)?;
    writer.i32(value)
}

fn tagged_u32(writer: &mut Writer<'_>, tag: ValueTag, value: u32) -> Result<(), CodecError> {
    writer.u8(tag as u8)?;
    writer.u32(value)
}

fn encode_value_list_into(
    values: ValueList<'_>,
    writer: &mut Writer<'_>,
) -> Result<(), CodecError> {
    let count = u16::try_from(values.len()).map_err(|_| CodecError::InvalidFrame)?;
    writer.u16(count)?;
    for value in values.iter() {
        encode_value_into(value, writer)?;
    }
    Ok(())
}

fn encode_field_list_into(
    fields: FieldList<'_>,
    writer: &mut Writer<'_>,
) -> Result<(), CodecError> {
    let count = u16::try_from(fields.len()).map_err(|_| CodecError::InvalidFrame)?;
    writer.u16(count)?;
    let mut previous = None;
    for field in fields.iter() {
        require_increasing_id(previous, field.id)?;
        previous = Some(field.id);
        writer.u32(field.id)?;
        encode_value_into(field.value, writer)?;
    }
    Ok(())
}

fn encode_create_payload_into(
    payload: CreatePayload<'_>,
    writer: &mut Writer<'_>,
) -> Result<(), CodecError> {
    writer.u16(payload.batch_ref)?;
    writer.u32(payload.type_id)?;
    match payload.destination {
        CreateDestinationRef::Root(name) => {
            writer.u8(CreateDestinationTag::Root as u8)?;
            writer.bytes_with_length(name.as_bytes())?;
        }
        CreateDestinationRef::Child(parent) => {
            writer.u8(CreateDestinationTag::Child as u8)?;
            encode_value_into(parent.as_value(), writer)?;
        }
    }
    encode_field_list_into(payload.constructor_fields, writer)
}

fn encode_batch_success_into(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
    writer: &mut Writer<'_>,
) -> Result<(), CodecError> {
    let count = u16::try_from(success.results.len()).map_err(|_| CodecError::InvalidFrame)?;
    writer.u64(success.result_revision)?;
    writer.u16(count)?;
    let mut previous = None;
    for result in success.results.iter() {
        require_operation_result_index(
            previous,
            result.operation_index,
            submitted_operation_count,
        )?;
        previous = Some(result.operation_index);
        let value_count =
            u16::try_from(result.values.len()).map_err(|_| CodecError::InvalidFrame)?;
        if value_count == 0 {
            return Err(CodecError::InvalidFrame);
        }
        writer.u16(result.operation_index)?;
        writer.u16(value_count)?;
        for value in result.values.iter() {
            encode_value_into(value, writer)?;
        }
    }
    Ok(())
}

fn require_increasing_id(previous: Option<u32>, id: u32) -> Result<(), CodecError> {
    if id == 0 || previous.is_some_and(|previous| id <= previous) {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

fn require_operation_result_index(
    previous: Option<u16>,
    operation_index: u16,
    submitted_operation_count: u16,
) -> Result<(), CodecError> {
    if operation_index >= submitted_operation_count
        || previous.is_some_and(|previous| operation_index <= previous)
    {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

fn validate_value_structure(value: ValueRef<'_>) -> Result<(), CodecError> {
    match value {
        ValueRef::Enum { domain, .. } => require_nonzero(domain),
        ValueRef::Text(value) => {
            let _ = u32::try_from(value.len()).map_err(|_| CodecError::InvalidFrame)?;
            Ok(())
        }
        ValueRef::Bytes(value) => {
            let _ = u32::try_from(value.len()).map_err(|_| CodecError::InvalidFrame)?;
            Ok(())
        }
        ValueRef::Object(value) => require_object_id(value),
        ValueRef::Resource { kind, id } => {
            require_nonzero(kind)?;
            require_nonzero_u64(id)
        }
        ValueRef::BatchObject(0) => Err(CodecError::InvalidFrame),
        ValueRef::None
        | ValueRef::Bool(_)
        | ValueRef::I32(_)
        | ValueRef::U32(_)
        | ValueRef::I64(_)
        | ValueRef::U64(_)
        | ValueRef::Precise(_)
        | ValueRef::Color(_)
        | ValueRef::Point { .. }
        | ValueRef::Size { .. }
        | ValueRef::Rect { .. }
        | ValueRef::BatchObject(_) => Ok(()),
    }
}

fn validate_value_list_structure(values: ValueList<'_>) -> Result<(), CodecError> {
    let _ = u16::try_from(values.len()).map_err(|_| CodecError::InvalidFrame)?;
    for value in values.iter() {
        validate_value_structure(value)?;
    }
    Ok(())
}

fn validate_field_list_structure(fields: FieldList<'_>) -> Result<(), CodecError> {
    let _ = u16::try_from(fields.len()).map_err(|_| CodecError::InvalidFrame)?;
    let mut previous = None;
    for field in fields.iter() {
        require_increasing_id(previous, field.id)?;
        previous = Some(field.id);
        validate_value_structure(field.value)?;
    }
    Ok(())
}

fn validate_create_payload_structure(payload: CreatePayload<'_>) -> Result<(), CodecError> {
    if payload.batch_ref == 0 {
        return Err(CodecError::InvalidFrame);
    }
    require_nonzero(payload.type_id)?;
    match payload.destination {
        CreateDestinationRef::Root(name) => {
            let _ = u32::try_from(name.len()).map_err(|_| CodecError::InvalidFrame)?;
        }
        CreateDestinationRef::Child(parent) => validate_value_structure(parent.as_value())?,
    }
    validate_field_list_structure(payload.constructor_fields)
}

fn validate_create_payload_limits(
    payload: CreatePayload<'_>,
    limits: Limits,
) -> Result<(), CodecError> {
    match payload.destination {
        CreateDestinationRef::Root(name) if name.len() > limits.max_text_bytes as usize => {
            return Err(CodecError::LimitExceeded);
        }
        CreateDestinationRef::Root(_) | CreateDestinationRef::Child(_) => {}
    }
    validate_field_count(payload.constructor_fields, limits.max_items_per_command)?;
    validate_field_list_payload_limits(payload.constructor_fields, limits)
}

fn validate_value_payload_limits(value: ValueRef<'_>, limits: Limits) -> Result<(), CodecError> {
    match value {
        ValueRef::Text(value) if value.len() > limits.max_text_bytes as usize => {
            Err(CodecError::LimitExceeded)
        }
        ValueRef::Bytes(value) if value.len() > limits.max_byte_payload as usize => {
            Err(CodecError::LimitExceeded)
        }
        _ => Ok(()),
    }
}

fn validate_value_list_payload_limits(
    values: ValueList<'_>,
    limits: Limits,
) -> Result<(), CodecError> {
    for value in values.iter() {
        validate_value_payload_limits(value, limits)?;
    }
    Ok(())
}

fn validate_field_list_payload_limits(
    fields: FieldList<'_>,
    limits: Limits,
) -> Result<(), CodecError> {
    for field in fields.iter() {
        validate_value_payload_limits(field.value, limits)?;
    }
    Ok(())
}

fn validate_batch_success_structure(
    success: BatchSuccess<'_>,
    submitted_operation_count: u16,
) -> Result<(), CodecError> {
    let _ = u16::try_from(success.results.len()).map_err(|_| CodecError::InvalidFrame)?;
    let mut previous = None;
    for result in success.results.iter() {
        require_operation_result_index(
            previous,
            result.operation_index,
            submitted_operation_count,
        )?;
        previous = Some(result.operation_index);
        if result.values.is_empty() {
            return Err(CodecError::InvalidFrame);
        }
        validate_value_list_structure(result.values)?;
    }
    Ok(())
}

fn validate_batch_success_payload_limits(
    success: BatchSuccess<'_>,
    limits: Limits,
) -> Result<(), CodecError> {
    for result in success.results.iter() {
        validate_value_list_payload_limits(result.values, limits)?;
    }
    Ok(())
}

fn validate_value_count(values: ValueList<'_>, maximum_values: u16) -> Result<(), CodecError> {
    if values.len() > maximum_values as usize {
        Err(CodecError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn validate_field_count(fields: FieldList<'_>, maximum_items: u16) -> Result<(), CodecError> {
    if fields.len() > maximum_items as usize {
        Err(CodecError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn validate_batch_success_value_count(
    success: BatchSuccess<'_>,
    maximum_values: u16,
) -> Result<(), CodecError> {
    let mut total = 0usize;
    for result in success.results.iter() {
        total = total
            .checked_add(result.values.len())
            .ok_or(CodecError::InvalidFrame)?;
        if total > maximum_values as usize {
            return Err(CodecError::LimitExceeded);
        }
    }
    Ok(())
}

fn encoded_operation_list_length(operations: OperationList<'_>) -> Result<u32, CodecError> {
    let mut length = 2usize;
    let _ = u16::try_from(operations.len()).map_err(|_| CodecError::InvalidFrame)?;
    for operation in operations.iter() {
        require_opcode(operation.opcode)?;
        require_operation_flags(operation.flags)?;
        let _ = u32::try_from(operation.payload.len()).map_err(|_| CodecError::InvalidFrame)?;
        length = length
            .checked_add(12)
            .and_then(|value| value.checked_add(operation.payload.len()))
            .ok_or(CodecError::InvalidFrame)?;
    }
    u32::try_from(length).map_err(|_| CodecError::InvalidFrame)
}

fn encode_operation_list_into(
    operations: OperationList<'_>,
    writer: &mut Writer<'_>,
) -> Result<(), CodecError> {
    let count = u16::try_from(operations.len()).map_err(|_| CodecError::InvalidFrame)?;
    writer.u16(count)?;
    for operation in operations.iter() {
        require_opcode(operation.opcode)?;
        require_operation_flags(operation.flags)?;
        writer.u32(operation.opcode)?;
        writer.u32(operation.flags)?;
        writer.bytes_with_length(operation.payload)?;
    }
    Ok(())
}

fn require_opcode(value: u32) -> Result<(), CodecError> {
    if value == opcode::INVALID || value == opcode::RESERVED {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

fn require_operation_flags(value: u32) -> Result<(), CodecError> {
    if value == 0 {
        Ok(())
    } else {
        Err(CodecError::InvalidFrame)
    }
}

fn validate_operation_count(
    operations: OperationList<'_>,
    maximum_items: u16,
) -> Result<(), CodecError> {
    if operations.len() > maximum_items as usize {
        Err(CodecError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn validate_frame_item_limit(frame: FrameRef<'_>, maximum_items: u16) -> Result<(), CodecError> {
    match frame {
        FrameRef::Batch(batch) => validate_operation_count(batch.operations, maximum_items),
        FrameRef::Hello(_)
        | FrameRef::Capabilities(_)
        | FrameRef::Command(_)
        | FrameRef::Result(_)
        | FrameRef::Cue(_)
        | FrameRef::RuntimeNotice(_) => Ok(()),
    }
}

fn nested_frame_error(error: CodecError) -> CodecError {
    match error {
        CodecError::Truncated => CodecError::InvalidFrame,
        other => other,
    }
}

fn nested_create_payload_error(error: CreatePayloadError) -> CreatePayloadError {
    match error {
        ObjectReferenceError::Codec(CodecError::Truncated) => {
            ObjectReferenceError::Codec(CodecError::InvalidFrame)
        }
        other => other,
    }
}

fn nested_mutation_target_error(error: MutationTargetError) -> MutationTargetError {
    match error {
        ObjectReferenceError::Codec(CodecError::Truncated) => {
            ObjectReferenceError::Codec(CodecError::InvalidFrame)
        }
        other => other,
    }
}

fn require_mutation_operation(value: u32, flags: u32) -> Result<(), CodecError> {
    if is_batch_mutation_opcode(value) && flags == 0 {
        Ok(())
    } else {
        Err(CodecError::InvalidFrame)
    }
}

fn require_stage_request(stage_id: u32, request_id: u32) -> Result<(), CodecError> {
    require_nonzero(stage_id)?;
    require_nonzero(request_id)
}

fn require_nonzero(value: u32) -> Result<(), CodecError> {
    if value == 0 {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

fn require_nonzero_u64(value: u64) -> Result<(), CodecError> {
    if value == 0 {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

fn require_object_id(value: u64) -> Result<(), CodecError> {
    if value as u32 == 0 || (value >> 32) as u32 == 0 {
        Err(CodecError::InvalidFrame)
    } else {
        Ok(())
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self {
            output,
            position: 0,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), CodecError> {
        let end = self
            .position
            .checked_add(value.len())
            .ok_or(CodecError::BufferTooSmall)?;
        let target = self
            .output
            .get_mut(self.position..end)
            .ok_or(CodecError::BufferTooSmall)?;
        target.copy_from_slice(value);
        self.position = end;
        Ok(())
    }

    fn bytes_with_length(&mut self, value: &[u8]) -> Result<(), CodecError> {
        let length = u32::try_from(value.len()).map_err(|_| CodecError::InvalidFrame)?;
        self.u32(length)?;
        self.bytes(value)
    }

    fn u8(&mut self, value: u8) -> Result<(), CodecError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), CodecError> {
        self.bytes(&value.to_le_bytes())
    }

    fn version(&mut self, value: ProtocolVersion) -> Result<(), CodecError> {
        self.bytes(&[value.major, value.minor, value.patch])
    }

    fn limits(&mut self, value: Limits) -> Result<(), CodecError> {
        self.u32(value.max_frame_bytes)?;
        self.u32(value.max_text_bytes)?;
        self.u32(value.max_byte_payload)?;
        self.u16(value.max_items_per_command)?;
        self.u16(value.max_values_per_result)
    }

    fn optional_u16(&mut self, value: Option<u16>) -> Result<(), CodecError> {
        match value {
            Some(value) => {
                self.u8(1)?;
                self.u16(value)
            }
            None => self.u8(0),
        }
    }

    fn optional_u32(&mut self, value: Option<u32>) -> Result<(), CodecError> {
        match value {
            Some(value) => {
                self.u8(1)?;
                self.u32(value)
            }
            None => self.u8(0),
        }
    }
}

struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn bytes(&mut self, length: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(CodecError::InvalidFrame)?;
        let value = self
            .input
            .get(self.position..end)
            .ok_or(CodecError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn bytes_with_length(&mut self) -> Result<&'a [u8], CodecError> {
        let length = self.u32()? as usize;
        self.bytes(length)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        let value = self.bytes(2)?;
        Ok(u16::from_le_bytes([value[0], value[1]]))
    }

    fn i32(&mut self) -> Result<i32, CodecError> {
        let value = self.bytes(4)?;
        Ok(i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn u32(&mut self) -> Result<u32, CodecError> {
        let value = self.bytes(4)?;
        Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    }

    fn i64(&mut self) -> Result<i64, CodecError> {
        let value = self.bytes(8)?;
        Ok(i64::from_le_bytes([
            value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
        ]))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        let value = self.bytes(8)?;
        Ok(u64::from_le_bytes([
            value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
        ]))
    }

    fn version(&mut self) -> Result<ProtocolVersion, CodecError> {
        Ok(ProtocolVersion {
            major: self.u8()?,
            minor: self.u8()?,
            patch: self.u8()?,
        })
    }

    fn limits(&mut self) -> Result<Limits, CodecError> {
        Ok(Limits {
            max_frame_bytes: self.u32()?,
            max_text_bytes: self.u32()?,
            max_byte_payload: self.u32()?,
            max_items_per_command: self.u16()?,
            max_values_per_result: self.u16()?,
        })
    }

    fn optional_u16(&mut self) -> Result<Option<u16>, CodecError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u16()?)),
            _ => Err(CodecError::InvalidFrame),
        }
    }

    fn optional_u32(&mut self) -> Result<Option<u32>, CodecError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u32()?)),
            _ => Err(CodecError::InvalidFrame),
        }
    }
}
