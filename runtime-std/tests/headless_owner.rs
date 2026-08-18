//! Native-only headless proof for the CPY host-runtime ownership boundary.

use std::{
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
};

use rlvgl_core::{
    cue::{CUE_FRAME_OVERHEAD_BYTES, CueLimits},
    endpoint::{Endpoint, EndpointLimits, EndpointState},
    subscription::{EndpointEpoch, SubscriptionLimits},
};
use rlvgl_runtime_std::{NativeTaskJoinError, spawn_owned_thread_task};

struct HeadlessState {
    endpoint: Endpoint,
    constructed_on: ThreadId,
    dropped_on: Arc<Mutex<Option<ThreadId>>>,
}

impl Drop for HeadlessState {
    fn drop(&mut self) {
        *self.dropped_on.lock().expect("drop record lock poisoned") = Some(thread::current().id());
    }
}

fn build_endpoint() -> Endpoint {
    Endpoint::new(
        EndpointEpoch::new(1).expect("nonzero epoch"),
        EndpointLimits::new(2, 2, 2, 4).expect("valid endpoint limits"),
        SubscriptionLimits::new(2, 32, 2, 2).expect("valid subscription limits"),
        CueLimits::new(4, 1, 2, 32, CUE_FRAME_OVERHEAD_BYTES + 32).expect("valid cue limits"),
    )
    .expect("headless Endpoint construction")
}

#[test]
fn non_send_endpoint_lives_and_drops_on_its_native_owner() {
    let caller = thread::current().id();
    let dropped_on = Arc::new(Mutex::new(None));
    let build_drop_record = Arc::clone(&dropped_on);
    let task = spawn_owned_thread_task(
        "rlvgl-headless-owner",
        move || HeadlessState {
            endpoint: build_endpoint(),
            constructed_on: thread::current().id(),
            dropped_on: build_drop_record,
        },
        |state| {
            assert_eq!(state.endpoint.state(), EndpointState::Ready);
            (
                state.constructed_on,
                thread::current().id(),
                state.endpoint.endpoint_epoch().get(),
            )
        },
    )
    .expect("spawn native owner");

    let owner = task.thread_id();
    assert_ne!(caller, owner);
    assert_eq!(task.thread_name(), Some("rlvgl-headless-owner"));
    let (constructed, ran, epoch) = task.join().expect("join native owner");
    assert_eq!(constructed, owner);
    assert_eq!(ran, owner);
    assert_eq!(epoch, 1);
    assert_eq!(
        *dropped_on.lock().expect("drop record lock poisoned"),
        Some(owner)
    );
}

#[test]
fn panic_is_projected_without_exposing_the_payload() {
    let task = spawn_owned_thread_task(
        "rlvgl-panic-proof",
        || (),
        |_| -> () {
            panic!("synthetic native failure");
        },
    )
    .expect("spawn panic proof");

    assert_eq!(task.join(), Err(NativeTaskJoinError::Panicked));
}
