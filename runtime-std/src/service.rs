//! Bounded native-owner service for CPY-03.

use std::{
    fmt, io,
    num::{NonZeroU64, NonZeroUsize},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{
    Receiver, SendError, Sender, TryRecvError, TrySendError, bounded, select_biased,
};
use rlvgl_core::cue::{CueDelivery, EndpointRecord};

use crate::readiness::{Notifier, ReadinessSignal, new_pair};

static NEXT_SERVICE_EPOCH: AtomicU64 = AtomicU64::new(1);

/// Monotonic process-local identity of one native service construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceEpoch(NonZeroU64);

impl ServiceEpoch {
    /// Return the nonzero numeric epoch.
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Monotonic request identity within one service epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(NonZeroU64);

impl RequestId {
    /// Return the nonzero numeric request id.
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// Stable identity carried by every accepted request and terminal record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceTicket {
    epoch: ServiceEpoch,
    request_id: RequestId,
}

impl ServiceTicket {
    /// Return the service epoch that accepted this request.
    pub fn epoch(self) -> ServiceEpoch {
        self.epoch
    }

    /// Return this request's identity within its service epoch.
    pub fn request_id(self) -> RequestId {
        self.request_id
    }
}

/// Rejection of a handle, ticket, or record from another service epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceEpochMismatch {
    current: ServiceEpoch,
    received: ServiceEpoch,
}

impl ServiceEpochMismatch {
    /// Return the epoch owned by the validating service.
    pub fn current(self) -> ServiceEpoch {
        self.current
    }

    /// Return the mismatched epoch carried by the retained value.
    pub fn received(self) -> ServiceEpoch {
        self.received
    }
}

impl fmt::Display for ServiceEpochMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "service epoch mismatch: current {}, received {}",
            self.current.get(),
            self.received.get()
        )
    }
}

impl std::error::Error for ServiceEpochMismatch {}

/// Explicit bounded capacities for one native service instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceConfig {
    ingress_capacity: NonZeroUsize,
    egress_capacity: NonZeroUsize,
    turn_budget: NonZeroUsize,
}

impl ServiceConfig {
    /// Validate explicit ingress, egress, and per-turn capacities.
    ///
    /// This constructor intentionally has no defaults. CPY-03/09 must select
    /// normative values only after representative host and board evidence.
    pub fn new(
        ingress_capacity: usize,
        egress_capacity: usize,
        turn_budget: usize,
    ) -> Result<Self, ServiceConfigError> {
        Ok(Self {
            ingress_capacity: NonZeroUsize::new(ingress_capacity)
                .ok_or(ServiceConfigError::ZeroIngressCapacity)?,
            egress_capacity: NonZeroUsize::new(egress_capacity)
                .ok_or(ServiceConfigError::ZeroEgressCapacity)?,
            turn_budget: NonZeroUsize::new(turn_budget)
                .ok_or(ServiceConfigError::ZeroTurnBudget)?,
        })
    }

    /// Return the bounded ingress queue capacity.
    pub fn ingress_capacity(self) -> usize {
        self.ingress_capacity.get()
    }

    /// Return the bounded egress queue capacity.
    pub fn egress_capacity(self) -> usize {
        self.egress_capacity.get()
    }

    /// Return the maximum requests admitted to one service turn.
    pub fn turn_budget(self) -> usize {
        self.turn_budget.get()
    }
}

/// Invalid zero-valued native service capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceConfigError {
    /// Ingress must hold at least one request.
    ZeroIngressCapacity,
    /// Egress must hold at least one record.
    ZeroEgressCapacity,
    /// A service turn must admit at least one request.
    ZeroTurnBudget,
}

impl fmt::Display for ServiceConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroIngressCapacity => "native service ingress capacity must be positive",
            Self::ZeroEgressCapacity => "native service egress capacity must be positive",
            Self::ZeroTurnBudget => "native service turn budget must be positive",
        })
    }
}

impl std::error::Error for ServiceConfigError {}

/// Explicit protected-capacity policy for services that project Endpoint records.
///
/// The reserve must hold one terminal record for every request in the largest
/// admitted turn. Ordinary Endpoint records cannot consume these slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointServiceConfig {
    service: ServiceConfig,
    critical_egress_reserve: NonZeroUsize,
}

impl EndpointServiceConfig {
    /// Validate one explicit critical reserve against the service capacities.
    pub fn new(
        service: ServiceConfig,
        critical_egress_reserve: usize,
    ) -> Result<Self, EndpointServiceConfigError> {
        let critical_egress_reserve = NonZeroUsize::new(critical_egress_reserve)
            .ok_or(EndpointServiceConfigError::ZeroCriticalEgressReserve)?;
        if critical_egress_reserve.get() >= service.egress_capacity() {
            return Err(EndpointServiceConfigError::NoOrdinaryEgressCapacity);
        }
        if critical_egress_reserve.get() < service.turn_budget() {
            return Err(EndpointServiceConfigError::ReserveBelowTurnBudget {
                reserve: critical_egress_reserve.get(),
                turn_budget: service.turn_budget(),
            });
        }
        Ok(Self {
            service,
            critical_egress_reserve,
        })
    }

    /// Return the shared ingress, egress, and turn capacities.
    pub fn service(self) -> ServiceConfig {
        self.service
    }

    /// Return the egress slots protected from ordinary Endpoint records.
    pub fn critical_egress_reserve(self) -> usize {
        self.critical_egress_reserve.get()
    }

    /// Return the egress slots available to ordinary Endpoint records.
    pub fn ordinary_egress_capacity(self) -> usize {
        self.service.egress_capacity() - self.critical_egress_reserve.get()
    }
}

/// Invalid protected-capacity policy for an Endpoint-record service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointServiceConfigError {
    /// At least one slot must be protected from ordinary Endpoint records.
    ZeroCriticalEgressReserve,
    /// The reserve must leave at least one slot for ordinary Endpoint records.
    NoOrdinaryEgressCapacity,
    /// The reserve cannot hold one terminal record per maximum-size turn.
    ReserveBelowTurnBudget {
        /// Configured protected egress slots.
        reserve: usize,
        /// Configured maximum requests per turn.
        turn_budget: usize,
    },
}

impl fmt::Display for EndpointServiceConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCriticalEgressReserve => {
                formatter.write_str("Endpoint service critical egress reserve must be positive")
            }
            Self::NoOrdinaryEgressCapacity => formatter
                .write_str("Endpoint service critical reserve must leave ordinary capacity"),
            Self::ReserveBelowTurnBudget {
                reserve,
                turn_budget,
            } => write!(
                formatter,
                "Endpoint service critical reserve {reserve} is below turn budget {turn_budget}"
            ),
        }
    }
}

impl std::error::Error for EndpointServiceConfigError {}

/// Closed lifecycle of one native service epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceLifecycle {
    /// The owner thread is constructing its non-`Send` state.
    Constructing = 0,
    /// The service accepts requests and runs bounded turns.
    Running = 1,
    /// The close fence rejects new requests while accepted work terminates.
    Closing = 2,
    /// A native turn or readiness failure stopped normal execution.
    Faulted = 3,
    /// The owner has destroyed its state and no more records can be emitted.
    Closed = 4,
}

impl ServiceLifecycle {
    fn from_atomic(value: u8) -> Self {
        match value {
            0 => Self::Constructing,
            1 => Self::Running,
            2 => Self::Closing,
            3 => Self::Faulted,
            4 => Self::Closed,
            _ => unreachable!("service lifecycle atomic contains an invalid value"),
        }
    }
}

/// Reason an already accepted request did not enter a native turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceRejection {
    /// The close fence linearized before this request's turn began.
    ServiceClosing,
    /// An earlier native turn faulted the service.
    ServiceFaulted,
}

/// Runtime-owned failure that is independent of the driver's fault type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFault {
    /// The native turn closure panicked; the payload was not projected.
    TurnPanicked,
    /// The turn returned a different outcome count than its admitted batch.
    OutcomeCountMismatch {
        /// Number of requests admitted to the turn.
        expected: usize,
        /// Number of outcomes returned by the turn.
        actual: usize,
    },
    /// Endpoint records were not strictly increasing in canonical sequence order.
    EndpointRecordOrder {
        /// Last accepted Endpoint sequence in this service epoch.
        previous: u32,
        /// Regressed or duplicated Endpoint sequence offered next.
        offered: u32,
    },
    /// The operating-system readiness primitive failed.
    Readiness(io::ErrorKind),
}

/// CPY transport class derived from one neutral egress record.
///
/// Classification never authorizes CPY-side cue coalescing. It preserves the
/// class already selected by the canonical Endpoint queue; any represented
/// loss or coalescing remains encoded in that owned neutral record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceRecordClass {
    /// Lifecycle, result, fault, rejection, RuntimeNotice, or Critical Cue.
    Critical,
    /// An admitted Cue that must remain ordered and non-coalesced.
    Ordered,
    /// A Cue already coalesced, when applicable, by the canonical Endpoint.
    LatestValueCoalescible,
}

/// Ordered egress record emitted by a native service.
#[derive(Debug, PartialEq, Eq)]
pub enum ServiceRecord<Output, Fault> {
    /// Observable lifecycle transition for this service epoch.
    Lifecycle {
        /// Service instance that made the transition.
        epoch: ServiceEpoch,
        /// New lifecycle state.
        state: ServiceLifecycle,
    },
    /// Successful terminal result for exactly one accepted request.
    Completed {
        /// Accepted request identity.
        ticket: ServiceTicket,
        /// Interpreter-neutral driver result.
        output: Output,
    },
    /// Driver-defined terminal fault for exactly one accepted request.
    DriverFault {
        /// Accepted request identity.
        ticket: ServiceTicket,
        /// Exact interpreter-neutral driver fault.
        fault: Fault,
    },
    /// Runtime-owned terminal fault for exactly one accepted request.
    RuntimeFault {
        /// Accepted request identity.
        ticket: ServiceTicket,
        /// Exact runtime failure class.
        fault: RuntimeFault,
    },
    /// Runtime-owned service fault that is not a request outcome.
    ServiceFault {
        /// Service instance whose infrastructure failed.
        epoch: ServiceEpoch,
        /// Exact runtime failure class.
        fault: RuntimeFault,
    },
    /// Terminal rejection for a request accepted before a close/fault fence.
    Rejected {
        /// Accepted request identity.
        ticket: ServiceTicket,
        /// Fence that prevented the request's turn from beginning.
        reason: ServiceRejection,
    },
    /// Canonical cue or RuntimeNotice drained from the owned Endpoint.
    Endpoint {
        /// Service instance that owns the Endpoint drain.
        epoch: ServiceEpoch,
        /// Unmodified neutral record with canonical sequence/loss metadata.
        record: EndpointRecord,
    },
}

impl<Output, Fault> ServiceRecord<Output, Fault> {
    /// Return the service epoch that emitted this record.
    pub fn epoch(&self) -> ServiceEpoch {
        match self {
            Self::Lifecycle { epoch, .. }
            | Self::ServiceFault { epoch, .. }
            | Self::Endpoint { epoch, .. } => *epoch,
            Self::Completed { ticket, .. }
            | Self::DriverFault { ticket, .. }
            | Self::RuntimeFault { ticket, .. }
            | Self::Rejected { ticket, .. } => ticket.epoch(),
        }
    }

    /// Return the request ticket when this is a terminal request record.
    pub fn ticket(&self) -> Option<ServiceTicket> {
        match self {
            Self::Lifecycle { .. } | Self::ServiceFault { .. } | Self::Endpoint { .. } => None,
            Self::Completed { ticket, .. }
            | Self::DriverFault { ticket, .. }
            | Self::RuntimeFault { ticket, .. }
            | Self::Rejected { ticket, .. } => Some(*ticket),
        }
    }

    /// Return the canonical Endpoint record, when this is an Endpoint projection.
    pub fn endpoint_record(&self) -> Option<&EndpointRecord> {
        match self {
            Self::Endpoint { record, .. } => Some(record),
            _ => None,
        }
    }

    /// Derive the non-droppable or ordinary transport class for this record.
    pub fn class(&self) -> ServiceRecordClass {
        match self {
            Self::Endpoint {
                record: EndpointRecord::Cue(cue),
                ..
            } => match cue.delivery() {
                CueDelivery::Critical => ServiceRecordClass::Critical,
                CueDelivery::Ordered => ServiceRecordClass::Ordered,
                CueDelivery::LatestValueCoalescible => ServiceRecordClass::LatestValueCoalescible,
            },
            Self::Endpoint {
                record: EndpointRecord::RuntimeNotice(_),
                ..
            }
            | Self::Lifecycle { .. }
            | Self::Completed { .. }
            | Self::DriverFault { .. }
            | Self::RuntimeFault { .. }
            | Self::ServiceFault { .. }
            | Self::Rejected { .. } => ServiceRecordClass::Critical,
        }
    }
}

/// Output of one Endpoint-owning native service turn.
///
/// Outcomes correspond positionally to admitted requests. Endpoint records
/// must remain in the exact order returned by the canonical Endpoint drain.
#[derive(Debug, PartialEq, Eq)]
pub struct EndpointServiceTurn<Output, Fault> {
    outcomes: Vec<Result<Output, Fault>>,
    endpoint_records: Vec<EndpointRecord>,
}

impl<Output, Fault> EndpointServiceTurn<Output, Fault> {
    /// Construct one turn from terminal outcomes and an owned Endpoint drain.
    pub fn new(
        outcomes: Vec<Result<Output, Fault>>,
        endpoint_records: Vec<EndpointRecord>,
    ) -> Self {
        Self {
            outcomes,
            endpoint_records,
        }
    }

    /// Borrow the positional terminal outcomes.
    pub fn outcomes(&self) -> &[Result<Output, Fault>] {
        &self.outcomes
    }

    /// Borrow the canonical Endpoint records in drain order.
    pub fn endpoint_records(&self) -> &[EndpointRecord] {
        &self.endpoint_records
    }

    fn into_parts(self) -> (Vec<Result<Output, Fault>>, Vec<EndpointRecord>) {
        (self.outcomes, self.endpoint_records)
    }
}

/// Typed failure to admit a caller-owned request.
#[derive(Debug, PartialEq, Eq)]
pub enum AdmissionError<Request> {
    /// The bounded ingress queue is currently full.
    Full(Request),
    /// The close fence has linearized.
    Closing(Request),
    /// The native service has faulted.
    Faulted(Request),
    /// The native service is closed or disconnected.
    Closed(Request),
    /// No further nonzero request id can be allocated in this epoch.
    RequestIdExhausted(Request),
}

impl<Request> AdmissionError<Request> {
    /// Recover the caller-owned request that was not accepted.
    pub fn into_request(self) -> Request {
        match self {
            Self::Full(request)
            | Self::Closing(request)
            | Self::Faulted(request)
            | Self::Closed(request)
            | Self::RequestIdExhausted(request) => request,
        }
    }
}

/// Failure to construct and publish a running native service.
#[derive(Debug)]
pub enum ServiceStartError {
    /// The readiness descriptor or native thread could not be created.
    Io(io::Error),
    /// The process-local monotonic service epoch space is exhausted.
    EpochExhausted,
    /// The owner-state builder panicked; its payload was not projected.
    OwnerBuildPanicked,
    /// Initial readiness publication failed before the service became usable.
    Readiness(io::ErrorKind),
    /// The owner exited before publishing its construction outcome.
    OwnerExitedDuringConstruction,
}

impl fmt::Display for ServiceStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "native service start failed: {error}"),
            Self::EpochExhausted => formatter.write_str("native service epoch space exhausted"),
            Self::OwnerBuildPanicked => formatter.write_str("native service owner build panicked"),
            Self::Readiness(kind) => {
                write!(
                    formatter,
                    "native service readiness initialization failed: {kind:?}"
                )
            }
            Self::OwnerExitedDuringConstruction => {
                formatter.write_str("native service owner exited during construction")
            }
        }
    }
}

impl std::error::Error for ServiceStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Failure while joining an explicitly closed native service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceJoinError {
    /// The owner thread unwound outside the guarded driver boundary.
    OwnerPanicked,
    /// Egress disconnected before the ordered `Closed` record arrived.
    ClosedRecordMissing,
}

impl fmt::Display for ServiceJoinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OwnerPanicked => "native service owner thread panicked",
            Self::ClosedRecordMissing => "native service closed without an ordered Closed record",
        })
    }
}

impl std::error::Error for ServiceJoinError {}

/// Point-in-time native service accounting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceMetricsSnapshot {
    /// Requests accepted into ingress.
    pub accepted_requests: u64,
    /// Caller observations of a full ingress queue.
    pub ingress_full_observations: u64,
    /// Successful admissions that observed ingress empty before publication.
    pub ingress_empty_to_nonempty_observations: u64,
    /// Native turns begun.
    pub service_turns: u64,
    /// Successful request results emitted.
    pub completed_requests: u64,
    /// Driver/runtime terminal faults emitted for accepted requests.
    pub faulted_requests: u64,
    /// Accepted requests rejected by a close or fault fence.
    pub rejected_requests: u64,
    /// Maximum sampled ingress depth.
    pub peak_ingress_depth: usize,
    /// Maximum sampled egress depth.
    pub peak_egress_depth: usize,
    /// Egress publications that waited for queue or class-lane capacity.
    pub egress_backpressured_records: u64,
    /// Time spent waiting for bounded egress capacity.
    pub egress_backpressure_ns: u64,
    /// Successful empty-to-nonempty/coalesced readiness writes.
    pub readiness_notifications: u64,
    /// Operating-system readiness write failures.
    pub readiness_failures: u64,
}

#[derive(Debug, Default)]
struct Metrics {
    accepted_requests: AtomicU64,
    ingress_full_observations: AtomicU64,
    ingress_empty_to_nonempty_observations: AtomicU64,
    service_turns: AtomicU64,
    completed_requests: AtomicU64,
    faulted_requests: AtomicU64,
    rejected_requests: AtomicU64,
    peak_ingress_depth: AtomicUsize,
    peak_egress_depth: AtomicUsize,
    egress_backpressured_records: AtomicU64,
    egress_backpressure_ns: AtomicU64,
    readiness_notifications: AtomicU64,
    readiness_failures: AtomicU64,
}

impl Metrics {
    fn snapshot(&self) -> ServiceMetricsSnapshot {
        ServiceMetricsSnapshot {
            accepted_requests: self.accepted_requests.load(Ordering::Relaxed),
            ingress_full_observations: self.ingress_full_observations.load(Ordering::Relaxed),
            ingress_empty_to_nonempty_observations: self
                .ingress_empty_to_nonempty_observations
                .load(Ordering::Relaxed),
            service_turns: self.service_turns.load(Ordering::Relaxed),
            completed_requests: self.completed_requests.load(Ordering::Relaxed),
            faulted_requests: self.faulted_requests.load(Ordering::Relaxed),
            rejected_requests: self.rejected_requests.load(Ordering::Relaxed),
            peak_ingress_depth: self.peak_ingress_depth.load(Ordering::Relaxed),
            peak_egress_depth: self.peak_egress_depth.load(Ordering::Relaxed),
            egress_backpressured_records: self.egress_backpressured_records.load(Ordering::Relaxed),
            egress_backpressure_ns: self.egress_backpressure_ns.load(Ordering::Relaxed),
            readiness_notifications: self.readiness_notifications.load(Ordering::Relaxed),
            readiness_failures: self.readiness_failures.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
struct RequestEnvelope<Request> {
    ticket: ServiceTicket,
    request: Request,
}

#[derive(Debug)]
struct Shared {
    lifecycle: AtomicU8,
    next_request_id: AtomicU64,
    admission_gate: Mutex<()>,
    drain_gate: Mutex<()>,
    metrics: Metrics,
}

impl Shared {
    fn lifecycle(&self) -> ServiceLifecycle {
        ServiceLifecycle::from_atomic(self.lifecycle.load(Ordering::Acquire))
    }

    fn set_lifecycle(&self, lifecycle: ServiceLifecycle) {
        self.lifecycle.store(lifecycle as u8, Ordering::Release);
    }
}

#[derive(Debug)]
struct EgressPermit {
    release: Sender<()>,
}

impl Drop for EgressPermit {
    fn drop(&mut self) {
        let _ = self.release.try_send(());
    }
}

#[derive(Debug)]
struct QueuedRecord<Output, Fault> {
    record: ServiceRecord<Output, Fault>,
    _permit: EgressPermit,
}

impl<Output, Fault> QueuedRecord<Output, Fault> {
    fn into_record(self) -> ServiceRecord<Output, Fault> {
        self.record
    }
}

#[derive(Debug)]
struct PermitLane {
    available: Receiver<()>,
    release: Sender<()>,
}

impl PermitLane {
    fn new(capacity: usize) -> Self {
        let (release, available) = bounded(capacity);
        for _ in 0..capacity {
            release
                .send(())
                .expect("new permit lane retains its receiver");
        }
        Self { available, release }
    }

    fn try_acquire(&self) -> Result<Option<EgressPermit>, PublishError> {
        match self.available.try_recv() {
            Ok(()) => Ok(Some(EgressPermit {
                release: self.release.clone(),
            })),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(PublishError::Disconnected),
        }
    }

    fn acquire(&self, metrics: &Metrics) -> Result<EgressPermit, PublishError> {
        if let Some(permit) = self.try_acquire()? {
            return Ok(permit);
        }

        let started = Instant::now();
        metrics
            .egress_backpressured_records
            .fetch_add(1, Ordering::Relaxed);
        self.available
            .recv()
            .map_err(|_| PublishError::Disconnected)?;
        metrics
            .egress_backpressure_ns
            .fetch_add(duration_ns(started.elapsed()), Ordering::Relaxed);
        Ok(EgressPermit {
            release: self.release.clone(),
        })
    }
}

#[derive(Debug)]
struct EgressPermits {
    ordinary: Option<PermitLane>,
    critical: PermitLane,
}

#[derive(Debug)]
enum TerminalPermits {
    OnDemand,
    Reserved(Vec<EgressPermit>),
}

impl EgressPermits {
    fn unpartitioned(capacity: usize) -> Self {
        Self {
            ordinary: None,
            critical: PermitLane::new(capacity),
        }
    }

    fn endpoint(config: EndpointServiceConfig) -> Self {
        Self {
            ordinary: Some(PermitLane::new(config.ordinary_egress_capacity())),
            critical: PermitLane::new(config.critical_egress_reserve()),
        }
    }

    fn acquire(
        &self,
        class: ServiceRecordClass,
        metrics: &Metrics,
    ) -> Result<EgressPermit, PublishError> {
        match class {
            ServiceRecordClass::Critical => {
                if let Some(ordinary) = &self.ordinary
                    && let Some(permit) = ordinary.try_acquire()?
                {
                    return Ok(permit);
                }
                self.critical.acquire(metrics)
            }
            ServiceRecordClass::Ordered | ServiceRecordClass::LatestValueCoalescible => self
                .ordinary
                .as_ref()
                .expect("ordinary records require partitioned egress")
                .acquire(metrics),
        }
    }

    fn reserve_terminals(
        &self,
        count: usize,
        metrics: &Metrics,
    ) -> Result<TerminalPermits, PublishError> {
        if self.ordinary.is_none() {
            return Ok(TerminalPermits::OnDemand);
        }
        Ok(TerminalPermits::Reserved(
            (0..count)
                .map(|_| self.critical.acquire(metrics))
                .collect::<Result<_, _>>()?,
        ))
    }
}

/// Running Python-neutral native service and its single ordered egress queue.
///
/// The service owns no Python objects and invokes no language callback. The
/// supplied state is constructed, used, and destroyed solely on the owner
/// thread even when that state is not [`Send`].
#[derive(Debug)]
pub struct NativeService<Request, Output, Fault> {
    epoch: ServiceEpoch,
    config: ServiceConfig,
    endpoint_config: Option<EndpointServiceConfig>,
    ingress: Sender<RequestEnvelope<Request>>,
    egress: Receiver<QueuedRecord<Output, Fault>>,
    close: Sender<()>,
    shared: Arc<Shared>,
    readiness: ReadinessSignal,
    owner: Option<thread::JoinHandle<()>>,
}

impl<Request, Output, Fault> NativeService<Request, Output, Fault> {
    /// Return this service instance's monotonic epoch.
    pub fn epoch(&self) -> ServiceEpoch {
        self.epoch
    }

    /// Reject a retained handle epoch that does not belong to this service.
    ///
    /// Adapter layers call this before binding a Stage, Actor, subscription,
    /// request, or other epoch-bearing handle to a restarted service.
    pub fn validate_epoch(&self, epoch: ServiceEpoch) -> Result<(), ServiceEpochMismatch> {
        if epoch == self.epoch {
            Ok(())
        } else {
            Err(ServiceEpochMismatch {
                current: self.epoch,
                received: epoch,
            })
        }
    }

    /// Reject a request ticket retained from another service epoch.
    pub fn validate_ticket(&self, ticket: ServiceTicket) -> Result<(), ServiceEpochMismatch> {
        self.validate_epoch(ticket.epoch())
    }

    /// Reject an egress record retained from another service epoch.
    pub fn validate_record(
        &self,
        record: &ServiceRecord<Output, Fault>,
    ) -> Result<(), ServiceEpochMismatch> {
        self.validate_epoch(record.epoch())
    }

    /// Return the explicit capacities used by this service.
    pub fn config(&self) -> ServiceConfig {
        self.config
    }

    /// Return protected Endpoint-record capacity when this is an Endpoint service.
    pub fn endpoint_config(&self) -> Option<EndpointServiceConfig> {
        self.endpoint_config
    }

    /// Return the current lifecycle state.
    pub fn lifecycle(&self) -> ServiceLifecycle {
        self.shared.lifecycle()
    }

    /// Return the pollable readiness handle for the ordered egress queue.
    pub fn readiness(&self) -> &ReadinessSignal {
        &self.readiness
    }

    /// Return a point-in-time accounting snapshot.
    pub fn metrics(&self) -> ServiceMetricsSnapshot {
        self.shared.metrics.snapshot()
    }

    /// Attempt one nonblocking admission without losing caller ownership.
    pub fn try_submit(&self, request: Request) -> Result<ServiceTicket, AdmissionError<Request>> {
        let _gate = self
            .shared
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match self.shared.lifecycle() {
            ServiceLifecycle::Running => {}
            ServiceLifecycle::Constructing | ServiceLifecycle::Closing => {
                return Err(AdmissionError::Closing(request));
            }
            ServiceLifecycle::Faulted => return Err(AdmissionError::Faulted(request)),
            ServiceLifecycle::Closed => return Err(AdmissionError::Closed(request)),
        }
        let Some(request_id) = self
            .shared
            .next_request_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(NonZeroU64::new)
        else {
            return Err(AdmissionError::RequestIdExhausted(request));
        };
        let ticket = ServiceTicket {
            epoch: self.epoch,
            request_id: RequestId(request_id),
        };
        let was_empty = self.ingress.is_empty();
        match self.ingress.try_send(RequestEnvelope { ticket, request }) {
            Ok(()) => {
                self.shared
                    .metrics
                    .accepted_requests
                    .fetch_add(1, Ordering::Relaxed);
                if was_empty {
                    self.shared
                        .metrics
                        .ingress_empty_to_nonempty_observations
                        .fetch_add(1, Ordering::Relaxed);
                }
                update_max(
                    &self.shared.metrics.peak_ingress_depth,
                    self.ingress.len().max(1),
                );
                Ok(ticket)
            }
            Err(TrySendError::Full(envelope)) => {
                self.shared
                    .metrics
                    .ingress_full_observations
                    .fetch_add(1, Ordering::Relaxed);
                Err(AdmissionError::Full(envelope.request))
            }
            Err(TrySendError::Disconnected(envelope)) => {
                Err(AdmissionError::Closed(envelope.request))
            }
        }
    }

    /// Establish the idempotent close fence and wake the owner thread.
    ///
    /// Returns `true` only for the caller that first moves `Running` to
    /// `Closing`. The owner finishes an active turn, rejects queued work, and
    /// emits ordered `Closing` then `Closed` lifecycle records.
    pub fn request_close(&self) -> bool {
        let _gate = self
            .shared
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.shared.lifecycle() != ServiceLifecycle::Running {
            return false;
        }
        self.shared.set_lifecycle(ServiceLifecycle::Closing);
        let _ = self.close.try_send(());
        true
    }

    /// Drain every record currently visible after clearing readiness.
    pub fn drain(&self) -> io::Result<Vec<ServiceRecord<Output, Fault>>> {
        self.drain_records(usize::MAX)
    }

    /// Drain at most the positive caller-supplied number of ordered records.
    pub fn drain_up_to(
        &self,
        limit: NonZeroUsize,
    ) -> io::Result<Vec<ServiceRecord<Output, Fault>>> {
        self.drain_records(limit.get())
    }

    fn drain_records(&self, limit: usize) -> io::Result<Vec<ServiceRecord<Output, Fault>>> {
        let _gate = self
            .shared
            .drain_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.readiness.clear_before_drain()?;
        let records: Vec<_> = self
            .egress
            .try_iter()
            .take(limit)
            .map(QueuedRecord::into_record)
            .collect();
        self.readiness.finish_drain(!self.egress.is_empty())?;
        Ok(records)
    }

    /// Close, drain through the ordered `Closed` record, and join the owner.
    ///
    /// This explicit path cannot deadlock on a full egress queue because it
    /// consumes records while the owner completes shutdown.
    pub fn shutdown(mut self) -> Result<Vec<ServiceRecord<Output, Fault>>, ServiceJoinError> {
        self.request_close();
        let mut records = Vec::new();
        let mut saw_closed = false;
        while let Ok(queued) = self.egress.recv() {
            let record = queued.into_record();
            saw_closed |= matches!(
                record,
                ServiceRecord::Lifecycle {
                    state: ServiceLifecycle::Closed,
                    ..
                }
            );
            records.push(record);
            if saw_closed {
                records.extend(self.egress.try_iter().map(QueuedRecord::into_record));
                break;
            }
        }
        let owner = self.owner.take().expect("native service owner joined once");
        if owner.join().is_err() {
            return Err(ServiceJoinError::OwnerPanicked);
        }
        if !saw_closed {
            return Err(ServiceJoinError::ClosedRecordMissing);
        }
        Ok(records)
    }
}

impl<Request, Output, Fault> Drop for NativeService<Request, Output, Fault> {
    fn drop(&mut self) {
        if self.owner.is_some() {
            self.request_close();
        }
    }
}

/// Spawn one bounded service whose non-`Send` state never leaves its owner.
///
/// `build` executes after the native thread starts. `run_turn` receives at
/// most `config.turn_budget()` owned requests and must return exactly one
/// ordered terminal outcome per request. Any driver fault or runtime contract
/// failure faults the service after the admitted turn completes.
pub fn spawn_native_service<State, Request, Output, Fault, Build, RunTurn>(
    name: impl Into<String>,
    config: ServiceConfig,
    build: Build,
    run_turn: RunTurn,
) -> Result<NativeService<Request, Output, Fault>, ServiceStartError>
where
    State: 'static,
    Request: Send + 'static,
    Output: Send + 'static,
    Fault: Send + 'static,
    Build: FnOnce() -> State + Send + 'static,
    RunTurn: FnMut(&mut State, Vec<Request>) -> Vec<Result<Output, Fault>> + Send + 'static,
{
    let mut run_turn = run_turn;
    spawn_service(
        name,
        config,
        None,
        EgressPermits::unpartitioned(config.egress_capacity()),
        build,
        move |state, requests| EndpointServiceTurn::new(run_turn(state, requests), Vec::new()),
    )
}

/// Spawn a bounded native service that projects canonical Endpoint records.
///
/// One protected critical permit is reserved before native execution for each
/// admitted request. The turn closure returns exactly one positional terminal
/// outcome per request plus Endpoint records in canonical drain order. CPY
/// transports those records without coalescing or dropping them.
pub fn spawn_native_endpoint_service<State, Request, Output, Fault, Build, RunTurn>(
    name: impl Into<String>,
    config: EndpointServiceConfig,
    build: Build,
    run_turn: RunTurn,
) -> Result<NativeService<Request, Output, Fault>, ServiceStartError>
where
    State: 'static,
    Request: Send + 'static,
    Output: Send + 'static,
    Fault: Send + 'static,
    Build: FnOnce() -> State + Send + 'static,
    RunTurn: FnMut(&mut State, Vec<Request>) -> EndpointServiceTurn<Output, Fault> + Send + 'static,
{
    spawn_service(
        name,
        config.service(),
        Some(config),
        EgressPermits::endpoint(config),
        build,
        run_turn,
    )
}

fn spawn_service<State, Request, Output, Fault, Build, RunTurn>(
    name: impl Into<String>,
    config: ServiceConfig,
    endpoint_config: Option<EndpointServiceConfig>,
    permits: EgressPermits,
    build: Build,
    run_turn: RunTurn,
) -> Result<NativeService<Request, Output, Fault>, ServiceStartError>
where
    State: 'static,
    Request: Send + 'static,
    Output: Send + 'static,
    Fault: Send + 'static,
    Build: FnOnce() -> State + Send + 'static,
    RunTurn: FnMut(&mut State, Vec<Request>) -> EndpointServiceTurn<Output, Fault> + Send + 'static,
{
    let epoch = next_epoch().ok_or(ServiceStartError::EpochExhausted)?;
    let (ingress_sender, ingress_receiver) = bounded(config.ingress_capacity());
    let (egress_sender, egress_receiver) = bounded(config.egress_capacity());
    let (close_sender, close_receiver) = bounded(1);
    let (initialized_sender, initialized_receiver) = bounded(1);
    let (readiness, notifier) = new_pair().map_err(ServiceStartError::Io)?;
    let shared = Arc::new(Shared {
        lifecycle: AtomicU8::new(ServiceLifecycle::Constructing as u8),
        next_request_id: AtomicU64::new(1),
        admission_gate: Mutex::new(()),
        drain_gate: Mutex::new(()),
        metrics: Metrics::default(),
    });
    let owner_shared = Arc::clone(&shared);
    let owner = thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let state = match catch_unwind(AssertUnwindSafe(build)) {
                Ok(state) => state,
                Err(_) => {
                    owner_shared.set_lifecycle(ServiceLifecycle::Closed);
                    let _ = initialized_sender.send(Err(RuntimeFault::TurnPanicked));
                    return;
                }
            };
            owner_shared.set_lifecycle(ServiceLifecycle::Running);
            if let Err(error) = publish(
                &egress_sender,
                &notifier,
                &owner_shared.metrics,
                &permits,
                ServiceRecord::Lifecycle {
                    epoch,
                    state: ServiceLifecycle::Running,
                },
            ) {
                drop(state);
                owner_shared.set_lifecycle(ServiceLifecycle::Closed);
                let kind = match error {
                    PublishError::Disconnected => io::ErrorKind::BrokenPipe,
                    PublishError::Readiness(kind) => kind,
                };
                let _ = initialized_sender.send(Err(RuntimeFault::Readiness(kind)));
                return;
            }
            if initialized_sender.send(Ok(())).is_err() {
                drop(state);
                owner_shared.set_lifecycle(ServiceLifecycle::Closed);
                return;
            }
            run_owner(
                state,
                run_turn,
                config,
                epoch,
                ingress_receiver,
                egress_sender,
                close_receiver,
                notifier,
                owner_shared,
                permits,
            );
        })
        .map_err(ServiceStartError::Io)?;

    match initialized_receiver.recv() {
        Ok(Ok(())) => Ok(NativeService {
            epoch,
            config,
            endpoint_config,
            ingress: ingress_sender,
            egress: egress_receiver,
            close: close_sender,
            shared,
            readiness,
            owner: Some(owner),
        }),
        Ok(Err(RuntimeFault::TurnPanicked)) => {
            let _ = owner.join();
            Err(ServiceStartError::OwnerBuildPanicked)
        }
        Ok(Err(RuntimeFault::Readiness(kind))) => {
            let _ = owner.join();
            Err(ServiceStartError::Readiness(kind))
        }
        Ok(Err(_)) => {
            let _ = owner.join();
            Err(ServiceStartError::OwnerExitedDuringConstruction)
        }
        Err(_) => {
            let _ = owner.join();
            Err(ServiceStartError::OwnerExitedDuringConstruction)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_owner<State, Request, Output, Fault, RunTurn>(
    state: State,
    mut run_turn: RunTurn,
    config: ServiceConfig,
    epoch: ServiceEpoch,
    ingress: Receiver<RequestEnvelope<Request>>,
    egress: Sender<QueuedRecord<Output, Fault>>,
    close: Receiver<()>,
    notifier: Notifier,
    shared: Arc<Shared>,
    permits: EgressPermits,
) where
    RunTurn: FnMut(&mut State, Vec<Request>) -> EndpointServiceTurn<Output, Fault>,
{
    // Terminal helpers retain state through lifecycle/rejection accounting,
    // then take it immediately before making `Closed` observable.
    let mut state = Some(state);
    let mut last_endpoint_sequence = None;
    loop {
        let first = select_biased! {
            recv(close) -> _ => {
                close_owner(epoch, &ingress, &egress, &notifier, &shared, &permits, ServiceRejection::ServiceClosing, &mut state);
                return;
            }
            recv(ingress) -> request => match request {
                Ok(request) => request,
                Err(_) => {
                    close_owner(epoch, &ingress, &egress, &notifier, &shared, &permits, ServiceRejection::ServiceClosing, &mut state);
                    return;
                }
            }
        };

        let mut envelopes = Vec::with_capacity(config.turn_budget());
        envelopes.push(first);
        while envelopes.len() < config.turn_budget() {
            match ingress.try_recv() {
                Ok(request) => envelopes.push(request),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        shared.metrics.service_turns.fetch_add(1, Ordering::Relaxed);
        let tickets: Vec<_> = envelopes.iter().map(|envelope| envelope.ticket).collect();
        let terminal_permits = match permits.reserve_terminals(tickets.len(), &shared.metrics) {
            Ok(permits) => permits,
            Err(PublishError::Disconnected | PublishError::Readiness(_)) => {
                finish_disconnected(&mut state, &shared);
                return;
            }
        };
        let requests = envelopes
            .into_iter()
            .map(|envelope| envelope.request)
            .collect();
        let turn = catch_unwind(AssertUnwindSafe(|| {
            run_turn(
                state.as_mut().expect("owner state exists during a turn"),
                requests,
            )
        }));
        let disposition = match turn {
            Ok(turn) => {
                let (outcomes, endpoint_records) = turn.into_parts();
                if outcomes.len() != tickets.len() {
                    let fault = RuntimeFault::OutcomeCountMismatch {
                        expected: tickets.len(),
                        actual: outcomes.len(),
                    };
                    publish_runtime_faults(
                        tickets,
                        terminal_permits,
                        fault,
                        &egress,
                        &notifier,
                        &shared.metrics,
                        &permits,
                    )
                    .map(|()| true)
                } else if let Err(fault) =
                    validate_endpoint_order(last_endpoint_sequence, &endpoint_records)
                {
                    publish_runtime_faults(
                        tickets,
                        terminal_permits,
                        fault,
                        &egress,
                        &notifier,
                        &shared.metrics,
                        &permits,
                    )
                    .map(|()| true)
                } else {
                    last_endpoint_sequence = endpoint_records
                        .last()
                        .map(|record| record.sequence().get())
                        .or(last_endpoint_sequence);
                    match publish_outcomes(
                        tickets,
                        outcomes,
                        terminal_permits,
                        &egress,
                        &notifier,
                        &shared.metrics,
                        &permits,
                    ) {
                        Ok(faulted) => publish_endpoint_records(
                            epoch,
                            endpoint_records,
                            &egress,
                            &notifier,
                            &shared.metrics,
                            &permits,
                        )
                        .map(|()| faulted),
                        Err(error) => Err(error),
                    }
                }
            }
            Err(_) => publish_runtime_faults(
                tickets,
                terminal_permits,
                RuntimeFault::TurnPanicked,
                &egress,
                &notifier,
                &shared.metrics,
                &permits,
            )
            .map(|()| true),
        };
        match disposition {
            Ok(false) => {}
            Ok(true) => {
                fault_owner(
                    epoch, &ingress, &egress, &notifier, &shared, &permits, &mut state,
                );
                return;
            }
            Err(PublishError::Readiness(kind)) => {
                infrastructure_fault_owner(
                    epoch,
                    RuntimeFault::Readiness(kind),
                    &ingress,
                    &egress,
                    &notifier,
                    &shared,
                    &permits,
                    &mut state,
                );
                return;
            }
            Err(PublishError::Disconnected) => {
                finish_disconnected(&mut state, &shared);
                return;
            }
        }
    }
}

fn publish_outcomes<Output, Fault>(
    tickets: Vec<ServiceTicket>,
    outcomes: Vec<Result<Output, Fault>>,
    terminal_permits: TerminalPermits,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    permits: &EgressPermits,
) -> Result<bool, PublishError> {
    let mut faulted = false;
    let mut publication_error = None;
    let mut reserved = match terminal_permits {
        TerminalPermits::OnDemand => None,
        TerminalPermits::Reserved(permits) => Some(permits.into_iter()),
    };
    for (ticket, outcome) in tickets.into_iter().zip(outcomes) {
        let record = match outcome {
            Ok(output) => {
                metrics.completed_requests.fetch_add(1, Ordering::Relaxed);
                ServiceRecord::Completed { ticket, output }
            }
            Err(fault) => {
                faulted = true;
                metrics.faulted_requests.fetch_add(1, Ordering::Relaxed);
                ServiceRecord::DriverFault { ticket, fault }
            }
        };
        let publication = match &mut reserved {
            Some(reserved) => publish_with_permit(
                egress,
                notifier,
                metrics,
                record,
                reserved.next().expect("one terminal permit per ticket"),
            ),
            None => publish(egress, notifier, metrics, permits, record),
        };
        match publication {
            Ok(()) => {}
            Err(PublishError::Readiness(kind)) => {
                publication_error.get_or_insert(PublishError::Readiness(kind));
            }
            Err(PublishError::Disconnected) => return Err(PublishError::Disconnected),
        }
    }
    publication_error.map_or(Ok(faulted), Err)
}

fn publish_runtime_faults<Output, Fault>(
    tickets: Vec<ServiceTicket>,
    terminal_permits: TerminalPermits,
    fault: RuntimeFault,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    permits: &EgressPermits,
) -> Result<(), PublishError> {
    let mut publication_error = None;
    let mut reserved = match terminal_permits {
        TerminalPermits::OnDemand => None,
        TerminalPermits::Reserved(permits) => Some(permits.into_iter()),
    };
    for ticket in tickets {
        metrics.faulted_requests.fetch_add(1, Ordering::Relaxed);
        let record = ServiceRecord::RuntimeFault { ticket, fault };
        let publication = match &mut reserved {
            Some(reserved) => publish_with_permit(
                egress,
                notifier,
                metrics,
                record,
                reserved.next().expect("one terminal permit per ticket"),
            ),
            None => publish(egress, notifier, metrics, permits, record),
        };
        match publication {
            Ok(()) => {}
            Err(PublishError::Readiness(kind)) => {
                publication_error.get_or_insert(PublishError::Readiness(kind));
            }
            Err(PublishError::Disconnected) => return Err(PublishError::Disconnected),
        }
    }
    publication_error.map_or(Ok(()), Err)
}

fn publish_endpoint_records<Output, Fault>(
    epoch: ServiceEpoch,
    records: Vec<EndpointRecord>,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    permits: &EgressPermits,
) -> Result<(), PublishError> {
    let mut publication_error = None;
    for record in records {
        match publish(
            egress,
            notifier,
            metrics,
            permits,
            ServiceRecord::Endpoint { epoch, record },
        ) {
            Ok(()) => {}
            Err(PublishError::Readiness(kind)) => {
                publication_error.get_or_insert(PublishError::Readiness(kind));
            }
            Err(PublishError::Disconnected) => return Err(PublishError::Disconnected),
        }
    }
    publication_error.map_or(Ok(()), Err)
}

fn validate_endpoint_order(
    mut previous: Option<u32>,
    records: &[EndpointRecord],
) -> Result<(), RuntimeFault> {
    for record in records {
        let offered = record.sequence().get();
        if previous.is_some_and(|previous| offered <= previous) {
            return Err(RuntimeFault::EndpointRecordOrder {
                previous: previous.expect("order regression has a previous sequence"),
                offered,
            });
        }
        previous = Some(offered);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn infrastructure_fault_owner<State, Request, Output, Fault>(
    epoch: ServiceEpoch,
    fault: RuntimeFault,
    ingress: &Receiver<RequestEnvelope<Request>>,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    shared: &Shared,
    permits: &EgressPermits,
    state: &mut Option<State>,
) {
    {
        let _gate = shared
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shared.set_lifecycle(ServiceLifecycle::Faulted);
    }
    let _ = publish(
        egress,
        notifier,
        &shared.metrics,
        permits,
        ServiceRecord::ServiceFault { epoch, fault },
    );
    let _ = publish(
        egress,
        notifier,
        &shared.metrics,
        permits,
        ServiceRecord::Lifecycle {
            epoch,
            state: ServiceLifecycle::Faulted,
        },
    );
    reject_queued(
        ingress,
        egress,
        notifier,
        &shared.metrics,
        permits,
        ServiceRejection::ServiceFaulted,
    );
    finish_closed_after_destruction(state, epoch, egress, notifier, shared, permits);
}

fn fault_owner<State, Request, Output, Fault>(
    epoch: ServiceEpoch,
    ingress: &Receiver<RequestEnvelope<Request>>,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    shared: &Shared,
    permits: &EgressPermits,
    state: &mut Option<State>,
) {
    {
        let _gate = shared
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shared.set_lifecycle(ServiceLifecycle::Faulted);
    }
    if matches!(
        publish(
            egress,
            notifier,
            &shared.metrics,
            permits,
            ServiceRecord::Lifecycle {
                epoch,
                state: ServiceLifecycle::Faulted,
            },
        ),
        Err(PublishError::Disconnected)
    ) {
        finish_disconnected(state, shared);
        return;
    }
    reject_queued(
        ingress,
        egress,
        notifier,
        &shared.metrics,
        permits,
        ServiceRejection::ServiceFaulted,
    );
    finish_closed_after_destruction(state, epoch, egress, notifier, shared, permits);
}

#[allow(clippy::too_many_arguments)]
fn close_owner<State, Request, Output, Fault>(
    epoch: ServiceEpoch,
    ingress: &Receiver<RequestEnvelope<Request>>,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    shared: &Shared,
    permits: &EgressPermits,
    reason: ServiceRejection,
    state: &mut Option<State>,
) {
    {
        let _gate = shared
            .admission_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shared.set_lifecycle(ServiceLifecycle::Closing);
    }
    if matches!(
        publish(
            egress,
            notifier,
            &shared.metrics,
            permits,
            ServiceRecord::Lifecycle {
                epoch,
                state: ServiceLifecycle::Closing,
            },
        ),
        Err(PublishError::Disconnected)
    ) {
        finish_disconnected(state, shared);
        return;
    }
    reject_queued(ingress, egress, notifier, &shared.metrics, permits, reason);
    finish_closed_after_destruction(state, epoch, egress, notifier, shared, permits);
}

fn reject_queued<Request, Output, Fault>(
    ingress: &Receiver<RequestEnvelope<Request>>,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    permits: &EgressPermits,
    reason: ServiceRejection,
) {
    while let Ok(envelope) = ingress.try_recv() {
        metrics.rejected_requests.fetch_add(1, Ordering::Relaxed);
        if matches!(
            publish(
                egress,
                notifier,
                metrics,
                permits,
                ServiceRecord::Rejected {
                    ticket: envelope.ticket,
                    reason,
                },
            ),
            Err(PublishError::Disconnected)
        ) {
            return;
        }
    }
}

fn finish_closed<Output, Fault>(
    epoch: ServiceEpoch,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    shared: &Shared,
    permits: &EgressPermits,
) {
    shared.set_lifecycle(ServiceLifecycle::Closed);
    let _ = publish(
        egress,
        notifier,
        &shared.metrics,
        permits,
        ServiceRecord::Lifecycle {
            epoch,
            state: ServiceLifecycle::Closed,
        },
    );
}

fn finish_closed_after_destruction<State, Output, Fault>(
    state: &mut Option<State>,
    epoch: ServiceEpoch,
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    shared: &Shared,
    permits: &EgressPermits,
) {
    drop(state.take());
    finish_closed(epoch, egress, notifier, shared, permits);
}

fn finish_disconnected<State>(state: &mut Option<State>, shared: &Shared) {
    drop(state.take());
    shared.set_lifecycle(ServiceLifecycle::Closed);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublishError {
    Disconnected,
    Readiness(io::ErrorKind),
}

fn publish<Output, Fault>(
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    permits: &EgressPermits,
    record: ServiceRecord<Output, Fault>,
) -> Result<(), PublishError> {
    let permit = permits.acquire(record.class(), metrics)?;
    publish_with_permit(egress, notifier, metrics, record, permit)
}

fn publish_with_permit<Output, Fault>(
    egress: &Sender<QueuedRecord<Output, Fault>>,
    notifier: &Notifier,
    metrics: &Metrics,
    record: ServiceRecord<Output, Fault>,
    permit: EgressPermit,
) -> Result<(), PublishError> {
    egress
        .send(QueuedRecord {
            record,
            _permit: permit,
        })
        .map_err(|SendError(_)| PublishError::Disconnected)?;
    update_max(&metrics.peak_egress_depth, egress.len().max(1));
    match notifier.notify() {
        Ok(true) => {
            metrics
                .readiness_notifications
                .fetch_add(1, Ordering::Relaxed);
        }
        Ok(false) => {}
        Err(error) => {
            metrics.readiness_failures.fetch_add(1, Ordering::Relaxed);
            return Err(PublishError::Readiness(error.kind()));
        }
    }
    Ok(())
}

fn next_epoch() -> Option<ServiceEpoch> {
    NEXT_SERVICE_EPOCH
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
        .and_then(NonZeroU64::new)
        .map(ServiceEpoch)
}

fn update_max(target: &AtomicUsize, candidate: usize) {
    let mut observed = target.load(Ordering::Relaxed);
    while candidate > observed {
        match target.compare_exchange_weak(
            observed,
            candidate,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(actual) => observed = actual,
        }
    }
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}
