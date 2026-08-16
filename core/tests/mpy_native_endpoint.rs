//! End-to-end MPY-05 native-input Endpoint conformance tests.

use rlvgl_api::protocol::ValueRef;
use rlvgl_core::{
    actor::{
        ActorIdentity, ConstructorInput, CreateDestination, RegistryLimits, StageId, StageRegistry,
        TypeDescriptor,
    },
    cue::{
        CUE_FRAME_OVERHEAD_BYTES, CallbackId, CueLimits, CueQueueError, DrainBudget,
        EndpointRecord, EventId, InputClass,
    },
    direction::{OwnedValue, RuntimeFlag, StageDirection},
    endpoint::{
        Endpoint, EndpointError, EndpointFault, EndpointLimits, EndpointNativeInput, EndpointState,
        NativeInputOutcome, RequestId,
    },
    event::Event,
    object::{DispatchPhase, Disposition},
    subscription::{
        EndpointEpoch, PropagationPolicy, SubscribeRequest, SubscriptionFilter, SubscriptionLimits,
    },
    widget::Rect,
};
use rlvgl_widgets::{mpy::CATALOG, slider};

const BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 20,
};

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

fn registry_limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 2,
        max_actors: 8,
        max_tree_depth: 4,
        max_children_per_actor: 4,
        max_text_bytes: 256,
        max_resources: 4,
    }
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

fn stage_with_slider_at(
    stage_id: u32,
) -> (StageRegistry, rlvgl_core::actor::ObjectId, ActorIdentity) {
    let mut stage =
        StageRegistry::new(StageId::new(stage_id).unwrap(), &CATALOG, registry_limits()).unwrap();
    let container = descriptor("container::Container");
    let root_id = stage
        .create(
            container.type_id,
            CreateDestination::Root { name: "main" },
            &[bounds_input(container)],
        )
        .unwrap();
    let actor = descriptor("slider::Slider");
    let object_id = stage
        .create(
            actor.type_id,
            CreateDestination::Child { parent: root_id },
            &[
                bounds_input(actor),
                ConstructorInput {
                    id: field(actor, "min"),
                    value: ValueRef::I32(0),
                },
                ConstructorInput {
                    id: field(actor, "max"),
                    value: ValueRef::I32(100),
                },
            ],
        )
        .unwrap();
    stage
        .apply_batch(&[StageDirection::SetFlag {
            object_id,
            flag: RuntimeFlag::Clickable,
            enabled: true,
        }])
        .unwrap();
    (
        stage,
        root_id,
        ActorIdentity {
            object_id,
            type_id: actor.type_id,
        },
    )
}

fn stage_with_slider() -> (StageRegistry, rlvgl_core::actor::ObjectId, ActorIdentity) {
    stage_with_slider_at(1)
}

fn endpoint(cue_limits: CueLimits) -> Endpoint {
    Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        EndpointLimits::new(2, 4, 4, 8).unwrap(),
        SubscriptionLimits::new(8, 32, 8, 16).unwrap(),
        cue_limits,
    )
    .unwrap()
}

fn standard_cue_limits() -> CueLimits {
    CueLimits::new(8, 2, 4, 32, CUE_FRAME_OVERHEAD_BYTES + 32).unwrap()
}

fn subscribe_slider(endpoint: &mut Endpoint, actor: ActorIdentity, callback_id: u32) {
    endpoint
        .subscribe(SubscribeRequest {
            stage_id: StageId::new(1).unwrap(),
            actor_identity: actor,
            event_id: EventId::new(slider::MPY_VALUE_CHANGED_EVENT_ID).unwrap(),
            callback_id: CallbackId::new(callback_id).unwrap(),
            phase: DispatchPhase::Target,
            filter: SubscriptionFilter::Any,
            propagation: PropagationPolicy::Observe,
        })
        .unwrap();
}

fn pointer(root_id: rlvgl_core::actor::ObjectId, x: i32) -> EndpointNativeInput {
    EndpointNativeInput::Pointer {
        root_id,
        x,
        y: 10,
        event: Event::PressRelease { x, y: 10 },
    }
}

fn slider_value(endpoint: &Endpoint, actor: ActorIdentity) -> i32 {
    match endpoint
        .stage(StageId::new(1).unwrap())
        .unwrap()
        .property(
            actor.object_id,
            property(descriptor("slider::Slider"), "value"),
        )
        .unwrap()
    {
        OwnedValue::I32(value) => value,
        other => panic!("unexpected slider value: {other:?}"),
    }
}

#[test]
fn native_pointer_dispatch_publishes_one_revision_and_one_cue() {
    let (stage, root_id, actor) = stage_with_slider();
    let starting_revision = stage.revision();
    let mut endpoint = endpoint(standard_cue_limits());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);

    let first = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched {
        input_sequence,
        native_event_sequence,
        stage_revision,
        disposition,
        cue_count,
    } = first
    else {
        panic!("first input was not dispatched")
    };
    assert_eq!(input_sequence.get(), 1);
    assert_eq!(native_event_sequence.get(), 1);
    assert_eq!(stage_revision.get(), starting_revision.get() + 1);
    assert_eq!(disposition, Disposition::Consumed);
    assert_eq!(cue_count, 1);
    assert_eq!(slider_value(&endpoint, actor), 75);

    let unchanged = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap();
    assert_eq!(
        unchanged,
        NativeInputOutcome::Dispatched {
            input_sequence: rlvgl_core::cue::InputSequence::new(2).unwrap(),
            native_event_sequence: rlvgl_core::cue::NativeEventSequence::new(2).unwrap(),
            stage_revision,
            disposition: Disposition::Consumed,
            cue_count: 0,
        }
    );

    let drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();
    assert_eq!(drain.records().len(), 1);
    let EndpointRecord::Cue(cue) = &drain.records()[0] else {
        panic!("expected semantic cue")
    };
    assert_eq!(cue.object_id(), actor.object_id);
    assert_eq!(cue.callback_id().get(), 1);
    assert_eq!(cue.stage_revision(), stage_revision);
    assert_eq!(cue.first_native_event_sequence().get(), 1);
    endpoint.acknowledge_records(drain).unwrap();
}

#[test]
fn saturation_rejects_before_mutation_and_preserves_native_sequence() {
    let (stage, root_id, actor) = stage_with_slider();
    let mut endpoint =
        endpoint(CueLimits::new(2, 1, 1, 32, CUE_FRAME_OVERHEAD_BYTES + 32).unwrap());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);

    let first = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched { stage_revision, .. } = first else {
        panic!("first input was not dispatched")
    };
    let rejected = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 25),
        )
        .unwrap();
    let NativeInputOutcome::RejectedBeforeDispatch {
        input_sequence,
        notice_sequence,
    } = rejected
    else {
        panic!("saturated input was not rejected")
    };
    assert_eq!(input_sequence.get(), 2);
    assert_eq!(notice_sequence.get(), 2);
    assert_eq!(slider_value(&endpoint, actor), 75);
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        stage_revision
    );

    let drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();
    assert_eq!(drain.records().len(), 2);
    let EndpointRecord::RuntimeNotice(notice) = &drain.records()[1] else {
        panic!("expected typed input overflow notice")
    };
    assert_eq!(notice.sequence(), notice_sequence);
    assert_eq!(
        notice.input_loss().input_class(),
        InputClass::new(1).unwrap()
    );
    assert_eq!(notice.input_loss().first_sequence(), input_sequence);
    endpoint.acknowledge_records(drain).unwrap();

    let retry = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 25),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched {
        input_sequence,
        native_event_sequence,
        cue_count,
        ..
    } = retry
    else {
        panic!("retry was not dispatched")
    };
    assert_eq!(input_sequence.get(), 3);
    assert_eq!(native_event_sequence.get(), 2);
    assert_eq!(cue_count, 1);
    assert_eq!(slider_value(&endpoint, actor), 25);
}

#[test]
fn no_target_consumes_only_the_raw_input_sequence() {
    let (stage, root_id, actor) = stage_with_slider();
    let mut endpoint = endpoint(standard_cue_limits());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);

    assert_eq!(
        endpoint
            .dispatch_native_event(
                StageId::new(1).unwrap(),
                InputClass::new(1).unwrap(),
                pointer(root_id, 200),
            )
            .unwrap(),
        NativeInputOutcome::NoTarget {
            input_sequence: rlvgl_core::cue::InputSequence::new(1).unwrap(),
        }
    );
    let dispatched = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 50),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched {
        input_sequence,
        native_event_sequence,
        ..
    } = dispatched
    else {
        panic!("valid target was not dispatched")
    };
    assert_eq!(input_sequence.get(), 2);
    assert_eq!(native_event_sequence.get(), 1);
}

#[test]
fn native_input_continues_while_a_vm_drain_is_outstanding() {
    let (stage, root_id, actor) = stage_with_slider();
    let mut endpoint = endpoint(standard_cue_limits());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);
    endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap();
    let first_drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();

    let second = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 25),
        )
        .unwrap();
    assert!(matches!(
        second,
        NativeInputOutcome::Dispatched { cue_count: 1, .. }
    ));
    endpoint.acknowledge_records(first_drain).unwrap();
    let second_drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();
    assert_eq!(second_drain.records().len(), 1);
    endpoint.acknowledge_records(second_drain).unwrap();
}

#[test]
fn subscription_workspace_saturation_reports_input_loss_before_mutation() {
    let (stage, root_id, actor) = stage_with_slider();
    let starting_revision = stage.revision();
    let mut endpoint = Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        EndpointLimits::new(2, 4, 4, 8).unwrap(),
        SubscriptionLimits::new(8, 32, 8, 1).unwrap(),
        standard_cue_limits(),
    )
    .unwrap();
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);
    subscribe_slider(&mut endpoint, actor, 2);

    assert_eq!(
        endpoint
            .dispatch_native_event(
                StageId::new(1).unwrap(),
                InputClass::new(7).unwrap(),
                pointer(root_id, 75),
            )
            .unwrap(),
        NativeInputOutcome::RejectedBeforeDispatch {
            input_sequence: rlvgl_core::cue::InputSequence::new(1).unwrap(),
            notice_sequence: rlvgl_core::cue::CueSequence::new(1).unwrap(),
        }
    );
    assert_eq!(endpoint.state(), EndpointState::Ready);
    assert_eq!(slider_value(&endpoint, actor), 0);
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        starting_revision
    );

    let drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();
    let [EndpointRecord::RuntimeNotice(notice)] = drain.records() else {
        panic!("expected one typed input-overflow notice")
    };
    assert_eq!(notice.input_loss().lost_count(), 1);
    assert_eq!(
        notice.input_loss().input_class(),
        InputClass::new(7).unwrap()
    );
    endpoint.acknowledge_records(drain).unwrap();
}

#[test]
fn route_busy_reports_input_loss_and_preserves_the_native_sequence() {
    let (stage, root_id, actor) = stage_with_slider();
    let starting_revision = stage.revision();
    let widget = stage.node(actor.object_id).unwrap().widget().clone();
    let retained_borrow = widget.borrow_mut();
    let mut endpoint = endpoint(standard_cue_limits());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);

    assert_eq!(
        endpoint
            .dispatch_native_event(
                StageId::new(1).unwrap(),
                InputClass::new(3).unwrap(),
                pointer(root_id, 75),
            )
            .unwrap(),
        NativeInputOutcome::RejectedBeforeDispatch {
            input_sequence: rlvgl_core::cue::InputSequence::new(1).unwrap(),
            notice_sequence: rlvgl_core::cue::CueSequence::new(1).unwrap(),
        }
    );
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        starting_revision
    );
    drop(retained_borrow);

    let drain = endpoint
        .drain_records(DrainBudget::new(8, usize::MAX))
        .unwrap();
    endpoint.acknowledge_records(drain).unwrap();
    let retry = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(3).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched {
        input_sequence,
        native_event_sequence,
        ..
    } = retry
    else {
        panic!("retry was not dispatched")
    };
    assert_eq!(input_sequence.get(), 2);
    assert_eq!(native_event_sequence.get(), 1);
}

#[test]
fn permanent_cue_contract_failure_faults_the_endpoint_before_mutation() {
    let (stage, root_id, actor) = stage_with_slider();
    let starting_revision = stage.revision();
    let mut endpoint = endpoint(CueLimits::new(8, 2, 4, 1, CUE_FRAME_OVERHEAD_BYTES + 1).unwrap());
    endpoint.register_stage(stage).unwrap();
    subscribe_slider(&mut endpoint, actor, 1);

    let error = endpoint
        .dispatch_native_event(
            StageId::new(1).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_id, 75),
        )
        .unwrap_err();
    assert_eq!(
        error,
        EndpointError::Faulted(EndpointFault::PreDispatchCue(
            CueQueueError::PayloadTooLarge {
                actual: 5,
                maximum: 1,
            }
        ))
    );
    assert_eq!(endpoint.state(), EndpointState::Faulted);
    assert_eq!(
        endpoint.fault(),
        Some(EndpointFault::PreDispatchCue(
            CueQueueError::PayloadTooLarge {
                actual: 5,
                maximum: 1,
            }
        ))
    );
    assert_eq!(slider_value(&endpoint, actor), 0);
    assert_eq!(
        endpoint.stage(StageId::new(1).unwrap()).unwrap().revision(),
        starting_revision
    );
}

#[test]
fn accepted_stage_teardown_fences_input_before_sequence_allocation() {
    let (stage_one, root_one, _) = stage_with_slider_at(1);
    let (stage_two, root_two, _) = stage_with_slider_at(2);
    let mut endpoint = endpoint(standard_cue_limits());
    endpoint.register_stage(stage_one).unwrap();
    endpoint.register_stage(stage_two).unwrap();
    endpoint
        .enqueue_stage_teardown(RequestId::new(1).unwrap(), StageId::new(1).unwrap())
        .unwrap();

    assert_eq!(
        endpoint
            .dispatch_native_event(
                StageId::new(1).unwrap(),
                InputClass::new(1).unwrap(),
                pointer(root_one, 75),
            )
            .unwrap_err(),
        EndpointError::StageTeardownPending
    );
    let outcome = endpoint
        .dispatch_native_event(
            StageId::new(2).unwrap(),
            InputClass::new(1).unwrap(),
            pointer(root_two, 75),
        )
        .unwrap();
    let NativeInputOutcome::Dispatched { input_sequence, .. } = outcome else {
        panic!("second Stage input was not dispatched")
    };
    assert_eq!(input_sequence.get(), 1);
}
