//! Native CPY-03 service capacity probe with JSON output.
//!
//! This executable is evidence tooling, not a source of runtime defaults.
//! Every queue and turn value arrives through the command line.

use std::{
    env,
    hint::black_box,
    mem::size_of,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

use rlvgl_core::{
    cue::{CUE_FRAME_OVERHEAD_BYTES, CueLimits},
    endpoint::{Endpoint, EndpointLimits},
    subscription::{EndpointEpoch, SubscriptionLimits},
};
use rlvgl_runtime_std::{
    AdmissionError, NativeService, ServiceConfig, ServiceLifecycle, ServiceRecord,
    spawn_native_service,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Scenario {
    ColdBurst,
    Sustained,
    ObserverStall,
}

impl Scenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cold-burst" => Ok(Self::ColdBurst),
            "sustained" => Ok(Self::Sustained),
            "observer-stall" => Ok(Self::ObserverStall),
            _ => Err(format!(
                "unsupported scenario {value:?}; expected cold-burst, sustained, or observer-stall"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ProbeConfig {
    scenario: Scenario,
    ingress_capacity: usize,
    egress_capacity: usize,
    turn_budget: usize,
    messages: usize,
    ingress_payload_bytes: usize,
    egress_payload_bytes: usize,
    observer_stall_us: u64,
    retry_backoff_us: u64,
    sampling_hold_us: u64,
}

impl ProbeConfig {
    fn parse() -> Result<Self, String> {
        let mut scenario = None;
        let mut ingress_capacity = None;
        let mut egress_capacity = None;
        let mut turn_budget = None;
        let mut messages = None;
        let mut ingress_payload_bytes = None;
        let mut egress_payload_bytes = None;
        let mut observer_stall_us = Some(0);
        let mut retry_backoff_us = Some(50);
        let mut sampling_hold_us = Some(5_000);
        let mut arguments = env::args().skip(1);

        while let Some(argument) = arguments.next() {
            if argument == "--help" || argument == "-h" {
                return Err(usage());
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value after {argument}\n{}", usage()))?;
            match argument.as_str() {
                "--scenario" => scenario = Some(Scenario::parse(&value)?),
                "--ingress-capacity" => ingress_capacity = Some(parse_positive(&argument, &value)?),
                "--egress-capacity" => egress_capacity = Some(parse_positive(&argument, &value)?),
                "--turn-budget" => turn_budget = Some(parse_positive(&argument, &value)?),
                "--messages" => messages = Some(parse_positive(&argument, &value)?),
                "--ingress-payload-bytes" => {
                    ingress_payload_bytes = Some(parse_positive(&argument, &value)?)
                }
                "--egress-payload-bytes" => {
                    egress_payload_bytes = Some(parse_positive(&argument, &value)?)
                }
                "--observer-stall-us" => observer_stall_us = Some(parse_u64(&argument, &value)?),
                "--retry-backoff-us" => retry_backoff_us = Some(parse_u64(&argument, &value)?),
                "--sampling-hold-us" => sampling_hold_us = Some(parse_u64(&argument, &value)?),
                _ => return Err(format!("unknown argument {argument:?}\n{}", usage())),
            }
        }

        let config = Self {
            scenario: scenario.ok_or_else(|| missing("--scenario"))?,
            ingress_capacity: ingress_capacity.ok_or_else(|| missing("--ingress-capacity"))?,
            egress_capacity: egress_capacity.ok_or_else(|| missing("--egress-capacity"))?,
            turn_budget: turn_budget.ok_or_else(|| missing("--turn-budget"))?,
            messages: messages.ok_or_else(|| missing("--messages"))?,
            ingress_payload_bytes: ingress_payload_bytes
                .ok_or_else(|| missing("--ingress-payload-bytes"))?,
            egress_payload_bytes: egress_payload_bytes
                .ok_or_else(|| missing("--egress-payload-bytes"))?,
            observer_stall_us: observer_stall_us.expect("defaulted"),
            retry_backoff_us: retry_backoff_us.expect("defaulted"),
            sampling_hold_us: sampling_hold_us.expect("defaulted"),
        };
        if !matches!(config.scenario, Scenario::ObserverStall) && config.observer_stall_us != 0 {
            return Err("--observer-stall-us must be zero outside observer-stall".to_owned());
        }
        Ok(config)
    }
}

fn usage() -> String {
    "usage: cpy_capacity_probe \
--scenario <cold-burst|sustained|observer-stall> \
--ingress-capacity <N> --egress-capacity <N> --turn-budget <N> \
--messages <N> --ingress-payload-bytes <N> --egress-payload-bytes <N> \
[--observer-stall-us <N>] [--retry-backoff-us <N>] [--sampling-hold-us <N>]"
        .to_owned()
}

fn missing(name: &str) -> String {
    format!("missing required {name}\n{}", usage())
}

fn parse_positive(name: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid {name} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn parse_u64(name: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid {name} value {value:?}: {error}"))
}

#[derive(Debug)]
struct ProbeRequest {
    sequence: u64,
    measured: bool,
    admitted_at: Instant,
    payload: Vec<u8>,
}

impl ProbeRequest {
    fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.payload.capacity()
    }
}

#[derive(Debug)]
struct ProbeRecord {
    sequence: u64,
    measured: bool,
    admitted_at: Instant,
    service_done_at: Instant,
    payload: Vec<u8>,
}

impl ProbeRecord {
    fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.payload.capacity()
    }
}

#[derive(Debug, Default)]
struct EnvelopeTracker {
    live_bytes: AtomicUsize,
    peak_bytes: AtomicUsize,
}

impl EnvelopeTracker {
    fn add(&self, bytes: usize) {
        let live = self.live_bytes.fetch_add(bytes, Ordering::SeqCst) + bytes;
        update_max(&self.peak_bytes, live);
    }

    fn remove(&self, bytes: usize) {
        let previous = self.live_bytes.fetch_sub(bytes, Ordering::SeqCst);
        assert!(
            previous >= bytes,
            "owned-envelope byte accounting underflow"
        );
    }
}

struct StartGate {
    started: SyncSender<()>,
    release: Receiver<()>,
}

struct ServiceState {
    endpoint: Endpoint,
    start_gate: Option<StartGate>,
    tracker: Arc<EnvelopeTracker>,
    service_checksum: Arc<AtomicU64>,
    egress_payload_bytes: usize,
}

fn run_service_turn(
    state: &mut ServiceState,
    requests: Vec<ProbeRequest>,
) -> Vec<Result<ProbeRecord, String>> {
    if let Some(gate) = state.start_gate.take() {
        gate.started
            .send(())
            .expect("cold-burst priming turn announces start");
        gate.release
            .recv()
            .expect("cold-burst priming turn receives release");
    }
    if let Err(error) = state.endpoint.run_safe_turn() {
        return requests
            .into_iter()
            .map(|_| Err(format!("empty Endpoint Safe Turn failed: {error:?}")))
            .collect();
    }

    requests
        .into_iter()
        .map(|request| {
            let request_bytes = request.retained_bytes();
            let payload_checksum = request.payload.iter().fold(0_u64, |checksum, byte| {
                checksum.wrapping_mul(16_777_619) ^ u64::from(*byte)
            });
            state
                .service_checksum
                .fetch_xor(black_box(payload_checksum), Ordering::SeqCst);
            let fill = u8::try_from(request.sequence % 251).expect("modulo value fits in u8");
            let record = ProbeRecord {
                sequence: request.sequence,
                measured: request.measured,
                admitted_at: request.admitted_at,
                service_done_at: Instant::now(),
                payload: vec![fill; state.egress_payload_bytes],
            };
            state.tracker.add(record.retained_bytes());
            state.tracker.remove(request_bytes);
            Ok(record)
        })
        .collect()
}

#[derive(Debug, Default)]
struct ConsumerStats {
    completed_records: usize,
    service_latency_ns: Vec<u64>,
    delivery_latency_ns: Vec<u64>,
    sequence_errors: usize,
    previous_sequence: Option<u64>,
    consumer_checksum: u64,
}

type ProbeService = NativeService<ProbeRequest, ProbeRecord, String>;

fn consume_records(
    records: impl IntoIterator<Item = ServiceRecord<ProbeRecord, String>>,
    stats: &mut ConsumerStats,
    tracker: &EnvelopeTracker,
) -> Result<(), String> {
    for service_record in records {
        match service_record {
            ServiceRecord::Lifecycle { .. } => {}
            ServiceRecord::Completed { output: record, .. } => {
                let received_at = Instant::now();
                if record.measured {
                    if stats
                        .previous_sequence
                        .is_some_and(|previous| record.sequence != previous + 1)
                    {
                        stats.sequence_errors += 1;
                    }
                    stats.previous_sequence = Some(record.sequence);
                    stats.service_latency_ns.push(duration_ns(
                        record.service_done_at.duration_since(record.admitted_at),
                    ));
                    stats
                        .delivery_latency_ns
                        .push(duration_ns(received_at.duration_since(record.admitted_at)));
                    stats.completed_records += 1;
                }
                stats.consumer_checksum ^= record.payload.iter().fold(0_u64, |checksum, byte| {
                    checksum.wrapping_mul(16_777_619) ^ u64::from(*byte)
                });
                let record_bytes = record.retained_bytes();
                drop(record);
                tracker.remove(record_bytes);
            }
            ServiceRecord::DriverFault { fault, .. } => {
                return Err(format!("native service driver fault: {fault}"));
            }
            ServiceRecord::RuntimeFault { fault, .. }
            | ServiceRecord::ServiceFault { fault, .. } => {
                return Err(format!("native service runtime fault: {fault:?}"));
            }
            ServiceRecord::Rejected { ticket, reason } => {
                return Err(format!(
                    "accepted request {} was rejected during measurement: {reason:?}",
                    ticket.request_id().get()
                ));
            }
        }
    }
    Ok(())
}

fn drain_service(
    service: &ProbeService,
    stats: &mut ConsumerStats,
    tracker: &EnvelopeTracker,
) -> Result<usize, String> {
    let records = service
        .drain()
        .map_err(|error| format!("drain native service readiness: {error}"))?;
    let count = records.len();
    consume_records(records, stats, tracker)?;
    Ok(count)
}

enum Admission {
    Accepted,
    Full(ProbeRequest),
}

fn try_admit(
    service: &ProbeService,
    mut request: ProbeRequest,
    tracker: &EnvelopeTracker,
) -> Result<Admission, String> {
    request.admitted_at = Instant::now();
    let request_bytes = request.retained_bytes();
    tracker.add(request_bytes);
    match service.try_submit(request) {
        Ok(_) => Ok(Admission::Accepted),
        Err(AdmissionError::Full(returned)) => {
            tracker.remove(request_bytes);
            Ok(Admission::Full(returned))
        }
        Err(error) => {
            tracker.remove(request_bytes);
            Err(format!(
                "native service rejected admission unexpectedly: {}",
                admission_class(&error)
            ))
        }
    }
}

fn admission_class(error: &AdmissionError<ProbeRequest>) -> &'static str {
    match error {
        AdmissionError::Full(_) => "full",
        AdmissionError::Closing(_) => "closing",
        AdmissionError::Faulted(_) => "faulted",
        AdmissionError::Closed(_) => "closed",
        AdmissionError::RequestIdExhausted(_) => "request-id-exhausted",
    }
}

fn retry_pause(microseconds: u64) {
    if microseconds == 0 {
        thread::yield_now();
    } else {
        thread::sleep(Duration::from_micros(microseconds));
    }
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn make_request(sequence: usize, payload_bytes: usize, measured: bool) -> ProbeRequest {
    let sequence = u64::try_from(sequence).expect("message sequence fits in u64");
    let fill = u8::try_from(sequence % 251).expect("modulo value fits in u8");
    ProbeRequest {
        sequence,
        measured,
        admitted_at: Instant::now(),
        payload: vec![fill; payload_bytes],
    }
}

fn make_priming_request(payload_bytes: usize) -> ProbeRequest {
    ProbeRequest {
        sequence: u64::MAX,
        measured: false,
        admitted_at: Instant::now(),
        payload: vec![0; payload_bytes],
    }
}

#[derive(Debug, Serialize)]
struct Distribution {
    samples: usize,
    minimum_ns: u64,
    p50_ns: u64,
    p95_ns: u64,
    p99_ns: u64,
    maximum_ns: u64,
    mean_ns: u64,
}

impl Distribution {
    fn from_samples(mut samples: Vec<u64>) -> Self {
        assert!(!samples.is_empty(), "probe produced no latency samples");
        samples.sort_unstable();
        let sum = samples
            .iter()
            .fold(0_u128, |total, value| total + u128::from(*value));
        let mean = sum / samples.len() as u128;
        Self {
            samples: samples.len(),
            minimum_ns: samples[0],
            p50_ns: percentile(&samples, 50),
            p95_ns: percentile(&samples, 95),
            p99_ns: percentile(&samples, 99),
            maximum_ns: *samples.last().expect("nonempty"),
            mean_ns: u64::try_from(mean).unwrap_or(u64::MAX),
        }
    }
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let numerator = (samples.len() - 1) * percentile;
    let index = numerator.div_ceil(100);
    samples[index]
}

#[derive(Debug, Serialize)]
struct ProbeResult {
    schema_version: &'static str,
    workload: &'static str,
    retained_bytes_scope: &'static str,
    config: ProbeConfig,
    offered_requests: usize,
    accepted_requests: usize,
    terminal_admission_rejections: usize,
    ingress_full_observations: u64,
    completed_records: usize,
    sequence_errors: usize,
    service_turns: u64,
    ingress_empty_to_nonempty_observations: u64,
    egress_empty_to_nonempty_observations: u64,
    egress_backpressured_records: u64,
    egress_backpressure_ns: u64,
    peak_ingress_depth: usize,
    peak_egress_depth: usize,
    peak_owned_envelope_bytes: usize,
    owned_envelope_bytes_at_end: usize,
    service_latency: Distribution,
    delivery_latency: Distribution,
    probe_duration_ns: u64,
    service_checksum: u64,
    consumer_checksum: u64,
}

fn build_endpoint(turn_budget: usize) -> Endpoint {
    Endpoint::new(
        EndpointEpoch::new(1).expect("nonzero epoch"),
        EndpointLimits::new(1, turn_budget, turn_budget, 1)
            .expect("positive probe endpoint limits"),
        SubscriptionLimits::new(1, 32, 1, 1).expect("valid probe subscription limits"),
        CueLimits::new(4, 1, 2, 32, CUE_FRAME_OVERHEAD_BYTES + 32).expect("valid probe cue limits"),
    )
    .expect("construct probe Endpoint on its owner thread")
}

fn metric_delta(after: u64, before: u64) -> u64 {
    after.saturating_sub(before)
}

fn execute(config: ProbeConfig) -> Result<ProbeResult, String> {
    let tracker = Arc::new(EnvelopeTracker::default());
    let service_checksum = Arc::new(AtomicU64::new(0));
    let (start_gate, started_receiver, release_sender) =
        if matches!(config.scenario, Scenario::ColdBurst) {
            let (started_sender, started_receiver) = sync_channel(1);
            let (release_sender, release_receiver) = sync_channel(1);
            (
                Some(StartGate {
                    started: started_sender,
                    release: release_receiver,
                }),
                Some(started_receiver),
                Some(release_sender),
            )
        } else {
            (None, None, None)
        };
    let build_tracker = Arc::clone(&tracker);
    let build_checksum = Arc::clone(&service_checksum);
    let service = spawn_native_service(
        "cpy-capacity-owner",
        ServiceConfig::new(
            config.ingress_capacity,
            config.egress_capacity,
            config.turn_budget,
        )
        .map_err(|error| format!("capacity configuration: {error}"))?,
        move || ServiceState {
            endpoint: build_endpoint(config.turn_budget),
            start_gate,
            tracker: build_tracker,
            service_checksum: build_checksum,
            egress_payload_bytes: config.egress_payload_bytes,
        },
        run_service_turn,
    )
    .map_err(|error| format!("spawn native service: {error}"))?;

    let mut consumed = ConsumerStats::default();
    let startup = service
        .drain()
        .map_err(|error| format!("drain service startup: {error}"))?;
    if !startup.iter().any(|record| {
        matches!(
            record,
            ServiceRecord::Lifecycle {
                state: ServiceLifecycle::Running,
                ..
            }
        )
    }) {
        return Err("native service omitted Running startup record".to_owned());
    }
    consume_records(startup, &mut consumed, &tracker)?;
    let baseline = service.metrics();
    if config.sampling_hold_us > 0 {
        thread::sleep(Duration::from_micros(config.sampling_hold_us));
    }
    let probe_started = Instant::now();
    let mut accepted = 0;

    if matches!(config.scenario, Scenario::ColdBurst) {
        if !matches!(
            try_admit(
                &service,
                make_priming_request(config.ingress_payload_bytes),
                &tracker,
            )?,
            Admission::Accepted
        ) {
            return Err("cold-burst priming request was not accepted".to_owned());
        }
        started_receiver
            .expect("cold-burst receiver exists")
            .recv()
            .map_err(|_| "cold-burst owner did not enter priming turn".to_owned())?;
        for sequence in 0..config.messages {
            let request = make_request(sequence, config.ingress_payload_bytes, true);
            if matches!(try_admit(&service, request, &tracker)?, Admission::Accepted) {
                accepted += 1;
            }
        }
        release_sender
            .expect("cold-burst sender exists")
            .send(())
            .map_err(|_| "cold-burst owner dropped its release gate".to_owned())?;
    } else {
        let observer_release = probe_started + Duration::from_micros(config.observer_stall_us);
        for sequence in 0..config.messages {
            let mut request = make_request(sequence, config.ingress_payload_bytes, true);
            loop {
                match try_admit(&service, request, &tracker)? {
                    Admission::Accepted => {
                        accepted += 1;
                        break;
                    }
                    Admission::Full(returned) => {
                        request = returned;
                        let should_pause = if matches!(config.scenario, Scenario::ObserverStall)
                            && Instant::now() < observer_release
                        {
                            true
                        } else {
                            drain_service(&service, &mut consumed, &tracker)? == 0
                        };
                        if should_pause {
                            retry_pause(config.retry_backoff_us);
                        }
                    }
                }
            }
        }
        if matches!(config.scenario, Scenario::ObserverStall) && Instant::now() < observer_release {
            thread::sleep(observer_release.saturating_duration_since(Instant::now()));
        }
    }

    while consumed.completed_records < accepted {
        if drain_service(&service, &mut consumed, &tracker)? == 0 {
            retry_pause(config.retry_backoff_us);
        }
    }
    let metrics = service.metrics();
    let shutdown_records = service
        .shutdown()
        .map_err(|error| format!("shutdown native service: {error}"))?;
    consume_records(shutdown_records, &mut consumed, &tracker)?;
    let probe_duration_ns = duration_ns(probe_started.elapsed());
    if config.sampling_hold_us > 0 {
        thread::sleep(Duration::from_micros(config.sampling_hold_us));
    }

    let terminal_admission_rejections = match config.scenario {
        Scenario::ColdBurst => config.messages - accepted,
        Scenario::Sustained | Scenario::ObserverStall => 0,
    };
    Ok(ProbeResult {
        schema_version: "CPY-CAPACITY-PROBE-2",
        workload: "bounded-native-service-with-empty-endpoint-safe-turn-and-os-readiness",
        retained_bytes_scope: "owned probe request and result structs plus payload capacities; native service/channel/readiness allocation and process overhead excluded",
        config,
        offered_requests: config.messages,
        accepted_requests: accepted,
        terminal_admission_rejections,
        ingress_full_observations: metric_delta(
            metrics.ingress_full_observations,
            baseline.ingress_full_observations,
        ),
        completed_records: consumed.completed_records,
        sequence_errors: consumed.sequence_errors,
        service_turns: metric_delta(metrics.service_turns, baseline.service_turns),
        ingress_empty_to_nonempty_observations: metric_delta(
            metrics.ingress_empty_to_nonempty_observations,
            baseline.ingress_empty_to_nonempty_observations,
        ),
        egress_empty_to_nonempty_observations: metric_delta(
            metrics.readiness_notifications,
            baseline.readiness_notifications,
        ),
        egress_backpressured_records: metric_delta(
            metrics.egress_backpressured_records,
            baseline.egress_backpressured_records,
        ),
        egress_backpressure_ns: metric_delta(
            metrics.egress_backpressure_ns,
            baseline.egress_backpressure_ns,
        ),
        peak_ingress_depth: metrics.peak_ingress_depth,
        peak_egress_depth: metrics.peak_egress_depth,
        peak_owned_envelope_bytes: tracker.peak_bytes.load(Ordering::SeqCst),
        owned_envelope_bytes_at_end: tracker.live_bytes.load(Ordering::SeqCst),
        service_latency: Distribution::from_samples(consumed.service_latency_ns),
        delivery_latency: Distribution::from_samples(consumed.delivery_latency_ns),
        probe_duration_ns,
        service_checksum: service_checksum.load(Ordering::SeqCst),
        consumer_checksum: black_box(consumed.consumer_checksum),
    })
}

fn update_max(target: &AtomicUsize, candidate: usize) {
    let mut observed = target.load(Ordering::Relaxed);
    while candidate > observed {
        match target.compare_exchange_weak(observed, candidate, Ordering::SeqCst, Ordering::Relaxed)
        {
            Ok(_) => return,
            Err(actual) => observed = actual,
        }
    }
}

fn main() -> ExitCode {
    let config = match ProbeConfig::parse() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    match execute(config).and_then(|result| {
        serde_json::to_string(&result).map_err(|error| format!("serialize result: {error}"))
    }) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cpy_capacity_probe: {error}");
            ExitCode::FAILURE
        }
    }
}
