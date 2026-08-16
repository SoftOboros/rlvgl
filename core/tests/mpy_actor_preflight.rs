//! Focused MPY actor deletion-preflight conformance tests.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use rlvgl_core::{
    actor::{
        ConstructorInput, CreateDestination, ObjectId, RegistryError, RegistryLimits, StageId,
        StageRegistry, TypeDescriptor, ValueRef,
    },
    direction::{ActorDirection, OwnedValue, RuntimeFlag, StageDirection, StageRevision},
    layout::FlexConfig,
    widget::Rect,
};
use rlvgl_widgets::mpy::CATALOG;

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

const BOUNDS: Rect = Rect {
    x: 0,
    y: 0,
    width: 320,
    height: 240,
};

fn registry() -> StageRegistry {
    StageRegistry::new(
        StageId::new(7).unwrap(),
        &CATALOG,
        RegistryLimits {
            max_roots: 4,
            max_actors: 16,
            max_tree_depth: 8,
            max_children_per_actor: 8,
            max_text_bytes: 256,
            max_resources: 8,
        },
    )
    .unwrap()
}

fn container_descriptor() -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with("container::Container"))
        .unwrap()
}

fn label_descriptor() -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with("label::Label"))
        .unwrap()
}

fn create_container(registry: &mut StageRegistry, destination: CreateDestination<'_>) -> ObjectId {
    let descriptor = container_descriptor();
    let bounds_field = descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == "bounds")
        .unwrap()
        .id;
    registry
        .create(
            descriptor.type_id,
            destination,
            &[ConstructorInput {
                id: bounds_field,
                value: ValueRef::Rect {
                    x: BOUNDS.x,
                    y: BOUNDS.y,
                    width: BOUNDS.width,
                    height: BOUNDS.height,
                },
            }],
        )
        .unwrap()
}

fn create_label(registry: &mut StageRegistry, parent: ObjectId, text: &str) -> ObjectId {
    let descriptor = label_descriptor();
    let bounds_field = descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == "bounds")
        .unwrap()
        .id;
    let text_field = descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == "text")
        .unwrap()
        .id;
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Child { parent },
            &[
                ConstructorInput {
                    id: bounds_field,
                    value: ValueRef::Rect {
                        x: BOUNDS.x,
                        y: BOUNDS.y,
                        width: BOUNDS.width,
                        height: BOUNDS.height,
                    },
                },
                ConstructorInput {
                    id: text_field,
                    value: ValueRef::Text(text),
                },
            ],
        )
        .unwrap()
}

#[test]
fn batch_preflight_reports_overlapping_deletes_once_in_child_first_order() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let branch = create_container(&mut registry, CreateDestination::Child { parent: root });
    let leaf = create_container(&mut registry, CreateDestination::Child { parent: branch });
    let sibling = create_container(&mut registry, CreateDestination::Child { parent: root });
    let sibling_leaf =
        create_container(&mut registry, CreateDestination::Child { parent: sibling });
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();
    let directions = [
        StageDirection::Delete { object_id: leaf },
        StageDirection::Delete { object_id: branch },
        StageDirection::Delete { object_id: root },
    ];

    let report = registry.preflight_batch(&directions).unwrap();

    assert_eq!(report.stage_id(), registry.stage_id());
    assert_eq!(report.starting_revision(), starting_revision);
    assert_eq!(report.deletion_count(), 5);
    assert_eq!(
        report.deleted_object_ids(),
        [leaf, branch, sibling_leaf, sibling, root]
    );
    assert_eq!(registry.revision(), starting_revision);
    assert_eq!(registry.usage(), starting_usage);
    assert_eq!(registry.root_id("main"), Some(root));
    for object_id in [root, branch, leaf, sibling, sibling_leaf] {
        assert!(registry.actor_info(object_id).is_ok());
    }

    let committed = registry.apply_batch(&directions).unwrap();
    assert_eq!(
        committed,
        rlvgl_core::direction::StageRevision::new(starting_revision.get() + 1)
    );
    assert_eq!(registry.usage().actors, 0);
    for object_id in report.deleted_object_ids() {
        assert_eq!(
            registry.actor_info(*object_id),
            Err(RegistryError::StaleObject {
                object_id: *object_id,
            })
        );
    }
}

#[test]
fn failed_preflight_leaves_tree_usage_and_revision_unchanged() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let child = create_container(&mut registry, CreateDestination::Child { parent: root });
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let error = registry
        .preflight_batch(&[
            StageDirection::Delete { object_id: root },
            StageDirection::Delete { object_id: child },
        ])
        .unwrap_err();

    assert_eq!(error, RegistryError::StaleObject { object_id: child });
    assert_eq!(registry.revision(), starting_revision);
    assert_eq!(registry.usage(), starting_usage);
    assert_eq!(registry.root_id("main"), Some(root));
    assert_eq!(registry.children(root).unwrap(), [child]);
    assert!(registry.actor_info(child).is_ok());
}

#[test]
fn teardown_preflight_enumerates_all_roots_without_closing_or_mutating_stage() {
    let mut registry = registry();
    let first_root = create_container(&mut registry, CreateDestination::Root { name: "first" });
    let first_child = create_container(
        &mut registry,
        CreateDestination::Child { parent: first_root },
    );
    let first_leaf = create_container(
        &mut registry,
        CreateDestination::Child {
            parent: first_child,
        },
    );
    let second_root = create_container(&mut registry, CreateDestination::Root { name: "second" });
    let second_child = create_container(
        &mut registry,
        CreateDestination::Child {
            parent: second_root,
        },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let report = registry.preflight_teardown().unwrap();

    assert_eq!(report.stage_id(), registry.stage_id());
    assert_eq!(report.starting_revision(), starting_revision);
    assert_eq!(
        report.deleted_object_ids(),
        [
            first_leaf,
            first_child,
            first_root,
            second_child,
            second_root,
        ]
    );
    assert_eq!(registry.revision(), starting_revision);
    assert_eq!(registry.usage(), starting_usage);
    assert_eq!(registry.root_id("first"), Some(first_root));
    assert_eq!(registry.root_id("second"), Some(second_root));
    assert!(registry.actor_info(first_leaf).is_ok());
    assert!(registry.actor_info(second_child).is_ok());
    assert_eq!(registry.preflight_teardown().unwrap(), report);
}

#[test]
fn prepared_tree_commit_allocates_and_deallocates_only_on_explicit_release() {
    let mut registry = registry();
    let main = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let branch = create_container(&mut registry, CreateDestination::Child { parent: main });
    let leaf = create_container(&mut registry, CreateDestination::Child { parent: branch });
    let spare = create_container(&mut registry, CreateDestination::Root { name: "spare" });
    registry
        .apply_batch(&[StageDirection::SetRequestedLayout {
            object_id: main,
            layout: rlvgl_core::direction::RequestedLayout::Flex(FlexConfig::default()),
        }])
        .unwrap();
    let starting = registry.revision();

    let prepared = registry
        .prepare_batch(vec![
            StageDirection::PromoteRoot {
                object_id: branch,
                name: "branch".into(),
                index: 1,
            },
            StageDirection::Reparent {
                object_id: leaf,
                new_parent: main,
                index: 0,
            },
            StageDirection::SetRequestedLayout {
                object_id: main,
                layout: rlvgl_core::direction::RequestedLayout::Flex(FlexConfig {
                    gap_main: 7,
                    ..FlexConfig::default()
                }),
            },
            StageDirection::Delete { object_id: branch },
        ])
        .unwrap();

    assert_eq!(prepared.stage_id(), registry.stage_id());
    assert_eq!(prepared.starting_revision(), starting);
    assert_eq!(
        prepared.next_revision(),
        StageRevision::new(starting.get() + 1)
    );
    assert_eq!(prepared.deleted_object_ids(), [branch]);

    let (committed, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(prepared));
    assert_eq!(allocations, 0, "commit allocated after preparation");
    assert_eq!(deallocations, 0, "commit released retained state early");
    let committed = committed.unwrap();

    assert_eq!(committed.revision(), StageRevision::new(starting.get() + 1));
    assert_eq!(committed.deleted_object_ids(), [branch]);
    assert_eq!(registry.children(main).unwrap(), [leaf]);
    assert_eq!(registry.root_id("spare"), Some(spare));
    assert_eq!(
        registry.actor_info(branch),
        Err(RegistryError::StaleObject { object_id: branch })
    );

    let (released, release_allocations, release_deallocations) =
        count_allocator_operations(|| registry.release_committed_batch(committed));
    released.unwrap();
    assert_eq!(release_allocations, 0);
    assert!(
        release_deallocations > 0,
        "retained tree and transaction storage must release after commit"
    );
    assert_eq!(registry.usage().actors, 3);
    assert_eq!(registry.usage().roots, 2);
}

#[test]
fn retained_actor_borrow_rejects_prepared_commit_without_mutation_or_release() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let label = create_label(&mut registry, root, "old");
    let property_id = label_descriptor().properties[0].id;
    let starting_revision = registry.revision();
    let prepared = registry
        .prepare_batch(vec![StageDirection::MutateActor {
            object_id: label,
            directions: vec![ActorDirection::SetProperty {
                id: property_id,
                value: OwnedValue::Text("new".into()),
            }],
        }])
        .unwrap();
    let widget = registry.node(label).unwrap().widget().clone();
    let retained_borrow = widget.borrow();

    let (error, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(prepared).unwrap_err());
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    assert_eq!(error.cause(), RegistryError::DispatchBusy);
    assert_eq!(registry.revision(), starting_revision);
    drop(retained_borrow);
    assert_eq!(
        registry.property(label, property_id).unwrap(),
        OwnedValue::Text("old".into())
    );

    let (committed, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(error.into_prepared()));
    assert_eq!(allocations, 0, "actor swap allocated during commit");
    assert_eq!(
        deallocations, 0,
        "retired actor text was released during commit"
    );
    let committed = committed.unwrap();
    assert_eq!(
        registry.property(label, property_id).unwrap(),
        OwnedValue::Text("new".into())
    );
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn stale_prepared_batch_is_returned_intact_without_rollback_work() {
    let mut registry = registry();
    let root = create_container(&mut registry, CreateDestination::Root { name: "main" });
    let child = create_container(&mut registry, CreateDestination::Child { parent: root });
    let prepared = registry
        .prepare_batch(vec![StageDirection::Reorder {
            object_id: child,
            index: 0,
        }])
        .unwrap();

    registry
        .apply_batch(&[StageDirection::SetFlag {
            object_id: child,
            flag: RuntimeFlag::Hidden,
            enabled: true,
        }])
        .unwrap();
    let current_revision = registry.revision();
    let current_usage = registry.usage();
    let current_children = registry.children(root).unwrap();

    let (error, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(prepared).unwrap_err());
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    assert_eq!(error.cause(), RegistryError::BatchInvalid);
    assert_eq!(error.into_prepared().deleted_object_ids(), []);
    assert_eq!(registry.revision(), current_revision);
    assert_eq!(registry.usage(), current_usage);
    assert_eq!(registry.children(root).unwrap(), current_children);
}
