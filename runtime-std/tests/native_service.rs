//! CPY-03 native lifecycle, capacity, readiness, fault, and ownership proofs.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

use crossbeam_channel::bounded;
use rlvgl_core::{
    cue::{CUE_FRAME_OVERHEAD_BYTES, CueLimits},
    endpoint::{Endpoint, EndpointLimits},
    subscription::{EndpointEpoch, SubscriptionLimits},
};
use rlvgl_runtime_std::{
    AdmissionError, ReadinessKind, RuntimeFault, ServiceConfig, ServiceLifecycle, ServiceRecord,
    ServiceRejection, spawn_native_service,
};

struct OwnerState {
    endpoint: Endpoint,
    constructed_on: ThreadId,
    dropped_on: Arc<Mutex<Option<ThreadId>>>,
}

impl Drop for OwnerState {
    fn drop(&mut self) {
        *self.dropped_on.lock().expect("drop record lock poisoned") = Some(thread::current().id());
    }
}

struct DropFlagState {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropFlagState {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn build_endpoint() -> Endpoint {
    Endpoint::new(
        EndpointEpoch::new(1).expect("nonzero epoch"),
        EndpointLimits::new(2, 4, 4, 4).expect("valid endpoint limits"),
        SubscriptionLimits::new(2, 32, 2, 2).expect("valid subscription limits"),
        CueLimits::new(4, 1, 2, 32, CUE_FRAME_OVERHEAD_BYTES + 32).expect("valid cue limits"),
    )
    .expect("headless Endpoint construction")
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for native service"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn endpoint_turns_and_drop_remain_on_one_owner_thread() {
    let caller = thread::current().id();
    let dropped_on = Arc::new(Mutex::new(None));
    let build_drop_record = Arc::clone(&dropped_on);
    let service = spawn_native_service(
        "cpy-native-owner",
        ServiceConfig::new(8, 16, 3).expect("valid explicit capacities"),
        move || OwnerState {
            endpoint: build_endpoint(),
            constructed_on: thread::current().id(),
            dropped_on: build_drop_record,
        },
        |state, requests: Vec<u64>| {
            state
                .endpoint
                .run_safe_turn()
                .expect("empty Endpoint Safe Turn");
            requests
                .into_iter()
                .map(|request| {
                    Ok::<_, &'static str>((request, state.constructed_on, thread::current().id()))
                })
                .collect()
        },
    )
    .expect("spawn native service");

    #[cfg(target_os = "linux")]
    assert_eq!(service.readiness().kind(), ReadinessKind::EventFd);
    #[cfg(all(unix, not(target_os = "linux")))]
    assert_eq!(service.readiness().kind(), ReadinessKind::SelfPipe);
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;

        use rustix::{
            fs::{OFlags, fcntl_getfl},
            io::{FdFlags, fcntl_getfd},
        };

        assert!(
            fcntl_getfd(service.readiness().as_fd())
                .expect("read readiness descriptor flags")
                .contains(FdFlags::CLOEXEC)
        );
        assert!(
            fcntl_getfl(service.readiness().as_fd())
                .expect("read readiness status flags")
                .contains(OFlags::NONBLOCK)
        );
    }

    let tickets: Vec<_> = (1..=5)
        .map(|request| service.try_submit(request).expect("admit request"))
        .collect();
    wait_until(|| service.metrics().completed_requests == 5);
    let mut records = service.drain().expect("drain ready records");
    records.extend(service.shutdown().expect("ordered shutdown"));

    let completions: Vec<_> = records
        .iter()
        .filter_map(|record| match record {
            ServiceRecord::Completed {
                ticket,
                output: (request, constructed, ran),
            } => Some((*ticket, *request, *constructed, *ran)),
            _ => None,
        })
        .collect();
    assert_eq!(completions.len(), tickets.len());
    for (index, (ticket, request, constructed, ran)) in completions.iter().enumerate() {
        assert_eq!(*ticket, tickets[index]);
        assert_eq!(*request, u64::try_from(index + 1).expect("small request"));
        assert_ne!(*constructed, caller);
        assert_eq!(constructed, ran);
    }
    let owner = completions[0].2;
    assert_eq!(
        *dropped_on.lock().expect("drop record lock poisoned"),
        Some(owner)
    );
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::Lifecycle {
            state: ServiceLifecycle::Running,
            ..
        }
    )));
    assert!(matches!(
        records.last(),
        Some(ServiceRecord::Lifecycle {
            state: ServiceLifecycle::Closed,
            ..
        })
    ));
}

#[test]
fn closed_is_observable_only_after_owner_state_destruction() {
    let dropped = Arc::new(AtomicBool::new(false));
    let owner_dropped = Arc::clone(&dropped);
    let service = spawn_native_service(
        "cpy-closed-after-drop",
        ServiceConfig::new(1, 1, 1).expect("valid explicit capacities"),
        move || DropFlagState {
            dropped: owner_dropped,
        },
        |_, requests: Vec<u64>| requests.into_iter().map(Ok::<_, &'static str>).collect(),
    )
    .expect("spawn owner-destruction service");

    let startup = service.drain().expect("drain Running lifecycle");
    assert!(matches!(
        startup.as_slice(),
        [ServiceRecord::Lifecycle {
            state: ServiceLifecycle::Running,
            ..
        }]
    ));
    assert!(service.request_close());
    wait_until(|| service.lifecycle() == ServiceLifecycle::Closed);
    let dropped_before_closed = dropped.load(Ordering::Acquire);
    let records = service.shutdown().expect("join closed service");

    assert!(
        dropped_before_closed,
        "Closed became observable before owner state destruction"
    );
    assert_eq!(
        records
            .iter()
            .filter_map(|record| match record {
                ServiceRecord::Lifecycle { state, .. } => Some(*state),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![ServiceLifecycle::Closing, ServiceLifecycle::Closed]
    );
}

#[test]
fn stable_backlog_forms_deterministic_fifo_turns_within_budget() {
    let (batch_sender, batch_receiver) = bounded(3);
    let (release_sender, release_receiver) = bounded(1);
    let service = spawn_native_service(
        "cpy-deterministic-turns",
        ServiceConfig::new(8, 16, 3).expect("valid explicit capacities"),
        || (),
        move |_, requests: Vec<u64>| {
            batch_sender
                .send(requests.clone())
                .expect("record committed turn batch");
            if requests == [0] {
                release_receiver.recv().expect("release first turn");
            }
            requests.into_iter().map(Ok::<_, &'static str>).collect()
        },
    )
    .expect("spawn deterministic-turn service");

    let mut tickets = vec![service.try_submit(0).expect("admit first request")];
    assert_eq!(
        batch_receiver.recv().expect("first turn committed"),
        vec![0]
    );
    tickets.extend(
        (1_u64..=6).map(|request| service.try_submit(request).expect("admit stable backlog")),
    );
    release_sender.send(()).expect("release first turn");

    assert_eq!(
        batch_receiver.recv().expect("second turn committed"),
        vec![1, 2, 3]
    );
    assert_eq!(
        batch_receiver.recv().expect("third turn committed"),
        vec![4, 5, 6]
    );
    wait_until(|| service.metrics().completed_requests == 7);
    let mut records = service.drain().expect("drain turn records");
    let metrics = service.metrics();
    records.extend(
        service
            .shutdown()
            .expect("close deterministic-turn service"),
    );

    assert_eq!(metrics.service_turns, 3);
    assert_eq!(
        records
            .iter()
            .filter_map(|record| match record {
                ServiceRecord::Completed { output, .. } => Some(*output),
                _ => None,
            })
            .collect::<Vec<_>>(),
        (0_u64..=6).collect::<Vec<_>>()
    );
    assert_eq!(
        records
            .iter()
            .filter_map(ServiceRecord::ticket)
            .collect::<Vec<_>>(),
        tickets
    );
}

#[test]
fn restart_stress_rejects_every_prior_epoch_ticket_and_record() {
    let mut prior_epochs = Vec::new();
    let mut prior_tickets = Vec::new();
    let mut prior_records = Vec::new();

    for cycle in 0_u64..64 {
        let service = spawn_native_service(
            "cpy-restart-epoch",
            ServiceConfig::new(2, 8, 1).expect("valid explicit capacities"),
            || (),
            |_, requests: Vec<u64>| requests.into_iter().map(Ok::<_, &'static str>).collect(),
        )
        .expect("spawn restarted native service");
        let epoch = service.epoch();

        if let Some(stale_epoch) = prior_epochs.last().copied() {
            assert!(epoch > stale_epoch, "service epochs must increase");
        }
        for &stale_epoch in &prior_epochs {
            let mismatch = service
                .validate_epoch(stale_epoch)
                .expect_err("prior service epoch must be stale");
            assert_eq!(mismatch.current(), epoch);
            assert_eq!(mismatch.received(), stale_epoch);
        }
        for &stale_ticket in &prior_tickets {
            let mismatch = service
                .validate_ticket(stale_ticket)
                .expect_err("prior request ticket must be stale");
            assert_eq!(mismatch.current(), epoch);
            assert_eq!(mismatch.received(), stale_ticket.epoch());
        }
        for record in &prior_records {
            let mismatch = service
                .validate_record(record)
                .expect_err("prior service record must be stale");
            assert_eq!(mismatch.current(), epoch);
            assert_eq!(mismatch.received(), record.epoch());
        }

        service
            .validate_epoch(epoch)
            .expect("current service epoch remains valid");
        let ticket = service
            .try_submit(cycle)
            .expect("admit one request in restarted service");
        service
            .validate_ticket(ticket)
            .expect("current request ticket remains valid");
        wait_until(|| service.metrics().completed_requests == 1);
        let mut records = service.drain().expect("drain restarted service");
        for record in &records {
            service
                .validate_record(record)
                .expect("current egress record remains valid");
        }
        records.extend(service.shutdown().expect("ordered restarted shutdown"));
        assert!(records.iter().all(|record| record.epoch() == epoch));

        prior_epochs.push(epoch);
        prior_tickets.push(ticket);
        prior_records.extend(records);
    }
}

#[test]
fn full_and_close_outcomes_preserve_exact_terminal_accounting() {
    let (started_sender, started_receiver) = bounded(1);
    let (release_sender, release_receiver) = bounded(1);
    let service = spawn_native_service(
        "cpy-capacity-close",
        ServiceConfig::new(1, 16, 1).expect("valid explicit capacities"),
        || (),
        move |_, requests: Vec<u64>| {
            started_sender.send(()).expect("announce active turn");
            release_receiver.recv().expect("release active turn");
            requests
                .into_iter()
                .map(|request| Ok::<_, &'static str>(request * 2))
                .collect()
        },
    )
    .expect("spawn native service");

    let first = service.try_submit(10).expect("first admission");
    started_receiver.recv().expect("turn became active");
    let second = service.try_submit(20).expect("queued admission");
    assert_eq!(service.try_submit(30), Err(AdmissionError::Full(30)));
    assert!(service.request_close());
    assert!(!service.request_close());
    assert_eq!(service.try_submit(40), Err(AdmissionError::Closing(40)));
    release_sender.send(()).expect("release active turn");

    let records = service.shutdown().expect("ordered shutdown");
    let terminals: Vec<_> = records.iter().filter_map(ServiceRecord::ticket).collect();
    assert_eq!(terminals, vec![first, second]);
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::Completed { ticket, output: 20 } if *ticket == first
    )));
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::Rejected {
            ticket,
            reason: ServiceRejection::ServiceClosing,
        } if *ticket == second
    )));
}

#[test]
fn close_stress_finishes_active_turn_and_rejects_every_queued_request_once() {
    for cycle in 0_u64..64 {
        let (started_sender, started_receiver) = bounded(1);
        let (release_sender, release_receiver) = bounded(1);
        let service = spawn_native_service(
            "cpy-close-stress",
            ServiceConfig::new(4, 1, 1).expect("valid explicit capacities"),
            || (),
            move |_, requests: Vec<u64>| {
                started_sender.send(()).expect("announce active turn");
                release_receiver.recv().expect("release active turn");
                requests.into_iter().map(Ok::<_, &'static str>).collect()
            },
        )
        .expect("spawn close-stress service");

        let mut records = service.drain().expect("drain startup lifecycle");
        let active = service
            .try_submit(cycle)
            .expect("admit active close-stress request");
        started_receiver.recv().expect("turn became active");
        let queued: Vec<_> = (1_u64..=4)
            .map(|offset| {
                service
                    .try_submit(cycle * 10 + offset)
                    .expect("fill bounded ingress behind active turn")
            })
            .collect();

        assert!(service.request_close(), "first caller owns close fence");
        assert!(!service.request_close(), "close fence is idempotent");
        assert_eq!(
            service.try_submit(u64::MAX),
            Err(AdmissionError::Closing(u64::MAX))
        );
        release_sender.send(()).expect("release active turn");
        records.extend(service.shutdown().expect("ordered close-stress shutdown"));

        let lifecycle: Vec<_> = records
            .iter()
            .filter_map(|record| match record {
                ServiceRecord::Lifecycle { state, .. } => Some(*state),
                _ => None,
            })
            .collect();
        assert_eq!(
            lifecycle,
            vec![
                ServiceLifecycle::Running,
                ServiceLifecycle::Closing,
                ServiceLifecycle::Closed,
            ]
        );
        let terminal_tickets: Vec<_> = records.iter().filter_map(ServiceRecord::ticket).collect();
        let expected_tickets: Vec<_> = core::iter::once(active)
            .chain(queued.iter().copied())
            .collect();
        assert_eq!(terminal_tickets, expected_tickets);
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record,
                    ServiceRecord::Completed { ticket, output } if *ticket == active && *output == cycle
                ))
                .count(),
            1
        );
        for queued_ticket in queued {
            assert_eq!(
                records
                    .iter()
                    .filter(|record| matches!(
                        record,
                        ServiceRecord::Rejected {
                            ticket,
                            reason: ServiceRejection::ServiceClosing,
                        } if *ticket == queued_ticket
                    ))
                    .count(),
                1
            );
        }
        assert!(!records.iter().any(|record| matches!(
            record,
            ServiceRecord::DriverFault { .. }
                | ServiceRecord::RuntimeFault { .. }
                | ServiceRecord::ServiceFault { .. }
        )));
    }
}

#[test]
fn driver_fault_fences_later_accepted_work() {
    let (started_sender, started_receiver) = bounded(1);
    let (release_sender, release_receiver) = bounded(1);
    let service = spawn_native_service(
        "cpy-driver-fault",
        ServiceConfig::new(4, 16, 1).expect("valid explicit capacities"),
        || (),
        move |_, requests: Vec<u64>| {
            started_sender.send(()).expect("announce faulting turn");
            release_receiver.recv().expect("release faulting turn");
            requests
                .into_iter()
                .map(|request| {
                    if request == 1 {
                        Err("synthetic fault")
                    } else {
                        Ok(request)
                    }
                })
                .collect()
        },
    )
    .expect("spawn native service");
    let first = service.try_submit(1).expect("first admission");
    started_receiver
        .recv()
        .expect("faulting turn became active");
    let second = service.try_submit(2).expect("second admission");
    release_sender.send(()).expect("release faulting turn");
    wait_until(|| service.lifecycle() == ServiceLifecycle::Closed);
    let records = service
        .shutdown()
        .expect("faulted shutdown remains joinable");

    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::DriverFault { ticket, fault: "synthetic fault" } if *ticket == first
    )));
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::Rejected {
            ticket,
            reason: ServiceRejection::ServiceFaulted,
        } if *ticket == second
    )));
    let lifecycle: Vec<_> = records
        .iter()
        .filter_map(|record| match record {
            ServiceRecord::Lifecycle { state, .. } => Some(*state),
            _ => None,
        })
        .collect();
    assert_eq!(
        lifecycle,
        vec![
            ServiceLifecycle::Running,
            ServiceLifecycle::Faulted,
            ServiceLifecycle::Closed,
        ]
    );
}

#[test]
fn panicked_turn_becomes_runtime_fault_without_payload_projection() {
    let service = spawn_native_service(
        "cpy-turn-panic",
        ServiceConfig::new(2, 16, 1).expect("valid explicit capacities"),
        || (),
        |_, _requests: Vec<u64>| -> Vec<Result<u64, &'static str>> {
            panic!("synthetic interpreter-neutral panic payload")
        },
    )
    .expect("spawn native service");
    let ticket = service.try_submit(1).expect("admit panicking request");
    wait_until(|| service.lifecycle() == ServiceLifecycle::Closed);
    let records = service
        .shutdown()
        .expect("panic is contained by owner loop");
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::RuntimeFault {
            ticket: actual,
            fault: RuntimeFault::TurnPanicked,
        } if *actual == ticket
    )));
}

#[test]
fn outcome_count_mismatch_faults_every_request_in_the_committed_turn() {
    let service = spawn_native_service(
        "cpy-outcome-count",
        ServiceConfig::new(4, 16, 2).expect("valid explicit capacities"),
        || (),
        |_, _requests: Vec<u64>| -> Vec<Result<u64, &'static str>> { Vec::new() },
    )
    .expect("spawn native service");
    let ticket = service.try_submit(1).expect("admit mismatched request");
    wait_until(|| service.lifecycle() == ServiceLifecycle::Closed);
    let records = service.shutdown().expect("mismatch is contained");
    assert!(records.iter().any(|record| matches!(
        record,
        ServiceRecord::RuntimeFault {
            ticket: actual,
            fault: RuntimeFault::OutcomeCountMismatch { expected: 1, actual: 0 },
        } if *actual == ticket
    )));
}

#[test]
fn zero_capacities_never_become_implicit_defaults() {
    assert!(ServiceConfig::new(0, 1, 1).is_err());
    assert!(ServiceConfig::new(1, 0, 1).is_err());
    assert!(ServiceConfig::new(1, 1, 0).is_err());
}
