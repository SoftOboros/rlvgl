//! Bounded MPY Safe Turn endpoint for actor-directed Stage batches.
//!
//! The endpoint owns Stage, subscription, cue, request, completion, and drain
//! authority. This slice deliberately excludes full Stage teardown and native
//! input dispatch; it implements callback-free actor batches and exact
//! subscription-release cue publication only.

use alloc::{collections::VecDeque, rc::Rc, vec::Vec};

use crate::{
    actor::{RegistryError, StageId, StageRegistry},
    cue::{
        CueDelivery, CueIdentity, CueInput, CueLimits, CueQueue, CueQueueError, DrainBudget,
        DrainedEndpointRecords, EndpointRecord, NativeEventSequence,
    },
    direction::{StageDirection, StageRevision},
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

    /// Return the simultaneous owned Stage capacity.
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
}

/// Pre-mutation reason one accepted batch was rejected.
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

/// Exactly one completion outcome for one accepted batch.
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
    /// FIFO batches completed during this turn.
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
}

struct QueuedBatch {
    request_id: RequestId,
    stage_id: StageId,
    eligible_turn: u64,
    directions: Vec<StageDirection>,
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
    pending: VecDeque<QueuedBatch>,
    completions: VecDeque<BatchCompletion>,
    current_turn: u64,
    last_request_id: Option<RequestId>,
    last_stage_id: Option<StageId>,
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
            current_turn: 0,
            last_request_id: None,
            last_stage_id: None,
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
        if self.stages.len() >= self.limits.max_stages {
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
        let index = self
            .stage_index(request.stage_id)
            .ok_or(EndpointError::StageNotFound)?;
        self.subscriptions
            .subscribe(&self.stages[index], request)
            .map_err(EndpointError::Subscription)
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
        if self.stage_index(stage_id).is_none() {
            return Err(EndpointError::StageNotFound);
        }
        if self
            .last_request_id
            .is_some_and(|previous| request_id <= previous)
        {
            return Err(EndpointError::RequestIdNotMonotonic);
        }
        if self.pending.len() >= self.limits.max_pending_batches {
            return Err(EndpointError::PendingCapacity);
        }
        if directions.len() > self.limits.max_directions_per_batch {
            return Err(EndpointError::DirectionCapacity);
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
        self.pending.push_back(QueuedBatch {
            request_id,
            stage_id,
            eligible_turn,
            directions,
        });
        self.last_request_id = Some(request_id);
        Ok(())
    }

    /// Advance one Safe Turn and process all eligible batches in FIFO order.
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
            let batch = self.pending.pop_front().expect("eligible batch at head");
            self.process_batch(batch)?;
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
        self.state = if self.fault.is_some() {
            EndpointState::Faulted
        } else {
            EndpointState::Ready
        };
        drop(drain);
        Ok(())
    }

    fn process_batch(&mut self, batch: QueuedBatch) -> Result<(), EndpointError> {
        let stage_index = self
            .stage_index(batch.stage_id)
            .expect("accepted Stage remains owned without Stage teardown");
        let observed_revision = self.stages[stage_index].revision();
        let prepared_stage = match self.stages[stage_index].prepare_batch(batch.directions) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.push_rejection(
                    batch.request_id,
                    batch.stage_id,
                    observed_revision,
                    error.into(),
                );
                return Ok(());
            }
        };
        let next_revision = prepared_stage.next_revision();
        let deleted_objects = prepared_stage.deleted_object_ids().len();
        let mut prepared_teardown = match self.subscriptions.prepare_teardown_objects_child_first(
            batch.stage_id,
            prepared_stage.deleted_object_ids(),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.push_rejection(
                    batch.request_id,
                    batch.stage_id,
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
                    batch.request_id,
                    batch.stage_id,
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
                batch.request_id,
                batch.stage_id,
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
                    batch.request_id,
                    batch.stage_id,
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
                self.push_rejection(batch.request_id, batch.stage_id, observed_revision, error);
                return Ok(());
            }
        };

        if released_subscriptions != 0 {
            self.next_native_event_sequence =
                self.next_native_event_sequence.checked_add(1).unwrap_or(0);
        }
        self.push_completion(BatchCompletion {
            request_id: batch.request_id,
            stage_id: batch.stage_id,
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
