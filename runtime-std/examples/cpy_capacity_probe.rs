//! Native CPY-03 bounded-channel capacity probe with JSON output.
//!
//! This executable is evidence tooling, not a service API. Candidate queue and
//! turn values arrive through the command line and never become defaults.

use std::{
    env,
    hint::black_box,
    mem::size_of,
    process::ExitCode,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};
use rlvgl_core::{
    cue::{CUE_FRAME_OVERHEAD_BYTES, CueLimits},
    endpoint::{Endpoint, EndpointLimits},
    subscription::{EndpointEpoch, SubscriptionLimits},
};
use rlvgl_runtime_std::spawn_owned_thread_task;
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

#[derive(Debug, Default)]
struct Counters {
    ingress_full_observations: AtomicU64,
    ingress_empty_to_nonempty: AtomicU64,
    egress_empty_to_nonempty: AtomicU64,
    peak_ingress_depth: AtomicUsize,
    peak_egress_depth: AtomicUsize,
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

struct ServiceState {
    endpoint: Endpoint,
    ingress: Receiver<ProbeRequest>,
    egress: Sender<ProbeRecord>,
    start: Arc<Barrier>,
    accepted_target: Arc<AtomicUsize>,
    tracker: Arc<EnvelopeTracker>,
    counters: Arc<Counters>,
    config: ProbeConfig,
}

#[derive(Debug, Default)]
struct WorkerStats {
    service_turns: usize,
    egress_backpressured_records: usize,
    egress_backpressure_ns: u64,
    service_checksum: u64,
}

fn run_service(state: &mut ServiceState) -> Result<WorkerStats, String> {
    state.start.wait();
    let target = state.accepted_target.load(Ordering::SeqCst);
    let mut processed = 0;
    let mut statistics = WorkerStats::default();

    while processed < target {
        let first = state
            .ingress
            .recv()
            .map_err(|_| "ingress disconnected before target was processed".to_owned())?;
        let mut batch = Vec::with_capacity(state.config.turn_budget);
        batch.push(first);
        while batch.len() < state.config.turn_budget && processed + batch.len() < target {
            match state.ingress.try_recv() {
                Ok(request) => batch.push(request),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("ingress disconnected while a turn was admitted".to_owned());
                }
            }
        }

        state
            .endpoint
            .run_safe_turn()
            .map_err(|error| format!("empty Endpoint Safe Turn failed: {error:?}"))?;
        statistics.service_turns += 1;

        for request in batch {
            let request_bytes = request.retained_bytes();
            let payload_checksum = request.payload.iter().fold(0_u64, |checksum, byte| {
                checksum.wrapping_mul(16777619) ^ u64::from(*byte)
            });
            statistics.service_checksum ^= black_box(payload_checksum);
            let sequence = request.sequence;
            let admitted_at = request.admitted_at;
            let service_done_at = Instant::now();
            let fill = u8::try_from(sequence % 251).expect("modulo value fits in u8");
            let mut record = ProbeRecord {
                sequence,
                admitted_at,
                service_done_at,
                payload: vec![fill; state.config.egress_payload_bytes],
            };
            let record_bytes = record.retained_bytes();
            state.tracker.add(record_bytes);
            let mut backpressure_started: Option<Instant> = None;

            loop {
                let was_empty = state.egress.is_empty();
                match state.egress.try_send(record) {
                    Ok(()) => {
                        if was_empty {
                            state
                                .counters
                                .egress_empty_to_nonempty
                                .fetch_add(1, Ordering::SeqCst);
                        }
                        update_max(&state.counters.peak_egress_depth, state.egress.len().max(1));
                        if let Some(started) = backpressure_started {
                            statistics.egress_backpressured_records += 1;
                            statistics.egress_backpressure_ns = statistics
                                .egress_backpressure_ns
                                .saturating_add(duration_ns(started.elapsed()));
                        }
                        break;
                    }
                    Err(TrySendError::Full(returned)) => {
                        record = returned;
                        backpressure_started.get_or_insert_with(Instant::now);
                        retry_pause(state.config.retry_backoff_us);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        return Err("egress disconnected before publication".to_owned());
                    }
                }
            }

            drop(request);
            state.tracker.remove(request_bytes);
            processed += 1;
        }
    }

    Ok(statistics)
}

#[derive(Debug)]
struct ConsumerStats {
    service_latency_ns: Vec<u64>,
    delivery_latency_ns: Vec<u64>,
    sequence_errors: usize,
    consumer_checksum: u64,
}

fn run_consumer(
    egress: Receiver<ProbeRecord>,
    start: Arc<Barrier>,
    accepted_target: Arc<AtomicUsize>,
    tracker: Arc<EnvelopeTracker>,
    observer_stall_us: u64,
) -> Result<ConsumerStats, String> {
    start.wait();
    if observer_stall_us > 0 {
        thread::sleep(Duration::from_micros(observer_stall_us));
    }
    let target = accepted_target.load(Ordering::SeqCst);
    let mut service_latency_ns = Vec::with_capacity(target);
    let mut delivery_latency_ns = Vec::with_capacity(target);
    let mut sequence_errors = 0;
    let mut previous = None;
    let mut consumer_checksum = 0_u64;

    for _ in 0..target {
        let record = egress
            .recv()
            .map_err(|_| "egress disconnected before target was consumed".to_owned())?;
        let received_at = Instant::now();
        if previous.is_some_and(|value| record.sequence != value + 1) {
            sequence_errors += 1;
        }
        previous = Some(record.sequence);
        service_latency_ns.push(duration_ns(
            record.service_done_at.duration_since(record.admitted_at),
        ));
        delivery_latency_ns.push(duration_ns(received_at.duration_since(record.admitted_at)));
        consumer_checksum ^= record.payload.iter().fold(0_u64, |checksum, byte| {
            checksum.wrapping_mul(16777619) ^ u64::from(*byte)
        });
        let record_bytes = record.retained_bytes();
        drop(record);
        tracker.remove(record_bytes);
    }

    Ok(ConsumerStats {
        service_latency_ns,
        delivery_latency_ns,
        sequence_errors,
        consumer_checksum: black_box(consumer_checksum),
    })
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

fn make_request(sequence: usize, payload_bytes: usize) -> ProbeRequest {
    let fill = u8::try_from(sequence % 251).expect("modulo value fits in u8");
    ProbeRequest {
        sequence: u64::try_from(sequence).expect("message sequence fits in u64"),
        admitted_at: Instant::now(),
        payload: vec![fill; payload_bytes],
    }
}

fn try_admit(
    ingress: &Sender<ProbeRequest>,
    mut request: ProbeRequest,
    tracker: &EnvelopeTracker,
    counters: &Counters,
) -> Result<(), ProbeRequest> {
    request.admitted_at = Instant::now();
    let request_bytes = request.retained_bytes();
    // Account before publication so the owner cannot remove an envelope that
    // the producer has not recorded yet. A rejected attempt is removed below;
    // its brief caller-owned allocation may still contribute to the peak.
    tracker.add(request_bytes);
    let was_empty = ingress.is_empty();
    match ingress.try_send(request) {
        Ok(()) => {
            if was_empty {
                counters
                    .ingress_empty_to_nonempty
                    .fetch_add(1, Ordering::SeqCst);
            }
            update_max(&counters.peak_ingress_depth, ingress.len().max(1));
            Ok(())
        }
        Err(TrySendError::Full(returned)) => {
            tracker.remove(request_bytes);
            counters
                .ingress_full_observations
                .fetch_add(1, Ordering::SeqCst);
            Err(returned)
        }
        Err(TrySendError::Disconnected(_)) => {
            tracker.remove(request_bytes);
            panic!("ingress disconnected during admission");
        }
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
    service_turns: usize,
    ingress_empty_to_nonempty_observations: u64,
    egress_empty_to_nonempty_observations: u64,
    egress_backpressured_records: usize,
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

fn execute(config: ProbeConfig) -> Result<ProbeResult, String> {
    let (ingress_sender, ingress_receiver) = bounded(config.ingress_capacity);
    let (egress_sender, egress_receiver) = bounded(config.egress_capacity);
    let start = Arc::new(Barrier::new(3));
    let accepted_target = Arc::new(AtomicUsize::new(0));
    let tracker = Arc::new(EnvelopeTracker::default());
    let counters = Arc::new(Counters::default());

    let consumer = {
        let start = Arc::clone(&start);
        let accepted_target = Arc::clone(&accepted_target);
        let tracker = Arc::clone(&tracker);
        thread::Builder::new()
            .name("cpy-capacity-consumer".to_owned())
            .spawn(move || {
                run_consumer(
                    egress_receiver,
                    start,
                    accepted_target,
                    tracker,
                    config.observer_stall_us,
                )
            })
            .map_err(|error| format!("spawn consumer: {error}"))?
    };

    let owner = {
        let start = Arc::clone(&start);
        let accepted_target = Arc::clone(&accepted_target);
        let tracker = Arc::clone(&tracker);
        let counters = Arc::clone(&counters);
        spawn_owned_thread_task(
            "cpy-capacity-owner",
            move || ServiceState {
                endpoint: build_endpoint(config.turn_budget),
                ingress: ingress_receiver,
                egress: egress_sender,
                start,
                accepted_target,
                tracker,
                counters,
                config,
            },
            run_service,
        )
        .map_err(|error| format!("spawn owner: {error}"))?
    };

    if config.sampling_hold_us > 0 {
        thread::sleep(Duration::from_micros(config.sampling_hold_us));
    }
    let probe_started = Instant::now();
    let mut accepted = 0;

    match config.scenario {
        Scenario::ColdBurst => {
            for sequence in 0..config.messages {
                let request = make_request(sequence, config.ingress_payload_bytes);
                if try_admit(&ingress_sender, request, &tracker, &counters).is_ok() {
                    accepted += 1;
                }
            }
            accepted_target.store(accepted, Ordering::SeqCst);
            start.wait();
        }
        Scenario::Sustained | Scenario::ObserverStall => {
            accepted_target.store(config.messages, Ordering::SeqCst);
            start.wait();
            for sequence in 0..config.messages {
                let mut request = make_request(sequence, config.ingress_payload_bytes);
                loop {
                    match try_admit(&ingress_sender, request, &tracker, &counters) {
                        Ok(()) => {
                            accepted += 1;
                            break;
                        }
                        Err(returned) => {
                            request = returned;
                            retry_pause(config.retry_backoff_us);
                        }
                    }
                }
            }
        }
    }

    let worker = owner
        .join()
        .map_err(|error| format!("join owner: {error}"))??;
    let consumed = consumer
        .join()
        .map_err(|_| "consumer thread panicked".to_owned())??;
    let probe_duration_ns = duration_ns(probe_started.elapsed());
    if config.sampling_hold_us > 0 {
        thread::sleep(Duration::from_micros(config.sampling_hold_us));
    }

    let terminal_admission_rejections = match config.scenario {
        Scenario::ColdBurst => config.messages - accepted,
        Scenario::Sustained | Scenario::ObserverStall => 0,
    };
    Ok(ProbeResult {
        schema_version: "CPY-CAPACITY-PROBE-1",
        workload: "bounded-crossbeam-transport-with-empty-endpoint-safe-turn",
        retained_bytes_scope: "owned caller/service/queue request and record structs plus payload capacities; channel allocator and process overhead excluded",
        config,
        offered_requests: config.messages,
        accepted_requests: accepted,
        terminal_admission_rejections,
        ingress_full_observations: counters.ingress_full_observations.load(Ordering::SeqCst),
        completed_records: consumed.delivery_latency_ns.len(),
        sequence_errors: consumed.sequence_errors,
        service_turns: worker.service_turns,
        ingress_empty_to_nonempty_observations: counters
            .ingress_empty_to_nonempty
            .load(Ordering::SeqCst),
        egress_empty_to_nonempty_observations: counters
            .egress_empty_to_nonempty
            .load(Ordering::SeqCst),
        egress_backpressured_records: worker.egress_backpressured_records,
        egress_backpressure_ns: worker.egress_backpressure_ns,
        peak_ingress_depth: counters.peak_ingress_depth.load(Ordering::SeqCst),
        peak_egress_depth: counters.peak_egress_depth.load(Ordering::SeqCst),
        peak_owned_envelope_bytes: tracker.peak_bytes.load(Ordering::SeqCst),
        owned_envelope_bytes_at_end: tracker.live_bytes.load(Ordering::SeqCst),
        service_latency: Distribution::from_samples(consumed.service_latency_ns),
        delivery_latency: Distribution::from_samples(consumed.delivery_latency_ns),
        probe_duration_ns,
        service_checksum: worker.service_checksum,
        consumer_checksum: consumed.consumer_checksum,
    })
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
