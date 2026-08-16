//! Focused conformance tests for the MPY-05 endpoint cue queue substrate.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use rlvgl_api::protocol::{FrameRef, MPY_V1, decode_frame, encode_frame};
use rlvgl_core::{
    actor::{ObjectId, StageId},
    cue::{
        CUE_FLAG_LATEST_VALUE_MERGED, CUE_FLAG_MPY05_METADATA, CUE_FLAG_SUBSCRIPTION_RELEASE,
        CUE_FRAME_OVERHEAD_BYTES, CUE_METADATA_ENVELOPE_BYTES, CallbackId, CoalescingKey,
        CueAdmission, CueDelivery, CueEndpointState, CueIdentity, CueInput, CueLimits,
        CueLimitsError, CuePayloadRef, CueQueue, CueQueueError, CueSequence, DrainBudget,
        EmergencyFault, EndpointRecord, EnqueueOutcome, EventId, INPUT_OVERFLOW_METADATA_BYTES,
        InputClass, InputSequence, NativeEventSequence, RUNTIME_NOTICE_INPUT_OVERFLOW,
        SubscriptionId,
    },
    direction::StageRevision,
};

struct TrackingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every operation delegates unchanged layouts and pointers to the
// process System allocator; thread-local bookkeeping only observes calls.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` is forwarded unchanged to the System allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = DEALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: both values came from the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` is forwarded unchanged to the System allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the allocation and layout belong to System; `size` is the
        // requested replacement size under GlobalAlloc's contract.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn count_allocations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    let (result, allocations, _) = count_allocator_operations(operation);
    (result, allocations)
}

fn count_allocator_operations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct TrackingGuard;

    impl Drop for TrackingGuard {
        fn drop(&mut self) {
            TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        }
    }

    ALLOCATION_COUNT.with(|count| count.set(0));
    DEALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let guard = TrackingGuard;
    let result = operation();
    drop(guard);
    let allocations = ALLOCATION_COUNT.with(Cell::get);
    let deallocations = DEALLOCATION_COUNT.with(Cell::get);
    (result, allocations, deallocations)
}

fn limits(total: usize, reserve: usize, quota: usize) -> CueLimits {
    CueLimits::new(total, reserve, quota, 64, CUE_FRAME_OVERHEAD_BYTES + 64).unwrap()
}

fn cue(stage: u32, event: u32, delivery: CueDelivery, payload: &[u8]) -> CueInput {
    CueInput::new(
        CueIdentity::new(
            StageId::new(stage).unwrap(),
            ObjectId::new((u64::from(stage) << 32) | 1).unwrap(),
            SubscriptionId::new(event).unwrap(),
            CallbackId::new(event).unwrap(),
            EventId::new(event).unwrap(),
        ),
        StageRevision::new(9),
        NativeEventSequence::new(u64::from(event)).unwrap(),
        delivery,
        payload.to_vec(),
    )
}

fn coalescible(stage: u32, event: u32, native_sequence: u64, key: u64, payload: &[u8]) -> CueInput {
    CueInput::new(
        CueIdentity::new(
            StageId::new(stage).unwrap(),
            ObjectId::new((u64::from(stage) << 32) | 1).unwrap(),
            SubscriptionId::new(event).unwrap(),
            CallbackId::new(event).unwrap(),
            EventId::new(event).unwrap(),
        ),
        StageRevision::new(9),
        NativeEventSequence::new(native_sequence).unwrap(),
        CueDelivery::LatestValueCoalescible,
        payload.to_vec(),
    )
    .with_coalescing_key(CoalescingKey::new(key))
}

fn cue_with_causality(
    stage: u32,
    revision: u64,
    native_sequence: u64,
    event: u32,
    subscription: u32,
    delivery: CueDelivery,
    payload: &[u8],
) -> CueInput {
    CueInput::new(
        CueIdentity::new(
            StageId::new(stage).unwrap(),
            ObjectId::new((u64::from(stage) << 32) | 1).unwrap(),
            SubscriptionId::new(subscription).unwrap(),
            CallbackId::new(subscription).unwrap(),
            EventId::new(event).unwrap(),
        ),
        StageRevision::new(revision),
        NativeEventSequence::new(native_sequence).unwrap(),
        delivery,
        payload.to_vec(),
    )
}

#[test]
fn ordinary_traffic_cannot_consume_critical_reserve() {
    let mut queue = CueQueue::new(limits(4, 1, 3)).unwrap();

    for event in 1..=3 {
        queue
            .enqueue(cue(1, event, CueDelivery::Ordered, &[event as u8]))
            .unwrap();
    }
    assert_eq!(queue.ordinary_len(), 3);
    assert_eq!(
        queue.enqueue(cue(1, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::OrdinaryCapacityExhausted {
            sequence: sequence(4),
        })
    );

    assert_eq!(
        queue.enqueue(cue(1, 5, CueDelivery::Critical, &[5])),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(5),
        })
    );
    assert_eq!(queue.len(), 4);
    assert_eq!(queue.critical_len(), 1);
}

#[test]
fn per_stage_quota_gates_other_stages_until_a_critical_notice_is_queued() {
    let mut queue = CueQueue::new(limits(6, 1, 2)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[2]))
        .unwrap();

    assert_eq!(
        queue.enqueue(cue(1, 3, CueDelivery::Ordered, &[3])),
        Err(CueQueueError::StageQuotaExhausted {
            sequence: sequence(3),
            stage_id: StageId::new(1).unwrap(),
        })
    );
    assert_eq!(
        queue.enqueue(cue(2, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::OrdinaryBackpressured {
            sequence: sequence(4),
        })
    );
    let loss = queue.pending_loss().unwrap();
    assert_eq!(loss.first_sequence(), sequence(3));
    assert_eq!(loss.last_sequence(), sequence(4));
    assert_eq!(loss.lost_count(), 2);
    let queued_before_notice = queue.len();
    assert_eq!(
        queue.drain(DrainBudget::for_limits(queue.limits(), 8)),
        Err(CueQueueError::DrainBackpressured)
    );
    assert_eq!(queue.len(), queued_before_notice);

    let notice = queue
        .enqueue_pending_loss_notice(cue(1, 5, CueDelivery::Critical, &[3, 4]))
        .unwrap();
    assert_eq!(notice.loss, loss);
    assert_eq!(notice.notice_sequence, sequence(5));
    assert_eq!(queue.state(), CueEndpointState::Ready);
    assert_eq!(
        queue.enqueue(cue(2, 6, CueDelivery::Ordered, &[6])),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(6),
        })
    );

    let drained = queue
        .drain(DrainBudget::for_limits(queue.limits(), 8))
        .unwrap();
    let sequences: Vec<_> = drained
        .cues
        .iter()
        .map(|record| record.last_sequence().get())
        .collect();
    assert_eq!(sequences, [1, 2, 5, 6]);
}

#[test]
fn one_queue_preserves_cross_stage_global_sequence_order() {
    let mut queue = CueQueue::new(limits(8, 2, 3)).unwrap();
    queue
        .enqueue(cue(2, 1, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Critical, &[1]))
        .unwrap();
    queue
        .enqueue(cue(3, 3, CueDelivery::Ordered, &[3]))
        .unwrap();

    let drained = queue.drain(DrainBudget::new(8, usize::MAX)).unwrap();
    let stages: Vec<_> = drained
        .cues
        .iter()
        .map(|record| record.stage_id().get())
        .collect();
    let sequences: Vec<_> = drained
        .cues
        .iter()
        .map(|record| record.last_sequence().get())
        .collect();
    assert_eq!(stages, [2, 1, 3]);
    assert_eq!(sequences, [1, 2, 3]);
}

#[test]
fn coalescing_is_tail_only_and_requires_the_full_exact_key() {
    let mut queue = CueQueue::new(limits(8, 2, 4)).unwrap();

    queue.enqueue(coalescible(1, 1, 1, 7, &[10])).unwrap();
    assert_eq!(
        queue.enqueue(coalescible(1, 1, 2, 7, &[11])),
        Ok(EnqueueOutcome::Coalesced {
            first_sequence: sequence(1),
            last_sequence: sequence(2),
            merge_count: 1,
        })
    );
    assert_eq!(queue.len(), 1);

    queue.enqueue(coalescible(2, 1, 3, 7, &[20])).unwrap();
    queue.enqueue(coalescible(1, 1, 4, 7, &[12])).unwrap();
    queue
        .enqueue(cue(1, 5, CueDelivery::Ordered, &[30]))
        .unwrap();
    queue.enqueue(coalescible(1, 1, 6, 7, &[13])).unwrap();

    let drained = queue.drain(DrainBudget::new(8, usize::MAX)).unwrap();
    assert_eq!(drained.cues.len(), 5);
    assert_eq!(drained.cues[0].first_sequence(), sequence(1));
    assert_eq!(drained.cues[0].last_sequence(), sequence(2));
    assert_eq!(drained.cues[0].stage_revision(), StageRevision::new(9));
    assert_eq!(
        drained.cues[0].first_native_event_sequence(),
        NativeEventSequence::new(1).unwrap()
    );
    assert_eq!(
        drained.cues[0].last_native_event_sequence(),
        NativeEventSequence::new(2).unwrap()
    );
    assert_eq!(drained.cues[0].merge_count(), 1);
    assert_eq!(drained.cues[0].payload(), [11]);
    assert_eq!(drained.cues[1].stage_id(), StageId::new(2).unwrap());
    assert_eq!(drained.cues[2].last_sequence(), sequence(4));
    assert_eq!(drained.cues[4].last_sequence(), sequence(6));
}

#[test]
fn limits_payloads_and_limits_derived_drain_budget_are_validated() {
    assert_eq!(CueLimits::new(0, 0, 0, 0, 0), Err(CueLimitsError::NoSlots));
    assert_eq!(
        CueLimits::new(4, 0, 1, 8, CUE_FRAME_OVERHEAD_BYTES + 8),
        Err(CueLimitsError::NoCriticalReserve)
    );
    assert_eq!(
        CueLimits::new(4, 1, 4, 8, CUE_FRAME_OVERHEAD_BYTES + 8),
        Err(CueLimitsError::StageQuotaExceedsOrdinaryCapacity)
    );
    assert_eq!(
        CueLimits::new(4, 1, 3, 8, CUE_FRAME_OVERHEAD_BYTES + 7),
        Err(CueLimitsError::FrameTooSmallForPayload)
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        CueLimits::new(4, 1, 3, u32::MAX as usize + 1, u32::MAX as usize + 1,),
        Err(CueLimitsError::LimitExceedsProtocolWidth)
    );

    let negotiated = CueLimits::new(4, 1, 3, 2, CUE_FRAME_OVERHEAD_BYTES + 2).unwrap();
    let budget = DrainBudget::for_limits(negotiated, 2);
    assert_eq!(budget.max_cues, 2);
    assert_eq!(budget.max_bytes, negotiated.max_frame_bytes() * 2);

    let mut queue = CueQueue::new(negotiated).unwrap();
    assert_eq!(
        queue.enqueue(cue(1, 1, CueDelivery::Ordered, &[1, 2, 3])),
        Err(CueQueueError::PayloadTooLarge {
            actual: 3,
            maximum: 2,
        })
    );
    assert!(queue.is_empty());

    let impossible = CueLimits::new(usize::MAX, 1, 1, 0, CUE_FRAME_OVERHEAD_BYTES).unwrap();
    assert!(matches!(
        CueQueue::new(impossible),
        Err(CueQueueError::AllocationFailed)
    ));
}

#[test]
fn canonical_payload_envelope_round_trips_through_the_mpy_codec() {
    let mut queue = CueQueue::new(limits(4, 1, 3)).unwrap();
    queue.enqueue(coalescible(1, 1, 1, 7, &[10])).unwrap();
    queue.enqueue(coalescible(1, 1, 2, 7, &[11])).unwrap();
    let drained = queue
        .drain(DrainBudget::for_limits(queue.limits(), 1))
        .unwrap();
    let record = &drained.cues[0];
    assert_eq!(record.frame_bytes(), CUE_FRAME_OVERHEAD_BYTES + 1);

    let mut payload = [0u8; CUE_METADATA_ENVELOPE_BYTES + 1];
    let payload_len = record.encode_payload_envelope(&mut payload).unwrap();
    let expected = [
        9, 0, 0, 0, 0, 0, 0, 0, // Stage Revision
        1, 0, 0, 0, 0, 0, 0, 0, // first native event
        2, 0, 0, 0, 0, 0, 0, 0, // last native event
        1, 0, 0, 0, // first cue
        2, 0, 0, 0, // last cue
        1, 0, 0, 0,  // merge count
        11, // latest event payload
    ];
    assert_eq!(payload_len, expected.len());
    assert_eq!(payload, expected);
    let decoded_payload = CuePayloadRef::decode(&payload).unwrap();
    assert_eq!(decoded_payload.metadata, record.payload_metadata());
    assert_eq!(decoded_payload.event_payload, [11]);

    let mut envelope = [0u8; CUE_METADATA_ENVELOPE_BYTES + 1];
    let frame_ref = record.protocol_frame(&mut envelope).unwrap();
    let mut encoded_frame = [0u8; 128];
    let encoded_len = encode_frame(MPY_V1, frame_ref, &mut encoded_frame).unwrap();
    assert_eq!(encoded_len, record.frame_bytes());

    let decoded_frame = decode_frame(&encoded_frame[..encoded_len]).unwrap();
    assert_eq!(decoded_frame.version, MPY_V1);
    let FrameRef::Cue(cue) = decoded_frame.frame else {
        panic!("expected Cue frame");
    };
    assert_eq!(cue.sequence, 2);
    assert_eq!(
        cue.flags,
        CUE_FLAG_MPY05_METADATA | CUE_FLAG_LATEST_VALUE_MERGED
    );
    let decoded_envelope = CuePayloadRef::decode(cue.payload).unwrap();
    assert_eq!(decoded_envelope.metadata, record.payload_metadata());
    assert_eq!(decoded_envelope.event_payload, [11]);
}

#[test]
fn causality_rejects_regressions_but_allows_one_event_for_multiple_subscriptions() {
    let mut queue = CueQueue::new(limits(8, 2, 4)).unwrap();
    queue
        .enqueue(cue_with_causality(
            1,
            2,
            10,
            7,
            1,
            CueDelivery::Ordered,
            &[1],
        ))
        .unwrap();
    queue
        .enqueue(cue_with_causality(
            1,
            2,
            10,
            7,
            2,
            CueDelivery::Ordered,
            &[2],
        ))
        .unwrap();

    assert_eq!(
        queue.enqueue(cue_with_causality(
            1,
            1,
            11,
            8,
            3,
            CueDelivery::Ordered,
            &[3],
        )),
        Err(CueQueueError::StageRevisionRegressed {
            stage_id: StageId::new(1).unwrap(),
            previous: StageRevision::new(2),
            offered: StageRevision::new(1),
        })
    );
    assert_eq!(
        queue.enqueue(cue_with_causality(
            1,
            3,
            9,
            8,
            3,
            CueDelivery::Ordered,
            &[3],
        )),
        Err(CueQueueError::NativeEventSequenceRegressed {
            previous: NativeEventSequence::new(10).unwrap(),
            offered: NativeEventSequence::new(9).unwrap(),
        })
    );
    assert_eq!(
        queue.enqueue(cue_with_causality(
            1,
            3,
            11,
            8,
            3,
            CueDelivery::Ordered,
            &[3],
        )),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(3),
        })
    );
}

#[test]
fn loss_notice_exhaustion_faults_instead_of_releasing_the_barrier() {
    let mut queue = CueQueue::new(limits(3, 1, 2)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(2, 3, CueDelivery::Critical, &[3]))
        .unwrap();
    assert!(matches!(
        queue.enqueue(cue(1, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::OrdinaryCapacityExhausted { .. })
    ));

    assert_eq!(
        queue.enqueue_pending_loss_notice(cue(1, 5, CueDelivery::Critical, &[4])),
        Err(CueQueueError::CriticalCapacityExhausted {
            sequence: sequence(5),
        })
    );
    assert_eq!(queue.state(), CueEndpointState::Faulted);
    assert!(queue.pending_loss().is_some());
    assert_eq!(
        queue.emergency_fault(),
        Some(&EmergencyFault::CriticalCapacityExhausted {
            sequence: sequence(5),
            stage_id: StageId::new(1).unwrap(),
            event_id: EventId::new(5).unwrap(),
        })
    );
}

#[test]
fn critical_new_stage_faults_when_epoch_causality_capacity_is_exhausted() {
    let mut queue = CueQueue::new(limits(2, 1, 1)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .drain(DrainBudget::for_limits(queue.limits(), 1))
        .unwrap();
    queue
        .enqueue(cue(2, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .drain(DrainBudget::for_limits(queue.limits(), 1))
        .unwrap();

    assert_eq!(
        queue.enqueue(cue(3, 3, CueDelivery::Critical, &[3])),
        Err(CueQueueError::StageCausalityCapacityExhausted {
            stage_id: StageId::new(3).unwrap(),
        })
    );
    assert_eq!(queue.state(), CueEndpointState::Faulted);
    assert_eq!(
        queue.emergency_fault(),
        Some(&EmergencyFault::CriticalStageCausalityCapacityExhausted {
            stage_id: StageId::new(3).unwrap(),
            event_id: EventId::new(3).unwrap(),
        })
    );
}

#[test]
fn drains_stop_at_both_cue_and_byte_budgets_without_reordering() {
    let mut queue = CueQueue::new(limits(8, 2, 4)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1, 2]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[3, 4, 5]))
        .unwrap();
    queue
        .enqueue(cue(2, 3, CueDelivery::Critical, &[6]))
        .unwrap();

    let too_small = queue
        .drain(DrainBudget::new(3, CUE_FRAME_OVERHEAD_BYTES + 1))
        .unwrap();
    assert!(too_small.cues.is_empty());
    assert_eq!(queue.len(), 3);

    let first_frame = CUE_FRAME_OVERHEAD_BYTES + 2;
    let first = queue.drain(DrainBudget::new(2, first_frame)).unwrap();
    assert_eq!(first.cues.len(), 1);
    assert_eq!(first.frame_bytes, first_frame);
    assert_eq!(first.cues[0].event_id(), EventId::new(1).unwrap());

    let second = queue.drain(DrainBudget::new(1, usize::MAX)).unwrap();
    assert_eq!(second.cues.len(), 1);
    assert_eq!(second.cues[0].event_id(), EventId::new(2).unwrap());
    assert_eq!(queue.len(), 1);
}

#[test]
fn critical_exhaustion_faults_endpoint_and_publishes_emergency_notice() {
    let mut queue = CueQueue::new(limits(3, 1, 2)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(2, 3, CueDelivery::Critical, &[3]))
        .unwrap();

    assert_eq!(
        queue.enqueue(cue(2, 4, CueDelivery::Critical, &[4])),
        Err(CueQueueError::CriticalCapacityExhausted {
            sequence: sequence(4),
        })
    );
    assert_eq!(queue.state(), CueEndpointState::Faulted);
    assert_eq!(
        queue.emergency_fault(),
        Some(&EmergencyFault::CriticalCapacityExhausted {
            sequence: sequence(4),
            stage_id: StageId::new(2).unwrap(),
            event_id: EventId::new(4).unwrap(),
        })
    );
    assert!(matches!(
        queue.enqueue(cue(2, 5, CueDelivery::Critical, &[5])),
        Err(CueQueueError::Faulted)
    ));

    queue.reset_epoch();
    assert_eq!(queue.state(), CueEndpointState::Ready);
    assert!(queue.is_empty());
    assert_eq!(
        queue.enqueue(cue(1, 1, CueDelivery::Ordered, &[1])),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(1),
        })
    );
}

#[test]
fn logical_reservation_prevents_slot_theft_and_releases_unused_counts() {
    let stage = StageId::new(1).unwrap();
    let mut queue = CueQueue::new(limits(5, 1, 3)).unwrap();

    {
        let mut reservation = queue
            .reserve(CueAdmission {
                stage_id: stage,
                ordinary_slots: 2,
                critical_slots: 0,
            })
            .unwrap();
        assert_eq!(reservation.remaining_ordinary(), 2);
        reservation
            .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
            .unwrap();
        assert_eq!(reservation.remaining_ordinary(), 1);
        reservation.finish();
    }

    // The unused count above was logical only and is released on finish.
    let mut reservation = queue
        .reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 2,
            critical_slots: 1,
        })
        .unwrap();
    assert_eq!(reservation.remaining_critical(), 1);
    assert_eq!(
        reservation.enqueue(cue(2, 2, CueDelivery::Ordered, &[2])),
        Err(CueQueueError::ReservationStageMismatch)
    );
    assert_eq!(reservation.remaining_ordinary(), 2);
    reservation
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    reservation
        .enqueue(cue(1, 3, CueDelivery::Ordered, &[3]))
        .unwrap();
    assert_eq!(
        reservation.enqueue(cue(1, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::ReservationClassExhausted)
    );
    reservation.finish();

    assert_eq!(queue.ordinary_len(), 3);
    assert!(matches!(
        queue.reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 1,
            critical_slots: 0,
        }),
        Err(CueQueueError::AdmissionStageQuota { stage_id }) if stage_id == stage
    ));
}

#[test]
fn reserved_enqueue_moves_preallocated_payload_without_allocating() {
    let stage = StageId::new(1).unwrap();
    let mut queue = CueQueue::new(limits(4, 1, 3)).unwrap();
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&[1, 2, 3, 4]);
    let payload_pointer = payload.as_ptr();
    let input = CueInput::new(
        CueIdentity::new(
            stage,
            ObjectId::new((1u64 << 32) | 1).unwrap(),
            SubscriptionId::new(1).unwrap(),
            CallbackId::new(1).unwrap(),
            EventId::new(1).unwrap(),
        ),
        StageRevision::new(1),
        NativeEventSequence::new(1).unwrap(),
        CueDelivery::Ordered,
        payload,
    );
    let mut reservation = queue
        .reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 1,
            critical_slots: 0,
        })
        .unwrap();

    let (outcome, allocations) = count_allocations(|| reservation.enqueue(input));
    assert_eq!(
        outcome,
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(1),
        })
    );
    assert_eq!(allocations, 0);
    reservation.finish();

    let drained = queue.drain(DrainBudget::new(1, usize::MAX)).unwrap();
    assert_eq!(drained.cues[0].payload().as_ptr(), payload_pointer);
}

#[test]
fn exact_input_commit_is_infallible_and_retains_displaced_payloads() {
    let mut queue = CueQueue::new(limits(6, 2, 4)).unwrap();
    queue.enqueue(coalescible(1, 1, 1, 7, &[0xaa])).unwrap();
    let mut prepared = queue
        .prepare_exact_inputs(vec![
            coalescible(1, 1, 2, 7, &[0xbb, 0xcc]),
            cue_with_causality(1, 9, 3, 2, 2, CueDelivery::Critical, &[0xdd]),
        ])
        .unwrap();
    assert_eq!(prepared.input_count(), 2);
    assert_eq!(prepared.inputs().len(), 2);
    assert!(!prepared.is_empty());

    let (guard, allocations, deallocations) =
        count_allocator_operations(|| queue.acquire_exact_commit(&mut prepared));
    assert_eq!((allocations, deallocations), (0, 0));
    let guard = guard.unwrap();
    let ((), allocations, deallocations) = count_allocator_operations(|| guard.commit());
    assert_eq!((allocations, deallocations), (0, 0));
    assert!(prepared.inputs().is_empty());
    assert_eq!(queue.len(), 2);

    let (committed, allocations, deallocations) =
        count_allocator_operations(|| queue.commit_exact_inputs(&mut prepared));
    assert_eq!(
        committed,
        Err(CueQueueError::PreparedInputsAlreadyCommitted)
    );
    assert_eq!((allocations, deallocations), (0, 0));
    let ((), allocations, deallocations) =
        count_allocator_operations(|| queue.release_exact_inputs(prepared));
    assert_eq!(allocations, 0);
    assert!(deallocations >= 3);

    let drained = queue.drain(DrainBudget::new(2, usize::MAX)).unwrap();
    assert_eq!(drained.cues[0].first_sequence(), sequence(1));
    assert_eq!(drained.cues[0].last_sequence(), sequence(2));
    assert_eq!(drained.cues[0].merge_count(), 1);
    assert_eq!(drained.cues[0].payload(), &[0xbb, 0xcc]);
    assert_eq!(drained.cues[1].last_sequence(), sequence(3));
}

#[test]
fn exact_input_stale_and_rollback_paths_leave_queue_state_explicit() {
    let mut queue = CueQueue::new(limits(6, 2, 4)).unwrap();
    let mut stale = queue
        .prepare_exact_inputs(vec![cue_with_causality(
            1,
            9,
            1,
            1,
            1,
            CueDelivery::Critical,
            &[1],
        )])
        .unwrap();
    queue
        .enqueue(cue_with_causality(
            1,
            9,
            2,
            2,
            2,
            CueDelivery::Critical,
            &[2],
        ))
        .unwrap();
    let (committed, allocations, deallocations) =
        count_allocator_operations(|| queue.commit_exact_inputs(&mut stale));
    assert_eq!(committed, Err(CueQueueError::StalePreparedInputs));
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(stale.inputs().len(), 1);
    assert_eq!(queue.len(), 1);
    let ((), allocations, deallocations) =
        count_allocator_operations(|| queue.release_exact_inputs(stale));
    assert_eq!(allocations, 0);
    assert!(deallocations >= 3);

    let mut rollback_queue = CueQueue::new(limits(4, 1, 3)).unwrap();
    let rollback = rollback_queue
        .prepare_exact_inputs(vec![cue(1, 1, CueDelivery::Critical, &[1])])
        .unwrap();
    let mut rollback = rollback;
    assert!(rollback_queue.is_empty());
    let (guard, allocations, deallocations) =
        count_allocator_operations(|| rollback_queue.acquire_exact_commit(&mut rollback));
    assert_eq!((allocations, deallocations), (0, 0));
    let ((), allocations, deallocations) = count_allocator_operations(|| guard.unwrap().rollback());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(rollback.inputs().len(), 1);
    assert!(rollback_queue.is_empty());
    let ((), allocations, deallocations) =
        count_allocator_operations(|| rollback_queue.release_exact_inputs(rollback));
    assert_eq!(allocations, 0);
    assert!(deallocations >= 3);
    assert!(rollback_queue.is_empty());
    assert_eq!(
        rollback_queue.enqueue(cue(1, 1, CueDelivery::Critical, &[1])),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(1),
        })
    );
}

#[test]
fn exact_preflight_rejects_full_batch_validation_matrix_without_queue_mutation() {
    let queue = CueQueue::new(limits(3, 1, 2)).unwrap();
    assert!(matches!(
        queue.prepare_exact_inputs(vec![CueInput::new(
            CueIdentity::new(
                StageId::new(1).unwrap(),
                ObjectId::new((1_u64 << 32) | 1).unwrap(),
                SubscriptionId::new(1).unwrap(),
                CallbackId::new(1).unwrap(),
                EventId::new(1).unwrap(),
            ),
            StageRevision::new(1),
            NativeEventSequence::new(1).unwrap(),
            CueDelivery::LatestValueCoalescible,
            vec![1],
        )]),
        Err(CueQueueError::MissingCoalescingKey)
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![
            cue(1, 1, CueDelivery::Ordered, &[1]).with_coalescing_key(CoalescingKey::new(1))
        ]),
        Err(CueQueueError::UnexpectedCoalescingKey)
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![cue(1, 1, CueDelivery::Ordered, &[0; 65])]),
        Err(CueQueueError::PayloadTooLarge {
            actual: 65,
            maximum: 64,
        })
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![
            cue_with_causality(1, 2, 1, 1, 1, CueDelivery::Critical, &[1]),
            cue_with_causality(1, 1, 2, 2, 2, CueDelivery::Critical, &[2]),
        ]),
        Err(CueQueueError::StageRevisionRegressed { .. })
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![
            cue_with_causality(1, 1, 2, 1, 1, CueDelivery::Critical, &[1]),
            cue_with_causality(1, 1, 1, 2, 2, CueDelivery::Critical, &[2]),
        ]),
        Err(CueQueueError::NativeEventSequenceRegressed { .. })
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![
            coalescible(1, 1, 1, 9, &[1]),
            coalescible(1, 1, 1, 9, &[2]),
        ]),
        Err(CueQueueError::NonMonotonicCoalescingEventSequence { .. })
    ));
    assert!(matches!(
        queue.prepare_exact_inputs(vec![
            cue(1, 1, CueDelivery::Critical, &[1]),
            cue(1, 2, CueDelivery::Critical, &[2]),
            cue(1, 3, CueDelivery::Critical, &[3]),
            cue(1, 4, CueDelivery::Critical, &[4]),
        ]),
        Err(CueQueueError::AdmissionCapacity)
    ));
    assert!(queue.is_empty());
}

#[test]
fn backpressure_permits_only_critical_count_and_exact_reservations() {
    let stage = StageId::new(1).unwrap();
    let mut queue = CueQueue::new(limits(5, 2, 3)).unwrap();
    for event in 1..=3 {
        queue
            .enqueue(cue(1, event, CueDelivery::Ordered, &[event as u8]))
            .unwrap();
    }
    assert!(matches!(
        queue.enqueue(cue(1, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::OrdinaryCapacityExhausted { .. })
    ));
    assert_eq!(queue.state(), CueEndpointState::Backpressured);
    assert!(matches!(
        queue.reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 1,
            critical_slots: 0,
        }),
        Err(CueQueueError::AdmissionBackpressured)
    ));
    let mut critical = queue
        .reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 0,
            critical_slots: 1,
        })
        .unwrap();
    critical
        .enqueue(cue(1, 5, CueDelivery::Critical, &[5]))
        .unwrap();
    critical.finish();

    assert!(matches!(
        queue.prepare_exact_inputs(vec![cue(1, 6, CueDelivery::Ordered, &[6])]),
        Err(CueQueueError::AdmissionBackpressured)
    ));
    let mut prepared = queue
        .prepare_exact_inputs(vec![cue(1, 6, CueDelivery::Critical, &[6])])
        .unwrap();
    queue.commit_exact_inputs(&mut prepared).unwrap();
    queue.release_exact_inputs(prepared);
    assert_eq!(queue.critical_len(), 2);
    assert_eq!(queue.state(), CueEndpointState::Backpressured);
}

#[test]
fn input_overflow_notice_gates_and_shares_global_record_order() {
    let stage = StageId::new(1).unwrap();
    let input_class = InputClass::new(7).unwrap();
    let mut queue = CueQueue::new(limits(6, 2, 4)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();

    let first_loss = queue
        .record_input_overflow(stage, input_class, InputSequence::new(10).unwrap())
        .unwrap();
    assert_eq!(first_loss.lost_count(), 1);
    let loss = queue
        .record_input_overflow(stage, input_class, InputSequence::new(11).unwrap())
        .unwrap();
    assert_eq!(loss.first_sequence(), InputSequence::new(10).unwrap());
    assert_eq!(loss.last_sequence(), InputSequence::new(11).unwrap());
    assert_eq!(loss.lost_count(), 2);
    assert_eq!(
        queue.record_input_overflow(stage, input_class, InputSequence::new(11).unwrap()),
        Err(CueQueueError::InputSequenceRegressed {
            previous: InputSequence::new(11).unwrap(),
            offered: InputSequence::new(11).unwrap(),
        })
    );
    assert_eq!(queue.state(), CueEndpointState::Backpressured);
    assert_eq!(queue.pending_input_loss(), Some(loss));
    assert_eq!(
        queue.drain_endpoint(DrainBudget::new(8, usize::MAX)),
        Err(CueQueueError::DrainBackpressured)
    );
    assert!(matches!(
        queue.reserve(CueAdmission {
            stage_id: stage,
            ordinary_slots: 1,
            critical_slots: 0,
        }),
        Err(CueQueueError::AdmissionBackpressured)
    ));
    assert_eq!(
        queue.record_input_overflow(
            StageId::new(2).unwrap(),
            input_class,
            InputSequence::new(12).unwrap(),
        ),
        Err(CueQueueError::InputBackpressured)
    );
    assert_eq!(
        queue.enqueue_pending_input_overflow_notice(vec![0; 65]),
        Err(CueQueueError::PayloadTooLarge {
            actual: 65,
            maximum: 64,
        })
    );
    assert_eq!(queue.pending_input_loss(), Some(loss));

    let mut detail = Vec::with_capacity(16);
    detail.extend_from_slice(&[0xaa, 0xbb]);
    let detail_pointer = detail.as_ptr();
    let (notice, allocations) =
        count_allocations(|| queue.enqueue_pending_input_overflow_notice(detail));
    assert_eq!(allocations, 0);
    let notice = notice.unwrap();
    assert_eq!(notice.loss, loss);
    assert_eq!(notice.notice_sequence, sequence(2));
    assert_eq!(queue.state(), CueEndpointState::Ready);

    queue
        .enqueue(cue(2, 2, CueDelivery::Critical, &[2]))
        .unwrap();
    let drained = queue
        .drain_endpoint(DrainBudget::new(8, usize::MAX))
        .unwrap();
    assert_eq!(drained.records.len(), 3);
    assert_eq!(drained.records[0].sequence(), sequence(1));
    assert_eq!(drained.records[1].sequence(), sequence(2));
    assert_eq!(drained.records[2].sequence(), sequence(3));

    let EndpointRecord::RuntimeNotice(record) = &drained.records[1] else {
        panic!("expected InputOverflow RuntimeNotice");
    };
    assert_eq!(record.kind(), RUNTIME_NOTICE_INPUT_OVERFLOW);
    assert_eq!(record.input_loss(), loss);
    assert_eq!(record.payload(), [0xaa, 0xbb]);
    assert_eq!(record.payload().as_ptr(), detail_pointer);

    let mut notice_payload = [0u8; INPUT_OVERFLOW_METADATA_BYTES + 2];
    let frame = record.protocol_frame(&mut notice_payload).unwrap();
    let mut encoded = [0u8; 96];
    let encoded_len = encode_frame(MPY_V1, frame, &mut encoded).unwrap();
    assert_eq!(encoded_len, record.frame_bytes());
    let decoded = decode_frame(&encoded[..encoded_len]).unwrap();
    let FrameRef::RuntimeNotice(decoded_notice) = decoded.frame else {
        panic!("expected RuntimeNotice frame");
    };
    assert_eq!(decoded_notice.sequence, 2);
    assert_eq!(decoded_notice.kind, RUNTIME_NOTICE_INPUT_OVERFLOW);
    assert_eq!(decoded_notice.payload, notice_payload);
    let expected_payload = [
        1, 0, 0, 0, // StageId
        7, 0, 0, 0, // input class
        10, 0, 0, 0, 0, 0, 0, 0, // first InputSequence
        11, 0, 0, 0, 0, 0, 0, 0, // last InputSequence
        2, 0, 0, 0, // loss count
        0xaa, 0xbb, // caller-owned detail
    ];
    assert_eq!(decoded_notice.payload, expected_payload);
}

#[test]
fn input_overflow_notice_capacity_exhaustion_faults_without_clearing_loss() {
    let mut queue = CueQueue::new(limits(3, 1, 2)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .enqueue(cue(1, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(2, 3, CueDelivery::Critical, &[3]))
        .unwrap();
    let loss = queue
        .record_input_overflow(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            InputSequence::new(1).unwrap(),
        )
        .unwrap();

    assert_eq!(
        queue.enqueue_pending_input_overflow_notice(vec![9]),
        Err(CueQueueError::CriticalNoticeCapacityExhausted {
            sequence: sequence(4),
            kind: RUNTIME_NOTICE_INPUT_OVERFLOW,
        })
    );
    assert_eq!(queue.state(), CueEndpointState::Faulted);
    assert_eq!(queue.pending_input_loss(), Some(loss));
    assert_eq!(
        queue.emergency_fault(),
        Some(&EmergencyFault::CriticalNoticeCapacityExhausted {
            sequence: sequence(4),
            kind: RUNTIME_NOTICE_INPUT_OVERFLOW,
        })
    );
}

#[test]
fn stage_purge_preserves_critical_release_records_and_global_order() {
    let mut queue = CueQueue::new(limits(6, 2, 3)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    queue
        .enqueue(cue(2, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(1, 3, CueDelivery::Critical, &[3]).with_subscription_release())
        .unwrap();
    queue
        .enqueue(cue(1, 4, CueDelivery::Ordered, &[4]))
        .unwrap();
    queue
        .enqueue(cue(2, 5, CueDelivery::Critical, &[5]))
        .unwrap();

    let removed = queue.remove_stage_ordinary(StageId::new(1).unwrap());
    assert_eq!(removed.count, 2);
    assert_eq!(removed.first_sequence, Some(sequence(1)));
    assert_eq!(removed.last_sequence, Some(sequence(4)));
    assert_eq!(queue.ordinary_len(), 1);
    assert_eq!(queue.critical_len(), 2);

    let drained = queue
        .drain_endpoint(DrainBudget::new(8, usize::MAX))
        .unwrap();
    let sequences: Vec<_> = drained
        .records
        .iter()
        .map(|record| record.sequence().get())
        .collect();
    assert_eq!(sequences, [2, 3, 5]);
    let EndpointRecord::Cue(release) = &drained.records[1] else {
        panic!("expected retained Critical release cue");
    };
    assert!(release.is_subscription_release());
    let mut payload = [0u8; CUE_METADATA_ENVELOPE_BYTES + 1];
    let FrameRef::Cue(protocol_cue) = release.protocol_frame(&mut payload).unwrap() else {
        panic!("expected Cue frame");
    };
    assert_eq!(
        protocol_cue.flags,
        CUE_FLAG_MPY05_METADATA | CUE_FLAG_SUBSCRIPTION_RELEASE
    );

    assert_eq!(
        queue.enqueue(cue(1, 6, CueDelivery::Ordered, &[6]).with_subscription_release()),
        Err(CueQueueError::SubscriptionReleaseMustBeCritical)
    );
}

#[test]
fn compatibility_cue_drain_never_bypasses_a_runtime_notice() {
    let mut queue = CueQueue::new(limits(4, 1, 3)).unwrap();
    queue
        .record_input_overflow(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            InputSequence::new(1).unwrap(),
        )
        .unwrap();
    queue
        .enqueue_pending_input_overflow_notice(Vec::new())
        .unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();

    let cue_only = queue.drain(DrainBudget::new(8, usize::MAX)).unwrap();
    assert!(cue_only.cues.is_empty());
    assert_eq!(queue.len(), 2);

    let global = queue
        .drain_endpoint(DrainBudget::new(8, usize::MAX))
        .unwrap();
    assert!(matches!(
        global.records.as_slice(),
        [EndpointRecord::RuntimeNotice(_), EndpointRecord::Cue(_)]
    ));
}

#[test]
fn finalized_stages_release_bounded_causality_capacity() {
    let mut queue = CueQueue::new(limits(2, 1, 1)).unwrap();

    queue
        .enqueue(cue(1, 1, CueDelivery::Ordered, &[1]))
        .unwrap();
    assert_eq!(
        queue.remove_stage_ordinary(StageId::new(1).unwrap()).count,
        1
    );
    assert_eq!(queue.finalize_stage(StageId::new(1).unwrap()), Ok(true));
    assert_eq!(queue.finalize_stage(StageId::new(1).unwrap()), Ok(false));

    queue
        .enqueue(cue(2, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    assert_eq!(
        queue.remove_stage_ordinary(StageId::new(2).unwrap()).count,
        1
    );
    assert_eq!(queue.finalize_stage(StageId::new(2).unwrap()), Ok(true));

    // This third Stage would exceed the two-entry causality history without
    // explicit finalization of the retired Stages above.
    assert_eq!(
        queue.enqueue(cue(3, 3, CueDelivery::Ordered, &[3])),
        Ok(EnqueueOutcome::Queued {
            sequence: sequence(3),
        })
    );
}

#[test]
fn stage_finalization_waits_for_queued_and_pending_teardown_outputs() {
    let stage = StageId::new(1).unwrap();
    let mut queue = CueQueue::new(limits(4, 1, 2)).unwrap();
    queue
        .enqueue(cue(1, 1, CueDelivery::Critical, &[1]).with_subscription_release())
        .unwrap();

    assert_eq!(
        queue.finalize_stage(stage),
        Err(CueQueueError::StageFinalizeBusy { stage_id: stage })
    );
    assert_eq!(queue.len(), 1);
    let drained = queue
        .drain_endpoint(DrainBudget::new(1, usize::MAX))
        .unwrap();
    assert!(matches!(
        drained.records.as_slice(),
        [EndpointRecord::Cue(record)] if record.is_subscription_release()
    ));
    // The endpoint calls finalize only after the returned release record has
    // been handled at the VM-safe boundary.
    assert_eq!(queue.finalize_stage(stage), Ok(true));

    let input_stage = StageId::new(2).unwrap();
    queue
        .record_input_overflow(
            input_stage,
            InputClass::new(1).unwrap(),
            InputSequence::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(
        queue.finalize_stage(input_stage),
        Err(CueQueueError::StageFinalizeBusy {
            stage_id: input_stage,
        })
    );
    queue
        .enqueue_pending_input_overflow_notice(Vec::new())
        .unwrap();
    assert_eq!(
        queue.finalize_stage(input_stage),
        Err(CueQueueError::StageFinalizeBusy {
            stage_id: input_stage,
        })
    );
    queue
        .drain_endpoint(DrainBudget::new(1, usize::MAX))
        .unwrap();
    assert_eq!(queue.finalize_stage(input_stage), Ok(false));

    let loss_stage = StageId::new(3).unwrap();
    queue
        .enqueue(cue(3, 2, CueDelivery::Ordered, &[2]))
        .unwrap();
    queue
        .enqueue(cue(3, 3, CueDelivery::Ordered, &[3]))
        .unwrap();
    assert!(matches!(
        queue.enqueue(cue(3, 4, CueDelivery::Ordered, &[4])),
        Err(CueQueueError::StageQuotaExhausted { .. })
    ));
    assert_eq!(queue.remove_stage_ordinary(loss_stage).count, 2);
    assert_eq!(
        queue.finalize_stage(loss_stage),
        Err(CueQueueError::StageFinalizeBusy {
            stage_id: loss_stage,
        })
    );
    queue
        .enqueue_pending_loss_notice(cue(3, 5, CueDelivery::Critical, &[4]))
        .unwrap();
    assert_eq!(
        queue.finalize_stage(loss_stage),
        Err(CueQueueError::StageFinalizeBusy {
            stage_id: loss_stage,
        })
    );
    queue
        .drain_endpoint(DrainBudget::new(1, usize::MAX))
        .unwrap();
    assert_eq!(queue.finalize_stage(loss_stage), Ok(true));
}

fn sequence(raw: u32) -> rlvgl_core::cue::CueSequence {
    CueSequence::new(raw).unwrap()
}
