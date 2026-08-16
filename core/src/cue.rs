//! MPY endpoint-owned cue queue and bounded delivery substrate.
//!
//! The queue is deliberately independent of any language runtime. Native event
//! adapters enqueue owned cue records, while a binding drains them later from a
//! VM-safe context. One queue and one sequence allocator serve every Stage so
//! Stage-specific admission limits never reorder delivery.
//!
//! Subscription installation, Safe Turn orchestration, and Stage-teardown
//! cleanup remain integration responsibilities of the MPY-05 runtime layer;
//! this module supplies the bounded endpoint queue those operations use.

use alloc::{collections::VecDeque, vec::Vec};

use rlvgl_api::protocol::{self, FrameRef};

use crate::{
    actor::{ObjectId, StageId},
    direction::StageRevision,
};

/// Bytes occupied by the fixed fields in the MPY-02 canonical Cue frame.
pub const MPY_V1_CUE_FRAME_FIXED_BYTES: usize = 44;

/// Bytes in the fixed MPY-05 cue-payload metadata envelope.
pub const CUE_METADATA_ENVELOPE_BYTES: usize = 36;

/// Fixed bytes in one complete MPY-05 Cue frame before event payload.
///
/// The total includes the MPY-02 logical-frame and Cue fields plus the MPY-05
/// payload metadata envelope used to preserve revision and sequence causality.
pub const CUE_FRAME_OVERHEAD_BYTES: usize =
    MPY_V1_CUE_FRAME_FIXED_BYTES + CUE_METADATA_ENVELOPE_BYTES;

/// Cue flag indicating that the payload begins with MPY-05 metadata.
pub const CUE_FLAG_MPY05_METADATA: u32 = 1 << 0;

/// Cue flag indicating latest-value coalescing represented multiple emissions.
pub const CUE_FLAG_LATEST_VALUE_MERGED: u32 = 1 << 1;

/// Cue flag indicating that MPY-06 must release, rather than invoke, the
/// addressed subscription callback token.
pub const CUE_FLAG_SUBSCRIPTION_RELEASE: u32 = 1 << 2;

/// Registered RuntimeNotice kind for a raw input rejected before dispatch.
pub const RUNTIME_NOTICE_INPUT_OVERFLOW: u32 = 1;

/// Fixed typed metadata bytes preceding an InputOverflow notice payload.
pub const INPUT_OVERFLOW_METADATA_BYTES: usize = 28;

/// Fixed canonical RuntimeNotice frame bytes before its typed payload.
pub const RUNTIME_NOTICE_FRAME_FIXED_BYTES: usize = 24;

/// Opaque callback token owned by the language adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CallbackId(u32);

impl CallbackId {
    /// Construct a callback identifier, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Runtime-owned subscription identifier within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SubscriptionId(u32);

impl SubscriptionId {
    /// Construct a subscription identifier, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Stable event identifier from the actor descriptor catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EventId(u32);

impl EventId {
    /// Construct an event identifier, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Endpoint-wide monotonic cue sequence within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CueSequence(u32);

impl CueSequence {
    /// Construct a CueSequence, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized `u32` representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Monotonic native event traversal sequence within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct NativeEventSequence(u64);

impl NativeEventSequence {
    /// Construct a native event sequence, rejecting the reserved zero value.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the runtime representation.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Endpoint-wide raw-input admission sequence within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InputSequence(u64);

impl InputSequence {
    /// Construct an InputSequence, rejecting the reserved zero value.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the runtime representation.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable nonzero input-class identifier carried by overflow notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InputClass(u32);

impl InputClass {
    /// Construct an input-class identifier, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Canonical MPY-05 causality and coalescing metadata carried in every cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CuePayloadMetadata {
    /// Committed Stage Revision visible when the represented events occurred.
    pub stage_revision: StageRevision,
    /// First native event traversal represented by this cue.
    pub first_native_event_sequence: NativeEventSequence,
    /// Last native event traversal represented by this cue.
    pub last_native_event_sequence: NativeEventSequence,
    /// First endpoint cue sequence represented by this cue.
    pub first_cue_sequence: CueSequence,
    /// Last endpoint cue sequence represented by this cue.
    pub last_cue_sequence: CueSequence,
    /// Number of later latest-value emissions merged into the retained record.
    pub merge_count: u32,
}

impl CuePayloadMetadata {
    /// Encode the fixed metadata envelope into caller-owned storage.
    pub fn encode(self, output: &mut [u8]) -> Result<usize, CuePayloadError> {
        self.validate()?;
        if output.len() < CUE_METADATA_ENVELOPE_BYTES {
            return Err(CuePayloadError::BufferTooSmall);
        }

        output[0..8].copy_from_slice(&self.stage_revision.get().to_le_bytes());
        output[8..16].copy_from_slice(&self.first_native_event_sequence.get().to_le_bytes());
        output[16..24].copy_from_slice(&self.last_native_event_sequence.get().to_le_bytes());
        output[24..28].copy_from_slice(&self.first_cue_sequence.get().to_le_bytes());
        output[28..32].copy_from_slice(&self.last_cue_sequence.get().to_le_bytes());
        output[32..36].copy_from_slice(&self.merge_count.to_le_bytes());
        Ok(CUE_METADATA_ENVELOPE_BYTES)
    }

    /// Decode and validate one fixed metadata envelope.
    pub fn decode(input: &[u8]) -> Result<Self, CuePayloadError> {
        if input.len() < CUE_METADATA_ENVELOPE_BYTES {
            return Err(CuePayloadError::Truncated);
        }

        let stage_revision = StageRevision::new(read_u64(input, 0));
        let first_native_event_sequence =
            NativeEventSequence::new(read_u64(input, 8)).ok_or(CuePayloadError::InvalidMetadata)?;
        let last_native_event_sequence = NativeEventSequence::new(read_u64(input, 16))
            .ok_or(CuePayloadError::InvalidMetadata)?;
        let first_cue_sequence =
            CueSequence::new(read_u32(input, 24)).ok_or(CuePayloadError::InvalidMetadata)?;
        let last_cue_sequence =
            CueSequence::new(read_u32(input, 28)).ok_or(CuePayloadError::InvalidMetadata)?;
        let merge_count = read_u32(input, 32);

        let metadata = Self {
            stage_revision,
            first_native_event_sequence,
            last_native_event_sequence,
            first_cue_sequence,
            last_cue_sequence,
            merge_count,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    fn validate(self) -> Result<(), CuePayloadError> {
        if self.last_native_event_sequence < self.first_native_event_sequence
            || self.last_cue_sequence < self.first_cue_sequence
            || self.last_cue_sequence.get() - self.first_cue_sequence.get() != self.merge_count
        {
            return Err(CuePayloadError::InvalidMetadata);
        }
        Ok(())
    }
}

/// Borrowed view of a canonical MPY-05 cue payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CuePayloadRef<'a> {
    /// Fixed causality and coalescing metadata.
    pub metadata: CuePayloadMetadata,
    /// Descriptor-owned event payload following the fixed envelope.
    pub event_payload: &'a [u8],
}

impl<'a> CuePayloadRef<'a> {
    /// Decode a metadata envelope and borrow its remaining event payload.
    pub fn decode(input: &'a [u8]) -> Result<Self, CuePayloadError> {
        let metadata = CuePayloadMetadata::decode(input)?;
        Ok(Self {
            metadata,
            event_payload: &input[CUE_METADATA_ENVELOPE_BYTES..],
        })
    }
}

/// Allocation-free cue-payload envelope failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CuePayloadError {
    /// Caller-owned output storage cannot hold metadata plus event payload.
    BufferTooSmall,
    /// The input ended before the fixed metadata envelope completed.
    Truncated,
    /// Sequence ranges or required nonzero identifiers are invalid.
    InvalidMetadata,
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
        input[offset + 4],
        input[offset + 5],
        input[offset + 6],
        input[offset + 7],
    ])
}

/// Delivery policy applied by the endpoint cue queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueDelivery {
    /// A required lifecycle, teardown, overflow, or recovery notification.
    Critical,
    /// A normal notification that preserves every admitted record in order.
    Ordered,
    /// A descriptor-authorized high-rate notification whose latest payload
    /// replaces only an exact matching record at the queue tail.
    LatestValueCoalescible,
}

impl CueDelivery {
    const fn is_ordinary(self) -> bool {
        !matches!(self, Self::Critical)
    }
}

/// Descriptor-supplied discriminator used for exact-key coalescing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CoalescingKey(u64);

impl CoalescingKey {
    /// Construct a coalescing discriminator.
    ///
    /// Zero is valid because the full exact key also contains every cue
    /// identity field.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the descriptor-supplied representation.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable routing identity shared by one cue input and queued record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CueIdentity {
    stage_id: StageId,
    object_id: ObjectId,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
}

impl CueIdentity {
    /// Construct the nonzero identity fields required by an MPY Cue frame.
    pub const fn new(
        stage_id: StageId,
        object_id: ObjectId,
        subscription_id: SubscriptionId,
        callback_id: CallbackId,
        event_id: EventId,
    ) -> Self {
        Self {
            stage_id,
            object_id,
            subscription_id,
            callback_id,
            event_id,
        }
    }

    /// Return the owning Stage.
    pub const fn stage_id(self) -> StageId {
        self.stage_id
    }

    /// Return the emitting actor.
    pub const fn object_id(self) -> ObjectId {
        self.object_id
    }

    /// Return the runtime subscription token.
    pub const fn subscription_id(self) -> SubscriptionId {
        self.subscription_id
    }

    /// Return the adapter callback token.
    pub const fn callback_id(self) -> CallbackId {
        self.callback_id
    }

    /// Return the registered event identifier.
    pub const fn event_id(self) -> EventId {
        self.event_id
    }
}

/// Validated resource limits for one endpoint-owned cue queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CueLimits {
    total_slots: usize,
    critical_reserve: usize,
    per_stage_ordinary_quota: usize,
    max_payload_bytes: usize,
    max_frame_bytes: usize,
}

impl CueLimits {
    /// Validate and construct endpoint cue limits.
    ///
    /// At least one slot must remain available to ordinary traffic and at
    /// least one slot must be protected for Critical traffic.
    pub fn new(
        total_slots: usize,
        critical_reserve: usize,
        per_stage_ordinary_quota: usize,
        max_payload_bytes: usize,
        max_frame_bytes: usize,
    ) -> Result<Self, CueLimitsError> {
        if total_slots == 0 {
            return Err(CueLimitsError::NoSlots);
        }
        if critical_reserve == 0 {
            return Err(CueLimitsError::NoCriticalReserve);
        }
        if critical_reserve >= total_slots {
            return Err(CueLimitsError::NoOrdinaryCapacity);
        }

        let ordinary_capacity = total_slots - critical_reserve;
        if per_stage_ordinary_quota == 0 {
            return Err(CueLimitsError::NoStageQuota);
        }
        if per_stage_ordinary_quota > ordinary_capacity {
            return Err(CueLimitsError::StageQuotaExceedsOrdinaryCapacity);
        }

        if u32::try_from(max_payload_bytes).is_err() || u32::try_from(max_frame_bytes).is_err() {
            return Err(CueLimitsError::LimitExceedsProtocolWidth);
        }

        let required_frame_bytes = CUE_FRAME_OVERHEAD_BYTES
            .checked_add(max_payload_bytes)
            .ok_or(CueLimitsError::FrameSizeOverflow)?;
        if max_frame_bytes < required_frame_bytes {
            return Err(CueLimitsError::FrameTooSmallForPayload);
        }

        Ok(Self {
            total_slots,
            critical_reserve,
            per_stage_ordinary_quota,
            max_payload_bytes,
            max_frame_bytes,
        })
    }

    /// Return the maximum number of queued records of all classes.
    pub const fn total_slots(self) -> usize {
        self.total_slots
    }

    /// Return the capacity protected from ordinary cue admission.
    pub const fn critical_reserve(self) -> usize {
        self.critical_reserve
    }

    /// Return the maximum pending ordinary records owned by one Stage.
    pub const fn per_stage_ordinary_quota(self) -> usize {
        self.per_stage_ordinary_quota
    }

    /// Return the maximum owned event payload size.
    pub const fn max_payload_bytes(self) -> usize {
        self.max_payload_bytes
    }

    /// Return the maximum accounted canonical Cue frame size.
    pub const fn max_frame_bytes(self) -> usize {
        self.max_frame_bytes
    }

    const fn ordinary_capacity(self) -> usize {
        self.total_slots - self.critical_reserve
    }
}

/// Invalid endpoint cue-limit declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueLimitsError {
    /// The queue has no record slots.
    NoSlots,
    /// No capacity is protected for Critical cues.
    NoCriticalReserve,
    /// The reserve consumes every slot, leaving no ordinary capacity.
    NoOrdinaryCapacity,
    /// A Stage is not allowed to hold any ordinary record.
    NoStageQuota,
    /// One Stage quota is larger than the endpoint's ordinary capacity.
    StageQuotaExceedsOrdinaryCapacity,
    /// Fixed overhead plus the payload limit overflowed `usize`.
    FrameSizeOverflow,
    /// The frame limit cannot contain the declared maximum payload.
    FrameTooSmallForPayload,
    /// A payload or frame limit cannot be represented by the MPY `u32` codec.
    LimitExceedsProtocolWidth,
}

/// Owned cue offered to the endpoint queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueInput {
    stage_id: StageId,
    stage_revision: StageRevision,
    native_event_sequence: NativeEventSequence,
    object_id: ObjectId,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
    delivery: CueDelivery,
    coalescing_key: Option<CoalescingKey>,
    flags: u32,
    payload: Vec<u8>,
}

impl CueInput {
    /// Construct an owned cue input.
    pub fn new(
        identity: CueIdentity,
        stage_revision: StageRevision,
        native_event_sequence: NativeEventSequence,
        delivery: CueDelivery,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            stage_id: identity.stage_id,
            stage_revision,
            native_event_sequence,
            object_id: identity.object_id,
            subscription_id: identity.subscription_id,
            callback_id: identity.callback_id,
            event_id: identity.event_id,
            delivery,
            coalescing_key: None,
            flags: 0,
            payload,
        }
    }

    /// Attach the descriptor-supplied discriminator required for coalescing.
    pub fn with_coalescing_key(mut self, key: CoalescingKey) -> Self {
        self.coalescing_key = Some(key);
        self
    }

    /// Mark this Critical cue as a subscription callback-token release.
    ///
    /// MPY-06 must release the addressed callback instead of invoking it.
    pub fn with_subscription_release(mut self) -> Self {
        self.flags |= CUE_FLAG_SUBSCRIPTION_RELEASE;
        self
    }

    /// Return the owning Stage.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the committed Stage Revision visible when the cue was emitted.
    pub const fn stage_revision(&self) -> StageRevision {
        self.stage_revision
    }

    /// Return the native event traversal sequence that emitted the cue.
    pub const fn native_event_sequence(&self) -> NativeEventSequence {
        self.native_event_sequence
    }

    /// Return the actor that emitted the cue.
    pub const fn object_id(&self) -> ObjectId {
        self.object_id
    }

    /// Return the runtime subscription token.
    pub const fn subscription_id(&self) -> SubscriptionId {
        self.subscription_id
    }

    /// Return the adapter callback token.
    pub const fn callback_id(&self) -> CallbackId {
        self.callback_id
    }

    /// Return the registered event identifier.
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    /// Return the requested delivery policy.
    pub const fn delivery(&self) -> CueDelivery {
        self.delivery
    }

    /// Return the optional descriptor-supplied coalescing discriminator.
    pub const fn coalescing_key(&self) -> Option<CoalescingKey> {
        self.coalescing_key
    }

    /// Return queue-owned cue flags requested by this input.
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// Borrow the owned event payload.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Immutable queued cue returned by a bounded drain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueRecord {
    stage_id: StageId,
    stage_revision: StageRevision,
    first_native_event_sequence: NativeEventSequence,
    last_native_event_sequence: NativeEventSequence,
    object_id: ObjectId,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
    delivery: CueDelivery,
    coalescing_key: Option<CoalescingKey>,
    flags: u32,
    first_sequence: CueSequence,
    last_sequence: CueSequence,
    merge_count: u32,
    payload: Vec<u8>,
}

impl CueRecord {
    fn from_input(input: CueInput, sequence: CueSequence) -> Self {
        Self {
            stage_id: input.stage_id,
            stage_revision: input.stage_revision,
            first_native_event_sequence: input.native_event_sequence,
            last_native_event_sequence: input.native_event_sequence,
            object_id: input.object_id,
            subscription_id: input.subscription_id,
            callback_id: input.callback_id,
            event_id: input.event_id,
            delivery: input.delivery,
            coalescing_key: input.coalescing_key,
            flags: input.flags,
            first_sequence: sequence,
            last_sequence: sequence,
            merge_count: 0,
            payload: input.payload,
        }
    }

    fn exact_coalescing_key_matches(&self, input: &CueInput) -> bool {
        self.delivery == CueDelivery::LatestValueCoalescible
            && input.delivery == CueDelivery::LatestValueCoalescible
            && self.stage_id == input.stage_id
            && self.stage_revision == input.stage_revision
            && self.object_id == input.object_id
            && self.subscription_id == input.subscription_id
            && self.callback_id == input.callback_id
            && self.event_id == input.event_id
            && self.coalescing_key == input.coalescing_key
            && self.flags == input.flags
    }

    fn replace_with(&mut self, input: CueInput, sequence: CueSequence) {
        debug_assert!(self.exact_coalescing_key_matches(&input));
        self.last_sequence = sequence;
        self.last_native_event_sequence = input.native_event_sequence;
        self.merge_count += 1;
        self.payload = input.payload;
    }

    fn replace_with_retained_payload(
        &mut self,
        mut input: CueInput,
        sequence: CueSequence,
    ) -> Vec<u8> {
        debug_assert!(self.exact_coalescing_key_matches(&input));
        self.last_sequence = sequence;
        self.last_native_event_sequence = input.native_event_sequence;
        self.merge_count += 1;
        core::mem::swap(&mut self.payload, &mut input.payload);
        input.payload
    }

    /// Return the owning Stage.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the committed Stage Revision shared by the represented events.
    pub const fn stage_revision(&self) -> StageRevision {
        self.stage_revision
    }

    /// Return the first native event sequence represented by this record.
    pub const fn first_native_event_sequence(&self) -> NativeEventSequence {
        self.first_native_event_sequence
    }

    /// Return the last native event sequence represented by this record.
    pub const fn last_native_event_sequence(&self) -> NativeEventSequence {
        self.last_native_event_sequence
    }

    /// Return the actor that emitted the cue.
    pub const fn object_id(&self) -> ObjectId {
        self.object_id
    }

    /// Return the runtime subscription token.
    pub const fn subscription_id(&self) -> SubscriptionId {
        self.subscription_id
    }

    /// Return the adapter callback token.
    pub const fn callback_id(&self) -> CallbackId {
        self.callback_id
    }

    /// Return the registered event identifier.
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    /// Return the queue delivery class.
    pub const fn delivery(&self) -> CueDelivery {
        self.delivery
    }

    /// Return the descriptor-supplied coalescing discriminator.
    pub const fn coalescing_key(&self) -> Option<CoalescingKey> {
        self.coalescing_key
    }

    /// Return queue-owned cue flags excluding derived metadata flags.
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// Return whether this record releases its callback token.
    pub const fn is_subscription_release(&self) -> bool {
        self.flags & CUE_FLAG_SUBSCRIPTION_RELEASE != 0
    }

    /// Return the first endpoint sequence represented by this record.
    pub const fn first_sequence(&self) -> CueSequence {
        self.first_sequence
    }

    /// Return the last and delivery-order endpoint sequence represented here.
    pub const fn last_sequence(&self) -> CueSequence {
        self.last_sequence
    }

    /// Return the number of later emissions merged into this record.
    pub const fn merge_count(&self) -> u32 {
        self.merge_count
    }

    /// Borrow the latest event payload.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Return the canonical MPY-05 metadata represented by this record.
    pub const fn payload_metadata(&self) -> CuePayloadMetadata {
        CuePayloadMetadata {
            stage_revision: self.stage_revision,
            first_native_event_sequence: self.first_native_event_sequence,
            last_native_event_sequence: self.last_native_event_sequence,
            first_cue_sequence: self.first_sequence,
            last_cue_sequence: self.last_sequence,
            merge_count: self.merge_count,
        }
    }

    /// Encode metadata followed by event payload into caller-owned storage.
    pub fn encode_payload_envelope(&self, output: &mut [u8]) -> Result<usize, CuePayloadError> {
        let payload_bytes = CUE_METADATA_ENVELOPE_BYTES
            .checked_add(self.payload.len())
            .ok_or(CuePayloadError::BufferTooSmall)?;
        if output.len() < payload_bytes {
            return Err(CuePayloadError::BufferTooSmall);
        }

        self.payload_metadata().encode(output)?;
        output[CUE_METADATA_ENVELOPE_BYTES..payload_bytes].copy_from_slice(&self.payload);
        Ok(payload_bytes)
    }

    /// Encode the payload envelope and borrow it through an MPY-02 Cue value.
    pub fn protocol_cue<'a>(
        &self,
        payload_output: &'a mut [u8],
    ) -> Result<protocol::Cue<'a>, CuePayloadError> {
        let payload_bytes = self.encode_payload_envelope(payload_output)?;
        let mut flags = self.flags | CUE_FLAG_MPY05_METADATA;
        if self.merge_count != 0 {
            flags |= CUE_FLAG_LATEST_VALUE_MERGED;
        }

        Ok(protocol::Cue {
            sequence: self.last_sequence.get(),
            stage_id: self.stage_id.get(),
            object_id: self.object_id.get(),
            subscription_id: self.subscription_id.get(),
            callback_id: self.callback_id.get(),
            event_id: self.event_id.get(),
            flags,
            payload: &payload_output[..payload_bytes],
        })
    }

    /// Encode the payload envelope and borrow a canonical MPY Cue frame.
    pub fn protocol_frame<'a>(
        &self,
        payload_output: &'a mut [u8],
    ) -> Result<FrameRef<'a>, CuePayloadError> {
        self.protocol_cue(payload_output).map(FrameRef::Cue)
    }

    /// Return the accounted canonical Cue frame size.
    pub fn frame_bytes(&self) -> usize {
        CUE_FRAME_OVERHEAD_BYTES + self.payload.len()
    }
}

/// Raw-input range rejected before native dispatch and actor mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingInputLoss {
    stage_id: StageId,
    input_class: InputClass,
    first_sequence: InputSequence,
    last_sequence: InputSequence,
    lost_count: u32,
}

impl PendingInputLoss {
    fn first(stage_id: StageId, input_class: InputClass, sequence: InputSequence) -> Self {
        Self {
            stage_id,
            input_class,
            first_sequence: sequence,
            last_sequence: sequence,
            lost_count: 1,
        }
    }

    fn extend(&mut self, sequence: InputSequence) -> Result<(), CueQueueError> {
        debug_assert!(sequence > self.last_sequence);
        let lost_count = self
            .lost_count
            .checked_add(1)
            .ok_or(CueQueueError::InputLossCountExhausted)?;
        self.last_sequence = sequence;
        self.lost_count = lost_count;
        Ok(())
    }

    /// Return the Stage whose raw input was rejected.
    pub const fn stage_id(self) -> StageId {
        self.stage_id
    }

    /// Return the endpoint-defined raw-input class.
    pub const fn input_class(self) -> InputClass {
        self.input_class
    }

    /// Return the first rejected raw-input sequence.
    pub const fn first_sequence(self) -> InputSequence {
        self.first_sequence
    }

    /// Return the last rejected raw-input sequence.
    pub const fn last_sequence(self) -> InputSequence {
        self.last_sequence
    }

    /// Return the number of rejected raw inputs represented by the notice.
    pub const fn lost_count(self) -> u32 {
        self.lost_count
    }

    fn encode(self, output: &mut [u8]) -> Result<usize, CuePayloadError> {
        if output.len() < INPUT_OVERFLOW_METADATA_BYTES {
            return Err(CuePayloadError::BufferTooSmall);
        }
        output[0..4].copy_from_slice(&self.stage_id.get().to_le_bytes());
        output[4..8].copy_from_slice(&self.input_class.get().to_le_bytes());
        output[8..16].copy_from_slice(&self.first_sequence.get().to_le_bytes());
        output[16..24].copy_from_slice(&self.last_sequence.get().to_le_bytes());
        output[24..28].copy_from_slice(&self.lost_count.to_le_bytes());
        Ok(INPUT_OVERFLOW_METADATA_BYTES)
    }
}

/// Immutable queued RuntimeNotice sharing endpoint order with callback cues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeNoticeRecord {
    sequence: CueSequence,
    kind: u32,
    input_loss: PendingInputLoss,
    payload: Vec<u8>,
}

impl RuntimeNoticeRecord {
    fn input_overflow(
        sequence: CueSequence,
        input_loss: PendingInputLoss,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            sequence,
            kind: RUNTIME_NOTICE_INPUT_OVERFLOW,
            input_loss,
            payload,
        }
    }

    /// Return the shared endpoint output sequence.
    pub const fn sequence(&self) -> CueSequence {
        self.sequence
    }

    /// Return the registered RuntimeNotice kind.
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    /// Return the represented raw-input loss range.
    pub const fn input_loss(&self) -> PendingInputLoss {
        self.input_loss
    }

    /// Borrow caller-supplied notice detail bytes following typed metadata.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Encode typed InputOverflow metadata followed by caller-owned detail.
    pub fn encode_payload(&self, output: &mut [u8]) -> Result<usize, CuePayloadError> {
        let length = INPUT_OVERFLOW_METADATA_BYTES
            .checked_add(self.payload.len())
            .ok_or(CuePayloadError::BufferTooSmall)?;
        if output.len() < length {
            return Err(CuePayloadError::BufferTooSmall);
        }
        self.input_loss.encode(output)?;
        output[INPUT_OVERFLOW_METADATA_BYTES..length].copy_from_slice(&self.payload);
        Ok(length)
    }

    /// Encode and borrow this notice through the canonical MPY RuntimeNotice.
    pub fn protocol_frame<'a>(
        &self,
        payload_output: &'a mut [u8],
    ) -> Result<FrameRef<'a>, CuePayloadError> {
        let payload_len = self.encode_payload(payload_output)?;
        Ok(FrameRef::RuntimeNotice(protocol::RuntimeNotice {
            sequence: self.sequence.get(),
            kind: self.kind,
            diagnostic: "",
            payload: &payload_output[..payload_len],
        }))
    }

    /// Return the accounted canonical RuntimeNotice frame size.
    pub fn frame_bytes(&self) -> usize {
        RUNTIME_NOTICE_FRAME_FIXED_BYTES + INPUT_OVERFLOW_METADATA_BYTES + self.payload.len()
    }
}

/// One globally ordered endpoint output record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointRecord {
    /// Descriptor-derived callback cue.
    Cue(CueRecord),
    /// Critical runtime notice that does not address a callback.
    RuntimeNotice(RuntimeNoticeRecord),
}

impl EndpointRecord {
    /// Return the shared endpoint sequence used for drain ordering.
    pub const fn sequence(&self) -> CueSequence {
        match self {
            Self::Cue(record) => record.last_sequence(),
            Self::RuntimeNotice(record) => record.sequence(),
        }
    }

    /// Return the accounted canonical frame size.
    pub fn frame_bytes(&self) -> usize {
        match self {
            Self::Cue(record) => record.frame_bytes(),
            Self::RuntimeNotice(record) => record.frame_bytes(),
        }
    }

    fn is_ordinary(&self) -> bool {
        matches!(self, Self::Cue(record) if record.delivery.is_ordinary())
    }

    fn as_cue(&self) -> Option<&CueRecord> {
        match self {
            Self::Cue(record) => Some(record),
            Self::RuntimeNotice(_) => None,
        }
    }

    fn as_cue_mut(&mut self) -> Option<&mut CueRecord> {
        match self {
            Self::Cue(record) => Some(record),
            Self::RuntimeNotice(_) => None,
        }
    }
}

fn is_stage_ordinary(record: &EndpointRecord, stage_id: StageId) -> bool {
    matches!(record, EndpointRecord::Cue(cue) if cue.stage_id == stage_id && cue.delivery.is_ordinary())
}

/// Outcome of a successful cue admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// A new record was appended to the endpoint queue.
    Queued {
        /// Sequence allocated to the appended record.
        sequence: CueSequence,
    },
    /// A descriptor-authorized emission replaced the exact matching tail.
    Coalesced {
        /// First sequence represented by the retained record.
        first_sequence: CueSequence,
        /// Sequence allocated to the replacing emission.
        last_sequence: CueSequence,
        /// Number of later emissions merged into the retained record.
        merge_count: u32,
    },
}

/// Endpoint cue-sequence range and count lost before overflow notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingCueLoss {
    first_sequence: CueSequence,
    last_sequence: CueSequence,
    lost_count: u32,
}

impl PendingCueLoss {
    fn first(sequence: CueSequence) -> Self {
        Self {
            first_sequence: sequence,
            last_sequence: sequence,
            lost_count: 1,
        }
    }

    fn extend(&mut self, sequence: CueSequence) {
        debug_assert!(sequence > self.last_sequence);
        self.last_sequence = sequence;
        self.lost_count += 1;
    }

    /// Return the first rejected ordinary cue sequence.
    pub const fn first_sequence(self) -> CueSequence {
        self.first_sequence
    }

    /// Return the last rejected ordinary cue sequence.
    pub const fn last_sequence(self) -> CueSequence {
        self.last_sequence
    }

    /// Return the number of rejected ordinary cue emissions.
    pub const fn lost_count(self) -> u32 {
        self.lost_count
    }
}

/// Successful Critical notice that resolved one pending cue-loss barrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverflowNoticeOutcome {
    /// Loss range reported by the admitted notice.
    pub loss: PendingCueLoss,
    /// Endpoint sequence assigned to the Critical notice.
    pub notice_sequence: CueSequence,
}

/// Successful Critical InputOverflow notice admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputOverflowNoticeOutcome {
    /// Raw-input loss range reported by the notice.
    pub loss: PendingInputLoss,
    /// Shared endpoint sequence assigned to the RuntimeNotice.
    pub notice_sequence: CueSequence,
}

/// Counts and sequence bounds removed during Stage teardown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RemovedStageOrdinary {
    /// Number of queued ordinary records removed for the Stage.
    pub count: usize,
    /// First removed endpoint sequence, when any record was removed.
    pub first_sequence: Option<CueSequence>,
    /// Last removed endpoint sequence, when any record was removed.
    pub last_sequence: Option<CueSequence>,
}

/// Logical worst-case queue admission reserved before native dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CueAdmission {
    /// Stage that will own every cue admitted through this reservation.
    pub stage_id: StageId,
    /// Worst-case number of Ordered or coalescible cue inputs.
    pub ordinary_slots: usize,
    /// Worst-case number of Critical cue inputs.
    pub critical_slots: usize,
}

/// Cue admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueQueueError {
    /// A queue-owned reservation could not be satisfied without panicking.
    AllocationFailed,
    /// The endpoint is faulted and requires an epoch reset or recovery action.
    Faulted,
    /// A logical pre-dispatch reservation was attempted behind a loss barrier.
    AdmissionBackpressured,
    /// The endpoint cannot reserve the requested record count.
    AdmissionCapacity,
    /// The Stage cannot reserve the requested ordinary record count.
    AdmissionStageQuota {
        /// Stage whose ordinary quota would be exceeded.
        stage_id: StageId,
    },
    /// Stage causality cannot be retired while its endpoint work is pending.
    StageFinalizeBusy {
        /// Stage whose records, release cues, or loss notice remain pending.
        stage_id: StageId,
    },
    /// A cue offered through a reservation belongs to a different Stage.
    ReservationStageMismatch,
    /// A reservation has no remaining slot for the offered delivery class.
    ReservationClassExhausted,
    /// Queue state changed after exact-input preparation.
    StalePreparedInputs,
    /// The exact-input transaction was already committed.
    PreparedInputsAlreadyCommitted,
    /// A purge-aware Stage teardown input belongs to another Stage.
    StageTeardownInputStageMismatch {
        /// Stage whose ordinary records the transaction would purge.
        expected: StageId,
        /// Stage carried by the mismatched Critical input.
        offered: StageId,
    },
    /// A purge-aware Stage teardown transaction only accepts Critical inputs.
    StageTeardownInputMustBeCritical,
    /// The internal exact-transaction revision can no longer advance.
    MutationRevisionExhausted,
    /// A latest-value coalescible cue omitted its exact-key discriminator.
    MissingCoalescingKey,
    /// A Critical or Ordered cue supplied a coalescing discriminator.
    UnexpectedCoalescingKey,
    /// The owned event payload exceeds the negotiated limit.
    PayloadTooLarge {
        /// Offered payload bytes.
        actual: usize,
        /// Negotiated maximum payload bytes.
        maximum: usize,
    },
    /// The canonical Cue frame exceeds the negotiated frame limit.
    FrameTooLarge {
        /// Accounted complete frame bytes.
        actual: usize,
        /// Negotiated maximum frame bytes.
        maximum: usize,
    },
    /// The endpoint ordinary lane has no free record slot.
    OrdinaryCapacityExhausted {
        /// Sequence allocated to the rejected emission.
        sequence: CueSequence,
    },
    /// The owning Stage reached its pending ordinary-record quota.
    StageQuotaExhausted {
        /// Sequence allocated to the rejected emission.
        sequence: CueSequence,
        /// Stage whose quota rejected the record.
        stage_id: StageId,
    },
    /// An ordinary cue was rejected behind a pending loss barrier.
    OrdinaryBackpressured {
        /// Sequence allocated to the rejected emission and added to the loss.
        sequence: CueSequence,
    },
    /// VM-side draining is gated until the pending loss notice is queued.
    DrainBackpressured,
    /// A loss-notice API was called without a pending ordinary loss.
    NoPendingCueLoss,
    /// The explicit overflow-notice API requires a Critical cue input.
    OverflowNoticeMustBeCritical,
    /// A callback-token release marker was attached to an ordinary cue.
    SubscriptionReleaseMustBeCritical,
    /// No raw input loss is awaiting an InputOverflow notice.
    NoPendingInputLoss,
    /// A different loss barrier already gates raw-input admission.
    InputBackpressured,
    /// Raw-input sequence did not advance monotonically.
    InputSequenceRegressed {
        /// Last raw-input sequence already recorded by the queue.
        previous: InputSequence,
        /// Non-advancing raw-input sequence offered by the endpoint.
        offered: InputSequence,
    },
    /// The bounded raw-input loss counter exhausted its wire representation.
    InputLossCountExhausted,
    /// A Critical record could not be admitted and faulted the endpoint.
    CriticalCapacityExhausted {
        /// Sequence allocated to the failed Critical emission.
        sequence: CueSequence,
    },
    /// A Critical RuntimeNotice could not be admitted and faulted the endpoint.
    CriticalNoticeCapacityExhausted {
        /// Shared endpoint sequence allocated to the failed notice.
        sequence: CueSequence,
        /// Registered RuntimeNotice kind that could not be queued.
        kind: u32,
    },
    /// The endpoint-wide `u32` sequence space was exhausted.
    SequenceExhausted,
    /// An exact-key coalescing candidate did not advance native event order.
    NonMonotonicCoalescingEventSequence {
        /// Last native event sequence already represented by the tail.
        previous: NativeEventSequence,
        /// Native event sequence offered by the new cue.
        offered: NativeEventSequence,
    },
    /// A Stage Revision regressed relative to an earlier cue in this epoch.
    StageRevisionRegressed {
        /// Stage whose revision regressed.
        stage_id: StageId,
        /// Last revision accepted for that Stage.
        previous: StageRevision,
        /// Regressing revision offered by the new cue.
        offered: StageRevision,
    },
    /// The endpoint-wide native event sequence regressed.
    NativeEventSequenceRegressed {
        /// Last native event sequence accepted by the queue.
        previous: NativeEventSequence,
        /// Regressing sequence offered by the new cue.
        offered: NativeEventSequence,
    },
    /// The epoch has no pre-reserved causality slot for another Stage.
    StageCausalityCapacityExhausted {
        /// Stage that could not be tracked without allocating during enqueue.
        stage_id: StageId,
    },
}

/// Out-of-band emergency notice retained after an endpoint fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmergencyFault {
    /// The queue could not admit a required Critical cue.
    CriticalCapacityExhausted {
        /// Sequence allocated to the failed Critical emission.
        sequence: CueSequence,
        /// Stage that emitted the required cue.
        stage_id: StageId,
        /// Event that could not be queued.
        event_id: EventId,
    },
    /// The endpoint-wide `u32` CueSequence space was exhausted.
    SequenceExhausted,
    /// A required Critical cue referenced an untrackable new Stage.
    CriticalStageCausalityCapacityExhausted {
        /// Stage that could not be tracked within the pre-reserved capacity.
        stage_id: StageId,
        /// Critical event that could not be admitted safely.
        event_id: EventId,
    },
    /// A Critical RuntimeNotice could not be admitted.
    CriticalNoticeCapacityExhausted {
        /// Shared endpoint sequence allocated to the failed notice.
        sequence: CueSequence,
        /// Registered RuntimeNotice kind that could not be queued.
        kind: u32,
    },
}

/// Runtime health of the endpoint cue queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueEndpointState {
    /// Admissions and drains operate normally.
    Ready,
    /// Ordinary admissions are gated until a Critical loss notice is queued.
    Backpressured,
    /// A required cue could not be represented or admitted.
    Faulted,
}

/// Maximum work allowed in one VM-safe cue drain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrainBudget {
    /// Maximum number of records returned by this drain.
    pub max_cues: usize,
    /// Maximum sum of accounted canonical frame bytes.
    pub max_bytes: usize,
}

impl DrainBudget {
    /// Construct a caller-selected cue-count and byte-count drain budget.
    ///
    /// Selecting fewer bytes than the queue head's accounted frame size is an
    /// intentional no-progress request. Use [`Self::for_limits`] when the
    /// caller requires a budget that can always admit the head record.
    pub const fn new(max_cues: usize, max_bytes: usize) -> Self {
        Self {
            max_cues,
            max_bytes,
        }
    }

    /// Construct a limits-derived budget that can carry every admitted cue.
    ///
    /// A positive cue budget reserves one negotiated maximum frame per cue.
    /// Saturating multiplication preserves that guarantee on `usize` overflow.
    /// A zero cue budget intentionally yields a zero-byte, no-progress drain.
    pub const fn for_limits(limits: CueLimits, max_cues: usize) -> Self {
        Self {
            max_cues,
            max_bytes: limits.max_frame_bytes.saturating_mul(max_cues),
        }
    }
}

/// Records and byte accounting produced by one bounded drain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrainedCues {
    /// Globally ordered records removed from the endpoint queue.
    pub cues: Vec<CueRecord>,
    /// Sum of [`CueRecord::frame_bytes`] for returned records.
    pub frame_bytes: usize,
}

/// Endpoint records and byte accounting produced by one bounded global drain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrainedEndpointRecords {
    /// Globally ordered cue and RuntimeNotice records removed from the queue.
    pub records: Vec<EndpointRecord>,
    /// Sum of [`EndpointRecord::frame_bytes`] for returned records.
    pub frame_bytes: usize,
}

/// One endpoint-owned sequence-ordered cue queue.
pub struct CueQueue {
    limits: CueLimits,
    mutation_revision: u64,
    records: VecDeque<EndpointRecord>,
    stage_causality: Vec<StageCausality>,
    last_native_event_sequence: Option<NativeEventSequence>,
    last_input_sequence: Option<InputSequence>,
    ordinary_records: usize,
    next_sequence: u32,
    state: CueEndpointState,
    pending_loss: Option<PendingCueLoss>,
    pending_input_loss: Option<PendingInputLoss>,
    emergency_fault: Option<EmergencyFault>,
}

/// Exclusive logical queue admission reserved before one native dispatch.
///
/// Holding this value mutably borrows the endpoint queue, so no other producer
/// can steal the preflighted slots. Dropping it releases any unused counts.
pub struct CueReservation<'a> {
    queue: &'a mut CueQueue,
    stage_id: StageId,
    remaining_ordinary: usize,
    remaining_critical: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedInputsState {
    Prepared,
    Committed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedCueAction {
    Queue { sequence: CueSequence },
    Coalesce { sequence: CueSequence },
}

/// Fully validated exact cue inputs retained across one Safe Turn.
///
/// Preparation validates the inputs in caller order against both current and
/// earlier prepared records. Commit then performs only infallible planned
/// moves after checking that the queue has not changed.
pub struct PreparedCueInputs {
    queue_revision: u64,
    input_count: usize,
    inputs: Vec<CueInput>,
    actions: Vec<PreparedCueAction>,
    retired_payloads: Vec<Vec<u8>>,
    state: PreparedInputsState,
}

/// Exclusive, fully validated exact-input commit capability.
///
/// Acquisition happens before Stage mutation and prevents any intervening
/// queue mutation. Consuming the guard commits the retained inputs infallibly.
pub struct ExactCueCommit<'a> {
    queue: &'a mut CueQueue,
    prepared: &'a mut PreparedCueInputs,
}

/// Purge-aware exact cue transaction for one full-Stage teardown.
///
/// Preparation simulates removal of every ordinary record for `stage_id`
/// before validating the retained Critical inputs. Purged records and any
/// displaced payloads remain owned here after commit until the endpoint calls
/// [`CueQueue::release_stage_teardown_inputs`] outside the atomic Safe Turn.
pub struct PreparedStageTeardownCues {
    stage_id: StageId,
    purged_ordinary: RemovedStageOrdinary,
    purged_records: Vec<EndpointRecord>,
    exact: PreparedCueInputs,
}

/// Exclusive full-Stage cue teardown commit capability.
///
/// Acquisition is the final fallible freshness check. The guard prevents an
/// intervening queue mutation, and consuming it commits the purge and exact
/// Critical inputs without allocation, deallocation, or error.
pub struct StageTeardownCueCommit<'a> {
    queue: &'a mut CueQueue,
    prepared: &'a mut PreparedStageTeardownCues,
}

impl StageTeardownCueCommit<'_> {
    /// Commit the planned Stage purge and Critical inputs infallibly.
    pub fn commit(self) {
        self.queue
            .commit_stage_teardown_inputs_infallible(self.prepared);
    }

    /// Release the exclusive guard without changing queue or prepared state.
    pub fn rollback(self) {}
}

impl ExactCueCommit<'_> {
    /// Commit every prepared input without allocation, deallocation, or error.
    pub fn commit(self) {
        self.queue.commit_exact_inputs_infallible(self.prepared);
    }

    /// Release the exclusive guard without changing queue or prepared state.
    pub fn rollback(self) {}
}

impl PreparedCueInputs {
    /// Return the exact number of validated inputs in this transaction.
    pub const fn input_count(&self) -> usize {
        self.input_count
    }

    /// Borrow the fully formed inputs before they are moved during commit.
    pub fn inputs(&self) -> &[CueInput] {
        &self.inputs
    }

    /// Return whether no cue input is represented by this transaction.
    pub const fn is_empty(&self) -> bool {
        self.input_count == 0
    }
}

impl PreparedStageTeardownCues {
    /// Return the Stage whose ordinary queue records will be purged.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the exact preflighted ordinary-record purge summary.
    pub const fn purged_ordinary(&self) -> RemovedStageOrdinary {
        self.purged_ordinary
    }

    /// Return the exact number of retained Critical teardown inputs.
    pub const fn input_count(&self) -> usize {
        self.exact.input_count
    }

    /// Borrow retained teardown inputs before commit moves them to the queue.
    pub fn inputs(&self) -> &[CueInput] {
        &self.exact.inputs
    }
}

#[derive(Clone, Copy)]
struct VirtualCueTail {
    stage_id: StageId,
    stage_revision: StageRevision,
    native_event_sequence: NativeEventSequence,
    object_id: ObjectId,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
    delivery: CueDelivery,
    coalescing_key: Option<CoalescingKey>,
    flags: u32,
}

impl VirtualCueTail {
    fn from_record(record: &CueRecord) -> Self {
        Self {
            stage_id: record.stage_id,
            stage_revision: record.stage_revision,
            native_event_sequence: record.last_native_event_sequence,
            object_id: record.object_id,
            subscription_id: record.subscription_id,
            callback_id: record.callback_id,
            event_id: record.event_id,
            delivery: record.delivery,
            coalescing_key: record.coalescing_key,
            flags: record.flags,
        }
    }

    fn from_input(input: &CueInput) -> Self {
        Self {
            stage_id: input.stage_id,
            stage_revision: input.stage_revision,
            native_event_sequence: input.native_event_sequence,
            object_id: input.object_id,
            subscription_id: input.subscription_id,
            callback_id: input.callback_id,
            event_id: input.event_id,
            delivery: input.delivery,
            coalescing_key: input.coalescing_key,
            flags: input.flags,
        }
    }

    fn exact_coalescing_key_matches(self, input: &CueInput) -> bool {
        self.delivery == CueDelivery::LatestValueCoalescible
            && input.delivery == CueDelivery::LatestValueCoalescible
            && self.stage_id == input.stage_id
            && self.stage_revision == input.stage_revision
            && self.object_id == input.object_id
            && self.subscription_id == input.subscription_id
            && self.callback_id == input.callback_id
            && self.event_id == input.event_id
            && self.coalescing_key == input.coalescing_key
            && self.flags == input.flags
    }
}

impl CueReservation<'_> {
    /// Return the Stage whose dispatch owns this reservation.
    pub const fn stage_id(&self) -> StageId {
        self.stage_id
    }

    /// Return the unused ordinary admission count.
    pub const fn remaining_ordinary(&self) -> usize {
        self.remaining_ordinary
    }

    /// Return the unused Critical admission count.
    pub const fn remaining_critical(&self) -> usize {
        self.remaining_critical
    }

    /// Admit one preflighted cue without any queue-storage allocation.
    pub fn enqueue(&mut self, input: CueInput) -> Result<EnqueueOutcome, CueQueueError> {
        if input.stage_id != self.stage_id {
            return Err(CueQueueError::ReservationStageMismatch);
        }

        let remaining = if input.delivery.is_ordinary() {
            &mut self.remaining_ordinary
        } else {
            &mut self.remaining_critical
        };
        if *remaining == 0 {
            return Err(CueQueueError::ReservationClassExhausted);
        }

        let outcome = self.queue.enqueue(input)?;
        *remaining -= 1;
        Ok(outcome)
    }

    /// Explicitly finish this reservation, releasing unused counts.
    pub fn finish(self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StageCausality {
    stage_id: StageId,
    revision: StageRevision,
}

impl CueQueue {
    /// Construct an empty queue with fallible up-front record reservations.
    pub fn new(limits: CueLimits) -> Result<Self, CueQueueError> {
        let mut records = VecDeque::new();
        records
            .try_reserve_exact(limits.total_slots)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut stage_causality = Vec::new();
        stage_causality
            .try_reserve_exact(limits.total_slots)
            .map_err(|_| CueQueueError::AllocationFailed)?;

        Ok(Self {
            limits,
            mutation_revision: 1,
            records,
            stage_causality,
            last_native_event_sequence: None,
            last_input_sequence: None,
            ordinary_records: 0,
            next_sequence: 1,
            state: CueEndpointState::Ready,
            pending_loss: None,
            pending_input_loss: None,
            emergency_fault: None,
        })
    }

    /// Return the endpoint limits governing this queue.
    pub const fn limits(&self) -> CueLimits {
        self.limits
    }

    /// Return the current endpoint queue health.
    pub const fn state(&self) -> CueEndpointState {
        self.state
    }

    /// Borrow the retained out-of-band emergency fault, when present.
    pub const fn emergency_fault(&self) -> Option<&EmergencyFault> {
        self.emergency_fault.as_ref()
    }

    /// Return the ordinary cue-loss range awaiting a Critical notice.
    pub const fn pending_loss(&self) -> Option<PendingCueLoss> {
        self.pending_loss
    }

    /// Return the raw-input loss range awaiting a Critical RuntimeNotice.
    pub const fn pending_input_loss(&self) -> Option<PendingInputLoss> {
        self.pending_input_loss
    }

    /// Return the number of pending records of all classes.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return whether no records are pending.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Return the number of pending ordinary records.
    pub const fn ordinary_len(&self) -> usize {
        self.ordinary_records
    }

    /// Return the number of pending Critical records.
    pub fn critical_len(&self) -> usize {
        self.records.len() - self.ordinary_records
    }

    /// Preflight worst-case cue admission for one native dispatch.
    ///
    /// Queue storage is physically reserved by [`Self::new`]. This method
    /// reserves bounded logical counts and returns an exclusive queue borrow,
    /// preventing another producer from consuming those counts before the
    /// dispatch completes.
    pub fn reserve(
        &mut self,
        admission: CueAdmission,
    ) -> Result<CueReservation<'_>, CueQueueError> {
        match self.state {
            CueEndpointState::Faulted => return Err(CueQueueError::Faulted),
            CueEndpointState::Backpressured if admission.ordinary_slots != 0 => {
                return Err(CueQueueError::AdmissionBackpressured);
            }
            CueEndpointState::Backpressured | CueEndpointState::Ready => {}
        }

        let total = admission
            .ordinary_slots
            .checked_add(admission.critical_slots)
            .ok_or(CueQueueError::AdmissionCapacity)?;
        if self
            .records
            .len()
            .checked_add(total)
            .is_none_or(|needed| needed > self.limits.total_slots)
            || self
                .ordinary_records
                .checked_add(admission.ordinary_slots)
                .is_none_or(|needed| needed > self.limits.ordinary_capacity())
        {
            return Err(CueQueueError::AdmissionCapacity);
        }
        if self
            .ordinary_for_stage(admission.stage_id)
            .checked_add(admission.ordinary_slots)
            .is_none_or(|needed| needed > self.limits.per_stage_ordinary_quota)
        {
            return Err(CueQueueError::AdmissionStageQuota {
                stage_id: admission.stage_id,
            });
        }
        if !self
            .stage_causality
            .iter()
            .any(|stage| stage.stage_id == admission.stage_id)
            && self.stage_causality.len() >= self.limits.total_slots
        {
            return Err(CueQueueError::StageCausalityCapacityExhausted {
                stage_id: admission.stage_id,
            });
        }
        self.preflight_sequence_capacity(total)?;

        Ok(CueReservation {
            queue: self,
            stage_id: admission.stage_id,
            remaining_ordinary: admission.ordinary_slots,
            remaining_critical: admission.critical_slots,
        })
    }

    /// Validate and retain an exact caller-owned input set before Stage mutation.
    ///
    /// Structural, causality, coalescing, quota, record-capacity, and sequence
    /// checks are evaluated in input order, including the effects earlier
    /// inputs would have on later inputs. No queue state changes on failure.
    pub fn prepare_exact_inputs(
        &self,
        inputs: Vec<CueInput>,
    ) -> Result<PreparedCueInputs, CueQueueError> {
        self.prepare_exact_inputs_after_stage_purge(inputs, None)
    }

    /// Prepare a full-Stage ordinary purge and exact Critical teardown inputs.
    ///
    /// Capacity, ordering, causality, payload, and sequence validation observes
    /// the virtual queue that would remain after every ordinary cue for
    /// `stage_id` is removed. RuntimeNotices, Critical cues, and all records for
    /// other Stages retain their relative order. Preparation never mutates the
    /// live queue.
    pub fn prepare_stage_teardown_inputs(
        &self,
        stage_id: StageId,
        inputs: Vec<CueInput>,
    ) -> Result<PreparedStageTeardownCues, CueQueueError> {
        for input in &inputs {
            if input.stage_id != stage_id {
                return Err(CueQueueError::StageTeardownInputStageMismatch {
                    expected: stage_id,
                    offered: input.stage_id,
                });
            }
            if input.delivery != CueDelivery::Critical {
                return Err(CueQueueError::StageTeardownInputMustBeCritical);
            }
        }

        let purged_ordinary = self.stage_ordinary_purge_summary(stage_id);
        let mut purged_records = Vec::new();
        purged_records
            .try_reserve_exact(purged_ordinary.count)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let exact = self.prepare_exact_inputs_after_stage_purge(inputs, Some(stage_id))?;
        Ok(PreparedStageTeardownCues {
            stage_id,
            purged_ordinary,
            purged_records,
            exact,
        })
    }

    fn prepare_exact_inputs_after_stage_purge(
        &self,
        inputs: Vec<CueInput>,
        purged_stage: Option<StageId>,
    ) -> Result<PreparedCueInputs, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        let purged_count = purged_stage
            .map(|stage_id| self.stage_ordinary_purge_summary(stage_id).count)
            .unwrap_or(0);
        if (!inputs.is_empty() || purged_count != 0) && self.mutation_revision == u64::MAX {
            return Err(CueQueueError::MutationRevisionExhausted);
        }
        if self.state == CueEndpointState::Backpressured
            && inputs.iter().any(|input| input.delivery.is_ordinary())
        {
            return Err(CueQueueError::AdmissionBackpressured);
        }
        self.validate_sequence_capacity(inputs.len())?;

        let mut actions = Vec::new();
        actions
            .try_reserve_exact(inputs.len())
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut retired_payloads = Vec::new();
        retired_payloads
            .try_reserve_exact(inputs.len())
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut virtual_causality = Vec::new();
        virtual_causality
            .try_reserve_exact(self.limits.total_slots)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        virtual_causality.extend_from_slice(&self.stage_causality);

        let mut virtual_records = self
            .records
            .len()
            .checked_sub(purged_count)
            .expect("purge count comes from queued records");
        let mut virtual_ordinary = self
            .ordinary_records
            .checked_sub(purged_count)
            .expect("purge count includes only ordinary records");
        let mut virtual_last_native_sequence = self.last_native_event_sequence;
        let mut virtual_tail = self
            .records
            .iter()
            .rev()
            .find(|record| purged_stage.is_none_or(|stage_id| !is_stage_ordinary(record, stage_id)))
            .and_then(EndpointRecord::as_cue)
            .map(VirtualCueTail::from_record);

        for (index, input) in inputs.iter().enumerate() {
            self.validate_input(input)?;
            validate_virtual_causality(
                &virtual_causality,
                virtual_last_native_sequence,
                self.limits.total_slots,
                input,
            )?;

            let exact_tail_match =
                virtual_tail.is_some_and(|tail| tail.exact_coalescing_key_matches(input));
            if exact_tail_match {
                let tail = virtual_tail.expect("exact virtual tail checked above");
                if input.native_event_sequence <= tail.native_event_sequence {
                    return Err(CueQueueError::NonMonotonicCoalescingEventSequence {
                        previous: tail.native_event_sequence,
                        offered: input.native_event_sequence,
                    });
                }
            }

            let sequence = self.sequence_at_offset(index);
            let action = if exact_tail_match {
                PreparedCueAction::Coalesce { sequence }
            } else {
                if input.delivery.is_ordinary() {
                    if virtual_ordinary >= self.limits.ordinary_capacity()
                        || virtual_records >= self.limits.total_slots
                    {
                        return Err(CueQueueError::AdmissionCapacity);
                    }
                    let planned_for_stage = inputs[..index]
                        .iter()
                        .zip(&actions)
                        .filter(|(planned, action)| {
                            planned.stage_id == input.stage_id
                                && planned.delivery.is_ordinary()
                                && matches!(action, PreparedCueAction::Queue { .. })
                        })
                        .count();
                    let purged_for_input_stage = if purged_stage == Some(input.stage_id) {
                        purged_count
                    } else {
                        0
                    };
                    if self
                        .ordinary_for_stage(input.stage_id)
                        .checked_sub(purged_for_input_stage)
                        .expect("Stage purge count matches Stage ordinary records")
                        .checked_add(planned_for_stage)
                        .is_none_or(|count| count >= self.limits.per_stage_ordinary_quota)
                    {
                        return Err(CueQueueError::AdmissionStageQuota {
                            stage_id: input.stage_id,
                        });
                    }
                    virtual_ordinary += 1;
                } else if virtual_records >= self.limits.total_slots {
                    return Err(CueQueueError::AdmissionCapacity);
                }
                virtual_records += 1;
                PreparedCueAction::Queue { sequence }
            };

            record_virtual_causality(&mut virtual_causality, input);
            virtual_last_native_sequence = Some(input.native_event_sequence);
            virtual_tail = Some(VirtualCueTail::from_input(input));
            actions.push(action);
        }

        Ok(PreparedCueInputs {
            queue_revision: self.mutation_revision,
            input_count: inputs.len(),
            inputs,
            actions,
            retired_payloads,
            state: PreparedInputsState::Prepared,
        })
    }

    /// Acquire an exclusive exact-input commit guard before Stage mutation.
    ///
    /// This is the last fallible step: it validates transaction state and queue
    /// freshness, then prevents any queue mutation until the guard is committed
    /// or dropped as a rollback.
    pub fn acquire_exact_commit<'a>(
        &'a mut self,
        prepared: &'a mut PreparedCueInputs,
    ) -> Result<ExactCueCommit<'a>, CueQueueError> {
        if prepared.state != PreparedInputsState::Prepared {
            return Err(CueQueueError::PreparedInputsAlreadyCommitted);
        }
        if prepared.queue_revision != self.mutation_revision {
            return Err(CueQueueError::StalePreparedInputs);
        }
        if prepared.input_count != 0 && self.mutation_revision == u64::MAX {
            return Err(CueQueueError::MutationRevisionExhausted);
        }
        Ok(ExactCueCommit {
            queue: self,
            prepared,
        })
    }

    /// Acquire the exclusive full-Stage cue teardown guard before Stage mutation.
    ///
    /// This is the final fallible queue step. It rejects stale or already
    /// committed preparations and then exclusively borrows the queue so commit
    /// cannot encounter a newly changed capacity, sequence, or causality state.
    pub fn acquire_stage_teardown_commit<'a>(
        &'a mut self,
        prepared: &'a mut PreparedStageTeardownCues,
    ) -> Result<StageTeardownCueCommit<'a>, CueQueueError> {
        if prepared.exact.state != PreparedInputsState::Prepared {
            return Err(CueQueueError::PreparedInputsAlreadyCommitted);
        }
        if prepared.exact.queue_revision != self.mutation_revision {
            return Err(CueQueueError::StalePreparedInputs);
        }
        if (prepared.purged_ordinary.count != 0 || prepared.exact.input_count != 0)
            && self.mutation_revision == u64::MAX
        {
            return Err(CueQueueError::MutationRevisionExhausted);
        }
        Ok(StageTeardownCueCommit {
            queue: self,
            prepared,
        })
    }

    /// Compatibility wrapper that acquires and immediately commits the guard.
    pub fn commit_exact_inputs(
        &mut self,
        prepared: &mut PreparedCueInputs,
    ) -> Result<(), CueQueueError> {
        self.acquire_exact_commit(prepared)?.commit();
        Ok(())
    }

    /// Compatibility wrapper that acquires and commits a Stage teardown guard.
    pub fn commit_stage_teardown_inputs(
        &mut self,
        prepared: &mut PreparedStageTeardownCues,
    ) -> Result<(), CueQueueError> {
        self.acquire_stage_teardown_commit(prepared)?.commit();
        Ok(())
    }

    fn commit_exact_inputs_infallible(&mut self, prepared: &mut PreparedCueInputs) {
        for action in prepared.actions.iter().copied().take(prepared.input_count) {
            let input = prepared.inputs.remove(0);
            let sequence = match action {
                PreparedCueAction::Queue { sequence }
                | PreparedCueAction::Coalesce { sequence } => sequence,
            };
            debug_assert_eq!(self.next_sequence, sequence.get());
            self.record_causality(&input);
            self.next_sequence = self.next_sequence.checked_add(1).unwrap_or(0);

            match action {
                PreparedCueAction::Queue { .. } => {
                    if input.delivery.is_ordinary() {
                        self.ordinary_records += 1;
                    }
                    self.records
                        .push_back(EndpointRecord::Cue(CueRecord::from_input(input, sequence)));
                }
                PreparedCueAction::Coalesce { .. } => {
                    let tail = self
                        .records
                        .back_mut()
                        .and_then(EndpointRecord::as_cue_mut)
                        .expect("prepared exact coalescing tail");
                    debug_assert!(tail.exact_coalescing_key_matches(&input));
                    let retired = tail.replace_with_retained_payload(input, sequence);
                    prepared.retired_payloads.push(retired);
                }
            }
        }
        if prepared.input_count != 0 {
            self.bump_mutation_revision();
        }
        prepared.state = PreparedInputsState::Committed;
    }

    fn commit_stage_teardown_inputs_infallible(
        &mut self,
        prepared: &mut PreparedStageTeardownCues,
    ) {
        let original_len = self.records.len();
        for _ in 0..original_len {
            let record = self
                .records
                .pop_front()
                .expect("bounded original queue length");
            if is_stage_ordinary(&record, prepared.stage_id) {
                prepared.purged_records.push(record);
            } else {
                self.records.push_back(record);
            }
        }
        debug_assert_eq!(
            prepared.purged_records.len(),
            prepared.purged_ordinary.count
        );
        self.ordinary_records -= prepared.purged_ordinary.count;

        let has_exact_inputs = prepared.exact.input_count != 0;
        self.commit_exact_inputs_infallible(&mut prepared.exact);
        if prepared.purged_ordinary.count != 0 && !has_exact_inputs {
            self.bump_mutation_revision();
        }
    }

    /// Release prepared scratch and any displaced coalesced payloads.
    ///
    /// Releasing an uncommitted transaction is the rollback path and leaves
    /// queue state unchanged.
    pub fn release_exact_inputs(&self, prepared: PreparedCueInputs) {
        drop(prepared);
    }

    /// Release Stage-teardown scratch and retained purged/displaced storage.
    ///
    /// Releasing an uncommitted preparation is an exact rollback because
    /// preparation and guard acquisition do not mutate queue state.
    pub fn release_stage_teardown_inputs(&self, prepared: PreparedStageTeardownCues) {
        drop(prepared);
    }

    /// Admit one cue or merge it into an exact matching tail record.
    ///
    /// Every structurally valid emission receives an endpoint-wide sequence,
    /// including ordinary emissions rejected by quota or capacity. That lets
    /// an overflow notice describe the exact lost range without silent gaps.
    pub fn enqueue(&mut self, input: CueInput) -> Result<EnqueueOutcome, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        self.validate_input(&input)?;
        self.validate_causality_for_input(&input)?;

        let exact_tail_match = input.delivery == CueDelivery::LatestValueCoalescible
            && self
                .records
                .back()
                .and_then(EndpointRecord::as_cue)
                .is_some_and(|tail| tail.exact_coalescing_key_matches(&input));
        if exact_tail_match {
            let tail = self
                .records
                .back()
                .and_then(EndpointRecord::as_cue)
                .expect("cue tail checked above");
            if input.native_event_sequence <= tail.last_native_event_sequence {
                return Err(CueQueueError::NonMonotonicCoalescingEventSequence {
                    previous: tail.last_native_event_sequence,
                    offered: input.native_event_sequence,
                });
            }
        }

        self.record_causality(&input);
        let sequence = self.allocate_sequence()?;

        if input.delivery.is_ordinary() && self.state == CueEndpointState::Backpressured {
            self.record_loss(sequence);
            self.bump_mutation_revision();
            return Err(CueQueueError::OrdinaryBackpressured { sequence });
        }

        if exact_tail_match {
            let tail = self
                .records
                .back_mut()
                .and_then(EndpointRecord::as_cue_mut)
                .expect("cue tail checked above");
            tail.replace_with(input, sequence);
            let outcome = EnqueueOutcome::Coalesced {
                first_sequence: tail.first_sequence,
                last_sequence: tail.last_sequence,
                merge_count: tail.merge_count,
            };
            self.bump_mutation_revision();
            return Ok(outcome);
        }

        if input.delivery.is_ordinary() {
            if self.ordinary_records >= self.limits.ordinary_capacity()
                || self.records.len() >= self.limits.total_slots
            {
                self.record_loss(sequence);
                self.bump_mutation_revision();
                return Err(CueQueueError::OrdinaryCapacityExhausted { sequence });
            }
            if self.ordinary_for_stage(input.stage_id) >= self.limits.per_stage_ordinary_quota {
                self.record_loss(sequence);
                self.bump_mutation_revision();
                return Err(CueQueueError::StageQuotaExhausted {
                    sequence,
                    stage_id: input.stage_id,
                });
            }
            self.ordinary_records += 1;
        } else if self.records.len() >= self.limits.total_slots {
            self.fault_critical_capacity(sequence, input.stage_id, input.event_id);
            self.bump_mutation_revision();
            return Err(CueQueueError::CriticalCapacityExhausted { sequence });
        }

        self.records
            .push_back(EndpointRecord::Cue(CueRecord::from_input(input, sequence)));
        self.bump_mutation_revision();
        Ok(EnqueueOutcome::Queued { sequence })
    }

    /// Admit the Critical overflow notice that resolves a pending loss barrier.
    ///
    /// The caller obtains the exact range from [`Self::pending_loss`] and owns
    /// the descriptor-specific notice payload. Ordinary admissions remain
    /// gated until this method appends the notice. Failure to fit the notice
    /// transitions the endpoint to [`CueEndpointState::Faulted`].
    pub fn enqueue_pending_loss_notice(
        &mut self,
        input: CueInput,
    ) -> Result<OverflowNoticeOutcome, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        let loss = self.pending_loss.ok_or(CueQueueError::NoPendingCueLoss)?;
        if input.delivery != CueDelivery::Critical {
            return Err(CueQueueError::OverflowNoticeMustBeCritical);
        }

        self.validate_input(&input)?;
        self.validate_causality_for_input(&input)?;
        self.record_causality(&input);
        let notice_sequence = self.allocate_sequence()?;
        if self.records.len() >= self.limits.total_slots {
            self.fault_critical_capacity(notice_sequence, input.stage_id, input.event_id);
            self.bump_mutation_revision();
            return Err(CueQueueError::CriticalCapacityExhausted {
                sequence: notice_sequence,
            });
        }

        self.records
            .push_back(EndpointRecord::Cue(CueRecord::from_input(
                input,
                notice_sequence,
            )));
        self.pending_loss = None;
        if self.pending_input_loss.is_none() {
            self.state = CueEndpointState::Ready;
        }
        self.bump_mutation_revision();
        Ok(OverflowNoticeOutcome {
            loss,
            notice_sequence,
        })
    }

    /// Record one raw input rejected before native dispatch.
    ///
    /// The endpoint must stop dispatching while this barrier is pending. A
    /// subsequent sequence for the same Stage and input class extends the
    /// bounded range; a different range is rejected so loss is never merged
    /// ambiguously.
    pub fn record_input_overflow(
        &mut self,
        stage_id: StageId,
        input_class: InputClass,
        sequence: InputSequence,
    ) -> Result<PendingInputLoss, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        if let Some(previous) = self
            .last_input_sequence
            .filter(|previous| sequence <= *previous)
        {
            return Err(CueQueueError::InputSequenceRegressed {
                previous,
                offered: sequence,
            });
        }

        match &mut self.pending_input_loss {
            Some(loss) if loss.stage_id == stage_id && loss.input_class == input_class => {
                loss.extend(sequence)?;
            }
            Some(_) => return Err(CueQueueError::InputBackpressured),
            None => {
                self.pending_input_loss =
                    Some(PendingInputLoss::first(stage_id, input_class, sequence));
            }
        }
        self.last_input_sequence = Some(sequence);
        self.state = CueEndpointState::Backpressured;
        self.bump_mutation_revision();
        Ok(self
            .pending_input_loss
            .expect("input loss was inserted or extended"))
    }

    /// Queue the Critical RuntimeNotice that resolves a raw-input loss barrier.
    ///
    /// `payload` is caller-owned, preallocated detail storage. It is moved into
    /// queue-owned storage without copying or allocating, while the fixed typed
    /// loss metadata is encoded only when the record is serialized.
    pub fn enqueue_pending_input_overflow_notice(
        &mut self,
        payload: Vec<u8>,
    ) -> Result<InputOverflowNoticeOutcome, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        let loss = self
            .pending_input_loss
            .ok_or(CueQueueError::NoPendingInputLoss)?;
        self.validate_runtime_notice_payload(payload.len())?;

        let notice_sequence = self.allocate_sequence()?;
        if self.records.len() >= self.limits.total_slots {
            self.state = CueEndpointState::Faulted;
            self.emergency_fault = Some(EmergencyFault::CriticalNoticeCapacityExhausted {
                sequence: notice_sequence,
                kind: RUNTIME_NOTICE_INPUT_OVERFLOW,
            });
            self.bump_mutation_revision();
            return Err(CueQueueError::CriticalNoticeCapacityExhausted {
                sequence: notice_sequence,
                kind: RUNTIME_NOTICE_INPUT_OVERFLOW,
            });
        }

        self.records.push_back(EndpointRecord::RuntimeNotice(
            RuntimeNoticeRecord::input_overflow(notice_sequence, loss, payload),
        ));
        self.pending_input_loss = None;
        if self.pending_loss.is_none() {
            self.state = CueEndpointState::Ready;
        }
        self.bump_mutation_revision();
        Ok(InputOverflowNoticeOutcome {
            loss,
            notice_sequence,
        })
    }

    /// Remove ordinary records for a Stage while preserving every Critical
    /// cue, RuntimeNotice, and surviving record's relative order.
    pub fn remove_stage_ordinary(&mut self, stage_id: StageId) -> RemovedStageOrdinary {
        let mut removed = RemovedStageOrdinary::default();
        self.records.retain(|record| {
            let remove = matches!(
                record,
                EndpointRecord::Cue(cue)
                    if cue.stage_id == stage_id && cue.delivery.is_ordinary()
            );
            if remove {
                let sequence = record.sequence();
                removed.count += 1;
                removed.first_sequence.get_or_insert(sequence);
                removed.last_sequence = Some(sequence);
            }
            !remove
        });
        self.ordinary_records -= removed.count;
        if removed.count != 0 {
            self.bump_mutation_revision();
        }
        removed
    }

    fn stage_ordinary_purge_summary(&self, stage_id: StageId) -> RemovedStageOrdinary {
        let mut removed = RemovedStageOrdinary::default();
        for record in self
            .records
            .iter()
            .filter(|record| is_stage_ordinary(record, stage_id))
        {
            let sequence = record.sequence();
            removed.count += 1;
            removed.first_sequence.get_or_insert(sequence);
            removed.last_sequence = Some(sequence);
        }
        removed
    }

    /// Retire one Stage from the bounded causality history after teardown.
    ///
    /// The call is idempotent for an already-retired Stage. It returns Busy
    /// while the Stage owns any queued cue or RuntimeNotice, while its raw-input
    /// loss notice is pending, or while an unqualified ordinary loss barrier
    /// could still belong to it. The endpoint must also call this only after
    /// records already returned by a drain, especially subscription releases,
    /// have been handled at the VM-safe boundary. Stage identity reuse within
    /// an endpoint epoch remains forbidden by the actor registry.
    pub fn finalize_stage(&mut self, stage_id: StageId) -> Result<bool, CueQueueError> {
        if self.state == CueEndpointState::Faulted {
            return Err(CueQueueError::Faulted);
        }
        let has_queued_record = self.records.iter().any(|record| match record {
            EndpointRecord::Cue(cue) => cue.stage_id == stage_id,
            EndpointRecord::RuntimeNotice(notice) => notice.input_loss.stage_id == stage_id,
        });
        let has_pending_input_loss = self
            .pending_input_loss
            .is_some_and(|loss| loss.stage_id == stage_id);
        if has_queued_record || has_pending_input_loss || self.pending_loss.is_some() {
            return Err(CueQueueError::StageFinalizeBusy { stage_id });
        }

        let Some(position) = self
            .stage_causality
            .iter()
            .position(|stage| stage.stage_id == stage_id)
        else {
            return Ok(false);
        };
        self.stage_causality.remove(position);
        self.bump_mutation_revision();
        Ok(true)
    }

    /// Remove a cue-only prefix bounded by cue count and frame bytes.
    ///
    /// If the head record alone exceeds the byte budget, no later record may
    /// bypass it. A RuntimeNotice at the head also stops this compatibility
    /// drain so it cannot reorder cues around the notice; new endpoint code
    /// should use [`Self::drain_endpoint`]. Output storage is reserved before
    /// the first pop, so an allocation failure leaves queue state unchanged.
    pub fn drain(&mut self, budget: DrainBudget) -> Result<DrainedCues, CueQueueError> {
        if self.state == CueEndpointState::Backpressured {
            return Err(CueQueueError::DrainBackpressured);
        }
        let (drain_count, drain_bytes) = self.cue_drain_extent(budget);
        let mut cues = Vec::new();
        cues.try_reserve_exact(drain_count)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut frame_bytes = 0usize;

        while cues.len() < drain_count {
            let EndpointRecord::Cue(cue) = self.records.pop_front().expect("front checked above")
            else {
                unreachable!("cue drain extent stops before RuntimeNotice records");
            };
            if cue.delivery.is_ordinary() {
                self.ordinary_records -= 1;
            }
            frame_bytes += cue.frame_bytes();
            cues.push(cue);
        }

        debug_assert_eq!(frame_bytes, drain_bytes);
        if drain_count != 0 {
            self.bump_mutation_revision();
        }
        Ok(DrainedCues { cues, frame_bytes })
    }

    /// Remove one globally ordered endpoint prefix of cues and RuntimeNotices.
    ///
    /// Output storage is reserved before the first pop. Queue-owned records and
    /// their caller-preallocated payload buffers are then moved without further
    /// allocation.
    pub fn drain_endpoint(
        &mut self,
        budget: DrainBudget,
    ) -> Result<DrainedEndpointRecords, CueQueueError> {
        if self.state == CueEndpointState::Backpressured {
            return Err(CueQueueError::DrainBackpressured);
        }
        let (drain_count, drain_bytes) = self.endpoint_drain_extent(budget);
        let mut records = Vec::new();
        records
            .try_reserve_exact(drain_count)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut frame_bytes = 0usize;

        while records.len() < drain_count {
            let record = self.records.pop_front().expect("front checked above");
            if record.is_ordinary() {
                self.ordinary_records -= 1;
            }
            frame_bytes += record.frame_bytes();
            records.push(record);
        }

        debug_assert_eq!(frame_bytes, drain_bytes);
        if drain_count != 0 {
            self.bump_mutation_revision();
        }
        Ok(DrainedEndpointRecords {
            records,
            frame_bytes,
        })
    }

    /// Reset queue state for a newly established endpoint epoch.
    ///
    /// All pending cues, sequence history, and emergency state are invalidated
    /// together as required by endpoint replacement.
    pub fn reset_epoch(&mut self) {
        self.records.clear();
        self.stage_causality.clear();
        self.last_native_event_sequence = None;
        self.last_input_sequence = None;
        self.ordinary_records = 0;
        self.next_sequence = 1;
        self.state = CueEndpointState::Ready;
        self.pending_loss = None;
        self.pending_input_loss = None;
        self.emergency_fault = None;
        self.bump_mutation_revision();
    }

    fn validate_input(&self, input: &CueInput) -> Result<(), CueQueueError> {
        match (input.delivery, input.coalescing_key) {
            (CueDelivery::LatestValueCoalescible, None) => {
                return Err(CueQueueError::MissingCoalescingKey);
            }
            (CueDelivery::Critical | CueDelivery::Ordered, Some(_)) => {
                return Err(CueQueueError::UnexpectedCoalescingKey);
            }
            _ => {}
        }
        if input.flags & CUE_FLAG_SUBSCRIPTION_RELEASE != 0
            && input.delivery != CueDelivery::Critical
        {
            return Err(CueQueueError::SubscriptionReleaseMustBeCritical);
        }

        let payload_bytes = input.payload.len();
        if payload_bytes > self.limits.max_payload_bytes {
            return Err(CueQueueError::PayloadTooLarge {
                actual: payload_bytes,
                maximum: self.limits.max_payload_bytes,
            });
        }
        let frame_bytes = CUE_FRAME_OVERHEAD_BYTES + payload_bytes;
        if frame_bytes > self.limits.max_frame_bytes {
            return Err(CueQueueError::FrameTooLarge {
                actual: frame_bytes,
                maximum: self.limits.max_frame_bytes,
            });
        }
        Ok(())
    }

    fn validate_runtime_notice_payload(&self, payload_bytes: usize) -> Result<(), CueQueueError> {
        if payload_bytes > self.limits.max_payload_bytes {
            return Err(CueQueueError::PayloadTooLarge {
                actual: payload_bytes,
                maximum: self.limits.max_payload_bytes,
            });
        }
        let frame_bytes = RUNTIME_NOTICE_FRAME_FIXED_BYTES
            .checked_add(INPUT_OVERFLOW_METADATA_BYTES)
            .and_then(|overhead| overhead.checked_add(payload_bytes))
            .ok_or(CueQueueError::FrameTooLarge {
                actual: usize::MAX,
                maximum: self.limits.max_frame_bytes,
            })?;
        if frame_bytes > self.limits.max_frame_bytes {
            return Err(CueQueueError::FrameTooLarge {
                actual: frame_bytes,
                maximum: self.limits.max_frame_bytes,
            });
        }
        Ok(())
    }

    fn preflight_sequence_capacity(&mut self, requested: usize) -> Result<(), CueQueueError> {
        if self.validate_sequence_capacity(requested).is_ok() {
            return Ok(());
        }
        self.state = CueEndpointState::Faulted;
        self.emergency_fault = Some(EmergencyFault::SequenceExhausted);
        self.bump_mutation_revision();
        Err(CueQueueError::SequenceExhausted)
    }

    fn validate_sequence_capacity(&self, requested: usize) -> Result<(), CueQueueError> {
        if requested == 0 {
            return Ok(());
        }
        let available = if self.next_sequence == 0 {
            0
        } else {
            u64::from(u32::MAX - self.next_sequence) + 1
        };
        let requested = u64::try_from(requested).unwrap_or(u64::MAX);
        if requested > available {
            return Err(CueQueueError::SequenceExhausted);
        }
        Ok(())
    }

    fn sequence_at_offset(&self, offset: usize) -> CueSequence {
        let offset = u32::try_from(offset).expect("sequence capacity validated");
        CueSequence(
            self.next_sequence
                .checked_add(offset)
                .expect("sequence capacity validated"),
        )
    }

    fn bump_mutation_revision(&mut self) {
        self.mutation_revision = self.mutation_revision.saturating_add(1);
    }

    fn allocate_sequence(&mut self) -> Result<CueSequence, CueQueueError> {
        if self.next_sequence == 0 {
            self.state = CueEndpointState::Faulted;
            self.emergency_fault = Some(EmergencyFault::SequenceExhausted);
            self.bump_mutation_revision();
            return Err(CueQueueError::SequenceExhausted);
        }

        let sequence = CueSequence(self.next_sequence);
        self.next_sequence = self.next_sequence.checked_add(1).unwrap_or(0);
        Ok(sequence)
    }

    fn validate_causality_for_input(&mut self, input: &CueInput) -> Result<(), CueQueueError> {
        let result = self.validate_causality(input);
        if matches!(
            result,
            Err(CueQueueError::StageCausalityCapacityExhausted { .. })
        ) && input.delivery == CueDelivery::Critical
        {
            self.state = CueEndpointState::Faulted;
            self.emergency_fault = Some(EmergencyFault::CriticalStageCausalityCapacityExhausted {
                stage_id: input.stage_id,
                event_id: input.event_id,
            });
            self.bump_mutation_revision();
        }
        result
    }

    fn validate_causality(&self, input: &CueInput) -> Result<(), CueQueueError> {
        if let Some(stage) = self
            .stage_causality
            .iter()
            .find(|stage| stage.stage_id == input.stage_id)
        {
            if input.stage_revision < stage.revision {
                return Err(CueQueueError::StageRevisionRegressed {
                    stage_id: input.stage_id,
                    previous: stage.revision,
                    offered: input.stage_revision,
                });
            }
        } else if self.stage_causality.len() >= self.limits.total_slots {
            return Err(CueQueueError::StageCausalityCapacityExhausted {
                stage_id: input.stage_id,
            });
        }

        if let Some(previous) = self
            .last_native_event_sequence
            .filter(|previous| input.native_event_sequence < *previous)
        {
            return Err(CueQueueError::NativeEventSequenceRegressed {
                previous,
                offered: input.native_event_sequence,
            });
        }
        Ok(())
    }

    fn record_causality(&mut self, input: &CueInput) {
        if let Some(stage) = self
            .stage_causality
            .iter_mut()
            .find(|stage| stage.stage_id == input.stage_id)
        {
            stage.revision = input.stage_revision;
        } else {
            debug_assert!(self.stage_causality.len() < self.limits.total_slots);
            self.stage_causality.push(StageCausality {
                stage_id: input.stage_id,
                revision: input.stage_revision,
            });
        }
        self.last_native_event_sequence = Some(input.native_event_sequence);
    }

    fn record_loss(&mut self, sequence: CueSequence) {
        match &mut self.pending_loss {
            Some(loss) => loss.extend(sequence),
            None => self.pending_loss = Some(PendingCueLoss::first(sequence)),
        }
        self.state = CueEndpointState::Backpressured;
    }

    fn fault_critical_capacity(
        &mut self,
        sequence: CueSequence,
        stage_id: StageId,
        event_id: EventId,
    ) {
        self.state = CueEndpointState::Faulted;
        self.emergency_fault = Some(EmergencyFault::CriticalCapacityExhausted {
            sequence,
            stage_id,
            event_id,
        });
    }

    fn endpoint_drain_extent(&self, budget: DrainBudget) -> (usize, usize) {
        let mut count = 0usize;
        let mut bytes = 0usize;
        for record in &self.records {
            if count >= budget.max_cues {
                break;
            }
            let Some(next_bytes) = bytes.checked_add(record.frame_bytes()) else {
                break;
            };
            if next_bytes > budget.max_bytes {
                break;
            }
            count += 1;
            bytes = next_bytes;
        }
        (count, bytes)
    }

    fn cue_drain_extent(&self, budget: DrainBudget) -> (usize, usize) {
        let mut count = 0usize;
        let mut bytes = 0usize;
        for record in &self.records {
            let Some(cue) = record.as_cue() else {
                break;
            };
            if count >= budget.max_cues {
                break;
            }
            let Some(next_bytes) = bytes.checked_add(cue.frame_bytes()) else {
                break;
            };
            if next_bytes > budget.max_bytes {
                break;
            }
            count += 1;
            bytes = next_bytes;
        }
        (count, bytes)
    }

    fn ordinary_for_stage(&self, stage_id: StageId) -> usize {
        self.records
            .iter()
            .filter(|record| {
                record
                    .as_cue()
                    .is_some_and(|cue| cue.delivery.is_ordinary() && cue.stage_id == stage_id)
            })
            .count()
    }
}

fn validate_virtual_causality(
    stage_causality: &[StageCausality],
    last_native_event_sequence: Option<NativeEventSequence>,
    stage_capacity: usize,
    input: &CueInput,
) -> Result<(), CueQueueError> {
    if let Some(stage) = stage_causality
        .iter()
        .find(|stage| stage.stage_id == input.stage_id)
    {
        if input.stage_revision < stage.revision {
            return Err(CueQueueError::StageRevisionRegressed {
                stage_id: input.stage_id,
                previous: stage.revision,
                offered: input.stage_revision,
            });
        }
    } else if stage_causality.len() >= stage_capacity {
        return Err(CueQueueError::StageCausalityCapacityExhausted {
            stage_id: input.stage_id,
        });
    }

    if let Some(previous) =
        last_native_event_sequence.filter(|previous| input.native_event_sequence < *previous)
    {
        return Err(CueQueueError::NativeEventSequenceRegressed {
            previous,
            offered: input.native_event_sequence,
        });
    }
    Ok(())
}

fn record_virtual_causality(stage_causality: &mut Vec<StageCausality>, input: &CueInput) {
    if let Some(stage) = stage_causality
        .iter_mut()
        .find(|stage| stage.stage_id == input.stage_id)
    {
        stage.revision = input.stage_revision;
    } else {
        stage_causality.push(StageCausality {
            stage_id: input.stage_id,
            revision: input.stage_revision,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn critical_input(stage_id: StageId) -> CueInput {
        CueInput::new(
            CueIdentity::new(
                stage_id,
                ObjectId::new((1u64 << 32) | 1).unwrap(),
                SubscriptionId::new(1).unwrap(),
                CallbackId::new(1).unwrap(),
                EventId::new(1).unwrap(),
            ),
            StageRevision::new(1),
            NativeEventSequence::new(1).unwrap(),
            CueDelivery::Critical,
            Vec::new(),
        )
    }

    #[test]
    fn exact_preflight_checks_sequence_space_without_faulting_or_mutating() {
        let limits = CueLimits::new(4, 1, 3, 0, CUE_FRAME_OVERHEAD_BYTES).unwrap();
        let stage_id = StageId::new(1).unwrap();
        let mut queue = CueQueue::new(limits).unwrap();
        queue.next_sequence = u32::MAX;

        assert!(matches!(
            queue.prepare_exact_inputs(vec![critical_input(stage_id), critical_input(stage_id)]),
            Err(CueQueueError::SequenceExhausted)
        ));
        assert!(queue.is_empty());
        assert_eq!(queue.state(), CueEndpointState::Ready);

        let prepared = queue
            .prepare_exact_inputs(vec![critical_input(stage_id)])
            .unwrap();
        assert!(queue.is_empty());
        queue.release_exact_inputs(prepared);
        assert_eq!(queue.next_sequence, u32::MAX);
    }

    #[test]
    fn exact_transaction_revision_never_wraps_or_revalidates_work() {
        let limits = CueLimits::new(4, 1, 3, 0, CUE_FRAME_OVERHEAD_BYTES).unwrap();
        let stage_id = StageId::new(1).unwrap();
        let mut queue = CueQueue::new(limits).unwrap();
        queue.mutation_revision = u64::MAX;

        assert!(matches!(
            queue.prepare_exact_inputs(vec![critical_input(stage_id)]),
            Err(CueQueueError::MutationRevisionExhausted)
        ));
        assert!(queue.is_empty());
        assert_eq!(queue.mutation_revision, u64::MAX);
        assert_eq!(queue.state(), CueEndpointState::Ready);
    }

    #[test]
    fn reservation_preflights_sequence_exhaustion_before_admission() {
        let limits = CueLimits::new(4, 1, 3, 0, CUE_FRAME_OVERHEAD_BYTES).unwrap();
        let stage_id = StageId::new(1).unwrap();
        let mut queue = CueQueue::new(limits).unwrap();
        queue.next_sequence = u32::MAX;

        assert!(matches!(
            queue.reserve(CueAdmission {
                stage_id,
                ordinary_slots: 1,
                critical_slots: 1,
            }),
            Err(CueQueueError::SequenceExhausted)
        ));
        assert!(queue.is_empty());
        assert_eq!(queue.state(), CueEndpointState::Faulted);
        assert_eq!(
            queue.emergency_fault(),
            Some(&EmergencyFault::SequenceExhausted)
        );

        queue.reset_epoch();
        queue.next_sequence = u32::MAX;
        let input = critical_input(stage_id);
        let mut reservation = queue
            .reserve(CueAdmission {
                stage_id,
                ordinary_slots: 0,
                critical_slots: 1,
            })
            .unwrap();
        assert_eq!(
            reservation.enqueue(input),
            Ok(EnqueueOutcome::Queued {
                sequence: CueSequence::new(u32::MAX).unwrap(),
            })
        );
        reservation.finish();

        assert!(matches!(
            queue.reserve(CueAdmission {
                stage_id,
                ordinary_slots: 0,
                critical_slots: 1,
            }),
            Err(CueQueueError::SequenceExhausted)
        ));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.state(), CueEndpointState::Faulted);
    }
}
