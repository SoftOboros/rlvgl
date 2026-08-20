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

use rlvgl_api::protocol::ValueRef;
use rlvgl_core::{
    actor::{
        ActorIdentity, ConstructorInput, CreateDestination, RegistryLimits, StageId, StageRegistry,
        TypeDescriptor,
    },
    cue::{CUE_FRAME_OVERHEAD_BYTES, CallbackId, CueLimits, DrainBudget, EventId, InputClass},
    direction::{ActorDirection, OwnedValue, RuntimeFlag, StageDirection},
    endpoint::{
        BatchOutcome, Endpoint, EndpointLimits, EndpointNativeInput, NativeInputOutcome,
        RequestId as EndpointRequestId,
    },
    event::Event,
    object::{DispatchPhase, Disposition},
    renderer::Renderer,
    subscription::{
        EndpointEpoch, PropagationPolicy, SubscribeRequest, SubscriptionFilter, SubscriptionLimits,
    },
    widget::{Color, Rect},
};
use rlvgl_runtime_std::{
    AdmissionError, NativeService, ServiceConfig, ServiceLifecycle, ServiceRecord,
    spawn_native_service,
};
use rlvgl_widgets::{mpy::CATALOG, slider};
use serde::Serialize;

const FRAME_WIDTH: usize = 320;
const FRAME_HEIGHT: usize = 240;
const FRAME_STRIDE: usize = FRAME_WIDTH * 4;
const STAGE_ID: u32 = 1;
const SLIDER_BOUNDS: Rect = Rect {
    x: 16,
    y: 100,
    width: 288,
    height: 40,
};

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
    frame_period_us: u64,
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
        let mut frame_period_us = None;
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
                "--frame-period-us" => {
                    frame_period_us = Some(parse_positive_u64(&argument, &value)?)
                }
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
            frame_period_us: frame_period_us.ok_or_else(|| missing("--frame-period-us"))?,
        };
        if !matches!(config.scenario, Scenario::ObserverStall) && config.observer_stall_us != 0 {
            return Err("--observer-stall-us must be zero outside observer-stall".to_owned());
        }
        if config.turn_budget > u16::MAX as usize {
            return Err("--turn-budget exceeds the neutral Stage direction width".to_owned());
        }
        Ok(config)
    }
}

fn usage() -> String {
    "usage: cpy_capacity_probe \
--scenario <cold-burst|sustained|observer-stall> \
--ingress-capacity <N> --egress-capacity <N> --turn-budget <N> \
--messages <N> --ingress-payload-bytes <N> --egress-payload-bytes <N> \
[--observer-stall-us <N>] [--retry-backoff-us <N>] [--sampling-hold-us <N>] \
--frame-period-us <N>"
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

fn parse_positive_u64(name: &str, value: &str) -> Result<u64, String> {
    let parsed = parse_u64(name, value)?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
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

#[derive(Debug, Default)]
struct SemanticTracker {
    workload_requests: AtomicU64,
    stage_batches: AtomicU64,
    stage_completions: AtomicU64,
    native_inputs: AtomicU64,
    cue_records: AtomicU64,
    frames_rendered: AtomicU64,
    cadence_misses: AtomicU64,
    final_stage_revision: AtomicU64,
    frame_checksum: AtomicU64,
}

struct PrivateRgbaFrame {
    pixels: Vec<u8>,
}

impl PrivateRgbaFrame {
    fn new() -> Self {
        Self {
            pixels: vec![0; FRAME_STRIDE * FRAME_HEIGHT],
        }
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn checksum(&self) -> u64 {
        self.pixels
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x as usize >= FRAME_WIDTH || y as usize >= FRAME_HEIGHT {
            return;
        }
        let index = y as usize * FRAME_STRIDE + x as usize * 4;
        let alpha = u16::from(color.3);
        let inverse = 255 - alpha;
        self.pixels[index] =
            ((u16::from(color.0) * alpha + u16::from(self.pixels[index]) * inverse) / 255) as u8;
        self.pixels[index + 1] = ((u16::from(color.1) * alpha
            + u16::from(self.pixels[index + 1]) * inverse)
            / 255) as u8;
        self.pixels[index + 2] = ((u16::from(color.2) * alpha
            + u16::from(self.pixels[index + 2]) * inverse)
            / 255) as u8;
        self.pixels[index + 3] = 0xff;
    }
}

impl Renderer for PrivateRgbaFrame {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let left = rect.x.max(0);
        let top = rect.y.max(0);
        let right = rect
            .x
            .saturating_add(rect.width)
            .clamp(0, FRAME_WIDTH as i32);
        let bottom = rect
            .y
            .saturating_add(rect.height)
            .clamp(0, FRAME_HEIGHT as i32);
        for y in top..bottom {
            for x in left..right {
                self.blend_pixel(x, y, color);
            }
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        for (index, byte) in text.bytes().enumerate() {
            if !byte.is_ascii_whitespace() {
                self.fill_rect(
                    Rect {
                        x: position.0 + index as i32 * 6,
                        y: position.1 - 6,
                        width: 4,
                        height: 6,
                    },
                    color,
                );
            }
        }
    }
}

struct RepresentativeRuntime {
    endpoint: Endpoint,
    stage_id: StageId,
    root_id: rlvgl_core::actor::ObjectId,
    slider_actor: ActorIdentity,
    slider_value_property: u32,
    next_request_id: u32,
    frame: PrivateRgbaFrame,
    next_frame_deadline: Instant,
    frame_period: Duration,
    tracker: Arc<SemanticTracker>,
}

impl RepresentativeRuntime {
    fn wait_for_cadence(&mut self) {
        let deadline = self.next_frame_deadline;
        let now = Instant::now();
        if now < deadline {
            thread::sleep(deadline.duration_since(now));
        }
        let started = Instant::now();
        let period_nanos = self.frame_period.as_nanos();
        let missed = if started > deadline {
            started.duration_since(deadline).as_nanos() / period_nanos
        } else {
            0
        };
        self.tracker
            .cadence_misses
            .fetch_add(u64::try_from(missed).unwrap_or(u64::MAX), Ordering::SeqCst);
        let advance = u32::try_from(missed.saturating_add(1)).unwrap_or(u32::MAX);
        self.next_frame_deadline = deadline
            .checked_add(self.frame_period.saturating_mul(advance))
            .unwrap_or_else(|| started + self.frame_period);
    }

    fn render_private_frame(&mut self) -> Result<(), String> {
        self.frame.clear();
        let stage = self
            .endpoint
            .stage(self.stage_id)
            .ok_or_else(|| "representative Stage disappeared before render".to_owned())?;
        stage
            .node(self.root_id)
            .map_err(|error| format!("resolve representative render root: {error:?}"))?
            .draw(&mut self.frame);
        let checksum = black_box(self.frame.checksum());
        self.tracker
            .frame_checksum
            .fetch_xor(checksum, Ordering::SeqCst);
        self.tracker.frames_rendered.fetch_add(1, Ordering::SeqCst);
        self.tracker
            .final_stage_revision
            .store(stage.revision().get(), Ordering::SeqCst);
        Ok(())
    }
}

struct StartGate {
    started: SyncSender<()>,
    release: Receiver<()>,
}

struct ServiceState {
    runtime: RepresentativeRuntime,
    start_gate: Option<StartGate>,
    tracker: Arc<EnvelopeTracker>,
    service_checksum: Arc<AtomicU64>,
    egress_payload_bytes: usize,
}

fn run_representative_workload(
    runtime: &mut RepresentativeRuntime,
    requests: &[ProbeRequest],
) -> Result<(), String> {
    runtime.wait_for_cadence();
    let mut directions = Vec::new();
    directions
        .try_reserve_exact(requests.len())
        .map_err(|_| "reserve representative Stage directions".to_owned())?;
    for _ in requests {
        directions.push(StageDirection::MutateActor {
            object_id: runtime.slider_actor.object_id,
            directions: vec![ActorDirection::SetProperty {
                id: runtime.slider_value_property,
                value: OwnedValue::I32(0),
            }],
        });
    }
    let endpoint_request = EndpointRequestId::new(runtime.next_request_id)
        .ok_or_else(|| "representative Endpoint request id exhausted".to_owned())?;
    runtime.next_request_id = runtime
        .next_request_id
        .checked_add(1)
        .ok_or_else(|| "representative Endpoint request id exhausted".to_owned())?;
    runtime
        .endpoint
        .enqueue_batch(endpoint_request, runtime.stage_id, directions)
        .map_err(|error| format!("enqueue representative Stage batch: {error:?}"))?;
    runtime.tracker.stage_batches.fetch_add(1, Ordering::SeqCst);
    runtime.tracker.workload_requests.fetch_add(
        u64::try_from(requests.len()).unwrap_or(u64::MAX),
        Ordering::SeqCst,
    );

    let summary = runtime
        .endpoint
        .run_safe_turn()
        .map_err(|error| format!("run representative Endpoint Safe Turn: {error:?}"))?;
    if summary.processed_batches != 1 {
        return Err(format!(
            "representative Safe Turn processed {} batches instead of one",
            summary.processed_batches
        ));
    }
    let completions = runtime
        .endpoint
        .drain_completions(2)
        .map_err(|error| format!("drain representative completion: {error:?}"))?;
    if completions.len() != 1
        || completions[0].request_id != endpoint_request
        || !matches!(completions[0].outcome, BatchOutcome::Committed { .. })
    {
        return Err("representative Stage batch did not commit exactly once".to_owned());
    }
    runtime
        .tracker
        .stage_completions
        .fetch_add(1, Ordering::SeqCst);

    let mut expected_cues = 0usize;
    for request in requests {
        let requested_value = 10 + request.sequence.wrapping_mul(37) % 80;
        let x = SLIDER_BOUNDS.x
            + i32::try_from(requested_value * SLIDER_BOUNDS.width as u64 / 100)
                .expect("slider coordinate fits in i32");
        let y = SLIDER_BOUNDS.y + SLIDER_BOUNDS.height / 2;
        match runtime
            .endpoint
            .dispatch_native_event(
                runtime.stage_id,
                InputClass::new(1).expect("nonzero input class"),
                EndpointNativeInput::Pointer {
                    root_id: runtime.root_id,
                    x,
                    y,
                    event: Event::PressRelease { x, y },
                },
            )
            .map_err(|error| format!("dispatch representative native input: {error:?}"))?
        {
            NativeInputOutcome::Dispatched {
                disposition: Disposition::Consumed,
                cue_count,
                ..
            } if cue_count == 1 => expected_cues += cue_count,
            outcome => {
                return Err(format!(
                    "representative native input produced unexpected outcome: {outcome:?}"
                ));
            }
        }
    }
    runtime.tracker.native_inputs.fetch_add(
        u64::try_from(requests.len()).unwrap_or(u64::MAX),
        Ordering::SeqCst,
    );
    let drain = runtime
        .endpoint
        .drain_records(DrainBudget::new(expected_cues, usize::MAX))
        .map_err(|error| format!("drain representative Cue records: {error:?}"))?;
    let cue_records = drain.records().len();
    if cue_records != expected_cues {
        return Err(format!(
            "representative Cue drain returned {cue_records} records for {expected_cues} inputs"
        ));
    }
    runtime
        .endpoint
        .acknowledge_records(drain)
        .map_err(|error| format!("acknowledge representative Cue records: {error:?}"))?;
    runtime.tracker.cue_records.fetch_add(
        u64::try_from(cue_records).unwrap_or(u64::MAX),
        Ordering::SeqCst,
    );
    runtime.render_private_frame()
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
    if let Err(error) = run_representative_workload(&mut state.runtime, &requests) {
        return requests.into_iter().map(|_| Err(error.clone())).collect();
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
    workload_requests: u64,
    stage_batches: u64,
    stage_completions: u64,
    native_inputs: u64,
    cue_records: u64,
    frames_rendered: u64,
    cadence_misses: u64,
    final_stage_revision: u64,
    frame_checksum: u64,
    frame_width: usize,
    frame_height: usize,
    frame_stride: usize,
    frame_format: &'static str,
    frame_private: bool,
}

fn descriptor(name: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(name))
        .expect("representative actor descriptor")
}

fn field(actor: &TypeDescriptor, name: &str) -> u32 {
    actor
        .constructor_fields
        .iter()
        .find(|field| field.name == name)
        .expect("representative constructor field")
        .id
}

fn property(actor: &TypeDescriptor, name: &str) -> u32 {
    actor
        .properties
        .iter()
        .find(|property| property.name == name)
        .expect("representative actor property")
        .id
}

fn bounds_input(actor: &TypeDescriptor, bounds: Rect) -> ConstructorInput<'static> {
    ConstructorInput {
        id: field(actor, "bounds"),
        value: ValueRef::Rect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        },
    }
}

fn registry_limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 1,
        max_actors: 2,
        max_tree_depth: 2,
        max_children_per_actor: 1,
        max_text_bytes: 64,
        max_resources: 1,
    }
}

fn build_representative_runtime(
    turn_budget: usize,
    frame_period: Duration,
    tracker: Arc<SemanticTracker>,
) -> RepresentativeRuntime {
    let stage_id = StageId::new(STAGE_ID).expect("nonzero representative Stage id");
    let mut stage = StageRegistry::new(stage_id, &CATALOG, registry_limits())
        .expect("construct representative Stage");
    let container = descriptor("container::Container");
    let root_id = stage
        .create(
            container.type_id,
            CreateDestination::Root { name: "main" },
            &[bounds_input(
                container,
                Rect {
                    x: 0,
                    y: 0,
                    width: FRAME_WIDTH as i32,
                    height: FRAME_HEIGHT as i32,
                },
            )],
        )
        .expect("construct representative root");
    let slider = descriptor("slider::Slider");
    let slider_id = stage
        .create(
            slider.type_id,
            CreateDestination::Child { parent: root_id },
            &[
                bounds_input(slider, SLIDER_BOUNDS),
                ConstructorInput {
                    id: field(slider, "min"),
                    value: ValueRef::I32(0),
                },
                ConstructorInput {
                    id: field(slider, "max"),
                    value: ValueRef::I32(100),
                },
            ],
        )
        .expect("construct representative slider");
    stage
        .apply_batch(&[StageDirection::SetFlag {
            object_id: slider_id,
            flag: RuntimeFlag::Clickable,
            enabled: true,
        }])
        .expect("make representative slider targetable");
    let slider_actor = ActorIdentity {
        object_id: slider_id,
        type_id: slider.type_id,
    };
    let cue_slots = turn_budget
        .checked_add(2)
        .expect("representative Cue slots fit usize");
    let mut endpoint = Endpoint::new(
        EndpointEpoch::new(1).expect("nonzero epoch"),
        EndpointLimits::new(1, 1, 1, turn_budget).expect("positive probe endpoint limits"),
        SubscriptionLimits::new(1, 32, 1, 1).expect("valid probe subscription limits"),
        CueLimits::new(cue_slots, 1, turn_budget, 32, CUE_FRAME_OVERHEAD_BYTES + 32)
            .expect("valid probe cue limits"),
    )
    .expect("construct probe Endpoint on its owner thread");
    endpoint
        .register_stage(stage)
        .expect("register representative Stage");
    endpoint
        .subscribe(SubscribeRequest {
            stage_id,
            actor_identity: slider_actor,
            event_id: EventId::new(slider::MPY_VALUE_CHANGED_EVENT_ID)
                .expect("nonzero slider event id"),
            callback_id: CallbackId::new(1).expect("nonzero callback id"),
            phase: DispatchPhase::Target,
            filter: SubscriptionFilter::Any,
            propagation: PropagationPolicy::Observe,
        })
        .expect("subscribe representative slider event");
    RepresentativeRuntime {
        endpoint,
        stage_id,
        root_id,
        slider_actor,
        slider_value_property: property(slider, "value"),
        next_request_id: 1,
        frame: PrivateRgbaFrame::new(),
        next_frame_deadline: Instant::now(),
        frame_period,
        tracker,
    }
}

fn metric_delta(after: u64, before: u64) -> u64 {
    after.saturating_sub(before)
}

fn execute(config: ProbeConfig) -> Result<ProbeResult, String> {
    let tracker = Arc::new(EnvelopeTracker::default());
    let semantic_tracker = Arc::new(SemanticTracker::default());
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
    let build_semantic_tracker = Arc::clone(&semantic_tracker);
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
            runtime: build_representative_runtime(
                config.turn_budget,
                Duration::from_micros(config.frame_period_us),
                build_semantic_tracker,
            ),
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
        schema_version: "CPY-CAPACITY-PROBE-3",
        workload: "bounded-native-service-with-stage-input-cues-private-rgba-and-os-readiness",
        retained_bytes_scope: "owned probe request and result structs plus payload capacities; private RGBA frame, native service/channel/readiness allocation, and process overhead excluded",
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
        workload_requests: semantic_tracker.workload_requests.load(Ordering::SeqCst),
        stage_batches: semantic_tracker.stage_batches.load(Ordering::SeqCst),
        stage_completions: semantic_tracker.stage_completions.load(Ordering::SeqCst),
        native_inputs: semantic_tracker.native_inputs.load(Ordering::SeqCst),
        cue_records: semantic_tracker.cue_records.load(Ordering::SeqCst),
        frames_rendered: semantic_tracker.frames_rendered.load(Ordering::SeqCst),
        cadence_misses: semantic_tracker.cadence_misses.load(Ordering::SeqCst),
        final_stage_revision: semantic_tracker.final_stage_revision.load(Ordering::SeqCst),
        frame_checksum: semantic_tracker.frame_checksum.load(Ordering::SeqCst),
        frame_width: FRAME_WIDTH,
        frame_height: FRAME_HEIGHT,
        frame_stride: FRAME_STRIDE,
        frame_format: "RGBA8888",
        frame_private: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_probe_executes_every_neutral_boundary() {
        let result = execute(ProbeConfig {
            scenario: Scenario::Sustained,
            ingress_capacity: 2,
            egress_capacity: 4,
            turn_budget: 2,
            messages: 4,
            ingress_payload_bytes: 16,
            egress_payload_bytes: 16,
            observer_stall_us: 0,
            retry_backoff_us: 0,
            sampling_hold_us: 0,
            frame_period_us: 1,
        })
        .expect("execute representative capacity probe");

        assert_eq!(result.accepted_requests, 4);
        assert_eq!(result.completed_records, 4);
        assert_eq!(result.workload_requests, 4);
        assert_eq!(result.native_inputs, 4);
        assert_eq!(result.cue_records, 4);
        assert_eq!(result.stage_batches, result.service_turns);
        assert_eq!(result.stage_completions, result.service_turns);
        assert_eq!(result.frames_rendered, result.service_turns);
        assert!(result.final_stage_revision > 0);
        assert_eq!(
            (result.frame_width, result.frame_height, result.frame_stride),
            (320, 240, 1280)
        );
        assert_eq!(result.frame_format, "RGBA8888");
        assert!(result.frame_private);
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
