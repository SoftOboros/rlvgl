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
    /// Maximum typed fields in one command.
    pub max_fields_per_command: u16,
    /// Maximum typed values in one result.
    pub max_values_per_result: u16,
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
    /// Ordered operation records, encoded by their registered opcode schemas.
    pub operations: &'a [u8],
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
    match value {
        ValueRef::None => writer.u8(ValueTag::None as u8)?,
        ValueRef::Bool(value) => {
            writer.u8(ValueTag::Bool as u8)?;
            writer.u8(u8::from(value))?;
        }
        ValueRef::I32(value) => tagged_i32(&mut writer, ValueTag::I32, value)?,
        ValueRef::U32(value) => tagged_u32(&mut writer, ValueTag::U32, value)?,
        ValueRef::I64(value) => {
            writer.u8(ValueTag::I64 as u8)?;
            writer.i64(value)?;
        }
        ValueRef::U64(value) => {
            writer.u8(ValueTag::U64 as u8)?;
            writer.u64(value)?;
        }
        ValueRef::Precise(value) => tagged_i32(&mut writer, ValueTag::Precise, value)?,
        ValueRef::Color(value) => tagged_u32(&mut writer, ValueTag::Color, value)?,
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
    Ok(writer.position)
}

/// Decodes one tagged value and returns the number of consumed bytes.
pub fn decode_value(input: &[u8]) -> Result<(ValueRef<'_>, usize), CodecError> {
    let mut reader = Reader::new(input);
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
    Ok((value, reader.position))
}

/// Encodes one complete canonical logical frame.
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

/// Decodes one complete canonical logical frame.
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
                writer.u32(opcode)?;
            }
        }
        FrameRef::Command(frame) => {
            require_stage_request(frame.stage_id, frame.request_id)?;
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
            writer.bytes_with_length(frame.operations)?;
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
            FrameRef::Command(Command {
                stage_id,
                request_id,
                opcode: reader.u32()?,
                flags: reader.u32()?,
                payload: reader.bytes_with_length()?,
            })
        }
        FrameKind::Batch => {
            let stage_id = reader.u32()?;
            let request_id = reader.u32()?;
            require_stage_request(stage_id, request_id)?;
            FrameRef::Batch(Batch {
                stage_id,
                request_id,
                flags: reader.u32()?,
                budget: BatchBudget {
                    actors: reader.u16()?,
                    text_bytes: reader.u32()?,
                    resources: reader.u16()?,
                    result_bytes: reader.u32()?,
                },
                operations: reader.bytes_with_length()?,
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
        self.u16(value.max_fields_per_command)?;
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
            max_fields_per_command: self.u16()?,
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
