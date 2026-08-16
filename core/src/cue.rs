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
            payload,
        }
    }

    /// Attach the descriptor-supplied discriminator required for coalescing.
    pub fn with_coalescing_key(mut self, key: CoalescingKey) -> Self {
        self.coalescing_key = Some(key);
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
    }

    fn replace_with(&mut self, input: CueInput, sequence: CueSequence) {
        debug_assert!(self.exact_coalescing_key_matches(&input));
        self.last_sequence = sequence;
        self.last_native_event_sequence = input.native_event_sequence;
        self.merge_count += 1;
        self.payload = input.payload;
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
        let mut flags = CUE_FLAG_MPY05_METADATA;
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

/// Cue admission failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CueQueueError {
    /// A queue-owned reservation could not be satisfied without panicking.
    AllocationFailed,
    /// The endpoint is faulted and requires an epoch reset or recovery action.
    Faulted,
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
    /// A Critical record could not be admitted and faulted the endpoint.
    CriticalCapacityExhausted {
        /// Sequence allocated to the failed Critical emission.
        sequence: CueSequence,
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

/// One endpoint-owned sequence-ordered cue queue.
pub struct CueQueue {
    limits: CueLimits,
    records: VecDeque<CueRecord>,
    stage_causality: Vec<StageCausality>,
    last_native_event_sequence: Option<NativeEventSequence>,
    ordinary_records: usize,
    next_sequence: u32,
    state: CueEndpointState,
    pending_loss: Option<PendingCueLoss>,
    emergency_fault: Option<EmergencyFault>,
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
            records,
            stage_causality,
            last_native_event_sequence: None,
            ordinary_records: 0,
            next_sequence: 1,
            state: CueEndpointState::Ready,
            pending_loss: None,
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
                .is_some_and(|tail| tail.exact_coalescing_key_matches(&input));
        if exact_tail_match {
            let tail = self.records.back().expect("tail checked above");
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
            return Err(CueQueueError::OrdinaryBackpressured { sequence });
        }

        if exact_tail_match {
            let tail = self.records.back_mut().expect("tail checked above");
            tail.replace_with(input, sequence);
            return Ok(EnqueueOutcome::Coalesced {
                first_sequence: tail.first_sequence,
                last_sequence: tail.last_sequence,
                merge_count: tail.merge_count,
            });
        }

        if input.delivery.is_ordinary() {
            if self.ordinary_records >= self.limits.ordinary_capacity()
                || self.records.len() >= self.limits.total_slots
            {
                self.record_loss(sequence);
                return Err(CueQueueError::OrdinaryCapacityExhausted { sequence });
            }
            if self.ordinary_for_stage(input.stage_id) >= self.limits.per_stage_ordinary_quota {
                self.record_loss(sequence);
                return Err(CueQueueError::StageQuotaExhausted {
                    sequence,
                    stage_id: input.stage_id,
                });
            }
            self.ordinary_records += 1;
        } else if self.records.len() >= self.limits.total_slots {
            self.fault_critical_capacity(sequence, input.stage_id, input.event_id);
            return Err(CueQueueError::CriticalCapacityExhausted { sequence });
        }

        self.records
            .push_back(CueRecord::from_input(input, sequence));
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
            return Err(CueQueueError::CriticalCapacityExhausted {
                sequence: notice_sequence,
            });
        }

        self.records
            .push_back(CueRecord::from_input(input, notice_sequence));
        self.pending_loss = None;
        self.state = CueEndpointState::Ready;
        Ok(OverflowNoticeOutcome {
            loss,
            notice_sequence,
        })
    }

    /// Remove a globally ordered prefix bounded by cue count and frame bytes.
    ///
    /// If the head record alone exceeds the byte budget, no later record may
    /// bypass it; the drain returns empty and preserves queue order. Output
    /// storage is reserved before the first pop, so an allocation failure
    /// leaves queue contents, counters, and endpoint state unchanged.
    pub fn drain(&mut self, budget: DrainBudget) -> Result<DrainedCues, CueQueueError> {
        if self.state == CueEndpointState::Backpressured {
            return Err(CueQueueError::DrainBackpressured);
        }
        let (drain_count, drain_bytes) = self.drain_extent(budget);
        let mut cues = Vec::new();
        cues.try_reserve_exact(drain_count)
            .map_err(|_| CueQueueError::AllocationFailed)?;
        let mut frame_bytes = 0usize;

        while cues.len() < drain_count {
            let cue = self.records.pop_front().expect("front checked above");
            if cue.delivery.is_ordinary() {
                self.ordinary_records -= 1;
            }
            frame_bytes += cue.frame_bytes();
            cues.push(cue);
        }

        debug_assert_eq!(frame_bytes, drain_bytes);
        Ok(DrainedCues { cues, frame_bytes })
    }

    /// Reset queue state for a newly established endpoint epoch.
    ///
    /// All pending cues, sequence history, and emergency state are invalidated
    /// together as required by endpoint replacement.
    pub fn reset_epoch(&mut self) {
        self.records.clear();
        self.stage_causality.clear();
        self.last_native_event_sequence = None;
        self.ordinary_records = 0;
        self.next_sequence = 1;
        self.state = CueEndpointState::Ready;
        self.pending_loss = None;
        self.emergency_fault = None;
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

    fn allocate_sequence(&mut self) -> Result<CueSequence, CueQueueError> {
        if self.next_sequence == 0 {
            self.state = CueEndpointState::Faulted;
            self.emergency_fault = Some(EmergencyFault::SequenceExhausted);
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

        if let Some(previous) = self.last_native_event_sequence
            && input.native_event_sequence < previous
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

    fn drain_extent(&self, budget: DrainBudget) -> (usize, usize) {
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

    fn ordinary_for_stage(&self, stage_id: StageId) -> usize {
        self.records
            .iter()
            .filter(|record| record.delivery.is_ordinary() && record.stage_id == stage_id)
            .count()
    }
}
