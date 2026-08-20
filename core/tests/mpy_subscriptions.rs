//! MPY-05 bounded subscription and descriptor-source conformance tests.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use rlvgl_api::protocol::{ValueRef, ValueTag, decode_value};
use rlvgl_core::{
    actor::{
        ActorIdentity, ActorPreparation, ConstructedActor, ConstructorArgs, CreateDestination,
        EventDelivery, EventDescriptor, EventFilterSet, EventPhaseSet, MpyActor, MutationEffects,
        NativeEventKind, ObjectId, RegistryError, RegistryLimits, StageId, StageRegistry,
        TypeDescriptor, TypeId, construct_native_actor, encode_event_values,
    },
    cue::{CallbackId, CueDelivery, EventId, NativeEventSequence},
    event::Event,
    object::{
        DispatchInput, DispatchPhase, EventContext, NativeEventObservation, NativeEventObserver,
        NativeObserverControl, ObjectEvent,
    },
    renderer::Renderer,
    subscription::{
        EndpointEpoch, NativeEventCompletion, NativeEventReservation, PropagationPolicy,
        SubscribeRequest, SubscriptionError, SubscriptionFilter, SubscriptionLimits,
        SubscriptionRegistry, UnsubscribeOutcome,
    },
    widget::{Rect, Widget},
};
use rlvgl_core::{
    direction::ActorDirection, direction::OwnedValue, direction::RuntimeFlag,
    direction::StageDirection,
};
use rlvgl_widgets::{button, container, label, list, mpy::CATALOG, slider};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

thread_local! {
    static TRACK_ALLOCATOR_OPERATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every operation delegates unchanged layouts and pointers to the
// process System allocator; thread-local bookkeeping only observes calls.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATOR_OPERATIONS
            .try_with(Cell::get)
            .unwrap_or(false)
        {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` is forwarded unchanged to the System allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACK_ALLOCATOR_OPERATIONS
            .try_with(Cell::get)
            .unwrap_or(false)
        {
            let _ = DEALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: both values came from the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATOR_OPERATIONS
            .try_with(Cell::get)
            .unwrap_or(false)
        {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` is forwarded unchanged to the System allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_ALLOCATOR_OPERATIONS
            .try_with(Cell::get)
            .unwrap_or(false)
        {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the allocation and layout belong to System; `size` is the
        // requested replacement size under GlobalAlloc's contract.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn count_allocator_operations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct TrackingGuard;

    impl Drop for TrackingGuard {
        fn drop(&mut self) {
            TRACK_ALLOCATOR_OPERATIONS.with(|tracking| tracking.set(false));
        }
    }

    ALLOCATION_COUNT.with(|count| count.set(0));
    DEALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATOR_OPERATIONS.with(|tracking| tracking.set(true));
    let guard = TrackingGuard;
    let result = operation();
    drop(guard);
    let allocations = ALLOCATION_COUNT.with(Cell::get);
    let deallocations = DEALLOCATION_COUNT.with(Cell::get);
    (result, allocations, deallocations)
}

const BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 100,
};

fn registry_limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 2,
        max_actors: 16,
        max_tree_depth: 8,
        max_children_per_actor: 8,
        max_text_bytes: 1024,
        max_resources: 8,
    }
}

fn subscription_limits(max: usize, observation: usize) -> SubscriptionLimits {
    SubscriptionLimits::new(max, 32, max, observation).unwrap()
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

fn rect_input(id: u32) -> rlvgl_core::actor::ConstructorInput<'static> {
    rlvgl_core::actor::ConstructorInput {
        id,
        value: ValueRef::Rect {
            x: BOUNDS.x,
            y: BOUNDS.y,
            width: BOUNDS.width,
            height: BOUNDS.height,
        },
    }
}

fn stage() -> StageRegistry {
    StageRegistry::new(StageId::new(1).unwrap(), &CATALOG, registry_limits()).unwrap()
}

fn create_root(stage: &mut StageRegistry) -> rlvgl_core::actor::ObjectId {
    let actor = descriptor("container::Container");
    stage
        .create(
            actor.type_id,
            CreateDestination::Root { name: "main" },
            &[rect_input(field(actor, "bounds"))],
        )
        .unwrap()
}

fn create_button(stage: &mut StageRegistry) -> ActorIdentity {
    let root = create_root(stage);
    let actor = descriptor("button::Button");
    let object_id = stage
        .create(
            actor.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(actor, "bounds")),
                rlvgl_core::actor::ConstructorInput {
                    id: field(actor, "text"),
                    value: ValueRef::Text("Go"),
                },
            ],
        )
        .unwrap();
    ActorIdentity {
        object_id,
        type_id: actor.type_id,
    }
}

fn create_slider(stage: &mut StageRegistry) -> ActorIdentity {
    let root = create_root(stage);
    let actor = descriptor("slider::Slider");
    let object_id = stage
        .create(
            actor.type_id,
            CreateDestination::Child { parent: root },
            &[
                rect_input(field(actor, "bounds")),
                rlvgl_core::actor::ConstructorInput {
                    id: field(actor, "min"),
                    value: ValueRef::I32(0),
                },
                rlvgl_core::actor::ConstructorInput {
                    id: field(actor, "max"),
                    value: ValueRef::I32(100),
                },
            ],
        )
        .unwrap();
    ActorIdentity {
        object_id,
        type_id: actor.type_id,
    }
}

fn request(
    stage_id: StageId,
    actor_identity: ActorIdentity,
    event_id: u32,
    callback_id: u32,
    propagation: PropagationPolicy,
) -> SubscribeRequest {
    SubscribeRequest {
        stage_id,
        actor_identity,
        event_id: EventId::new(event_id).unwrap(),
        callback_id: CallbackId::new(callback_id).unwrap(),
        phase: DispatchPhase::Target,
        filter: SubscriptionFilter::Any,
        propagation,
    }
}

fn reserve(
    subscriptions: &SubscriptionRegistry,
    stage: &mut StageRegistry,
    actor_identity: ActorIdentity,
    event: &ObjectEvent,
    sequence: u64,
) -> rlvgl_core::subscription::ObservationWorkspace {
    subscriptions
        .reserve_observation(
            stage,
            NativeEventReservation {
                actor_identity,
                phase: DispatchPhase::Target,
                event,
                native_event_sequence: NativeEventSequence::new(sequence).unwrap(),
            },
        )
        .unwrap()
}

fn completion<'a>(
    actor_identity: ActorIdentity,
    event: &'a ObjectEvent,
    widget_invoked: bool,
    native_consumed: bool,
) -> NativeEventCompletion<'a> {
    NativeEventCompletion {
        actor_identity,
        phase: DispatchPhase::Target,
        event,
        widget_invoked,
        native_consumed,
    }
}

#[test]
fn proof_descriptor_matrix_and_payload_adapters_are_canonical() {
    let button_event = &button::MPY_DESCRIPTOR.events[0];
    assert_eq!(button::MPY_DESCRIPTOR.schema_revision, 4);
    assert_eq!(button_event.id, button::MPY_CLICKED_EVENT_ID);
    assert_eq!(button_event.name, "clicked");
    assert_eq!(button_event.payload, &[ValueTag::I32, ValueTag::I32]);
    assert_eq!(button_event.max_payload_bytes, 10);
    assert_eq!(button_event.delivery, EventDelivery::Ordered);
    assert_eq!(button_event.native_effects, MutationEffects::NONE);
    assert!(button_event.requires_widget_invocation);

    let slider_event = &slider::MPY_DESCRIPTOR.events[0];
    assert_eq!(slider::MPY_DESCRIPTOR.schema_revision, 4);
    assert_eq!(slider_event.id, slider::MPY_VALUE_CHANGED_EVENT_ID);
    assert_eq!(slider_event.payload, &[ValueTag::I32]);
    assert_eq!(slider_event.delivery, EventDelivery::Ordered);
    assert_eq!(slider_event.coalescing_key, None);
    assert_eq!(
        slider_event.native_effects,
        MutationEffects::DRAW.union(MutationEffects::SNAPSHOT)
    );

    let list_event = &list::MPY_DESCRIPTOR.events[0];
    assert_eq!(list::MPY_DESCRIPTOR.schema_revision, 4);
    assert_eq!(list_event.id, list::MPY_SELECTION_CHANGED_EVENT_ID);
    assert_eq!(list_event.payload, &[ValueTag::U32]);
    assert_eq!(list_event.delivery, EventDelivery::Ordered);
    assert_eq!(container::MPY_DESCRIPTOR.events, &[]);
    assert_eq!(label::MPY_DESCRIPTOR.events, &[]);
    StageRegistry::new(StageId::new(9).unwrap(), &CATALOG, registry_limits()).unwrap();

    let mut slider = slider::Slider::new(BOUNDS, 0, 100);
    assert!(slider.handle_event(&Event::PressRelease { x: 0, y: 10 }));
    assert_eq!(
        slider
            .event_payload(
                slider::MPY_VALUE_CHANGED_EVENT_ID,
                &ObjectEvent::Clicked { x: 0, y: 10 },
                &mut [0; 5],
            )
            .unwrap(),
        None
    );
    assert!(slider.handle_event(&Event::PressRelease { x: 50, y: 10 }));
    let mut bytes = [0; 5];
    assert_eq!(
        slider
            .event_payload(
                slider::MPY_VALUE_CHANGED_EVENT_ID,
                &ObjectEvent::Clicked { x: 50, y: 10 },
                &mut bytes,
            )
            .unwrap(),
        Some(5)
    );
    assert_eq!(decode_value(&bytes).unwrap().0, ValueRef::I32(50));
    assert!(slider.handle_event(&Event::PressRelease { x: 50, y: 10 }));
    assert_eq!(
        slider
            .event_payload(
                slider::MPY_VALUE_CHANGED_EVENT_ID,
                &ObjectEvent::Clicked { x: 50, y: 10 },
                &mut bytes,
            )
            .unwrap(),
        None
    );

    let mut list = list::List::new(BOUNDS);
    list.add_item("A");
    assert!(list.handle_event(&Event::PressRelease { x: 4, y: 4 }));
    assert_eq!(
        list.event_payload(
            list::MPY_SELECTION_CHANGED_EVENT_ID,
            &ObjectEvent::Clicked { x: 4, y: 4 },
            &mut bytes,
        )
        .unwrap(),
        Some(5)
    );
    assert!(list.handle_event(&Event::PressRelease { x: 4, y: 4 }));
    assert_eq!(
        list.event_payload(
            list::MPY_SELECTION_CHANGED_EVENT_ID,
            &ObjectEvent::Clicked { x: 4, y: 4 },
            &mut bytes,
        )
        .unwrap(),
        None
    );
}

#[test]
fn reservation_completion_preserves_registration_order_and_semantic_gate() {
    let mut stage = stage();
    let actor = create_button(&mut stage);
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(1).unwrap(), subscription_limits(4, 4))
            .unwrap();
    let first = subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                button::MPY_CLICKED_EVENT_ID,
                10,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    let second = subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                button::MPY_CLICKED_EVENT_ID,
                11,
                PropagationPolicy::ConsumeAtTarget,
            ),
        )
        .unwrap();
    let enumerated: Vec<_> = subscriptions.subscriptions().collect();
    assert_eq!(enumerated[0].subscription_id, first);
    assert_eq!(enumerated[1].subscription_id, second);
    assert!(enumerated[0].registration_order < enumerated[1].registration_order);

    let event = ObjectEvent::Clicked { x: 7, y: 9 };
    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 1);
    assert_eq!(workspace.cue_admission_counts().critical, 0);
    assert_eq!(workspace.cue_admission_counts().ordered, 2);
    assert_eq!(workspace.cue_admission_counts().latest_value_coalescible, 0);
    assert_eq!(workspace.cue_admission_counts().ordinary(), 2);
    assert_eq!(workspace.cue_admission_counts().total(), 2);
    let (control, allocations, deallocations) = count_allocator_operations(|| {
        subscriptions.complete_observation(&mut workspace, completion(actor, &event, false, false))
    });
    let control = control.unwrap();
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(control, NativeObserverControl::Continue);
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    assert!(result.cues.is_empty());

    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 2);
    let (control, allocations, deallocations) = count_allocator_operations(|| {
        subscriptions.complete_observation(&mut workspace, completion(actor, &event, true, true))
    });
    let control = control.unwrap();
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(control, NativeObserverControl::ConsumePredeclared);
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    assert_eq!(result.cues.len(), 2);
    assert_eq!(result.cues[0].subscription_id(), first);
    assert_eq!(result.cues[1].subscription_id(), second);
    assert_eq!(result.cues[0].delivery(), CueDelivery::Ordered);
    let (x, used) = decode_value(result.cues[0].payload()).unwrap();
    let (y, rest) = decode_value(&result.cues[0].payload()[used..]).unwrap();
    assert_eq!(
        (x, y, used + rest),
        (ValueRef::I32(7), ValueRef::I32(9), 10)
    );
}

#[test]
fn actual_transition_controls_revision_cue_and_predeclared_consume() {
    let mut stage = stage();
    let actor = create_slider(&mut stage);
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(2).unwrap(), subscription_limits(2, 2))
            .unwrap();
    subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                slider::MPY_VALUE_CHANGED_EVENT_ID,
                20,
                PropagationPolicy::ConsumeAtTarget,
            ),
        )
        .unwrap();

    let starting_revision = stage.revision();
    let event = ObjectEvent::Clicked { x: 50, y: 10 };
    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 1);
    assert!(
        stage
            .node(actor.object_id)
            .unwrap()
            .widget()
            .borrow_mut()
            .handle_event(&Event::PressRelease { x: 50, y: 10 })
    );
    let control = subscriptions
        .complete_observation(&mut workspace, completion(actor, &event, true, true))
        .unwrap();
    assert_eq!(control, NativeObserverControl::ConsumePredeclared);
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    assert_eq!(stage.revision().get(), starting_revision.get() + 1);
    assert_eq!(result.cues.len(), 1);
    assert_eq!(result.cues[0].stage_revision(), stage.revision());

    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 2);
    assert!(
        stage
            .node(actor.object_id)
            .unwrap()
            .widget()
            .borrow_mut()
            .handle_event(&Event::PressRelease { x: 50, y: 10 })
    );
    let control = subscriptions
        .complete_observation(&mut workspace, completion(actor, &event, true, true))
        .unwrap();
    assert_eq!(control, NativeObserverControl::Continue);
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    assert!(result.cues.is_empty());
    assert_eq!(stage.revision().get(), starting_revision.get() + 1);
}

#[test]
fn descriptor_runs_and_publishes_native_mutation_without_subscribers() {
    let mut stage = stage();
    let actor = create_slider(&mut stage);
    let subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(3).unwrap(), subscription_limits(2, 2))
            .unwrap();
    let starting_revision = stage.revision();
    let event = ObjectEvent::Clicked { x: 75, y: 10 };
    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 1);
    assert!(
        stage
            .node(actor.object_id)
            .unwrap()
            .widget()
            .borrow_mut()
            .handle_event(&Event::PressRelease { x: 75, y: 10 })
    );
    subscriptions
        .complete_observation(&mut workspace, completion(actor, &event, true, true))
        .unwrap();
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    assert!(result.cues.is_empty());
    assert_eq!(stage.revision().get(), starting_revision.get() + 1);
    assert_eq!(
        stage.last_commit_effects(),
        MutationEffects::DRAW.union(MutationEffects::SNAPSHOT)
    );
    assert_eq!(stage.last_invalidations(), &[BOUNDS]);
}

#[test]
fn invalid_policy_filter_identity_callback_and_capacity_are_rejected() {
    let mut stage = stage();
    let actor = create_button(&mut stage);
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(4).unwrap(), subscription_limits(2, 2))
            .unwrap();
    let mut invalid = request(
        stage.stage_id(),
        actor,
        button::MPY_CLICKED_EVENT_ID,
        1,
        PropagationPolicy::Observe,
    );
    invalid.phase = DispatchPhase::Bubble;
    assert_eq!(
        subscriptions.subscribe(&stage, invalid),
        Err(SubscriptionError::InvalidPhase)
    );
    invalid.phase = DispatchPhase::Target;
    invalid.propagation = PropagationPolicy::PreventDefault;
    assert_eq!(
        subscriptions.subscribe(&stage, invalid),
        Err(SubscriptionError::UnsupportedPolicy)
    );
    invalid.propagation = PropagationPolicy::Observe;
    invalid.filter = SubscriptionFilter::PointerRegion(Rect { width: 0, ..BOUNDS });
    assert_eq!(
        subscriptions.subscribe(&stage, invalid),
        Err(SubscriptionError::InvalidFilter)
    );
    invalid.filter = SubscriptionFilter::Any;
    subscriptions.subscribe(&stage, invalid).unwrap();
    invalid.callback_id = CallbackId::new(1).unwrap();
    assert_eq!(
        subscriptions.subscribe(&stage, invalid),
        Err(SubscriptionError::DuplicateCallback)
    );
    invalid.callback_id = CallbackId::new(2).unwrap();
    subscriptions.subscribe(&stage, invalid).unwrap();
    invalid.callback_id = CallbackId::new(3).unwrap();
    assert_eq!(
        subscriptions.subscribe(&stage, invalid),
        Err(SubscriptionError::Capacity)
    );

    let wrong = ActorIdentity {
        object_id: actor.object_id,
        type_id: slider::MPY_TYPE_ID,
    };
    let event = ObjectEvent::Clicked { x: 1, y: 1 };
    assert!(matches!(
        subscriptions.reserve_observation(
            &mut stage,
            NativeEventReservation {
                actor_identity: wrong,
                phase: DispatchPhase::Target,
                event: &event,
                native_event_sequence: NativeEventSequence::new(1).unwrap(),
            },
        ),
        Err(SubscriptionError::ActorIdentityMismatch)
    ));
}

#[test]
fn insufficient_preflight_and_stale_workspace_fail_before_cue_loss() {
    let mut stage = stage();
    let actor = create_button(&mut stage);
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(5).unwrap(), subscription_limits(3, 1))
            .unwrap();
    for callback in [1, 2] {
        subscriptions
            .subscribe(
                &stage,
                request(
                    stage.stage_id(),
                    actor,
                    button::MPY_CLICKED_EVENT_ID,
                    callback,
                    PropagationPolicy::Observe,
                ),
            )
            .unwrap();
    }
    let event = ObjectEvent::Clicked { x: 2, y: 3 };
    assert!(matches!(
        subscriptions.reserve_observation(
            &mut stage,
            NativeEventReservation {
                actor_identity: actor,
                phase: DispatchPhase::Target,
                event: &event,
                native_event_sequence: NativeEventSequence::new(1).unwrap(),
            },
        ),
        Err(SubscriptionError::ObservationCapacity)
    ));

    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(6).unwrap(), subscription_limits(3, 3))
            .unwrap();
    subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                button::MPY_CLICKED_EVENT_ID,
                3,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 2);
    subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                button::MPY_CLICKED_EVENT_ID,
                4,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    assert!(matches!(
        subscriptions.complete_observation(&mut workspace, completion(actor, &event, true, true)),
        Err(SubscriptionError::StaleWorkspace)
    ));
}

const PARENT_EVENT_ID: u32 = 0x0001_fffe;
const PARENT_EVENT: EventDescriptor = EventDescriptor {
    id: PARENT_EVENT_ID,
    name: "parent_clicked",
    payload: &[],
    max_payload_bytes: 0,
    native_event: NativeEventKind::Clicked,
    phases: EventPhaseSet::TARGET,
    filters: EventFilterSet::ANY,
    requires_widget_invocation: false,
    requires_native_consumed: false,
    allow_consume_at_target: false,
    allow_stop_after_phase: false,
    native_effects: MutationEffects::NONE,
    delivery: EventDelivery::Ordered,
    coalescing_key: None,
};
const EVENTFUL_CONTAINER: TypeDescriptor = TypeDescriptor {
    schema_revision: 3,
    events: &[PARENT_EVENT],
    ..container::MPY_DESCRIPTOR
};
static TREE_CATALOG: [TypeDescriptor; 2] = [EVENTFUL_CONTAINER, button::MPY_DESCRIPTOR];

#[test]
fn teardown_is_caller_postorder_exact_once_and_tombstones_are_bounded() {
    let mut stage =
        StageRegistry::new(StageId::new(7).unwrap(), &TREE_CATALOG, registry_limits()).unwrap();
    let parent = stage
        .create(
            EVENTFUL_CONTAINER.type_id,
            CreateDestination::Root { name: "main" },
            &[rect_input(field(&EVENTFUL_CONTAINER, "bounds"))],
        )
        .unwrap();
    let child = stage
        .create(
            button::MPY_TYPE_ID,
            CreateDestination::Child { parent },
            &[
                rect_input(field(&button::MPY_DESCRIPTOR, "bounds")),
                rlvgl_core::actor::ConstructorInput {
                    id: field(&button::MPY_DESCRIPTOR, "text"),
                    value: ValueRef::Text("child"),
                },
            ],
        )
        .unwrap();
    let parent_identity = ActorIdentity {
        object_id: parent,
        type_id: EVENTFUL_CONTAINER.type_id,
    };
    let child_identity = ActorIdentity {
        object_id: child,
        type_id: button::MPY_TYPE_ID,
    };
    let mut subscriptions = SubscriptionRegistry::new(
        EndpointEpoch::new(7).unwrap(),
        SubscriptionLimits::new(4, 32, 1, 4).unwrap(),
    )
    .unwrap();
    let parent_subscription = subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                parent_identity,
                PARENT_EVENT_ID,
                1,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    let child_subscription = subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                child_identity,
                button::MPY_CLICKED_EVENT_ID,
                2,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    assert_eq!(
        subscriptions.teardown_stage_child_first(stage.stage_id(), &[child]),
        Err(SubscriptionError::TeardownOrderIncomplete)
    );
    let mut prepared = subscriptions
        .prepare_teardown_stage_child_first(stage.stage_id(), &[child, parent])
        .unwrap();
    assert_eq!(prepared.report_count(), 2);
    assert!(!prepared.is_empty());
    assert_eq!(prepared.reports()[0].subscription_id, child_subscription);
    assert_eq!(prepared.reports()[1].subscription_id, parent_subscription);
    assert_eq!(
        prepared.reports()[0].event_id.get(),
        button::MPY_CLICKED_EVENT_ID
    );
    assert_eq!(subscriptions.len(), 2);

    let (committed, allocations, deallocations) = count_allocator_operations(|| {
        let guard = subscriptions.prepare_teardown_commit(&mut prepared)?;
        guard.commit();
        Ok::<(), SubscriptionError>(())
    });
    assert_eq!(committed, Ok(()));
    assert_eq!((allocations, deallocations), (0, 0));
    assert!(subscriptions.is_empty());
    assert_eq!(prepared.report_count(), 2);
    let (committed, allocations, deallocations) =
        count_allocator_operations(|| subscriptions.commit_teardown(&mut prepared));
    assert_eq!(committed, Err(SubscriptionError::TeardownAlreadyCommitted));
    assert_eq!((allocations, deallocations), (0, 0));
    let ((), allocations, deallocations) =
        count_allocator_operations(|| subscriptions.release_teardown(prepared));
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 1);
    assert_eq!(
        subscriptions
            .unsubscribe(stage.stage_id(), parent_identity, parent_subscription)
            .unwrap(),
        UnsubscribeOutcome::AlreadyRemoved
    );
    assert_eq!(
        subscriptions.unsubscribe(stage.stage_id(), child_identity, child_subscription),
        Err(SubscriptionError::StaleSubscription)
    );
}

#[test]
fn prepared_teardown_drop_rolls_back_and_revision_changes_make_it_stale() {
    let mut stage = stage();
    let actor = create_button(&mut stage);
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(11).unwrap(), subscription_limits(3, 3))
            .unwrap();
    let first = subscriptions
        .subscribe(
            &stage,
            request(
                stage.stage_id(),
                actor,
                button::MPY_CLICKED_EVENT_ID,
                1,
                PropagationPolicy::Observe,
            ),
        )
        .unwrap();
    let prepared = subscriptions
        .prepare_teardown_objects_child_first(stage.stage_id(), &[actor.object_id])
        .unwrap();
    assert_eq!(prepared.report_count(), 1);
    assert_eq!(subscriptions.len(), 1);
    let ((), allocations, deallocations) =
        count_allocator_operations(|| subscriptions.release_teardown(prepared));
    assert_eq!((allocations, deallocations), (0, 1));
    assert_eq!(subscriptions.len(), 1);

    let mut prepared = subscriptions
        .prepare_teardown_stage_child_first(stage.stage_id(), &[actor.object_id])
        .unwrap();
    assert!(matches!(
        subscriptions
            .unsubscribe(stage.stage_id(), actor, first)
            .unwrap(),
        UnsubscribeOutcome::Removed(_)
    ));
    let (committed, allocations, deallocations) =
        count_allocator_operations(|| subscriptions.commit_teardown(&mut prepared));
    assert_eq!(committed, Err(SubscriptionError::StaleTeardown));
    assert_eq!((allocations, deallocations), (0, 0));
    let ((), allocations, deallocations) =
        count_allocator_operations(|| subscriptions.release_teardown(prepared));
    assert_eq!((allocations, deallocations), (0, 1));
}

const DUPLICATE_EVENT_CONTAINER: TypeDescriptor = TypeDescriptor {
    schema_revision: 3,
    events: &[EventDescriptor {
        id: button::MPY_CLICKED_EVENT_ID,
        name: "duplicate_clicked",
        ..PARENT_EVENT
    }],
    ..container::MPY_DESCRIPTOR
};
static INVALID_CATALOG: [TypeDescriptor; 2] = [DUPLICATE_EVENT_CONTAINER, button::MPY_DESCRIPTOR];

#[test]
fn catalog_rejects_global_event_id_collision() {
    assert!(matches!(
        StageRegistry::new(
            StageId::new(8).unwrap(),
            &INVALID_CATALOG,
            registry_limits()
        ),
        Err(RegistryError::InvalidCatalog)
    ));
}

#[test]
fn object_id_requires_nonzero_words_and_roundtrips_reused_slots() {
    assert_eq!(ObjectId::new(1), None);
    assert_eq!(ObjectId::new(1_u64 << 32), None);
    let serialized = (7_u64 << 32) | 9;
    let parsed = ObjectId::new(serialized).unwrap();
    assert_eq!(parsed.get(), serialized);
    assert_eq!(parsed.generation(), 7);
    assert_eq!(parsed.slot(), 9);

    let mut stage = stage();
    let first = create_root(&mut stage);
    assert_eq!(first.slot(), 1);
    assert_eq!(ObjectId::new(first.get()), Some(first));
    stage.delete(first).unwrap();
    assert_eq!(
        stage.actor_info(first),
        Err(RegistryError::StaleObject { object_id: first })
    );
    let replacement = create_root(&mut stage);
    assert_eq!(replacement.slot(), first.slot());
    assert_eq!(replacement.generation(), first.generation() + 1);
}

const ORDER_EVENT_ONE: u32 = 0x0001_f001;
const ORDER_EVENT_TWO: u32 = 0x0001_f002;
static EVENT_ONE_CALLS: AtomicUsize = AtomicUsize::new(0);
static EVENT_TWO_CALLS: AtomicUsize = AtomicUsize::new(0);

struct OrderedEventActor;

impl Widget for OrderedEventActor {
    fn bounds(&self) -> Rect {
        BOUNDS
    }

    fn draw(&self, _renderer: &mut dyn Renderer) {}

    fn handle_event(&mut self, _event: &Event) -> bool {
        true
    }
}

impl MpyActor for OrderedEventActor {
    type Prepared = ();

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        Err(RegistryError::UnknownProperty { property_id: id })
    }

    fn event_payload(
        &self,
        event_id: u32,
        event: &ObjectEvent,
        output: &mut [u8],
    ) -> Result<Option<usize>, RegistryError> {
        if !matches!(event, ObjectEvent::Clicked { .. }) {
            return Err(RegistryError::Internal);
        }
        let value = match event_id {
            ORDER_EVENT_ONE => {
                EVENT_ONE_CALLS.fetch_add(1, Ordering::SeqCst);
                1
            }
            ORDER_EVENT_TWO => {
                EVENT_TWO_CALLS.fetch_add(1, Ordering::SeqCst);
                2
            }
            _ => return Err(RegistryError::UnknownEvent { event_id }),
        };
        encode_event_values(&[ValueRef::U32(value)], output).map(Some)
    }

    fn prepare(
        &self,
        directions: &[ActorDirection],
    ) -> Result<ActorPreparation<Self::Prepared>, RegistryError> {
        if directions.is_empty() {
            Ok(ActorPreparation {
                prepared: (),
                text_delta: 0,
            })
        } else {
            Err(RegistryError::BatchInvalid)
        }
    }

    fn commit(&mut self, (): Self::Prepared) {}
}

fn construct_ordered_actor(_args: ConstructorArgs<'_>) -> Result<ConstructedActor, RegistryError> {
    Ok(construct_native_actor(
        ORDERED_ACTOR_TYPE,
        OrderedEventActor,
    ))
}

const ORDERED_ACTOR_TYPE: TypeId = TypeId::registered(0x0001_f000);
const ORDER_EVENTS: [EventDescriptor; 2] = [
    EventDescriptor {
        id: ORDER_EVENT_ONE,
        name: "first_emission",
        payload: &[ValueTag::U32],
        max_payload_bytes: 5,
        native_event: NativeEventKind::Clicked,
        phases: EventPhaseSet::TARGET,
        filters: EventFilterSet::ANY,
        requires_widget_invocation: false,
        requires_native_consumed: false,
        allow_consume_at_target: true,
        allow_stop_after_phase: false,
        native_effects: MutationEffects::NONE,
        delivery: EventDelivery::Critical,
        coalescing_key: None,
    },
    EventDescriptor {
        id: ORDER_EVENT_TWO,
        name: "second_emission",
        payload: &[ValueTag::U32],
        max_payload_bytes: 5,
        native_event: NativeEventKind::Clicked,
        phases: EventPhaseSet::TARGET,
        filters: EventFilterSet::ANY,
        requires_widget_invocation: false,
        requires_native_consumed: false,
        allow_consume_at_target: true,
        allow_stop_after_phase: false,
        native_effects: MutationEffects::NONE,
        delivery: EventDelivery::LatestValueCoalescible,
        coalescing_key: Some(0x1_f002),
    },
];
const ORDERED_ACTOR_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: ORDERED_ACTOR_TYPE,
    stable_name: "tests::OrderedEventActor",
    schema_revision: 1,
    constructor_fields: &[],
    properties: &[],
    actions: &[],
    events: &ORDER_EVENTS,
    constructor: construct_ordered_actor,
    ..container::MPY_DESCRIPTOR
};
static ORDERED_ACTOR_CATALOG: [TypeDescriptor; 1] = [ORDERED_ACTOR_DESCRIPTOR];

#[test]
fn descriptor_order_precedes_per_descriptor_registration_order() {
    EVENT_ONE_CALLS.store(0, Ordering::SeqCst);
    EVENT_TWO_CALLS.store(0, Ordering::SeqCst);
    let mut stage = StageRegistry::new(
        StageId::new(10).unwrap(),
        &ORDERED_ACTOR_CATALOG,
        registry_limits(),
    )
    .unwrap();
    let object_id = stage
        .create(
            ORDERED_ACTOR_TYPE,
            CreateDestination::Root { name: "ordered" },
            &[],
        )
        .unwrap();
    let actor = ActorIdentity {
        object_id,
        type_id: ORDERED_ACTOR_TYPE,
    };
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(10).unwrap(), subscription_limits(4, 4))
            .unwrap();
    for (event_id, callback) in [
        (ORDER_EVENT_TWO, 1),
        (ORDER_EVENT_ONE, 2),
        (ORDER_EVENT_TWO, 3),
    ] {
        subscriptions
            .subscribe(
                &stage,
                request(
                    stage.stage_id(),
                    actor,
                    event_id,
                    callback,
                    PropagationPolicy::Observe,
                ),
            )
            .unwrap();
    }

    let event = ObjectEvent::Clicked { x: 8, y: 9 };
    let mut workspace = reserve(&subscriptions, &mut stage, actor, &event, 1);
    assert_eq!(workspace.cue_admission_counts().critical, 1);
    assert_eq!(workspace.cue_admission_counts().ordered, 0);
    assert_eq!(workspace.cue_admission_counts().latest_value_coalescible, 2);
    assert_eq!(workspace.cue_admission_counts().ordinary(), 2);
    assert_eq!(workspace.cue_admission_counts().total(), 3);
    assert_eq!(workspace.cue_admission().stage_id, stage.stage_id());
    assert_eq!(workspace.cue_admission().ordinary_slots, 2);
    assert_eq!(workspace.cue_admission().critical_slots, 1);
    subscriptions
        .complete_observation(&mut workspace, completion(actor, &event, true, true))
        .unwrap();
    let result = subscriptions
        .publish_observation(&mut stage, workspace)
        .unwrap();
    let order: Vec<_> = result
        .cues
        .iter()
        .map(|cue| (cue.event_id().get(), cue.callback_id().get()))
        .collect();
    assert_eq!(
        order,
        [
            (ORDER_EVENT_ONE, 2),
            (ORDER_EVENT_TWO, 1),
            (ORDER_EVENT_TWO, 3),
        ]
    );
    assert_eq!(EVENT_ONE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(EVENT_TWO_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn dispatch_wide_workspace_is_allocation_free_and_preserves_descriptor_fanout_order() {
    EVENT_ONE_CALLS.store(0, Ordering::SeqCst);
    EVENT_TWO_CALLS.store(0, Ordering::SeqCst);
    let mut stage = StageRegistry::new(
        StageId::new(11).unwrap(),
        &ORDERED_ACTOR_CATALOG,
        registry_limits(),
    )
    .unwrap();
    let object_id = stage
        .create(
            ORDERED_ACTOR_TYPE,
            CreateDestination::Root { name: "ordered" },
            &[],
        )
        .unwrap();
    let actor = ActorIdentity {
        object_id,
        type_id: ORDERED_ACTOR_TYPE,
    };
    let mut subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(11).unwrap(), subscription_limits(4, 4))
            .unwrap();
    for (event_id, callback) in [
        (ORDER_EVENT_TWO, 1),
        (ORDER_EVENT_ONE, 2),
        (ORDER_EVENT_TWO, 3),
    ] {
        subscriptions
            .subscribe(
                &stage,
                request(
                    stage.stage_id(),
                    actor,
                    event_id,
                    callback,
                    PropagationPolicy::Observe,
                ),
            )
            .unwrap();
    }

    let starting_revision = stage.revision();
    let route = stage
        .resolve_actor_dispatch(actor, ObjectEvent::Clicked { x: 8, y: 9 })
        .unwrap();
    let prepared = subscriptions
        .reserve_native_dispatch(&mut stage, &route, NativeEventSequence::new(41).unwrap())
        .unwrap();
    assert_eq!(prepared.cue_admission_counts().critical, 1);
    assert_eq!(prepared.cue_admission_counts().latest_value_coalescible, 2);
    assert_eq!(prepared.maximum_payload_bytes(), 5);
    assert_eq!(prepared.possible_observation_count(), 2);
    let mut observer = subscriptions.arm_native_dispatch(&stage, prepared).unwrap();

    let ((completed, published), allocations, deallocations) = count_allocator_operations(|| {
        let completed = stage
            .dispatch_resolved_native(route, &mut observer)
            .unwrap();
        let observed = match observer.finish() {
            Ok(observed) => observed,
            Err(failed) => panic!("native adapter failed: {:?}", failed.cause()),
        };
        let published = subscriptions.publish_native_dispatch(&mut stage, observed);
        (completed, published)
    });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(published.stage_revision(), starting_revision);
    assert_eq!(published.native_event_sequence().get(), 41);
    assert_eq!(EVENT_ONE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(EVENT_TWO_CALLS.load(Ordering::SeqCst), 1);

    let mut published = published;
    let (cues, allocations, deallocations) = count_allocator_operations(|| published.take_cues());
    assert_eq!((allocations, deallocations), (0, 0));
    let order: Vec<_> = cues
        .iter()
        .map(|cue| (cue.event_id().get(), cue.callback_id().get()))
        .collect();
    assert_eq!(
        order,
        [
            (ORDER_EVENT_ONE, 2),
            (ORDER_EVENT_TWO, 1),
            (ORDER_EVENT_TWO, 3),
        ]
    );
    assert!(
        cues.iter()
            .all(|cue| cue.stage_revision() == starting_revision)
    );
    completed.release();
    published.release();
}

#[test]
fn dispatch_wide_mutation_advances_once_and_unchanged_transition_does_not() {
    let mut stage = stage();
    let actor = create_slider(&mut stage);
    stage
        .apply_batch(&[StageDirection::SetFlag {
            object_id: actor.object_id,
            flag: RuntimeFlag::Clickable,
            enabled: true,
        }])
        .unwrap();
    let subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(12).unwrap(), subscription_limits(4, 4))
            .unwrap();
    let root_id = stage.root_id("main").unwrap();
    let before = stage.revision();

    for (sequence, expected_revision) in [(1, before.get() + 1), (2, before.get() + 1)] {
        let route = stage
            .resolve_root_dispatch(
                root_id,
                DispatchInput::Pointer {
                    x: 75,
                    y: 10,
                    event: Event::PressRelease { x: 75, y: 10 },
                },
            )
            .unwrap();
        let prepared = subscriptions
            .reserve_native_dispatch(
                &mut stage,
                &route,
                NativeEventSequence::new(sequence).unwrap(),
            )
            .unwrap();
        assert_eq!(prepared.possible_observation_count(), 4);
        let mut observer = subscriptions.arm_native_dispatch(&stage, prepared).unwrap();
        let completed = stage
            .dispatch_resolved_native(route, &mut observer)
            .unwrap();
        let observed = match observer.finish() {
            Ok(observed) => observed,
            Err(failed) => panic!("native adapter failed: {:?}", failed.cause()),
        };
        let published = subscriptions.publish_native_dispatch(&mut stage, observed);
        assert_eq!(published.stage_revision().get(), expected_revision);
        assert!(stage.last_invalidations().contains(&BOUNDS));
        completed.release();
        published.release();
    }
}

#[test]
fn dispatch_wide_workspace_matches_the_full_non_clicked_event() {
    let mut stage = stage();
    let root_id = create_root(&mut stage);
    let root_info = stage.actor_info(root_id).unwrap();
    let root = ActorIdentity {
        object_id: root_id,
        type_id: root_info.type_id,
    };
    let subscriptions =
        SubscriptionRegistry::new(EndpointEpoch::new(13).unwrap(), subscription_limits(2, 2))
            .unwrap();
    let route = stage
        .resolve_actor_dispatch(root, ObjectEvent::Focused)
        .unwrap();
    let planned = route.possible_observations().next().unwrap();
    let prepared = subscriptions
        .reserve_native_dispatch(&mut stage, &route, NativeEventSequence::new(1).unwrap())
        .unwrap();
    let mut observer = subscriptions.arm_native_dispatch(&stage, prepared).unwrap();
    let wrong_event = ObjectEvent::Defocused;
    assert_eq!(
        observer.observe(NativeEventObservation {
            phase: planned.phase,
            node: planned.node,
            event: &wrong_event,
            context: EventContext {
                target_tag: None,
                current_tag: None,
            },
            native_consumed: false,
            widget_invoked: false,
        }),
        NativeObserverControl::ConsumePredeclared
    );
    let failed = match observer.finish() {
        Ok(observed) => {
            observed.release();
            panic!("mismatched non-clicked event unexpectedly completed")
        }
        Err(failed) => failed,
    };
    assert_eq!(failed.cause(), SubscriptionError::WorkspaceMismatch);
    failed.release();
}

#[test]
fn stage_dispatch_routes_direct_actors_to_their_owning_root() {
    let mut stage = stage();
    let first_root = create_root(&mut stage);
    let container = descriptor("container::Container");
    let second_root = stage
        .create(
            container.type_id,
            CreateDestination::Root { name: "second" },
            &[rect_input(field(container, "bounds"))],
        )
        .unwrap();
    let button = descriptor("button::Button");
    let target_id = stage
        .create(
            button.type_id,
            CreateDestination::Child {
                parent: second_root,
            },
            &[
                rect_input(field(button, "bounds")),
                rlvgl_core::actor::ConstructorInput {
                    id: field(button, "text"),
                    value: ValueRef::Text("Second"),
                },
            ],
        )
        .unwrap();
    let target = ActorIdentity {
        object_id: target_id,
        type_id: button.type_id,
    };

    let route = stage
        .resolve_actor_dispatch(target, ObjectEvent::Clicked { x: 1, y: 1 })
        .unwrap();
    assert_eq!(route.root_id(), second_root);
    assert_eq!(route.target_identity(), Some(target));
    assert_eq!(route.possible_observations().len(), 4);
    assert!(matches!(
        stage.resolve_root_dispatch(
            first_root,
            DispatchInput::Container {
                path: vec![0],
                event: ObjectEvent::Clicked { x: 1, y: 1 },
            },
        ),
        Err(rlvgl_core::actor::StageDispatchError::Object(
            rlvgl_core::object::ObjectDispatchError::InvalidPath
        ))
    ));
}
