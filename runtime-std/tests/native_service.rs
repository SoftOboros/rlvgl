//! CPY-03 native lifecycle, capacity, readiness, fault, and ownership proofs.

use std::{
    sync::{Arc, Mutex},
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
