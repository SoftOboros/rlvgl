//! Focused conformance tests for the MPY-05 endpoint cue queue substrate.

use rlvgl_api::protocol::{FrameRef, MPY_V1, decode_frame, encode_frame};
use rlvgl_core::{
    actor::{ObjectId, StageId},
    cue::{
        CUE_FLAG_LATEST_VALUE_MERGED, CUE_FLAG_MPY05_METADATA, CUE_FRAME_OVERHEAD_BYTES,
        CUE_METADATA_ENVELOPE_BYTES, CallbackId, CoalescingKey, CueDelivery, CueEndpointState,
        CueIdentity, CueInput, CueLimits, CueLimitsError, CuePayloadRef, CueQueue, CueQueueError,
        CueSequence, DrainBudget, EmergencyFault, EnqueueOutcome, EventId, NativeEventSequence,
        SubscriptionId,
    },
    direction::StageRevision,
};

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

fn sequence(raw: u32) -> rlvgl_core::cue::CueSequence {
    CueSequence::new(raw).unwrap()
}
