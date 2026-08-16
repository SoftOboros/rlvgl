//! Focused actor-delete Safe Turn endpoint conformance tests.

use rlvgl_api::protocol::ValueRef;
use rlvgl_core::{
    actor::{
        ActorIdentity, ConstructorInput, CreateDestination, RegistryError, RegistryLimits, StageId,
        StageRegistry, TypeDescriptor,
    },
    cue::{
        CUE_FRAME_OVERHEAD_BYTES, CallbackId, CueDelivery, CueLimits, DrainBudget, EndpointRecord,
        EventId,
    },
    direction::{ActorDirection, OwnedValue, StageDirection, StageRevision},
    endpoint::{
        BatchOutcome, BatchRejection, Endpoint, EndpointError, EndpointLimits, EndpointState,
        RequestId,
    },
    object::DispatchPhase,
    subscription::{
        EndpointEpoch, PropagationPolicy, SubscribeRequest, SubscriptionFilter, SubscriptionLimits,
    },
    widget::Rect,
};
use rlvgl_widgets::{button, mpy::CATALOG};

const BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 100,
};

fn registry_limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 4,
        max_actors: 16,
        max_tree_depth: 8,
        max_children_per_actor: 8,
        max_text_bytes: 1024,
        max_resources: 8,
    }
}

fn descriptor(name: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(name))
        .unwrap()
}

fn field(actor: &TypeDescriptor, name: &str) -> u32 {
    actor
        .constructor_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .id
}

fn property(actor: &TypeDescriptor, name: &str) -> u32 {
    actor
        .properties
        .iter()
        .find(|property| property.name == name)
        .unwrap()
        .id
}

fn bounds_input(actor: &TypeDescriptor) -> ConstructorInput<'static> {
    ConstructorInput {
        id: field(actor, "bounds"),
        value: ValueRef::Rect {
            x: BOUNDS.x,
            y: BOUNDS.y,
            width: BOUNDS.width,
            height: BOUNDS.height,
        },
    }
}

fn stage(stage_id: u32) -> StageRegistry {
    StageRegistry::new(StageId::new(stage_id).unwrap(), &CATALOG, registry_limits()).unwrap()
}

fn create_root(stage: &mut StageRegistry) -> rlvgl_core::actor::ObjectId {
    let actor = descriptor("container::Container");
    stage
        .create(
            actor.type_id,
            CreateDestination::Root { name: "main" },
            &[bounds_input(actor)],
        )
        .unwrap()
}

fn create_button(
    stage: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
    text: &'static str,
) -> ActorIdentity {
    let actor = descriptor("button::Button");
    let object_id = stage
        .create(
            actor.type_id,
            CreateDestination::Child { parent },
            &[
                bounds_input(actor),
                ConstructorInput {
                    id: field(actor, "text"),
                    value: ValueRef::Text(text),
                },
            ],
        )
        .unwrap();
    ActorIdentity {
        object_id,
        type_id: actor.type_id,
    }
}

fn endpoint_limits() -> EndpointLimits {
    EndpointLimits::new(4, 8, 8, 16).unwrap()
}

fn cue_limits() -> CueLimits {
    CueLimits::new(16, 4, 8, 64, CUE_FRAME_OVERHEAD_BYTES + 64).unwrap()
}

fn endpoint_with_limits(limits: EndpointLimits) -> Endpoint {
    Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        limits,
        SubscriptionLimits::new(16, 64, 16, 16).unwrap(),
        cue_limits(),
    )
    .unwrap()
}

fn subscribe_request(
    stage_id: StageId,
    actor_identity: ActorIdentity,
    callback: u32,
) -> SubscribeRequest {
    SubscribeRequest {
        stage_id,
        actor_identity,
        event_id: EventId::new(button::MPY_CLICKED_EVENT_ID).unwrap(),
        callback_id: CallbackId::new(callback).unwrap(),
        phase: DispatchPhase::Target,
        filter: SubscriptionFilter::Any,
        propagation: PropagationPolicy::Observe,
    }
}

#[test]
fn accepted_and_rejected_batches_are_fenced_and_complete_exactly_once() {
    let mut owned_stage = stage(1);
    let root = create_root(&mut owned_stage);
    let actor = create_button(&mut owned_stage, root, "Go");
    let starting_revision = owned_stage.revision();
    let committed_revision = StageRevision::new(starting_revision.get() + 1);
    let text_property = property(descriptor("button::Button"), "text");
    let mut endpoint = endpoint_with_limits(endpoint_limits());
    endpoint.register_stage(owned_stage).unwrap();

    endpoint
        .enqueue_batch(
            RequestId::new(1).unwrap(),
            StageId::new(1).unwrap(),
            vec![StageDirection::MutateActor {
                object_id: actor.object_id,
                directions: vec![ActorDirection::SetProperty {
                    id: text_property,
                    value: OwnedValue::Text(String::from("Go")),
                }],
            }],
        )
        .unwrap();
    assert_eq!(endpoint.current_turn(), 0);
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        starting_revision
    );
    let summary = endpoint.run_safe_turn().unwrap();
    assert_eq!((summary.turn, summary.processed_batches), (1, 1));
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        committed_revision
    );
    assert_eq!(
        endpoint
            .stage(StageId::new(1).unwrap())
            .unwrap()
            .property(actor.object_id, text_property)
            .unwrap(),
        OwnedValue::Text(String::from("Go"))
    );
    let completions = endpoint.drain_completions(8).unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].request_id, RequestId::new(1).unwrap());
    assert_eq!(
        completions[0].outcome,
        BatchOutcome::Committed {
            revision: committed_revision,
            deleted_objects: 0,
            released_subscriptions: 0,
        }
    );

    endpoint
        .enqueue_batch(
            RequestId::new(2).unwrap(),
            StageId::new(1).unwrap(),
            vec![StageDirection::MutateActor {
                object_id: actor.object_id,
                directions: vec![
                    ActorDirection::SetProperty {
                        id: text_property,
                        value: OwnedValue::Text(String::from("must-not-commit")),
                    },
                    ActorDirection::SetProperty {
                        id: u32::MAX,
                        value: OwnedValue::Bool(true),
                    },
                ],
            }],
        )
        .unwrap();
    assert_eq!(
        endpoint.enqueue_batch(
            RequestId::new(2).unwrap(),
            StageId::new(1).unwrap(),
            Vec::new(),
        ),
        Err(EndpointError::RequestIdNotMonotonic)
    );
    endpoint.run_safe_turn().unwrap();
    let completions = endpoint.drain_completions(8).unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(
        completions[0].outcome,
        BatchOutcome::Rejected {
            observed_revision: committed_revision,
            operation_index: None,
            error: BatchRejection::Registry(RegistryError::UnknownProperty {
                property_id: u32::MAX,
            }),
        }
    );
    let stage = endpoint.stage(StageId::new(1).unwrap()).unwrap();
    assert_eq!(stage.revision(), committed_revision);
    assert_eq!(
        stage.property(actor.object_id, text_property).unwrap(),
        OwnedValue::Text(String::from("Go"))
    );

    for request in [3, 4] {
        endpoint
            .enqueue_batch(
                RequestId::new(request).unwrap(),
                StageId::new(1).unwrap(),
                Vec::new(),
            )
            .unwrap();
    }
    let summary = endpoint.run_safe_turn().unwrap();
    assert_eq!((summary.turn, summary.processed_batches), (3, 2));
    let completions = endpoint.drain_completions(8).unwrap();
    assert_eq!(
        completions
            .iter()
            .map(|completion| completion.request_id.get())
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        committed_revision
    );
}

#[test]
fn deletion_releases_callbacks_in_child_first_registration_order_and_blocks_turns_until_ack() {
    let mut owned_stage = stage(1);
    let root = create_root(&mut owned_stage);
    let first_actor = create_button(&mut owned_stage, root, "First");
    let second_actor = create_button(&mut owned_stage, root, "Second");
    let stage_id = owned_stage.stage_id();
    let committed_revision = StageRevision::new(owned_stage.revision().get() + 1);
    let mut endpoint = endpoint_with_limits(endpoint_limits());
    endpoint.register_stage(owned_stage).unwrap();
    for (actor, callback) in [(first_actor, 10), (first_actor, 11), (second_actor, 12)] {
        endpoint
            .subscribe(subscribe_request(stage_id, actor, callback))
            .unwrap();
    }

    endpoint
        .enqueue_batch(
            RequestId::new(1).unwrap(),
            stage_id,
            vec![StageDirection::Delete { object_id: root }],
        )
        .unwrap();
    endpoint.run_safe_turn().unwrap();
    assert_eq!(endpoint.stage(stage_id).unwrap().usage().actors, 0);
    assert_eq!(
        endpoint.stage(stage_id).unwrap().revision(),
        committed_revision
    );

    let drain = endpoint
        .drain_records(DrainBudget::for_limits(cue_limits(), 8))
        .unwrap();
    assert_eq!(endpoint.state(), EndpointState::DrainOutstanding);
    let cues: Vec<_> = drain
        .records()
        .iter()
        .map(|record| match record {
            EndpointRecord::Cue(cue) => cue,
            EndpointRecord::RuntimeNotice(_) => panic!("unexpected RuntimeNotice"),
        })
        .collect();
    assert_eq!(cues.len(), 3);
    assert_eq!(
        cues.iter()
            .map(|cue| cue.callback_id().get())
            .collect::<Vec<_>>(),
        [10, 11, 12]
    );
    for cue in &cues {
        assert_eq!(cue.delivery(), CueDelivery::Critical);
        assert!(cue.is_subscription_release());
        assert!(cue.payload().is_empty());
        assert_eq!(cue.stage_revision(), committed_revision);
        assert_eq!(
            cue.first_native_event_sequence(),
            cues[0].first_native_event_sequence()
        );
    }

    endpoint
        .enqueue_batch(RequestId::new(2).unwrap(), stage_id, Vec::new())
        .unwrap();
    assert_eq!(
        endpoint.run_safe_turn(),
        Err(EndpointError::DrainOutstanding)
    );
    assert_eq!(endpoint.current_turn(), 1);
    endpoint.acknowledge_records(drain).unwrap();
    assert_eq!(endpoint.state(), EndpointState::Ready);
    let summary = endpoint.run_safe_turn().unwrap();
    assert_eq!((summary.turn, summary.processed_batches), (2, 1));

    let completions = endpoint.drain_completions(8).unwrap();
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].request_id, RequestId::new(1).unwrap());
    assert_eq!(
        completions[0].outcome,
        BatchOutcome::Committed {
            revision: committed_revision,
            deleted_objects: 3,
            released_subscriptions: 3,
        }
    );
    assert_eq!(completions[1].request_id, RequestId::new(2).unwrap());
    assert_eq!(
        completions[1].outcome,
        BatchOutcome::Rejected {
            observed_revision: committed_revision,
            operation_index: None,
            error: BatchRejection::Registry(RegistryError::BatchInvalid),
        }
    );
}

#[test]
fn stage_and_request_identities_are_monotonic_and_completion_credited() {
    let limits = EndpointLimits::new(2, 1, 1, 1).unwrap();
    let mut endpoint = endpoint_with_limits(limits);
    endpoint.register_stage(stage(2)).unwrap();
    assert_eq!(
        endpoint.register_stage(stage(1)),
        Err(EndpointError::StageIdNotMonotonic)
    );
    assert_eq!(
        endpoint.register_stage(stage(2)),
        Err(EndpointError::StageIdNotMonotonic)
    );
    endpoint.register_stage(stage(3)).unwrap();
    assert_eq!(
        endpoint.register_stage(stage(4)),
        Err(EndpointError::StageCapacity)
    );
    assert!(endpoint.stage(StageId::new(2).unwrap()).is_some());
    assert!(endpoint.stage(StageId::new(3).unwrap()).is_some());

    endpoint
        .enqueue_batch(
            RequestId::new(2).unwrap(),
            StageId::new(2).unwrap(),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        endpoint.enqueue_batch(
            RequestId::new(3).unwrap(),
            StageId::new(2).unwrap(),
            Vec::new(),
        ),
        Err(EndpointError::PendingCapacity)
    );
    endpoint.run_safe_turn().unwrap();
    assert_eq!(
        endpoint.enqueue_batch(
            RequestId::new(3).unwrap(),
            StageId::new(2).unwrap(),
            Vec::new(),
        ),
        Err(EndpointError::CompletionCapacity)
    );
    endpoint.drain_completions(1).unwrap();
    endpoint
        .enqueue_batch(
            RequestId::new(3).unwrap(),
            StageId::new(2).unwrap(),
            Vec::new(),
        )
        .unwrap();
}

#[test]
fn oversized_batch_is_rejected_before_request_acceptance() {
    assert_eq!(
        EndpointLimits::new(1, 1, 1, u16::MAX as usize + 1),
        Err(EndpointError::InvalidLimits)
    );
    let mut owned_stage = stage(1);
    let root = create_root(&mut owned_stage);
    let stage_id = owned_stage.stage_id();
    let mut endpoint = endpoint_with_limits(EndpointLimits::new(1, 1, 1, 1).unwrap());
    endpoint.register_stage(owned_stage).unwrap();

    assert_eq!(
        endpoint.enqueue_batch(
            RequestId::new(1).unwrap(),
            stage_id,
            vec![
                StageDirection::Delete { object_id: root },
                StageDirection::Delete { object_id: root },
            ],
        ),
        Err(EndpointError::DirectionCapacity)
    );
    endpoint
        .enqueue_batch(RequestId::new(1).unwrap(), stage_id, Vec::new())
        .unwrap();
}

#[test]
fn mismatched_acknowledgment_preserves_the_opaque_drain() {
    let mut owner = endpoint_with_limits(endpoint_limits());
    let mut other = Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        endpoint_limits(),
        SubscriptionLimits::new(16, 64, 16, 16).unwrap(),
        cue_limits(),
    )
    .unwrap();
    let drain = owner
        .drain_records(DrainBudget::for_limits(cue_limits(), 1))
        .unwrap();
    let other_drain = other
        .drain_records(DrainBudget::for_limits(cue_limits(), 1))
        .unwrap();

    let error = other.acknowledge_records(drain).unwrap_err();
    assert_eq!(error.error(), EndpointError::DrainMismatch);
    assert!(error.drain().is_empty());
    let drain = error.into_drain();
    assert_eq!(owner.run_safe_turn(), Err(EndpointError::DrainOutstanding));
    assert_eq!(other.run_safe_turn(), Err(EndpointError::DrainOutstanding));
    owner.acknowledge_records(drain).unwrap();
    other.acknowledge_records(other_drain).unwrap();
    assert_eq!(owner.state(), EndpointState::Ready);
    assert_eq!(other.state(), EndpointState::Ready);
}

#[test]
fn dropping_a_drain_cannot_transfer_ownership_to_a_replacement_endpoint() {
    let mut owner = endpoint_with_limits(endpoint_limits());
    let lost_drain = owner
        .drain_records(DrainBudget::for_limits(cue_limits(), 1))
        .unwrap();
    drop(lost_drain);

    let mut replacement = endpoint_with_limits(endpoint_limits());
    let replacement_drain = replacement
        .drain_records(DrainBudget::for_limits(cue_limits(), 1))
        .unwrap();
    let error = owner.acknowledge_records(replacement_drain).unwrap_err();
    assert_eq!(error.error(), EndpointError::DrainMismatch);
    assert_eq!(owner.run_safe_turn(), Err(EndpointError::DrainOutstanding));

    replacement.acknowledge_records(error.into_drain()).unwrap();
    assert_eq!(replacement.state(), EndpointState::Ready);
}

#[test]
fn exact_release_capacity_rejects_delete_before_stage_mutation() {
    let mut owned_stage = stage(1);
    let root = create_root(&mut owned_stage);
    let actor = create_button(&mut owned_stage, root, "Button");
    let stage_id = owned_stage.stage_id();
    let starting_revision = owned_stage.revision();
    let mut endpoint = Endpoint::new(
        EndpointEpoch::new(2).unwrap(),
        endpoint_limits(),
        SubscriptionLimits::new(4, 64, 4, 4).unwrap(),
        CueLimits::new(2, 1, 1, 64, CUE_FRAME_OVERHEAD_BYTES + 64).unwrap(),
    )
    .unwrap();
    endpoint.register_stage(owned_stage).unwrap();
    for callback in [1, 2, 3] {
        endpoint
            .subscribe(subscribe_request(stage_id, actor, callback))
            .unwrap();
    }
    endpoint
        .enqueue_batch(
            RequestId::new(1).unwrap(),
            stage_id,
            vec![StageDirection::Delete { object_id: root }],
        )
        .unwrap();
    endpoint.run_safe_turn().unwrap();

    let stage = endpoint.stage(stage_id).unwrap();
    assert_eq!(stage.revision(), starting_revision);
    assert_eq!(stage.usage().actors, 2);
    assert_eq!(stage.root_id("main"), Some(root));
    let completions = endpoint.drain_completions(1).unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(
        completions[0].outcome,
        BatchOutcome::Rejected {
            observed_revision: starting_revision,
            operation_index: None,
            error: BatchRejection::Cue(rlvgl_core::cue::CueQueueError::AdmissionCapacity),
        }
    );
}
