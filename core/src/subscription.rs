//! Bounded MPY native-event subscriptions and descriptor-derived cue inputs.
//!
//! This registry owns validated routing metadata and opaque callback tokens;
//! it never calls a language runtime. Observation is deliberately two-phase:
//! an endpoint reserves all cue and payload storage before native dispatch,
//! then completes the workspace allocation-free after widget semantics run.

use alloc::vec::Vec;

use crate::{
    actor::{
        ActorEventHandle, ActorIdentity, EventDelivery, EventDescriptor, EventFilterSet,
        NativeMutationPublication, ObjectId, RegistryError, StageId, StageRegistry,
    },
    cue::{
        CallbackId, CoalescingKey, CueAdmission, CueDelivery, CueIdentity, CueInput, EventId,
        NativeEventSequence, SubscriptionId,
    },
    direction::StageRevision,
    object::{DispatchPhase, NativeObserverControl, ObjectEvent},
    widget::Rect,
};

/// Stable nonzero identity for one endpoint lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EndpointEpoch(u32);

impl EndpointEpoch {
    /// Construct an endpoint epoch, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized representation.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Validated limits for one endpoint subscription registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubscriptionLimits {
    max_subscriptions: usize,
    max_event_payload_bytes: usize,
    max_tombstones: usize,
    max_observation_emissions: usize,
}

impl SubscriptionLimits {
    /// Validate registry, completion-window, and preflight capacities.
    pub fn new(
        max_subscriptions: usize,
        max_event_payload_bytes: usize,
        max_tombstones: usize,
        max_observation_emissions: usize,
    ) -> Result<Self, SubscriptionError> {
        if max_subscriptions == 0
            || max_subscriptions > u32::MAX as usize
            || max_event_payload_bytes > u32::MAX as usize
            || max_tombstones == 0
            || max_tombstones > max_subscriptions
            || max_observation_emissions == 0
            || max_observation_emissions > max_subscriptions
        {
            return Err(SubscriptionError::InvalidLimits);
        }
        Ok(Self {
            max_subscriptions,
            max_event_payload_bytes,
            max_tombstones,
            max_observation_emissions,
        })
    }

    /// Return the endpoint-wide simultaneous-subscription capacity.
    pub const fn max_subscriptions(self) -> usize {
        self.max_subscriptions
    }

    /// Return the largest descriptor payload accepted by this endpoint.
    pub const fn max_event_payload_bytes(self) -> usize {
        self.max_event_payload_bytes
    }

    /// Return the bounded idempotent-close completion window.
    pub const fn max_tombstones(self) -> usize {
        self.max_tombstones
    }

    /// Return the largest preflight workspace for one native observation.
    pub const fn max_observation_emissions(self) -> usize {
        self.max_observation_emissions
    }
}

/// Predeclared native propagation behavior for one subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagationPolicy {
    /// Observe without changing native propagation.
    Observe,
    /// Stop after an emitted target observation when the descriptor permits it.
    ConsumeAtTarget,
    /// Stop after an emitted selected phase when the descriptor permits it.
    StopAfterPhase,
    /// Reserved policy unsupported by the MPY-05 minimum profile.
    PreventDefault,
}

/// Bounded filter installed before native dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriptionFilter {
    /// Match every descriptor-qualified source observation.
    Any,
    /// Match pointer coordinates inside a positive-size logical rectangle.
    PointerRegion(Rect),
}

impl SubscriptionFilter {
    fn descriptor_bit(self) -> EventFilterSet {
        match self {
            Self::Any => EventFilterSet::ANY,
            Self::PointerRegion(_) => EventFilterSet::POINTER_REGION,
        }
    }

    fn validate(self) -> Result<(), SubscriptionError> {
        match self {
            Self::Any => Ok(()),
            Self::PointerRegion(region) if region.width > 0 && region.height > 0 => Ok(()),
            Self::PointerRegion(_) => Err(SubscriptionError::InvalidFilter),
        }
    }

    fn matches(self, event: &ObjectEvent) -> bool {
        match self {
            Self::Any => true,
            Self::PointerRegion(region) => {
                let ObjectEvent::Clicked { x, y } = event else {
                    return false;
                };
                let left = i64::from(region.x);
                let top = i64::from(region.y);
                let right = left + i64::from(region.width);
                let bottom = top + i64::from(region.height);
                let x = i64::from(*x);
                let y = i64::from(*y);
                x >= left && x < right && y >= top && y < bottom
            }
        }
    }
}

/// Fully specified subscription installation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubscribeRequest {
    /// Stage identity selected by the endpoint.
    pub stage_id: StageId,
    /// Exact generation-checked actor identity.
    pub actor_identity: ActorIdentity,
    /// Descriptor event identifier.
    pub event_id: EventId,
    /// Unique active callback token retained for later VM-safe delivery.
    pub callback_id: CallbackId,
    /// Native propagation phase to observe.
    pub phase: DispatchPhase,
    /// Predeclared bounded filter.
    pub filter: SubscriptionFilter,
    /// Predeclared propagation behavior.
    pub propagation: PropagationPolicy,
}

/// Allocation-free projection of one active subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubscriptionInfo {
    /// Unique runtime subscription token.
    pub subscription_id: SubscriptionId,
    /// Owning Stage.
    pub stage_id: StageId,
    /// Exact generation-checked actor identity.
    pub actor_identity: ActorIdentity,
    /// Opaque callback token.
    pub callback_id: CallbackId,
    /// Descriptor event identifier.
    pub event_id: EventId,
    /// Selected native phase.
    pub phase: DispatchPhase,
    /// Predeclared filter.
    pub filter: SubscriptionFilter,
    /// Predeclared propagation behavior.
    pub propagation: PropagationPolicy,
    /// Endpoint-wide registration order.
    pub registration_order: u32,
}

/// Native event facts known before dispatch begins.
#[derive(Clone, Copy, Debug)]
pub struct NativeEventReservation<'a> {
    /// Exact target/current-node actor identity.
    pub actor_identity: ActorIdentity,
    /// Native phase whose possible emissions are being reserved.
    pub phase: DispatchPhase,
    /// Object-semantic event that will be dispatched.
    pub event: &'a ObjectEvent,
    /// Endpoint-wide native traversal sequence assigned before dispatch.
    pub native_event_sequence: NativeEventSequence,
}

/// Post-widget facts supplied to allocation-free completion.
#[derive(Clone, Copy, Debug)]
pub struct NativeEventCompletion<'a> {
    /// Exact observed actor identity.
    pub actor_identity: ActorIdentity,
    /// Native phase that completed.
    pub phase: DispatchPhase,
    /// Object-semantic event delivered during traversal.
    pub event: &'a ObjectEvent,
    /// Whether the target widget semantic adapter ran.
    pub widget_invoked: bool,
    /// Whether the target widget semantic adapter consumed the event.
    pub native_consumed: bool,
}

/// Cue-ready result of one completed native observation.
#[derive(Debug, PartialEq, Eq)]
pub struct SubscriptionObservation {
    /// Descriptor-derived cue inputs in subscription registration order.
    pub cues: Vec<CueInput>,
    /// Predeclared native propagation decision for the object observer.
    pub control: NativeObserverControl,
}

/// Exact pre-dispatch cue admissions reserved for one native observation.
///
/// Counts include every descriptor-qualified subscription that can emit from
/// the reserved observation. Post-widget semantic gates may reduce the actual
/// cue count, but can never increase it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CueAdmissionCounts {
    /// Required lifecycle or control-path cue admissions.
    pub critical: usize,
    /// Ordinary admissions that preserve every record in order.
    pub ordered: usize,
    /// Ordinary admissions eligible for exact-key queue-tail replacement.
    pub latest_value_coalescible: usize,
}

impl CueAdmissionCounts {
    /// Return all possible cue admissions for the observation.
    pub const fn total(self) -> usize {
        self.critical + self.ordered + self.latest_value_coalescible
    }

    /// Return all possible ordinary cue admissions for the observation.
    pub const fn ordinary(self) -> usize {
        self.ordered + self.latest_value_coalescible
    }

    fn add(&mut self, delivery: EventDelivery, count: usize) -> Result<(), SubscriptionError> {
        let destination = match delivery {
            EventDelivery::Critical => &mut self.critical,
            EventDelivery::Ordered => &mut self.ordered,
            EventDelivery::LatestValueCoalescible => &mut self.latest_value_coalescible,
        };
        *destination = destination
            .checked_add(count)
            .ok_or(SubscriptionError::ObservationCapacity)?;
        Ok(())
    }
}

/// Exact callback-token report emitted during subscription teardown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeardownReport {
    /// Endpoint lifetime that assigned the subscription token.
    pub endpoint_epoch: EndpointEpoch,
    /// Stage that owned the subscription.
    pub stage_id: StageId,
    /// Exact actor identity retained independently of live actor lookup.
    pub actor_identity: ActorIdentity,
    /// Unique runtime subscription token.
    pub subscription_id: SubscriptionId,
    /// Descriptor event identifier for a later Critical release notice.
    pub event_id: EventId,
    /// Opaque callback token released by the language adapter later.
    pub callback_id: CallbackId,
}

/// Result of one exact unsubscribe request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsubscribeOutcome {
    /// The active record was removed and its callback token is reported once.
    Removed(TeardownReport),
    /// The exact identity was removed within the bounded completion window.
    AlreadyRemoved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedTeardownState {
    Prepared,
    Committed,
}

/// Fallibly prepared, child-first subscription teardown transaction.
///
/// Preparation owns the exact ordered release reports but does not mutate the
/// registry. The endpoint may inspect them and reserve Critical release cues
/// before committing a Stage batch, then call
/// [`SubscriptionRegistry::commit_teardown`] allocation-free afterward.
pub struct PreparedSubscriptionTeardown {
    endpoint_epoch: EndpointEpoch,
    registry_revision: u64,
    next_revision: Option<u64>,
    reports: Vec<TeardownReport>,
    state: PreparedTeardownState,
}

impl PreparedSubscriptionTeardown {
    /// Return the exact number of subscriptions that commit will remove.
    pub fn report_count(&self) -> usize {
        self.reports.len()
    }

    /// Return exact callback-token reports in current child-first order.
    pub fn reports(&self) -> &[TeardownReport] {
        &self.reports
    }

    /// Return whether the prepared transaction has no matching subscriptions.
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    fn into_reports(mut self) -> Vec<TeardownReport> {
        core::mem::take(&mut self.reports)
    }
}

/// Exclusively validated subscription teardown ready for infallible commit.
///
/// Construct this guard only after every release cue has been prepared and
/// reserved. Holding it prevents subscription state from changing between the
/// final freshness check and the Stage commit it accompanies.
pub struct SubscriptionTeardownCommit<'a> {
    registry: &'a mut SubscriptionRegistry,
    prepared: &'a mut PreparedSubscriptionTeardown,
}

impl SubscriptionTeardownCommit<'_> {
    /// Remove the prepared records and publish their tombstones infallibly.
    ///
    /// All revision, identity, and capacity checks ran before this guard was
    /// created. The operation performs no allocation or deallocation.
    pub fn commit(self) {
        let Self { registry, prepared } = self;
        for report in &prepared.reports {
            registry.push_tombstone(Tombstone {
                stage_id: report.stage_id,
                actor_identity: report.actor_identity,
                subscription_id: report.subscription_id,
            });
        }
        registry.records.retain(|record| {
            !prepared
                .reports
                .iter()
                .any(|report| record_matches_report(record, report))
        });
        if let Some(revision) = prepared.next_revision {
            registry.revision = revision;
        }
        prepared.state = PreparedTeardownState::Committed;
    }
}

/// Subscription validation, capacity, identity, or adapter failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriptionError {
    /// Registry limits are zero, inconsistent, or exceed MPY wire widths.
    InvalidLimits,
    /// A bounded allocation failed before native dispatch or state change.
    AllocationFailed,
    /// The negotiated simultaneous-subscription capacity is full.
    Capacity,
    /// Possible emissions exceed the negotiated per-observation workspace.
    ObservationCapacity,
    /// The endpoint exhausted a non-reusable identifier or revision space.
    IdentifierExhausted,
    /// The supplied Stage does not match the requested or retained identity.
    StageMismatch,
    /// The supplied object generation or actor type does not match.
    ActorIdentityMismatch,
    /// The requested native phase is not declared by the event.
    InvalidPhase,
    /// The filter is malformed or not declared by the event.
    InvalidFilter,
    /// The requested propagation policy is unsupported by the profile or descriptor.
    UnsupportedPolicy,
    /// Another active subscription already owns the callback token.
    DuplicateCallback,
    /// The subscription is unknown or outside the idempotent-close window.
    StaleSubscription,
    /// Caller-supplied postorder omitted an actor that still has a matching record.
    TeardownOrderIncomplete,
    /// A prepared teardown belongs to a different endpoint registry.
    TeardownRegistryMismatch,
    /// Subscription state changed after teardown preparation.
    StaleTeardown,
    /// The prepared teardown transaction was already committed.
    TeardownAlreadyCommitted,
    /// Subscription state changed after observation preflight.
    StaleWorkspace,
    /// Completion facts do not match the reserved native observation.
    WorkspaceMismatch,
    /// The workspace has already begun or completed native observation.
    WorkspaceAlreadyCompleted,
    /// Publication was requested before successful native completion.
    WorkspaceNotCompleted,
    /// Stage Registry lookup or actor payload adapter failure.
    Registry(RegistryError),
}

impl From<RegistryError> for SubscriptionError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

struct SubscriptionRecord {
    subscription_id: SubscriptionId,
    stage_id: StageId,
    actor_identity: ActorIdentity,
    callback_id: CallbackId,
    event_id: EventId,
    descriptor: &'static EventDescriptor,
    phase: DispatchPhase,
    filter: SubscriptionFilter,
    propagation: PropagationPolicy,
    registration_order: u32,
}

impl SubscriptionRecord {
    fn info(&self) -> SubscriptionInfo {
        SubscriptionInfo {
            subscription_id: self.subscription_id,
            stage_id: self.stage_id,
            actor_identity: self.actor_identity,
            callback_id: self.callback_id,
            event_id: self.event_id,
            phase: self.phase,
            filter: self.filter,
            propagation: self.propagation,
            registration_order: self.registration_order,
        }
    }
}

#[derive(Clone, Copy)]
struct Tombstone {
    stage_id: StageId,
    actor_identity: ActorIdentity,
    subscription_id: SubscriptionId,
}

struct ReservedSubscriber {
    stage_id: StageId,
    actor_identity: ActorIdentity,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
    propagation: PropagationPolicy,
    payload: Vec<u8>,
}

struct ReservedDescriptorEmission {
    descriptor: &'static EventDescriptor,
    payload: Vec<u8>,
    subscribers: Vec<ReservedSubscriber>,
}

struct ReservedReadyCue {
    stage_id: StageId,
    actor_identity: ActorIdentity,
    subscription_id: SubscriptionId,
    callback_id: CallbackId,
    event_id: EventId,
    delivery: CueDelivery,
    coalescing_key: Option<CoalescingKey>,
    payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObservationWorkspaceState {
    Reserved,
    Completing,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeEventFingerprint {
    Clicked { x: i32, y: i32 },
    Other,
}

impl NativeEventFingerprint {
    fn capture(event: &ObjectEvent) -> Self {
        match event {
            ObjectEvent::Clicked { x, y } => Self::Clicked { x: *x, y: *y },
            _ => Self::Other,
        }
    }
}

/// Fully allocated one-observation workspace produced before native dispatch.
///
/// Fields are private so completion cannot be attempted with an unreserved or
/// partially reserved buffer set.
pub struct ObservationWorkspace {
    registry_revision: u64,
    stage_id: StageId,
    stage_revision: StageRevision,
    actor_identity: ActorIdentity,
    phase: DispatchPhase,
    event: NativeEventFingerprint,
    native_event_sequence: NativeEventSequence,
    event_handle: ActorEventHandle,
    cue_admission_counts: CueAdmissionCounts,
    emissions: Vec<ReservedDescriptorEmission>,
    publications: Vec<NativeMutationPublication>,
    ready_cues: Vec<ReservedReadyCue>,
    cues: Vec<CueInput>,
    control: NativeObserverControl,
    state: ObservationWorkspaceState,
}

impl ObservationWorkspace {
    /// Return the exact descriptor-qualified pre-dispatch admission counts.
    ///
    /// These counts are computed by the same matching pass that reserves the
    /// descriptor and subscriber scratch buffers; no later rematching is
    /// required before reserving cue-queue capacity.
    pub const fn cue_admission_counts(&self) -> CueAdmissionCounts {
        self.cue_admission_counts
    }

    /// Return the aggregate logical reservation accepted by [`crate::cue::CueQueue`].
    pub const fn cue_admission(&self) -> CueAdmission {
        CueAdmission {
            stage_id: self.stage_id,
            ordinary_slots: self.cue_admission_counts.ordinary(),
            critical_slots: self.cue_admission_counts.critical,
        }
    }
}

/// Endpoint-owned, preallocated registry of validated native subscriptions.
pub struct SubscriptionRegistry {
    endpoint_epoch: EndpointEpoch,
    limits: SubscriptionLimits,
    records: Vec<SubscriptionRecord>,
    tombstones: Vec<Tombstone>,
    next_subscription_id: u32,
    next_registration_order: u32,
    revision: u64,
}

impl SubscriptionRegistry {
    /// Construct an empty registry and reserve record and tombstone capacities.
    pub fn new(
        endpoint_epoch: EndpointEpoch,
        limits: SubscriptionLimits,
    ) -> Result<Self, SubscriptionError> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(limits.max_subscriptions)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        let mut tombstones = Vec::new();
        tombstones
            .try_reserve_exact(limits.max_tombstones)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        Ok(Self {
            endpoint_epoch,
            limits,
            records,
            tombstones,
            next_subscription_id: 1,
            next_registration_order: 1,
            revision: 1,
        })
    }

    /// Return the endpoint epoch that owns all assigned tokens.
    pub const fn endpoint_epoch(&self) -> EndpointEpoch {
        self.endpoint_epoch
    }

    /// Return the number of installed subscriptions.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return whether no subscriptions are installed.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Enumerate active metadata allocation-free in registration order.
    pub fn subscriptions(&self) -> impl ExactSizeIterator<Item = SubscriptionInfo> + '_ {
        self.records.iter().map(SubscriptionRecord::info)
    }

    /// Enumerate one Stage's active metadata allocation-free.
    pub fn subscriptions_for_stage(
        &self,
        stage_id: StageId,
    ) -> impl Iterator<Item = SubscriptionInfo> + '_ {
        self.subscriptions()
            .filter(move |info| info.stage_id == stage_id)
    }

    /// Validate and install one descriptor-qualified subscription.
    pub fn subscribe(
        &mut self,
        stage: &StageRegistry,
        request: SubscribeRequest,
    ) -> Result<SubscriptionId, SubscriptionError> {
        if request.stage_id != stage.stage_id() {
            return Err(SubscriptionError::StageMismatch);
        }
        let actor = stage.actor_info(request.actor_identity.object_id)?;
        if actor.type_id != request.actor_identity.type_id {
            return Err(SubscriptionError::ActorIdentityMismatch);
        }
        let descriptor = stage.event_descriptor(actor.object_id, request.event_id.get())?;
        if !descriptor.phases.allows(request.phase) {
            return Err(SubscriptionError::InvalidPhase);
        }
        request.filter.validate()?;
        if !descriptor.filters.contains(request.filter.descriptor_bit()) {
            return Err(SubscriptionError::InvalidFilter);
        }
        validate_propagation(descriptor, request.phase, request.propagation)?;
        if descriptor.max_payload_bytes as usize > self.limits.max_event_payload_bytes {
            return Err(SubscriptionError::Capacity);
        }
        if self.records.len() >= self.limits.max_subscriptions {
            return Err(SubscriptionError::Capacity);
        }
        if self
            .records
            .iter()
            .any(|record| record.callback_id == request.callback_id)
        {
            return Err(SubscriptionError::DuplicateCallback);
        }
        let subscription_id = SubscriptionId::new(self.next_subscription_id)
            .ok_or(SubscriptionError::IdentifierExhausted)?;
        let registration_order = self.next_registration_order;
        if registration_order == 0 {
            return Err(SubscriptionError::IdentifierExhausted);
        }
        let revision = self.next_revision()?;
        self.records.push(SubscriptionRecord {
            subscription_id,
            stage_id: request.stage_id,
            actor_identity: request.actor_identity,
            callback_id: request.callback_id,
            event_id: request.event_id,
            descriptor,
            phase: request.phase,
            filter: request.filter,
            propagation: request.propagation,
            registration_order,
        });
        self.next_subscription_id = self.next_subscription_id.checked_add(1).unwrap_or(0);
        self.next_registration_order = self.next_registration_order.checked_add(1).unwrap_or(0);
        self.revision = revision;
        Ok(subscription_id)
    }

    /// Reserve all possible cue and payload storage before native dispatch.
    pub fn reserve_observation(
        &self,
        stage: &mut StageRegistry,
        input: NativeEventReservation<'_>,
    ) -> Result<ObservationWorkspace, SubscriptionError> {
        self.validate_actor(stage, input.actor_identity)?;
        let actor = stage.actor_info(input.actor_identity.object_id)?;
        let actor_descriptor = stage
            .descriptor(actor.type_id)
            .ok_or(SubscriptionError::ActorIdentityMismatch)?;
        let descriptor_count = actor_descriptor
            .events
            .iter()
            .filter(|descriptor| {
                descriptor.phases.allows(input.phase)
                    && descriptor.native_event.matches(input.event)
            })
            .count();
        if descriptor_count > self.limits.max_observation_emissions {
            return Err(SubscriptionError::ObservationCapacity);
        }
        stage.reserve_native_event_publications(descriptor_count)?;
        let event_handle = stage.actor_event_handle(input.actor_identity.object_id)?;

        let mut emissions = Vec::new();
        emissions
            .try_reserve_exact(descriptor_count)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        let mut publications = Vec::new();
        publications
            .try_reserve_exact(descriptor_count)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        let mut cue_admission_counts = CueAdmissionCounts::default();

        for descriptor in actor_descriptor.events.iter().filter(|descriptor| {
            descriptor.phases.allows(input.phase) && descriptor.native_event.matches(input.event)
        }) {
            let payload_capacity = descriptor.max_payload_bytes as usize;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(payload_capacity)
                .map_err(|_| SubscriptionError::AllocationFailed)?;
            payload.resize(payload_capacity, 0);
            let matching_subscribers = self
                .records
                .iter()
                .filter(|record| {
                    descriptor_reservation_matches(
                        record,
                        descriptor,
                        stage.stage_id(),
                        input.actor_identity,
                        input.phase,
                        input.event,
                    )
                })
                .count();
            cue_admission_counts.add(descriptor.delivery, matching_subscribers)?;
            if cue_admission_counts.total() > self.limits.max_observation_emissions {
                return Err(SubscriptionError::ObservationCapacity);
            }
            let mut subscribers = Vec::new();
            subscribers
                .try_reserve_exact(matching_subscribers)
                .map_err(|_| SubscriptionError::AllocationFailed)?;
            for record in self.records.iter().filter(|record| {
                descriptor_reservation_matches(
                    record,
                    descriptor,
                    stage.stage_id(),
                    input.actor_identity,
                    input.phase,
                    input.event,
                )
            }) {
                let mut subscriber_payload = Vec::new();
                subscriber_payload
                    .try_reserve_exact(payload_capacity)
                    .map_err(|_| SubscriptionError::AllocationFailed)?;
                subscriber_payload.resize(payload_capacity, 0);
                subscribers.push(ReservedSubscriber {
                    stage_id: record.stage_id,
                    actor_identity: record.actor_identity,
                    subscription_id: record.subscription_id,
                    callback_id: record.callback_id,
                    event_id: record.event_id,
                    propagation: record.propagation,
                    payload: subscriber_payload,
                });
            }
            emissions.push(ReservedDescriptorEmission {
                descriptor,
                payload,
                subscribers,
            });
        }
        let possible_cues = cue_admission_counts.total();
        let mut cues = Vec::new();
        cues.try_reserve_exact(possible_cues)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        let mut ready_cues = Vec::new();
        ready_cues
            .try_reserve_exact(possible_cues)
            .map_err(|_| SubscriptionError::AllocationFailed)?;

        Ok(ObservationWorkspace {
            registry_revision: self.revision,
            stage_id: stage.stage_id(),
            stage_revision: stage.revision(),
            actor_identity: input.actor_identity,
            phase: input.phase,
            event: NativeEventFingerprint::capture(input.event),
            native_event_sequence: input.native_event_sequence,
            event_handle,
            cue_admission_counts,
            emissions,
            publications,
            ready_cues,
            cues,
            control: NativeObserverControl::Continue,
            state: ObservationWorkspaceState::Reserved,
        })
    }

    /// Complete a preflighted observation without Stage access or allocation.
    ///
    /// The cloneable actor handle captured before dispatch prevents a mutable
    /// Stage/root borrow conflict. The borrowed workspace retains every
    /// descriptor, subscriber, and payload allocation until post-dispatch
    /// publication or explicit release.
    pub fn complete_observation(
        &self,
        workspace: &mut ObservationWorkspace,
        input: NativeEventCompletion<'_>,
    ) -> Result<NativeObserverControl, SubscriptionError> {
        if workspace.state != ObservationWorkspaceState::Reserved {
            return Err(SubscriptionError::WorkspaceAlreadyCompleted);
        }
        if workspace.registry_revision != self.revision {
            return Err(SubscriptionError::StaleWorkspace);
        }
        if workspace.actor_identity != input.actor_identity
            || workspace.phase != input.phase
            || workspace.event != NativeEventFingerprint::capture(input.event)
        {
            return Err(SubscriptionError::WorkspaceMismatch);
        }
        if workspace.event_handle.actor_identity() != input.actor_identity {
            return Err(SubscriptionError::ActorIdentityMismatch);
        }

        workspace.state = ObservationWorkspaceState::Completing;
        let mut control = NativeObserverControl::Continue;
        for reserved in &mut workspace.emissions {
            if !reserved.descriptor.matches_native(
                input.phase,
                input.event,
                input.widget_invoked,
                input.native_consumed,
            ) {
                continue;
            }
            let Some(payload_bytes) = workspace.event_handle.event_payload(
                reserved.descriptor,
                input.event,
                &mut reserved.payload,
            )?
            else {
                continue;
            };
            workspace.publications.push(NativeMutationPublication {
                object_id: input.actor_identity.object_id,
                effects: reserved.descriptor.native_effects,
            });
            let payload = &reserved.payload[..payload_bytes];
            for subscriber in &mut reserved.subscribers {
                subscriber.payload[..payload_bytes].copy_from_slice(payload);
                subscriber.payload.truncate(payload_bytes);
                let delivery = match reserved.descriptor.delivery {
                    EventDelivery::Critical => CueDelivery::Critical,
                    EventDelivery::Ordered => CueDelivery::Ordered,
                    EventDelivery::LatestValueCoalescible => CueDelivery::LatestValueCoalescible,
                };
                workspace.ready_cues.push(ReservedReadyCue {
                    stage_id: subscriber.stage_id,
                    actor_identity: subscriber.actor_identity,
                    subscription_id: subscriber.subscription_id,
                    callback_id: subscriber.callback_id,
                    event_id: subscriber.event_id,
                    delivery,
                    coalescing_key: reserved.descriptor.coalescing_key.map(CoalescingKey::new),
                    payload: core::mem::take(&mut subscriber.payload),
                });
                if !matches!(subscriber.propagation, PropagationPolicy::Observe) {
                    control = NativeObserverControl::ConsumePredeclared;
                }
            }
        }
        workspace.control = control;
        workspace.state = ObservationWorkspaceState::Completed;
        Ok(control)
    }

    /// Publish completed native mutations and expose their cue inputs.
    ///
    /// This call is allocation-free and must run after native dispatch releases
    /// the Stage root borrow.
    pub fn publish_observation(
        &self,
        stage: &mut StageRegistry,
        workspace: ObservationWorkspace,
    ) -> Result<SubscriptionObservation, SubscriptionError> {
        if workspace.state != ObservationWorkspaceState::Completed {
            return Err(SubscriptionError::WorkspaceNotCompleted);
        }
        if workspace.stage_id != stage.stage_id() {
            return Err(SubscriptionError::StageMismatch);
        }
        let revision =
            stage.publish_native_mutations(workspace.stage_revision, &workspace.publications)?;
        let mut cues = workspace.cues;
        for ready in workspace.ready_cues {
            let identity = CueIdentity::new(
                ready.stage_id,
                ready.actor_identity.object_id,
                ready.subscription_id,
                ready.callback_id,
                ready.event_id,
            );
            let mut cue = CueInput::new(
                identity,
                revision,
                workspace.native_event_sequence,
                ready.delivery,
                ready.payload,
            );
            if let Some(key) = ready.coalescing_key {
                cue = cue.with_coalescing_key(key);
            }
            cues.push(cue);
        }
        Ok(SubscriptionObservation {
            cues,
            control: workspace.control,
        })
    }

    /// Explicitly release a reserved or completed workspace after dispatch.
    ///
    /// Endpoints use this path when native dispatch or later admission fails
    /// and publication will not consume the retained scratch allocations.
    pub fn release_observation(&self, workspace: ObservationWorkspace) {
        drop(workspace);
    }

    /// Remove one exact subscription with bounded idempotent-close detection.
    pub fn unsubscribe(
        &mut self,
        stage_id: StageId,
        actor_identity: ActorIdentity,
        subscription_id: SubscriptionId,
    ) -> Result<UnsubscribeOutcome, SubscriptionError> {
        let Some(index) = self.records.iter().position(|record| {
            record.subscription_id == subscription_id
                && record.stage_id == stage_id
                && record.actor_identity == actor_identity
        }) else {
            return if self.tombstones.iter().any(|tombstone| {
                tombstone.subscription_id == subscription_id
                    && tombstone.stage_id == stage_id
                    && tombstone.actor_identity == actor_identity
            }) {
                Ok(UnsubscribeOutcome::AlreadyRemoved)
            } else {
                Err(SubscriptionError::StaleSubscription)
            };
        };
        let revision = self.next_revision()?;
        let record = self.records.remove(index);
        let report = self.report(&record);
        self.push_tombstone(Tombstone {
            stage_id,
            actor_identity,
            subscription_id,
        });
        self.revision = revision;
        Ok(UnsubscribeOutcome::Removed(report))
    }

    /// Remove selected actors' records in caller-supplied current child-first order.
    ///
    /// This compatibility wrapper prepares and commits the same transaction
    /// exposed by [`Self::prepare_teardown_objects_child_first`].
    pub fn teardown_objects_child_first(
        &mut self,
        stage_id: StageId,
        object_ids_child_first: &[ObjectId],
    ) -> Result<Vec<TeardownReport>, SubscriptionError> {
        let mut prepared =
            self.prepare_teardown_objects_child_first(stage_id, object_ids_child_first)?;
        self.commit_teardown(&mut prepared)?;
        Ok(prepared.into_reports())
    }

    /// Prepare selected actors' exact reports without mutating registry state.
    pub fn prepare_teardown_objects_child_first(
        &self,
        stage_id: StageId,
        object_ids_child_first: &[ObjectId],
    ) -> Result<PreparedSubscriptionTeardown, SubscriptionError> {
        self.prepare_teardown_matching(object_ids_child_first, |record| {
            record.stage_id == stage_id
                && object_ids_child_first.contains(&record.actor_identity.object_id)
        })
    }

    /// Remove every Stage record in caller-supplied current child-first order.
    ///
    /// This compatibility wrapper prepares and commits the same transaction
    /// exposed by [`Self::prepare_teardown_stage_child_first`].
    pub fn teardown_stage_child_first(
        &mut self,
        stage_id: StageId,
        object_ids_child_first: &[ObjectId],
    ) -> Result<Vec<TeardownReport>, SubscriptionError> {
        let mut prepared =
            self.prepare_teardown_stage_child_first(stage_id, object_ids_child_first)?;
        self.commit_teardown(&mut prepared)?;
        Ok(prepared.into_reports())
    }

    /// Prepare every Stage record in caller-supplied current child-first order.
    pub fn prepare_teardown_stage_child_first(
        &self,
        stage_id: StageId,
        object_ids_child_first: &[ObjectId],
    ) -> Result<PreparedSubscriptionTeardown, SubscriptionError> {
        self.prepare_teardown_matching(object_ids_child_first, |record| record.stage_id == stage_id)
    }

    /// Commit one prepared teardown without allocation or deallocation.
    ///
    /// The caller retains the preparation and its report storage until after
    /// the Safe Turn. A successful commit removes each represented record and
    /// enters its exact identity into the bounded completion window once.
    pub fn commit_teardown(
        &mut self,
        prepared: &mut PreparedSubscriptionTeardown,
    ) -> Result<(), SubscriptionError> {
        self.prepare_teardown_commit(prepared)?.commit();
        Ok(())
    }

    /// Acquire the exclusive, fully validated commit guard for a preparation.
    ///
    /// The endpoint acquires this guard before committing the corresponding
    /// Stage transaction. Once returned, [`SubscriptionTeardownCommit::commit`]
    /// cannot fail or allocate.
    pub fn prepare_teardown_commit<'a>(
        &'a mut self,
        prepared: &'a mut PreparedSubscriptionTeardown,
    ) -> Result<SubscriptionTeardownCommit<'a>, SubscriptionError> {
        if prepared.state != PreparedTeardownState::Prepared {
            return Err(SubscriptionError::TeardownAlreadyCommitted);
        }
        if prepared.endpoint_epoch != self.endpoint_epoch {
            return Err(SubscriptionError::TeardownRegistryMismatch);
        }
        if prepared.registry_revision != self.revision {
            return Err(SubscriptionError::StaleTeardown);
        }
        let matching_records = self
            .records
            .iter()
            .filter(|record| {
                prepared
                    .reports
                    .iter()
                    .any(|report| record_matches_report(record, report))
            })
            .count();
        if matching_records != prepared.reports.len() {
            return Err(SubscriptionError::StaleTeardown);
        }
        Ok(SubscriptionTeardownCommit {
            registry: self,
            prepared,
        })
    }

    /// Release an uncommitted or committed preparation outside the Safe Turn.
    ///
    /// Releasing an uncommitted preparation is the rollback path: the registry
    /// remains unchanged because preparation never mutates it.
    pub fn release_teardown(&self, prepared: PreparedSubscriptionTeardown) {
        drop(prepared);
    }

    fn prepare_teardown_matching(
        &self,
        object_ids_child_first: &[ObjectId],
        matches: impl Fn(&SubscriptionRecord) -> bool,
    ) -> Result<PreparedSubscriptionTeardown, SubscriptionError> {
        let count = self.records.iter().filter(|record| matches(record)).count();
        let mut reports: Vec<TeardownReport> = Vec::new();
        reports
            .try_reserve_exact(count)
            .map_err(|_| SubscriptionError::AllocationFailed)?;
        for object_id in object_ids_child_first {
            for record in self
                .records
                .iter()
                .filter(|record| matches(record) && record.actor_identity.object_id == *object_id)
            {
                if !reports
                    .iter()
                    .any(|report| report.subscription_id == record.subscription_id)
                {
                    reports.push(self.report(record));
                }
            }
        }
        if reports.len() != count {
            return Err(SubscriptionError::TeardownOrderIncomplete);
        }
        Ok(PreparedSubscriptionTeardown {
            endpoint_epoch: self.endpoint_epoch,
            registry_revision: self.revision,
            next_revision: if reports.is_empty() {
                None
            } else {
                Some(self.next_revision()?)
            },
            reports,
            state: PreparedTeardownState::Prepared,
        })
    }

    fn validate_actor(
        &self,
        stage: &StageRegistry,
        identity: ActorIdentity,
    ) -> Result<(), SubscriptionError> {
        let actor = stage.actor_info(identity.object_id)?;
        if actor.type_id != identity.type_id {
            return Err(SubscriptionError::ActorIdentityMismatch);
        }
        Ok(())
    }

    fn next_revision(&self) -> Result<u64, SubscriptionError> {
        self.revision
            .checked_add(1)
            .ok_or(SubscriptionError::IdentifierExhausted)
    }

    fn push_tombstone(&mut self, tombstone: Tombstone) {
        if self.tombstones.len() == self.limits.max_tombstones {
            self.tombstones.remove(0);
        }
        self.tombstones.push(tombstone);
    }

    fn report(&self, record: &SubscriptionRecord) -> TeardownReport {
        TeardownReport {
            endpoint_epoch: self.endpoint_epoch,
            stage_id: record.stage_id,
            actor_identity: record.actor_identity,
            subscription_id: record.subscription_id,
            event_id: record.event_id,
            callback_id: record.callback_id,
        }
    }
}

fn validate_propagation(
    descriptor: &EventDescriptor,
    phase: DispatchPhase,
    policy: PropagationPolicy,
) -> Result<(), SubscriptionError> {
    match policy {
        PropagationPolicy::Observe => Ok(()),
        PropagationPolicy::ConsumeAtTarget
            if phase == DispatchPhase::Target && descriptor.allow_consume_at_target =>
        {
            Ok(())
        }
        PropagationPolicy::StopAfterPhase if descriptor.allow_stop_after_phase => Ok(()),
        PropagationPolicy::ConsumeAtTarget
        | PropagationPolicy::StopAfterPhase
        | PropagationPolicy::PreventDefault => Err(SubscriptionError::UnsupportedPolicy),
    }
}

fn reservation_matches(
    record: &SubscriptionRecord,
    stage_id: StageId,
    actor_identity: ActorIdentity,
    phase: DispatchPhase,
    event: &ObjectEvent,
) -> bool {
    record.stage_id == stage_id
        && record.actor_identity == actor_identity
        && record.phase == phase
        && record.descriptor.phases.allows(phase)
        && record.descriptor.native_event.matches(event)
        && record.filter.matches(event)
}

fn descriptor_reservation_matches(
    record: &SubscriptionRecord,
    descriptor: &EventDescriptor,
    stage_id: StageId,
    actor_identity: ActorIdentity,
    phase: DispatchPhase,
    event: &ObjectEvent,
) -> bool {
    record.event_id.get() == descriptor.id
        && reservation_matches(record, stage_id, actor_identity, phase, event)
}

fn record_matches_report(record: &SubscriptionRecord, report: &TeardownReport) -> bool {
    record.stage_id == report.stage_id
        && record.actor_identity == report.actor_identity
        && record.subscription_id == report.subscription_id
        && record.event_id == report.event_id
        && record.callback_id == report.callback_id
}
