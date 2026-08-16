//! Bounded MPY Safe Turn endpoint for actor-directed Stage requests.
//!
//! The endpoint owns Stage, subscription, cue, request, completion, and drain
//! authority. It implements callback-free actor batches, full-Stage teardown,
//! exact subscription-release publication, and preflighted native input that
//! emits descriptor-derived cues without entering a language runtime. No
//! distinct Stage-teardown RuntimeNotice is synthesized because the queue has
//! no registered typed surface for one; the request completion is the
//! authoritative teardown result.

use alloc::{collections::VecDeque, rc::Rc, vec::Vec};

use crate::{
    actor::{ActorIdentity, ObjectId, RegistryError, StageDispatchError, StageId, StageRegistry},
    cue::{
        CueDelivery, CueIdentity, CueInput, CueLimits, CueQueue, CueQueueError, CueSequence,
        DrainBudget, DrainedEndpointRecords, EndpointRecord, InputClass, InputSequence,
        NativeCueAdmission, NativeEventSequence,
    },
    direction::{StageDirection, StageRevision},
    event::Event,
    object::{DispatchInput, Disposition, ObjectDispatchError, ObjectEvent},
    subscription::{
        EndpointEpoch, SubscribeRequest, SubscriptionError, SubscriptionLimits,
        SubscriptionRegistry,
    },
};

/// Nonzero caller-assigned request identity, monotonic within one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RequestId(u32);

impl RequestId {
    /// Construct a request identity, rejecting the reserved zero value.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    /// Return the serialized request identity.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Validated bounded capacities for one endpoint epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointLimits {
    max_stages: usize,
    max_pending_batches: usize,
    max_completions: usize,
    max_directions_per_batch: usize,
}

impl EndpointLimits {
    /// Validate Stage, pending-request, and completion-credit capacities.
    pub fn new(
        max_stages: usize,
        max_pending_batches: usize,
        max_completions: usize,
        max_directions_per_batch: usize,
    ) -> Result<Self, EndpointError> {
        if max_stages == 0
            || max_stages > u32::MAX as usize
            || max_pending_batches == 0
            || max_pending_batches > max_completions
            || max_completions == 0
            || max_completions > u32::MAX as usize
            || max_directions_per_batch == 0
            || max_directions_per_batch > u16::MAX as usize
        {
            return Err(EndpointError::InvalidLimits);
        }
        Ok(Self {
            max_stages,
            max_pending_batches,
            max_completions,
            max_directions_per_batch,
        })
    }

    /// Return the combined live and cue-finalization-pending Stage capacity.
    pub const fn max_stages(self) -> usize {
        self.max_stages
    }

    /// Return the queued batch capacity.
    pub const fn max_pending_batches(self) -> usize {
        self.max_pending_batches
    }

    /// Return the shared pending-plus-completion credit capacity.
    pub const fn max_completions(self) -> usize {
        self.max_completions
    }

    /// Return the maximum top-level directions accepted in one batch envelope.
    pub const fn max_directions_per_batch(self) -> usize {
        self.max_directions_per_batch
    }
}

/// Runtime ownership state of the endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointState {
    /// Safe Turns and drains may begin.
    Ready,
    /// A Safe Turn is executing synchronously.
    Running,
    /// A global record drain awaits VM-safe acknowledgment.
    DrainOutstanding,
    /// An impossible post-commit failure made the epoch unrecoverable.
    Faulted,
}

/// Terminal endpoint failure retained for inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointFault {
    /// Post-commit Stage scratch or lifecycle release failed unexpectedly.
    PostCommitStageRelease(RegistryError),
    /// Cue causality retirement failed after a committed Stage teardown.
    PostCommitCueFinalize(CueQueueError),
    /// A descriptor payload adapter failed after native widget mutation.
    PostDispatchAdapter(SubscriptionError),
    /// Actual cue output violated its pre-dispatch reservation contract.
    PostDispatchCueContract(CueQueueError),
    /// Subscription or Stage ingress invariants failed before native dispatch.
    PreDispatchSubscription(SubscriptionError),
    /// A resolved native route failed its final pre-dispatch invariant guard.
    PreDispatchNative(StageDispatchError),
    /// A permanent cue health or descriptor contract failure blocked dispatch.
    PreDispatchCue(CueQueueError),
    /// Required Critical raw-input loss accounting or notice publication failed.
    InputOverflowNotice(CueQueueError),
}

/// One endpoint-scoped native input routed through an explicit Stage root or actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointNativeInput {
    /// Translate and hit-test a native pointer stream below one selected root.
    Pointer {
        /// Exact Stage root constraining hit-test resolution.
        root_id: ObjectId,
        /// Pointer horizontal coordinate.
        x: i32,
        /// Pointer vertical coordinate.
        y: i32,
        /// Native stream event translated before traversal.
        event: Event,
    },
    /// Hit-test an already translated object event below one selected root.
    PointerObject {
        /// Exact Stage root constraining hit-test resolution.
        root_id: ObjectId,
        /// Pointer horizontal coordinate.
        x: i32,
        /// Pointer vertical coordinate.
        y: i32,
        /// Object-semantic event delivered to the resolved target.
        event: ObjectEvent,
    },
    /// Route an object event to the focused actor below one selected root.
    Focused {
        /// Exact Stage root constraining focus resolution.
        root_id: ObjectId,
        /// Object-semantic event delivered to the focused target.
        event: ObjectEvent,
    },
    /// Deliver an object event directly to one generation-checked actor.
    Actor {
        /// Exact actor identity; its owning root and path are derived natively.
        target: ActorIdentity,
        /// Object-semantic event delivered to the actor.
        event: ObjectEvent,
    },
}

/// Observable result of one raw-input admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInputOutcome {
    /// The explicit root had no hit-test or focused target, so no traversal began.
    NoTarget {
        /// Endpoint-wide raw-input sequence consumed by this attempt.
        input_sequence: InputSequence,
    },
    /// Cue saturation rejected the raw event before native mutation.
    RejectedBeforeDispatch {
        /// Endpoint-wide raw-input sequence reported as lost.
        input_sequence: InputSequence,
        /// Global record sequence assigned to the Critical overflow notice.
        notice_sequence: CueSequence,
    },
    /// Native traversal completed and any semantic cues were committed.
    Dispatched {
        /// Endpoint-wide raw-input sequence consumed by this attempt.
        input_sequence: InputSequence,
        /// Native traversal sequence shared by every emitted cue.
        native_event_sequence: NativeEventSequence,
        /// Stage Revision visible after native semantic publication.
        stage_revision: StageRevision,
        /// Native propagation result.
        disposition: Disposition,
        /// Number of actual descriptor/subscription cues committed.
        cue_count: usize,
    },
}

/// Pre-mutation reason one accepted batch or Stage teardown was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRejection {
    /// Stage preparation or the Stage pre-commit guard rejected the batch.
    Registry(RegistryError),
    /// Subscription teardown preparation or freshness validation failed.
    Subscription(SubscriptionError),
    /// Exact release-cue preparation or freshness validation failed.
    Cue(CueQueueError),
    /// Release-cue scratch could not be allocated before Stage mutation.
    AllocationFailed,
    /// The endpoint-wide native event sequence was exhausted.
    NativeEventSequenceExhausted,
}

/// Exactly one completion outcome for one accepted endpoint request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchOutcome {
    /// Stage, subscription teardown, and release cues committed atomically.
    Committed {
        /// Single Stage Revision published by the accepted batch.
        revision: StageRevision,
        /// Number of exact actor identities deleted by the final tree shadow.
        deleted_objects: usize,
        /// Number of callback-token release cues appended.
        released_subscriptions: usize,
    },
    /// A full Stage teardown committed and removed the Stage from lookup.
    StageTeardownCommitted {
        /// Single final Stage Revision published while closing the Stage.
        revision: StageRevision,
        /// Number of actors retired in child-first order.
        deleted_objects: usize,
        /// Number of callback-token release cues appended.
        released_subscriptions: usize,
        /// Number of pending ordinary cues purged for the closed Stage.
        purged_ordinary: usize,
    },
    /// Every fallible check rejected before Stage mutation.
    Rejected {
        /// Stage Revision that remained visible after rejection.
        observed_revision: StageRevision,
        /// Reserved per-operation result index; this endpoint slice reports `None`.
        operation_index: Option<u16>,
        /// Truthful underlying failure class.
        error: BatchRejection,
    },
}

/// Globally ordered completion for one caller request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchCompletion {
    /// Caller-assigned monotonic request identity.
    pub request_id: RequestId,
    /// Stage selected by the request.
    pub stage_id: StageId,
    /// Atomic commit or pre-mutation rejection.
    pub outcome: BatchOutcome,
}

/// Work performed by one Safe Turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeTurnSummary {
    /// Monotonic endpoint turn after advancing this boundary.
    pub turn: u64,
    /// FIFO batch or Stage-teardown requests completed during this turn.
    pub processed_batches: usize,
}

/// Endpoint construction, admission, drain, or terminal failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointError {
    /// Endpoint capacities are zero, inconsistent, or exceed wire widths.
    InvalidLimits,
    /// A bounded allocation failed before externally visible state changed.
    AllocationFailed,
    /// Endpoint construction or operation failed in the subscription registry.
    Subscription(SubscriptionError),
    /// Endpoint construction or drain failed in the cue queue.
    Cue(CueQueueError),
    /// Stage route resolution or its final pre-dispatch guard failed.
    NativeDispatch(StageDispatchError),
    /// The endpoint is terminally faulted.
    Faulted(EndpointFault),
    /// Another global record drain still awaits acknowledgment.
    DrainOutstanding,
    /// The supplied drain does not belong to the outstanding endpoint drain.
    DrainMismatch,
    /// The Stage ownership capacity is full.
    StageCapacity,
    /// Stage identifiers must increase strictly and can never be reused.
    StageIdNotMonotonic,
    /// No owned Stage has the supplied identity.
    StageNotFound,
    /// A full teardown is already accepted and fences later Stage admission.
    StageTeardownPending,
    /// Request identities must increase strictly across accepted batches.
    RequestIdNotMonotonic,
    /// The bounded pending-request queue is full.
    PendingCapacity,
    /// The supplied batch envelope exceeds the negotiated direction bound.
    DirectionCapacity,
    /// No completion credit remains for another accepted batch.
    CompletionCapacity,
    /// Safe Turn identity or drain identity space was exhausted.
    IdentifierExhausted,
    /// Endpoint-wide raw-input identity space was exhausted.
    InputSequenceExhausted,
}

enum QueuedRequestKind {
    Batch(Vec<StageDirection>),
    StageTeardown,
}

struct QueuedRequest {
    request_id: RequestId,
    stage_id: StageId,
    accepted_revision: StageRevision,
    eligible_turn: u64,
    kind: QueuedRequestKind,
}

/// Opaque non-clone global record drain awaiting VM-safe acknowledgment.
#[must_use = "record drains must be acknowledged to unblock Safe Turns"]
pub struct EndpointDrain {
    endpoint_epoch: EndpointEpoch,
    owner_token: Rc<()>,
    drain_id: u64,
    drained: DrainedEndpointRecords,
}

/// Owning acknowledgment failure that preserves the unconsumed drain token.
#[must_use = "recover the drain and acknowledge it against its owning endpoint"]
pub struct DrainAcknowledgeError {
    error: EndpointError,
    drain: EndpointDrain,
}

impl core::fmt::Debug for DrainAcknowledgeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DrainAcknowledgeError")
            .field("error", &self.error)
            .field("endpoint_epoch", &self.drain.endpoint_epoch)
            .field("drain_id", &self.drain.drain_id)
            .field("record_count", &self.drain.drained.records.len())
            .finish()
    }
}

impl DrainAcknowledgeError {
    /// Return the acknowledgment failure without consuming the owning error.
    pub const fn error(&self) -> EndpointError {
        self.error
    }

    /// Borrow the preserved drain token and its records.
    pub const fn drain(&self) -> &EndpointDrain {
        &self.drain
    }

    /// Recover the drain token for a later acknowledgment attempt.
    pub fn into_drain(self) -> EndpointDrain {
        self.drain
    }
}

impl EndpointDrain {
    /// Borrow globally ordered cues and RuntimeNotices.
    pub fn records(&self) -> &[EndpointRecord] {
        &self.drained.records
    }

    /// Return the sum of canonical frame bytes in this drain.
    pub const fn frame_bytes(&self) -> usize {
        self.drained.frame_bytes
    }

    /// Return whether this drain contains no records.
    pub fn is_empty(&self) -> bool {
        self.drained.records.is_empty()
    }
}

/// Bounded owner of Stage, request, subscription, cue, and Safe Turn state.
pub struct Endpoint {
    endpoint_epoch: EndpointEpoch,
    owner_token: Rc<()>,
    limits: EndpointLimits,
    stages: Vec<StageRegistry>,
    subscriptions: SubscriptionRegistry,
    cues: CueQueue,
    pending: VecDeque<QueuedRequest>,
    completions: VecDeque<BatchCompletion>,
    pending_teardown_stages: Vec<StageId>,
    pending_cue_finalization: Vec<StageId>,
    current_turn: u64,
    last_request_id: Option<RequestId>,
    last_stage_id: Option<StageId>,
    next_input_sequence: u64,
    next_native_event_sequence: u64,
    next_drain_id: u64,
    outstanding_drain_id: Option<u64>,
    state: EndpointState,
    fault: Option<EndpointFault>,
}

impl Endpoint {
    /// Construct an empty endpoint and reserve every long-lived container.
    pub fn new(
        endpoint_epoch: EndpointEpoch,
        limits: EndpointLimits,
        subscription_limits: SubscriptionLimits,
        cue_limits: CueLimits,
    ) -> Result<Self, EndpointError> {
        let owner_token = Rc::new(());
        let mut stages = Vec::new();
        stages
            .try_reserve_exact(limits.max_stages)
            .map_err(|_| EndpointError::AllocationFailed)?;
        let mut pending = VecDeque::new();
        pending
            .try_reserve_exact(limits.max_pending_batches)
            .map_err(|_| EndpointError::AllocationFailed)?;
        let mut completions = VecDeque::new();
        completions
            .try_reserve_exact(limits.max_completions)
            .map_err(|_| EndpointError::AllocationFailed)?;
        let mut pending_teardown_stages = Vec::new();
        pending_teardown_stages
            .try_reserve_exact(limits.max_stages)
            .map_err(|_| EndpointError::AllocationFailed)?;
        let mut pending_cue_finalization = Vec::new();
        pending_cue_finalization
            .try_reserve_exact(limits.max_stages)
            .map_err(|_| EndpointError::AllocationFailed)?;
        let subscriptions = SubscriptionRegistry::new(endpoint_epoch, subscription_limits)
            .map_err(EndpointError::Subscription)?;
        let cues = CueQueue::new(cue_limits).map_err(EndpointError::Cue)?;
        Ok(Self {
            endpoint_epoch,
            owner_token,
            limits,
            stages,
            subscriptions,
            cues,
            pending,
            completions,
            pending_teardown_stages,
            pending_cue_finalization,
            current_turn: 0,
            last_request_id: None,
            last_stage_id: None,
            next_input_sequence: 1,
            next_native_event_sequence: 1,
            next_drain_id: 1,
            outstanding_drain_id: None,
            state: EndpointState::Ready,
            fault: None,
        })
    }

    /// Return the endpoint epoch owning runtime identities.
    pub const fn endpoint_epoch(&self) -> EndpointEpoch {
        self.endpoint_epoch
    }

    /// Return negotiated endpoint capacities.
    pub const fn limits(&self) -> EndpointLimits {
        self.limits
    }

    /// Return current endpoint ownership state.
    pub const fn state(&self) -> EndpointState {
        self.state
    }

    /// Return a retained terminal failure, when faulted.
    pub const fn fault(&self) -> Option<EndpointFault> {
        self.fault
    }

    /// Return the current completed Safe Turn number.
    pub const fn current_turn(&self) -> u64 {
        self.current_turn
    }

    /// Return whether a closed Stage still occupies cue causality history.
    pub fn stage_finalization_pending(&self, stage_id: StageId) -> bool {
        self.pending_cue_finalization.contains(&stage_id)
    }

    /// Register one owned Stage under strictly increasing identity order.
    pub fn register_stage(&mut self, stage: StageRegistry) -> Result<(), EndpointError> {
        self.ensure_not_faulted()?;
        let stage_id = stage.stage_id();
        if self
            .last_stage_id
            .is_some_and(|previous| stage_id <= previous)
        {
            return Err(EndpointError::StageIdNotMonotonic);
        }
        if self
            .stages
            .len()
            .checked_add(self.pending_cue_finalization.len())
            .is_none_or(|owned| owned >= self.limits.max_stages)
        {
            return Err(EndpointError::StageCapacity);
        }
        self.stages.push(stage);
        self.last_stage_id = Some(stage_id);
        Ok(())
    }

    /// Look up one owned Stage read-only.
    pub fn stage(&self, stage_id: StageId) -> Option<&StageRegistry> {
        self.stages
            .iter()
            .find(|stage| stage.stage_id() == stage_id)
    }

    /// Install one validated subscription against an endpoint-owned Stage.
    pub fn subscribe(
        &mut self,
        request: SubscribeRequest,
    ) -> Result<crate::cue::SubscriptionId, EndpointError> {
        self.ensure_not_faulted()?;
        if self.pending_teardown_stages.contains(&request.stage_id) {
            return Err(EndpointError::StageTeardownPending);
        }
        let index = self
            .stage_index(request.stage_id)
            .ok_or(EndpointError::StageNotFound)?;
        self.subscriptions
            .subscribe(&self.stages[index], request)
            .map_err(EndpointError::Subscription)
    }

    /// Admit and synchronously traverse one raw native input.
    ///
    /// Target resolution happens before a native-event sequence is assigned.
    /// Subscription, Stage-publication, and cue storage are then fully
    /// reserved before handlers or widget semantics run. Queue saturation
    /// rejects the raw event before mutation and appends a Critical typed
    /// overflow notice. A payload-adapter or cue-contract error discovered
    /// after widget mutation terminally faults the endpoint epoch.
    ///
    /// A VM record drain does not pause native input: the remaining bounded
    /// queue capacity and the MPY-05 backpressure policy continue to govern
    /// admission while the opaque drain awaits acknowledgment.
    pub fn dispatch_native_event(
        &mut self,
        stage_id: StageId,
        input_class: InputClass,
        input: EndpointNativeInput,
    ) -> Result<NativeInputOutcome, EndpointError> {
        self.ensure_not_faulted()?;
        let stage_index = self
            .stage_index(stage_id)
            .ok_or(EndpointError::StageNotFound)?;
        if self.pending_teardown_stages.contains(&stage_id) {
            return Err(EndpointError::StageTeardownPending);
        }
        let input_sequence = InputSequence::new(self.next_input_sequence)
            .ok_or(EndpointError::InputSequenceExhausted)?;
        self.next_input_sequence = self.next_input_sequence.checked_add(1).unwrap_or(0);

        let route = match input {
            EndpointNativeInput::Pointer {
                root_id,
                x,
                y,
                event,
            } => self.stages[stage_index]
                .resolve_root_dispatch(root_id, DispatchInput::Pointer { x, y, event }),
            EndpointNativeInput::PointerObject {
                root_id,
                x,
                y,
                event,
            } => self.stages[stage_index]
                .resolve_root_dispatch(root_id, DispatchInput::PointerObject { x, y, event }),
            EndpointNativeInput::Focused { root_id, event } => self.stages[stage_index]
                .resolve_root_dispatch(root_id, DispatchInput::Focused { event }),
            EndpointNativeInput::Actor { target, event } => {
                self.stages[stage_index].resolve_actor_dispatch(target, event)
            }
        };
        let route = match route {
            Ok(route) => route,
            Err(StageDispatchError::Object(ObjectDispatchError::NoTarget)) => {
                return Ok(NativeInputOutcome::NoTarget { input_sequence });
            }
            Err(error) if is_raw_input_stage_dispatch_rejection(error) => {
                return self.reject_raw_input(stage_id, input_class, input_sequence);
            }
            Err(error) => return Err(EndpointError::NativeDispatch(error)),
        };

        let native_event_sequence = NativeEventSequence::new(self.next_native_event_sequence)
            .ok_or(EndpointError::IdentifierExhausted)?;
        let prepared_dispatch = match self.subscriptions.reserve_native_dispatch(
            &mut self.stages[stage_index],
            &route,
            native_event_sequence,
        ) {
            Ok(prepared) => prepared,
            Err(error) if is_raw_input_subscription_rejection(error) => {
                return self.reject_raw_input(stage_id, input_class, input_sequence);
            }
            Err(error) => {
                return Err(
                    self.record_terminal_fault(EndpointFault::PreDispatchSubscription(error))
                );
            }
        };
        let counts = prepared_dispatch.cue_admission_counts();
        let admission = NativeCueAdmission {
            stage_id,
            critical_slots: counts.critical,
            ordered_slots: counts.ordered,
            latest_value_coalescible_slots: counts.latest_value_coalescible,
            maximum_payload_bytes: prepared_dispatch.maximum_payload_bytes(),
        };
        let mut prepared_cues = match self.cues.prepare_native_cues(admission) {
            Ok(prepared) => prepared,
            Err(error) if is_raw_input_admission_rejection(error) => {
                self.subscriptions
                    .release_native_dispatch(prepared_dispatch);
                return self.reject_raw_input(stage_id, input_class, input_sequence);
            }
            Err(error) => {
                self.subscriptions
                    .release_native_dispatch(prepared_dispatch);
                return Err(self.record_terminal_fault(EndpointFault::PreDispatchCue(error)));
            }
        };
        let mut observer = match self
            .subscriptions
            .arm_native_dispatch(&self.stages[stage_index], prepared_dispatch)
        {
            Ok(observer) => observer,
            Err(error) => {
                self.cues.release_native_cues(prepared_cues);
                return if is_raw_input_subscription_rejection(error) {
                    self.reject_raw_input(stage_id, input_class, input_sequence)
                } else {
                    Err(self.record_terminal_fault(EndpointFault::PreDispatchSubscription(error)))
                };
            }
        };
        let cue_reservation = match self.cues.acquire_native_cue_reservation(&mut prepared_cues) {
            Ok(reservation) => reservation,
            Err(error) => {
                observer.release();
                self.cues.release_native_cues(prepared_cues);
                return if is_raw_input_admission_rejection(error) {
                    self.reject_raw_input(stage_id, input_class, input_sequence)
                } else {
                    Err(self.record_terminal_fault(EndpointFault::PreDispatchCue(error)))
                };
            }
        };

        let completed =
            match self.stages[stage_index].dispatch_resolved_native(route, &mut observer) {
                Ok(completed) => completed,
                Err(error) => {
                    cue_reservation.rollback();
                    observer.release();
                    self.cues.release_native_cues(prepared_cues);
                    return if is_raw_input_stage_dispatch_rejection(error) {
                        self.reject_raw_input(stage_id, input_class, input_sequence)
                    } else {
                        Err(self.record_terminal_fault(EndpointFault::PreDispatchNative(error)))
                    };
                }
            };
        self.next_native_event_sequence =
            self.next_native_event_sequence.checked_add(1).unwrap_or(0);
        let disposition = completed.disposition();
        let observed = match observer.finish() {
            Ok(observed) => observed,
            Err(failed) => {
                let cause = failed.cause();
                failed.release();
                completed.release();
                cue_reservation.rollback();
                self.cues.release_native_cues(prepared_cues);
                return Err(self.record_terminal_fault(EndpointFault::PostDispatchAdapter(cause)));
            }
        };
        let mut published = self
            .subscriptions
            .publish_native_dispatch(&mut self.stages[stage_index], observed);
        let stage_revision = published.stage_revision();
        let cues = published.take_cues();
        let cue_count = cues.len();
        let cue_commit = match cue_reservation.accept(stage_revision, native_event_sequence, cues) {
            Ok(commit) => commit,
            Err(error) => {
                completed.release();
                published.release();
                self.cues.release_native_cues(prepared_cues);
                return Err(
                    self.record_terminal_fault(EndpointFault::PostDispatchCueContract(error))
                );
            }
        };
        cue_commit.commit();
        completed.release();
        published.release();
        self.cues.release_native_cues(prepared_cues);

        Ok(NativeInputOutcome::Dispatched {
            input_sequence,
            native_event_sequence,
            stage_revision,
            disposition,
            cue_count,
        })
    }

    /// Queue one owned atomic Stage batch and reserve exactly one completion credit.
    ///
    /// The accepted batch becomes eligible at `current_turn + 1`; callers
    /// cannot bypass this fence, including while a record drain is outstanding.
    pub fn enqueue_batch(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        directions: Vec<StageDirection>,
    ) -> Result<(), EndpointError> {
        self.ensure_not_faulted()?;
        let Some(stage_index) = self.stage_index(stage_id) else {
            return Err(EndpointError::StageNotFound);
        };
        if self.pending_teardown_stages.contains(&stage_id) {
            return Err(EndpointError::StageTeardownPending);
        }
        if directions.len() > self.limits.max_directions_per_batch {
            return Err(EndpointError::DirectionCapacity);
        }
        let accepted_revision = self.stages[stage_index].revision();
        self.enqueue_request(
            request_id,
            stage_id,
            accepted_revision,
            QueuedRequestKind::Batch(directions),
        )
    }

    /// Queue one full-Stage teardown under the global next-turn FIFO fence.
    ///
    /// Once accepted, later batches, subscriptions, or teardowns for this Stage
    /// are rejected until teardown either commits or produces its one rejection
    /// completion. Requests accepted earlier remain ahead of teardown.
    pub fn enqueue_stage_teardown(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
    ) -> Result<(), EndpointError> {
        self.ensure_not_faulted()?;
        let Some(stage_index) = self.stage_index(stage_id) else {
            return Err(EndpointError::StageNotFound);
        };
        if self.pending_teardown_stages.contains(&stage_id) {
            return Err(EndpointError::StageTeardownPending);
        }
        let accepted_revision = self.stages[stage_index].revision();
        self.enqueue_request(
            request_id,
            stage_id,
            accepted_revision,
            QueuedRequestKind::StageTeardown,
        )?;
        debug_assert!(self.pending_teardown_stages.len() < self.limits.max_stages);
        self.pending_teardown_stages.push(stage_id);
        Ok(())
    }

    /// Advance one Safe Turn and process all eligible requests in FIFO order.
    pub fn run_safe_turn(&mut self) -> Result<SafeTurnSummary, EndpointError> {
        self.ensure_not_faulted()?;
        if self.state == EndpointState::DrainOutstanding {
            return Err(EndpointError::DrainOutstanding);
        }
        let next_turn = self
            .current_turn
            .checked_add(1)
            .ok_or(EndpointError::IdentifierExhausted)?;
        self.current_turn = next_turn;
        self.state = EndpointState::Running;
        let mut processed_batches = 0usize;
        while self
            .pending
            .front()
            .is_some_and(|batch| batch.eligible_turn <= self.current_turn)
        {
            let request = self.pending.pop_front().expect("eligible request at head");
            self.process_request(request)?;
            processed_batches += 1;
        }
        self.state = EndpointState::Ready;
        Ok(SafeTurnSummary {
            turn: self.current_turn,
            processed_batches,
        })
    }

    /// Drain completed requests globally in FIFO order.
    ///
    /// Output allocation completes before the first completion is removed.
    pub fn drain_completions(
        &mut self,
        maximum: usize,
    ) -> Result<Vec<BatchCompletion>, EndpointError> {
        let count = core::cmp::min(maximum, self.completions.len());
        let mut drained = Vec::new();
        drained
            .try_reserve_exact(count)
            .map_err(|_| EndpointError::AllocationFailed)?;
        for _ in 0..count {
            drained.push(self.completions.pop_front().expect("drain count bounded"));
        }
        Ok(drained)
    }

    /// Drain one globally ordered cue/RuntimeNotice prefix for VM-safe handling.
    pub fn drain_records(&mut self, budget: DrainBudget) -> Result<EndpointDrain, EndpointError> {
        if self.outstanding_drain_id.is_some() {
            return Err(EndpointError::DrainOutstanding);
        }
        if self.next_drain_id == 0 {
            return Err(EndpointError::IdentifierExhausted);
        }
        let drain_id = self.next_drain_id;
        let drained = self
            .cues
            .drain_endpoint(budget)
            .map_err(EndpointError::Cue)?;
        self.next_drain_id = self.next_drain_id.checked_add(1).unwrap_or(0);
        self.outstanding_drain_id = Some(drain_id);
        if self.fault.is_none() {
            self.state = EndpointState::DrainOutstanding;
        }
        Ok(EndpointDrain {
            endpoint_epoch: self.endpoint_epoch,
            owner_token: Rc::clone(&self.owner_token),
            drain_id,
            drained,
        })
    }

    /// Acknowledge VM-safe handling and release one opaque global drain.
    pub fn acknowledge_records(
        &mut self,
        drain: EndpointDrain,
    ) -> Result<(), DrainAcknowledgeError> {
        if !Rc::ptr_eq(&drain.owner_token, &self.owner_token)
            || drain.endpoint_epoch != self.endpoint_epoch
            || self.outstanding_drain_id != Some(drain.drain_id)
        {
            return Err(DrainAcknowledgeError {
                error: EndpointError::DrainMismatch,
                drain,
            });
        }
        self.outstanding_drain_id = None;
        drop(drain);
        self.finalize_acknowledged_stages();
        self.state = if self.fault.is_some() {
            EndpointState::Faulted
        } else {
            EndpointState::Ready
        };
        Ok(())
    }

    fn process_request(&mut self, request: QueuedRequest) -> Result<(), EndpointError> {
        match request.kind {
            QueuedRequestKind::Batch(directions) => self.process_batch(
                request.request_id,
                request.stage_id,
                request.accepted_revision,
                directions,
            ),
            QueuedRequestKind::StageTeardown => self.process_stage_teardown(
                request.request_id,
                request.stage_id,
                request.accepted_revision,
            ),
        }
    }

    fn process_batch(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        accepted_revision: StageRevision,
        directions: Vec<StageDirection>,
    ) -> Result<(), EndpointError> {
        let Some(stage_index) = self.stage_index(stage_id) else {
            self.push_rejection(
                request_id,
                stage_id,
                accepted_revision,
                BatchRejection::Registry(RegistryError::InvalidStage),
            );
            return Ok(());
        };
        let observed_revision = self.stages[stage_index].revision();
        let prepared_stage = match self.stages[stage_index].prepare_batch(directions) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.push_rejection(request_id, stage_id, observed_revision, error.into());
                return Ok(());
            }
        };
        let next_revision = prepared_stage.next_revision();
        let deleted_objects = prepared_stage.deleted_object_ids().len();
        let mut prepared_teardown = match self
            .subscriptions
            .prepare_teardown_objects_child_first(stage_id, prepared_stage.deleted_object_ids())
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.push_rejection(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::Subscription(error),
                );
                return Ok(());
            }
        };
        let released_subscriptions = prepared_teardown.report_count();
        let native_event_sequence = if released_subscriptions == 0 {
            NativeEventSequence::new(1).expect("constant is nonzero")
        } else {
            let Some(sequence) = NativeEventSequence::new(self.next_native_event_sequence) else {
                self.subscriptions.release_teardown(prepared_teardown);
                self.push_rejection(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::NativeEventSequenceExhausted,
                );
                return Ok(());
            };
            sequence
        };
        let mut release_inputs = Vec::new();
        if release_inputs
            .try_reserve_exact(released_subscriptions)
            .is_err()
        {
            self.subscriptions.release_teardown(prepared_teardown);
            self.push_rejection(
                request_id,
                stage_id,
                observed_revision,
                BatchRejection::AllocationFailed,
            );
            return Ok(());
        }
        for report in prepared_teardown.reports() {
            release_inputs.push(
                CueInput::new(
                    CueIdentity::new(
                        report.stage_id,
                        report.actor_identity.object_id,
                        report.subscription_id,
                        report.callback_id,
                        report.event_id,
                    ),
                    next_revision,
                    native_event_sequence,
                    CueDelivery::Critical,
                    Vec::new(),
                )
                .with_subscription_release(),
            );
        }
        let mut prepared_cues = match self.cues.prepare_exact_inputs(release_inputs) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.subscriptions.release_teardown(prepared_teardown);
                self.push_rejection(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::Cue(error),
                );
                return Ok(());
            }
        };

        let commit_attempt = {
            let stage = &mut self.stages[stage_index];
            let subscriptions = &mut self.subscriptions;
            let cues = &mut self.cues;
            (|| {
                let subscription_commit = subscriptions
                    .prepare_teardown_commit(&mut prepared_teardown)
                    .map_err(BatchRejection::Subscription)?;
                let cue_commit = cues
                    .acquire_exact_commit(&mut prepared_cues)
                    .map_err(BatchRejection::Cue)?;
                let committed = stage
                    .commit_prepared_batch(prepared_stage)
                    .map_err(|error| BatchRejection::Registry(error.cause()))?;
                subscription_commit.commit();
                cue_commit.commit();
                Ok::<_, BatchRejection>(committed)
            })()
        };

        let committed = match commit_attempt {
            Ok(committed) => committed,
            Err(error) => {
                self.subscriptions.release_teardown(prepared_teardown);
                self.cues.release_exact_inputs(prepared_cues);
                self.push_rejection(request_id, stage_id, observed_revision, error);
                return Ok(());
            }
        };

        if released_subscriptions != 0 {
            self.next_native_event_sequence =
                self.next_native_event_sequence.checked_add(1).unwrap_or(0);
        }
        self.push_completion(BatchCompletion {
            request_id,
            stage_id,
            outcome: BatchOutcome::Committed {
                revision: next_revision,
                deleted_objects,
                released_subscriptions,
            },
        });
        let release_result = self.stages[stage_index].release_committed_batch(committed);
        self.subscriptions.release_teardown(prepared_teardown);
        self.cues.release_exact_inputs(prepared_cues);
        if let Err(error) = release_result {
            let fault = EndpointFault::PostCommitStageRelease(error);
            self.state = EndpointState::Faulted;
            self.fault = Some(fault);
            return Err(EndpointError::Faulted(fault));
        }
        Ok(())
    }

    fn process_stage_teardown(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        accepted_revision: StageRevision,
    ) -> Result<(), EndpointError> {
        let Some(stage_index) = self.stage_index(stage_id) else {
            self.reject_stage_teardown(
                request_id,
                stage_id,
                accepted_revision,
                BatchRejection::Registry(RegistryError::InvalidStage),
            );
            return Ok(());
        };
        let observed_revision = self.stages[stage_index].revision();
        let prepared_stage = match self.stages[stage_index].prepare_stage_teardown() {
            Ok(prepared) => prepared,
            Err(error) => {
                self.reject_stage_teardown(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::Registry(error),
                );
                return Ok(());
            }
        };
        let next_revision = prepared_stage.next_revision();
        let deleted_objects = prepared_stage.deletion_count();
        let mut prepared_subscriptions = match self
            .subscriptions
            .prepare_teardown_stage_child_first(stage_id, prepared_stage.deleted_object_ids())
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.reject_stage_teardown(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::Subscription(error),
                );
                return Ok(());
            }
        };
        let released_subscriptions = prepared_subscriptions.report_count();
        let native_event_sequence = if released_subscriptions == 0 {
            NativeEventSequence::new(1).expect("constant is nonzero")
        } else {
            let Some(sequence) = NativeEventSequence::new(self.next_native_event_sequence) else {
                self.subscriptions.release_teardown(prepared_subscriptions);
                self.reject_stage_teardown(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::NativeEventSequenceExhausted,
                );
                return Ok(());
            };
            sequence
        };
        let mut release_inputs = Vec::new();
        if release_inputs
            .try_reserve_exact(released_subscriptions)
            .is_err()
        {
            self.subscriptions.release_teardown(prepared_subscriptions);
            self.reject_stage_teardown(
                request_id,
                stage_id,
                observed_revision,
                BatchRejection::AllocationFailed,
            );
            return Ok(());
        }
        for report in prepared_subscriptions.reports() {
            release_inputs.push(
                CueInput::new(
                    CueIdentity::new(
                        report.stage_id,
                        report.actor_identity.object_id,
                        report.subscription_id,
                        report.callback_id,
                        report.event_id,
                    ),
                    next_revision,
                    native_event_sequence,
                    CueDelivery::Critical,
                    Vec::new(),
                )
                .with_subscription_release(),
            );
        }
        let mut prepared_cues = match self
            .cues
            .prepare_stage_teardown_inputs(stage_id, release_inputs)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                self.subscriptions.release_teardown(prepared_subscriptions);
                self.reject_stage_teardown(
                    request_id,
                    stage_id,
                    observed_revision,
                    BatchRejection::Cue(error),
                );
                return Ok(());
            }
        };
        let purged_ordinary = prepared_cues.purged_ordinary().count;

        let commit_attempt = {
            let stage = &mut self.stages[stage_index];
            let subscriptions = &mut self.subscriptions;
            let cues = &mut self.cues;
            (|| {
                let subscription_commit = subscriptions
                    .prepare_teardown_commit(&mut prepared_subscriptions)
                    .map_err(BatchRejection::Subscription)?;
                let cue_commit = cues
                    .acquire_stage_teardown_commit(&mut prepared_cues)
                    .map_err(BatchRejection::Cue)?;
                let committed = stage
                    .commit_prepared_teardown(prepared_stage)
                    .map_err(|error| BatchRejection::Registry(error.cause()))?;
                subscription_commit.commit();
                cue_commit.commit();
                Ok::<_, BatchRejection>(committed)
            })()
        };

        let committed = match commit_attempt {
            Ok(committed) => committed,
            Err(error) => {
                self.subscriptions.release_teardown(prepared_subscriptions);
                self.cues.release_stage_teardown_inputs(prepared_cues);
                self.reject_stage_teardown(request_id, stage_id, observed_revision, error);
                return Ok(());
            }
        };

        if released_subscriptions != 0 {
            self.next_native_event_sequence =
                self.next_native_event_sequence.checked_add(1).unwrap_or(0);
        }
        self.clear_pending_teardown(stage_id);
        let mut closed_stage = self.stages.remove(stage_index);
        self.push_completion(BatchCompletion {
            request_id,
            stage_id,
            outcome: BatchOutcome::StageTeardownCommitted {
                revision: next_revision,
                deleted_objects,
                released_subscriptions,
                purged_ordinary,
            },
        });

        let release_result = closed_stage.release_committed_teardown(committed);
        self.subscriptions.release_teardown(prepared_subscriptions);
        self.cues.release_stage_teardown_inputs(prepared_cues);
        let finalize_result = self.track_stage_finalization(stage_id);
        drop(closed_stage);
        if let Err(error) = release_result {
            let fault = EndpointFault::PostCommitStageRelease(error);
            self.state = EndpointState::Faulted;
            self.fault = Some(fault);
            return Err(EndpointError::Faulted(fault));
        }
        if let Err(error) = finalize_result {
            let fault = EndpointFault::PostCommitCueFinalize(error);
            self.state = EndpointState::Faulted;
            self.fault = Some(fault);
            return Err(EndpointError::Faulted(fault));
        }
        Ok(())
    }

    fn enqueue_request(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        accepted_revision: StageRevision,
        kind: QueuedRequestKind,
    ) -> Result<(), EndpointError> {
        if self
            .last_request_id
            .is_some_and(|previous| request_id <= previous)
        {
            return Err(EndpointError::RequestIdNotMonotonic);
        }
        if self.pending.len() >= self.limits.max_pending_batches {
            return Err(EndpointError::PendingCapacity);
        }
        if self
            .pending
            .len()
            .checked_add(self.completions.len())
            .is_none_or(|credits| credits >= self.limits.max_completions)
        {
            return Err(EndpointError::CompletionCapacity);
        }
        let eligible_turn = self
            .current_turn
            .checked_add(1)
            .ok_or(EndpointError::IdentifierExhausted)?;
        self.pending.push_back(QueuedRequest {
            request_id,
            stage_id,
            accepted_revision,
            eligible_turn,
            kind,
        });
        self.last_request_id = Some(request_id);
        Ok(())
    }

    fn reject_stage_teardown(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        observed_revision: StageRevision,
        error: BatchRejection,
    ) {
        self.clear_pending_teardown(stage_id);
        self.push_rejection(request_id, stage_id, observed_revision, error);
    }

    fn clear_pending_teardown(&mut self, stage_id: StageId) {
        if let Some(index) = self
            .pending_teardown_stages
            .iter()
            .position(|candidate| *candidate == stage_id)
        {
            self.pending_teardown_stages.remove(index);
        }
    }

    fn track_stage_finalization(&mut self, stage_id: StageId) -> Result<(), CueQueueError> {
        match self.cues.finalize_stage(stage_id) {
            Ok(_) => Ok(()),
            Err(CueQueueError::StageFinalizeBusy { .. }) => {
                debug_assert!(self.pending_cue_finalization.len() < self.limits.max_stages);
                self.pending_cue_finalization.push(stage_id);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn finalize_acknowledged_stages(&mut self) {
        let mut index = 0usize;
        while index < self.pending_cue_finalization.len() {
            let stage_id = self.pending_cue_finalization[index];
            match self.cues.finalize_stage(stage_id) {
                Ok(_) => {
                    self.pending_cue_finalization.remove(index);
                }
                Err(CueQueueError::StageFinalizeBusy { .. }) => index += 1,
                Err(error) => {
                    self.fault = Some(EndpointFault::PostCommitCueFinalize(error));
                    return;
                }
            }
        }
    }

    fn push_rejection(
        &mut self,
        request_id: RequestId,
        stage_id: StageId,
        observed_revision: StageRevision,
        error: BatchRejection,
    ) {
        self.push_completion(BatchCompletion {
            request_id,
            stage_id,
            outcome: BatchOutcome::Rejected {
                observed_revision,
                operation_index: None,
                error,
            },
        });
    }

    fn push_completion(&mut self, completion: BatchCompletion) {
        debug_assert!(self.completions.len() < self.limits.max_completions);
        self.completions.push_back(completion);
    }

    fn reject_raw_input(
        &mut self,
        stage_id: StageId,
        input_class: InputClass,
        input_sequence: InputSequence,
    ) -> Result<NativeInputOutcome, EndpointError> {
        if let Err(error) = self
            .cues
            .record_input_overflow(stage_id, input_class, input_sequence)
        {
            return Err(self.record_terminal_fault(EndpointFault::InputOverflowNotice(error)));
        }
        match self.cues.enqueue_pending_input_overflow_notice(Vec::new()) {
            Ok(notice) => Ok(NativeInputOutcome::RejectedBeforeDispatch {
                input_sequence,
                notice_sequence: notice.notice_sequence,
            }),
            Err(error) => {
                Err(self.record_terminal_fault(EndpointFault::InputOverflowNotice(error)))
            }
        }
    }

    fn record_terminal_fault(&mut self, fault: EndpointFault) -> EndpointError {
        self.fault = Some(fault);
        self.state = EndpointState::Faulted;
        EndpointError::Faulted(fault)
    }

    fn stage_index(&self, stage_id: StageId) -> Option<usize> {
        self.stages
            .iter()
            .position(|stage| stage.stage_id() == stage_id)
    }

    fn ensure_not_faulted(&self) -> Result<(), EndpointError> {
        match self.fault {
            Some(fault) => Err(EndpointError::Faulted(fault)),
            None => Ok(()),
        }
    }
}

impl From<RegistryError> for BatchRejection {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

fn is_raw_input_admission_rejection(error: CueQueueError) -> bool {
    matches!(
        error,
        CueQueueError::AllocationFailed
            | CueQueueError::AdmissionBackpressured
            | CueQueueError::AdmissionCapacity
            | CueQueueError::AdmissionStageQuota { .. }
            | CueQueueError::StageCausalityCapacityExhausted { .. }
            | CueQueueError::SequenceExhausted
    )
}

fn is_raw_input_subscription_rejection(error: SubscriptionError) -> bool {
    matches!(
        error,
        SubscriptionError::AllocationFailed
            | SubscriptionError::ObservationCapacity
            | SubscriptionError::Registry(
                RegistryError::Capacity { .. } | RegistryError::DispatchBusy
            )
    )
}

fn is_raw_input_stage_dispatch_rejection(error: StageDispatchError) -> bool {
    matches!(
        error,
        StageDispatchError::Object(
            ObjectDispatchError::AllocationFailed | ObjectDispatchError::DispatchBusy
        )
    )
}
