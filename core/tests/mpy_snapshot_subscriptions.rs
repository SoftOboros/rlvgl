//! Native MPY subscription snapshot bounds, revision isolation, and retry proof.
use rlvgl_api::protocol::ValueRef;
use rlvgl_core::{
    actor::{
        ActorIdentity, CapacityKind, ConstructorInput, CreateDestination, MpyActor, RegistryError,
        RegistryLimits, StageId, StageRegistry,
    },
    cue::{CallbackId, CueLimits, EventId},
    direction::{OwnedValue, SnapshotError, StageDirection},
    endpoint::{BatchOutcome, Endpoint, EndpointError, EndpointLimits, RequestId},
    object::DispatchPhase,
    subscription::{
        EndpointEpoch, PropagationPolicy, SubscribeRequest, SubscriptionError, SubscriptionFilter,
        SubscriptionLimits, UnsubscribeOutcome,
    },
};
use rlvgl_widgets::{button, label, mpy::CATALOG};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    ptr,
};

struct FailureAllocator;
thread_local! { static FAIL_AFTER: Cell<Option<usize>> = const { Cell::new(None) }; }
fn fail() -> bool {
    FAIL_AFTER
        .try_with(|remaining| match remaining.get() {
            Some(0) => {
                remaining.set(None);
                true
            }
            Some(n) => {
                remaining.set(Some(n - 1));
                false
            }
            None => false,
        })
        .unwrap_or(false)
}
// SAFETY: every successful operation delegates unchanged pointers/layouts to
// System. Only the selected thread's fallible snapshot allocations return null.
unsafe impl GlobalAlloc for FailureAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if fail() {
            ptr::null_mut()
        } else {
            // SAFETY: the unchanged layout is valid for System.
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if fail() {
            ptr::null_mut()
        } else {
            // SAFETY: the unchanged layout is valid for System.
            unsafe { System.alloc_zeroed(layout) }
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if fail() {
            ptr::null_mut()
        } else {
            // SAFETY: pointer/layout belong to System; size is forwarded unchanged.
            unsafe { System.realloc(pointer, layout, size) }
        }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pointer/layout came from the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: FailureAllocator = FailureAllocator;

fn id(raw: u32) -> StageId {
    StageId::new(raw).unwrap()
}
fn stage(raw: u32) -> (StageRegistry, ActorIdentity) {
    stage_with_text(raw, "Button text")
}

fn stage_with_text(raw: u32, text: &str) -> (StageRegistry, ActorIdentity) {
    let mut stage = StageRegistry::new(
        id(raw),
        &CATALOG,
        RegistryLimits {
            max_roots: 2,
            max_actors: 8,
            max_tree_depth: 4,
            max_children_per_actor: 4,
            max_text_bytes: 1024,
            max_resources: 8,
        },
    )
    .unwrap();
    let container = CATALOG
        .iter()
        .find(|d| d.stable_name.ends_with("container::Container"))
        .unwrap();
    let button = CATALOG
        .iter()
        .find(|d| d.type_id == button::MPY_TYPE_ID)
        .unwrap();
    let bounds = |descriptor: &rlvgl_core::actor::TypeDescriptor| ConstructorInput {
        id: descriptor
            .constructor_fields
            .iter()
            .find(|f| f.name == "bounds")
            .unwrap()
            .id,
        value: ValueRef::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
    };
    let root = stage
        .create(
            container.type_id,
            CreateDestination::Root { name: "main" },
            &[bounds(container)],
        )
        .unwrap();
    let actor = stage
        .create(
            button.type_id,
            CreateDestination::Child { parent: root },
            &[
                bounds(button),
                ConstructorInput {
                    id: button
                        .constructor_fields
                        .iter()
                        .find(|f| f.name == "text")
                        .unwrap()
                        .id,
                    value: ValueRef::Text(text),
                },
            ],
        )
        .unwrap();
    (
        stage,
        ActorIdentity {
            object_id: actor,
            type_id: button.type_id,
        },
    )
}
fn endpoint() -> (Endpoint, ActorIdentity, ActorIdentity) {
    endpoint_with_text("Button text")
}

fn endpoint_with_text(text: &str) -> (Endpoint, ActorIdentity, ActorIdentity) {
    let mut endpoint = Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        EndpointLimits::new(2, 4, 4, 8).unwrap(),
        SubscriptionLimits::new(8, 32, 8, 16).unwrap(),
        CueLimits::new(16, 4, 4, 32, 128).unwrap(),
    )
    .unwrap();
    let (one, a) = stage_with_text(1, text);
    let (two, b) = stage(2);
    endpoint.register_stage(one).unwrap();
    endpoint.register_stage(two).unwrap();
    (endpoint, a, b)
}
fn request(stage: StageId, actor: ActorIdentity, callback: u32) -> SubscribeRequest {
    SubscribeRequest {
        stage_id: stage,
        actor_identity: actor,
        event_id: EventId::new(button::MPY_CLICKED_EVENT_ID).unwrap(),
        callback_id: CallbackId::new(callback).unwrap(),
        phase: DispatchPhase::Target,
        filter: SubscriptionFilter::Any,
        propagation: PropagationPolicy::Observe,
    }
}

#[test]
fn bounded_active_prefix_reports_truthful_zero_and_independent_text_budget() {
    let (mut endpoint, a, b) = endpoint();
    let first = endpoint.subscribe(request(id(1), a, 10)).unwrap();
    let removed = endpoint.subscribe(request(id(1), a, 11)).unwrap();
    let last = endpoint.subscribe(request(id(1), a, 12)).unwrap();
    endpoint.subscribe(request(id(2), b, 13)).unwrap();
    endpoint.unsubscribe(id(1), a, removed).unwrap();
    for budget in [0, 1, 2, usize::MAX] {
        let token = endpoint.snapshot_begin(id(1)).unwrap();
        let page = endpoint
            .snapshot_read(id(1), token, 8, 1024, 0, budget)
            .unwrap();
        assert!(page.ended);
        assert_eq!(
            endpoint.snapshot_end(id(1), token),
            Err(EndpointError::Snapshot(SnapshotError::InvalidCursor))
        );
        assert_eq!(page.records[0].total_subscriptions, 0);
        let record = &page.records[1];
        assert_eq!(record.actor.object_id, a.object_id);
        assert_eq!(record.total_subscriptions, 2);
        assert_eq!(record.subscriptions.len(), budget.min(2));
        assert_eq!(record.subscriptions_truncated, budget < 2);
        assert_eq!(record.actor.truncated, budget < 2);
        let expected = [first, last];
        for (row, expected_id) in record.subscriptions.iter().zip(expected) {
            assert_eq!(row.subscription_id, expected_id);
            assert_eq!(row.stage_id, id(1));
            assert_eq!(row.actor_identity, a);
            assert_eq!(row.event_id.get(), button::MPY_CLICKED_EVENT_ID);
            assert_eq!(row.phase, DispatchPhase::Target);
            assert_eq!(row.filter, SubscriptionFilter::Any);
            assert_eq!(row.propagation, PropagationPolicy::Observe);
            assert!(row.callback_id.get() == 10 || row.callback_id.get() == 12);
        }
        assert!(
            record
                .subscriptions
                .windows(2)
                .all(|rows| rows[0].registration_order < rows[1].registration_order)
        );
    }
    let token = endpoint.snapshot_begin(id(1)).unwrap();
    let page = endpoint.snapshot_read(id(1), token, 8, 0, 0, 8).unwrap();
    assert_eq!(page.records[1].subscriptions.len(), 2);
    assert!(!page.records[1].subscriptions_truncated);
    assert!(page.records[1].actor.truncated); // Text bound independent of subscription bound.
}

#[test]
fn actual_changes_stale_only_the_owning_stage_and_failed_or_repeated_changes_do_not() {
    let (mut endpoint, a, b) = endpoint();
    let starting = endpoint.stage(id(1)).unwrap().revision();
    let cursor = endpoint.snapshot_begin(id(1)).unwrap();
    endpoint.subscribe(request(id(2), b, 20)).unwrap();
    let first_page = endpoint
        .snapshot_read(id(1), cursor, 1, 1024, 0, 8)
        .unwrap();
    assert!(!first_page.ended);
    assert_eq!(first_page.revision, starting);
    let subscription = endpoint.subscribe(request(id(1), a, 21)).unwrap();
    let current = endpoint.stage(id(1)).unwrap().revision();
    assert_eq!(current.get(), starting.get() + 1);
    assert_eq!(
        endpoint.snapshot_read(id(1), cursor, 1, 1024, 0, 8),
        Err(EndpointError::Snapshot(SnapshotError::Stale {
            starting,
            current
        }))
    );
    let cursor = endpoint.snapshot_begin(id(1)).unwrap();
    assert_eq!(
        endpoint.subscribe(request(id(1), a, 21)),
        Err(EndpointError::Subscription(
            SubscriptionError::DuplicateCallback
        ))
    );
    assert_eq!(endpoint.stage(id(1)).unwrap().revision(), current);
    endpoint
        .snapshot_read(id(1), cursor, 8, 1024, 0, 8)
        .unwrap();
    let cursor = endpoint.snapshot_begin(id(1)).unwrap();
    assert!(matches!(
        endpoint.unsubscribe(id(1), a, subscription).unwrap(),
        UnsubscribeOutcome::Removed(_)
    ));
    let after = endpoint.stage(id(1)).unwrap().revision();
    assert_eq!(after.get(), current.get() + 1);
    assert_eq!(
        endpoint.snapshot_read(id(1), cursor, 8, 1024, 0, 8),
        Err(EndpointError::Snapshot(SnapshotError::Stale {
            starting: current,
            current: after
        }))
    );
    let cursor = endpoint.snapshot_begin(id(1)).unwrap();
    assert_eq!(
        endpoint.unsubscribe(id(1), a, subscription).unwrap(),
        UnsubscribeOutcome::AlreadyRemoved
    );
    assert_eq!(
        endpoint.unsubscribe(id(2), b, subscription),
        Err(EndpointError::Subscription(
            SubscriptionError::StaleSubscription
        ))
    );
    assert_eq!(endpoint.stage(id(1)).unwrap().revision(), after);
    let page = endpoint
        .snapshot_read(id(1), cursor, 8, 1024, 0, 8)
        .unwrap();
    assert_eq!(page.records[1].total_subscriptions, 0);
}

#[test]
fn actor_deletion_cleanup_advances_once_and_removes_snapshot_rows() {
    let (mut endpoint, a, _) = endpoint();
    endpoint.subscribe(request(id(1), a, 30)).unwrap();
    let before = endpoint.stage(id(1)).unwrap().revision();
    endpoint
        .enqueue_batch(
            RequestId::new(1).unwrap(),
            id(1),
            vec![StageDirection::Delete {
                object_id: a.object_id,
            }],
        )
        .unwrap();
    endpoint.run_safe_turn().unwrap();
    assert_eq!(
        endpoint.stage(id(1)).unwrap().revision().get(),
        before.get() + 1
    );
    let completion = endpoint.drain_completions(1).unwrap().remove(0);
    assert!(matches!(
        completion.outcome,
        BatchOutcome::Committed {
            released_subscriptions: 1,
            ..
        }
    ));
    let token = endpoint.snapshot_begin(id(1)).unwrap();
    let page = endpoint.snapshot_read(id(1), token, 8, 1024, 0, 8).unwrap();
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].total_subscriptions, 0);
}

#[test]
fn failed_complete_page_preparation_retries_identical_partial_and_final_pages() {
    // Real nonempty text exercises actor readback as well as every fallible
    // complete-page and subscription reservation before cursor publication.
    let (mut endpoint, a, _) = endpoint();
    endpoint.subscribe(request(id(1), a, 40)).unwrap();
    endpoint.subscribe(request(id(1), a, 41)).unwrap();
    for (max_records, skip_first) in [(1, false), (1, true), (8, false)] {
        let token = endpoint.snapshot_begin(id(1)).unwrap();
        if skip_first {
            endpoint.snapshot_read(id(1), token, 1, 1024, 0, 8).unwrap();
        }
        let expected = endpoint
            .snapshot_read(id(1), token, max_records, 1024, 0, 8)
            .unwrap();
        if !expected.ended {
            endpoint.snapshot_end(id(1), token).unwrap();
        }
        let mut failures = 0;
        for offset in 0..32 {
            let token = endpoint.snapshot_begin(id(1)).unwrap();
            if skip_first {
                endpoint.snapshot_read(id(1), token, 1, 1024, 0, 8).unwrap();
            }
            let stage_revision = endpoint.stage(id(1)).unwrap().revision();
            FAIL_AFTER.with(|n| n.set(Some(offset)));
            let result = endpoint.snapshot_read(id(1), token, max_records, 1024, 0, 8);
            FAIL_AFTER.with(|n| n.set(None));
            assert_eq!(endpoint.stage(id(1)).unwrap().revision(), stage_revision);
            match result {
                Err(EndpointError::Snapshot(SnapshotError::Registry(_))) => {
                    failures += 1;
                    let retried = endpoint
                        .snapshot_read(id(1), token, max_records, 1024, 0, 8)
                        .unwrap();
                    assert_eq!(retried, expected);
                    if !retried.ended {
                        endpoint.snapshot_end(id(1), token).unwrap();
                    }
                }
                Ok(page) => {
                    assert_eq!(page, expected);
                    if !page.ended {
                        endpoint.snapshot_end(id(1), token).unwrap();
                    }
                    break;
                }
                other => panic!("unexpected allocation outcome: {other:?}"),
            }
        }
        assert!(failures >= 2);
        if max_records == 8 {
            assert!(failures >= 5);
        } // Includes late rich-row reservations.
    }
}

#[test]
fn invalid_actor_limit_preserves_cursor_and_legacy_stage_shape_remains_available() {
    let (mut endpoint, _, _) = endpoint();
    let token = endpoint.snapshot_begin(id(1)).unwrap();
    assert_eq!(
        endpoint.snapshot_read(id(1), token, 0, 0, 0, 0),
        Err(EndpointError::Snapshot(SnapshotError::InvalidLimit))
    );
    endpoint.snapshot_read(id(1), token, 8, 1024, 0, 0).unwrap();
    let (mut standalone, _) = stage(3);
    let token = standalone.snapshot_begin().unwrap();
    let legacy: rlvgl_core::direction::SnapshotPage =
        standalone.snapshot_read(token, 8, 1024).unwrap();
    assert_eq!(legacy.records.len(), 2);
}

#[test]
fn stage_teardown_releases_once_and_closes_the_snapshot_surface() {
    let (mut endpoint, a, _) = endpoint();
    endpoint.subscribe(request(id(1), a, 50)).unwrap();
    let before = endpoint.stage(id(1)).unwrap().revision();
    endpoint
        .enqueue_stage_teardown(RequestId::new(1).unwrap(), id(1))
        .unwrap();
    endpoint.run_safe_turn().unwrap();
    let completion = endpoint.drain_completions(1).unwrap().remove(0);
    assert!(
        matches!(completion.outcome,BatchOutcome::StageTeardownCommitted { revision,released_subscriptions:1,.. } if revision.get()==before.get()+1)
    );
    assert!(endpoint.stage(id(1)).is_none());
    assert_eq!(
        endpoint.snapshot_begin(id(1)),
        Err(EndpointError::StageNotFound)
    );
}

#[test]
fn text_style_and_subscription_budgets_are_independent() {
    let (mut stage, a) = stage(1);
    stage
        .apply_batch(&[StageDirection::SetLocalStyle {
            object_id: a.object_id,
            part_id: 0,
            state_mask: 0,
            property_id: 4,
            value: OwnedValue::U32(7),
        }])
        .unwrap();
    let mut endpoint = Endpoint::new(
        EndpointEpoch::new(1).unwrap(),
        EndpointLimits::new(1, 4, 4, 8).unwrap(),
        SubscriptionLimits::new(8, 32, 8, 16).unwrap(),
        CueLimits::new(16, 4, 4, 32, 128).unwrap(),
    )
    .unwrap();
    endpoint.register_stage(stage).unwrap();
    endpoint.subscribe(request(id(1), a, 60)).unwrap();
    for text_budget in [0, 1024] {
        for style_budget in [0, 1] {
            for subscription_budget in [0, 1] {
                let token = endpoint.snapshot_begin(id(1)).unwrap();
                let page = endpoint
                    .snapshot_read(
                        id(1),
                        token,
                        8,
                        text_budget,
                        style_budget,
                        subscription_budget,
                    )
                    .unwrap();
                let record = &page.records[1];
                assert_eq!(record.actor.total_style_values, 1);
                assert_eq!(record.actor.styles.len(), style_budget);
                assert_eq!(record.total_subscriptions, 1);
                assert_eq!(record.subscriptions.len(), subscription_budget);
                assert_eq!(record.subscriptions_truncated, subscription_budget == 0);
                assert_eq!(
                    record.actor.truncated,
                    text_budget == 0 || style_budget == 0 || subscription_budget == 0
                );
            }
        }
    }
}

#[test]
fn label_nonempty_property_readback_failure_is_reported_without_mutation() {
    let label = label::Label::new(
        "Label allocation proof",
        rlvgl_core::widget::Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        },
    );
    let text_property = CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with("label::Label"))
        .unwrap()
        .properties
        .iter()
        .find(|property| property.name == "text")
        .unwrap()
        .id;
    FAIL_AFTER.with(|n| n.set(Some(0)));
    let failed = label.property(text_property);
    FAIL_AFTER.with(|n| n.set(None));
    assert_eq!(
        failed,
        Err(RegistryError::Capacity {
            kind: CapacityKind::SnapshotValues
        })
    );
    assert_eq!(label.text(), "Label allocation proof");
    assert_eq!(
        label.property(text_property).unwrap(),
        OwnedValue::Text("Label allocation proof".into())
    );
}
