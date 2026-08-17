//! Focused MPY atomic Create batch conformance tests.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    rc::Rc,
};

use rlvgl_core::{
    actor::{
        ActorCapabilities, ActorPreparation, CapacityKind, ConstructedActor, ConstructorArgs,
        ConstructorFieldDescriptor, MpyActor, MutationEffects, PropertyAccess, PropertyConstraint,
        PropertyDefault, PropertyDescriptor, RegistryError, RegistryLimits, StageId, StageRegistry,
        TypeDescriptor, TypeId, ValueRef, ValueTag, construct_native_actor,
    },
    direction::{
        ActorDirection, BatchCreateDestination, BatchObjectReference, BatchStageDirection,
        CreateDirection, CreateField, OwnedValue,
    },
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::mpy::CATALOG;

struct TrackingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    static LEAKED_PRECONSTRUCTED_WIDGET: RefCell<Option<Rc<RefCell<dyn Widget>>>> = RefCell::new(None);
}

// SAFETY: operations delegate the original pointer/layout to System; the
// thread-local counters only observe allocator calls made by this test thread.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the layout is forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = DEALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the pointer and layout came from System.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the layout is forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        // SAFETY: all arguments are forwarded under GlobalAlloc's contract.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn count_allocator_operations<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        }
    }

    ALLOCATION_COUNT.with(|count| count.set(0));
    DEALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let guard = Guard;
    let result = operation();
    drop(guard);
    (
        result,
        ALLOCATION_COUNT.with(Cell::get),
        DEALLOCATION_COUNT.with(Cell::get),
    )
}

const BOUNDS: Rect = Rect {
    x: 1,
    y: 2,
    width: 30,
    height: 20,
};

fn descriptor(name: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(name))
        .unwrap()
}

fn field(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .id
}

fn property(descriptor: &TypeDescriptor, name: &str) -> u32 {
    descriptor
        .properties
        .iter()
        .find(|property| property.name == name)
        .unwrap()
        .id
}

fn bounds_field(descriptor: &TypeDescriptor) -> CreateField {
    CreateField {
        id: field(descriptor, "bounds"),
        value: OwnedValue::Rect {
            x: BOUNDS.x,
            y: BOUNDS.y,
            width: BOUNDS.width,
            height: BOUNDS.height,
        },
    }
}

fn create(
    batch_ref: u16,
    descriptor: &TypeDescriptor,
    destination: BatchCreateDestination,
    mut fields: Vec<CreateField>,
) -> BatchStageDirection {
    if descriptor
        .constructor_fields
        .iter()
        .any(|field| field.name == "bounds")
    {
        fields.insert(0, bounds_field(descriptor));
    }
    BatchStageDirection::Create(CreateDirection {
        batch_ref,
        type_id: descriptor.type_id,
        destination,
        fields,
    })
}

fn raw_create(
    batch_ref: u16,
    descriptor: &TypeDescriptor,
    fields: Vec<CreateField>,
) -> BatchStageDirection {
    BatchStageDirection::Create(CreateDirection {
        batch_ref,
        type_id: descriptor.type_id,
        destination: BatchCreateDestination::Root {
            name: "schema".into(),
        },
        fields,
    })
}

fn limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 4,
        max_actors: 12,
        max_tree_depth: 6,
        max_children_per_actor: 6,
        max_text_bytes: 256,
        max_resources: 8,
    }
}

fn registry() -> StageRegistry {
    StageRegistry::new(StageId::new(91).unwrap(), &CATALOG, limits()).unwrap()
}

fn create_stable_container(
    registry: &mut StageRegistry,
    destination: rlvgl_core::actor::CreateDestination<'_>,
) -> rlvgl_core::actor::ObjectId {
    let container = descriptor("container::Container");
    registry
        .create(
            container.type_id,
            destination,
            &[rlvgl_core::actor::ConstructorInput {
                id: field(container, "bounds"),
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

fn create_stable_label(
    registry: &mut StageRegistry,
    parent: rlvgl_core::actor::ObjectId,
) -> rlvgl_core::actor::ObjectId {
    let label = descriptor("label::Label");
    registry
        .create(
            label.type_id,
            rlvgl_core::actor::CreateDestination::Child { parent },
            &[
                rlvgl_core::actor::ConstructorInput {
                    id: field(label, "bounds"),
                    value: ValueRef::Rect {
                        x: BOUNDS.x,
                        y: BOUNDS.y,
                        width: BOUNDS.width,
                        height: BOUNDS.height,
                    },
                },
                rlvgl_core::actor::ConstructorInput {
                    id: field(label, "text"),
                    value: ValueRef::Text("leaf"),
                },
            ],
        )
        .unwrap()
}

#[test]
fn create_children_later_mutation_and_reorder_commit_once_with_owned_outputs() {
    let container = descriptor("container::Container");
    let label = descriptor("label::Label");
    let text_field = field(label, "text");
    let text_property = property(label, "text");
    let mut registry = registry();
    let starting = registry.revision();

    let prepared = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Root {
                    name: "main".into(),
                },
                vec![],
            ),
            create(
                2,
                label,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(1),
                },
                vec![CreateField {
                    id: text_field,
                    value: OwnedValue::Text("old".into()),
                }],
            ),
            create(
                3,
                label,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(1),
                },
                vec![CreateField {
                    id: text_field,
                    value: OwnedValue::Text("sibling".into()),
                }],
            ),
            BatchStageDirection::MutateActor {
                object: BatchObjectReference::EarlierBatch(2),
                directions: vec![ActorDirection::SetProperty {
                    id: text_property,
                    value: OwnedValue::Text("committed".into()),
                }],
            },
            BatchStageDirection::Reorder {
                object: BatchObjectReference::EarlierBatch(2),
                index: 1,
            },
        ])
        .unwrap();
    assert_eq!(prepared.prepared_creates().len(), 3);
    assert_eq!(prepared.prepared_creates()[0].operation_index(), 0);
    assert_eq!(prepared.prepared_creates()[1].batch_ref(), 2);
    assert_eq!(prepared.prepared_creates()[1].type_id(), label.type_id);
    assert_eq!(registry.root_id("main"), None);
    assert_eq!(registry.revision(), starting);

    let (committed, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(prepared));
    assert_eq!((allocations, deallocations), (0, 0));
    let mut committed = committed.unwrap();
    assert_eq!(committed.revision().get(), starting.get() + 1);
    assert_eq!(committed.create_outputs().len(), 3);
    let root = committed.create_outputs()[0].object_id;
    let first = committed.create_outputs()[1].object_id;
    let sibling = committed.create_outputs()[2].object_id;
    assert_eq!(registry.root_id("main"), Some(root));
    assert_eq!(registry.children(root).unwrap(), [sibling, first]);
    assert_eq!(
        registry.property(first, text_property).unwrap(),
        OwnedValue::Text("committed".into())
    );
    assert!(!registry.last_invalidations().is_empty());

    let (outputs, allocations, deallocations) =
        count_allocator_operations(|| committed.take_create_outputs());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(
        outputs
            .iter()
            .map(|output| output.operation_index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(committed.create_outputs().is_empty());
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn invalid_bindings_and_create_then_delete_publish_nothing() {
    let container = descriptor("container::Container");
    for directions in [
        vec![create(
            0,
            container,
            BatchCreateDestination::Root {
                name: "zero".into(),
            },
            vec![],
        )],
        vec![
            create(
                1,
                container,
                BatchCreateDestination::Root { name: "one".into() },
                vec![],
            ),
            create(
                1,
                container,
                BatchCreateDestination::Root {
                    name: "duplicate".into(),
                },
                vec![],
            ),
        ],
        vec![create(
            1,
            container,
            BatchCreateDestination::Child {
                parent: BatchObjectReference::EarlierBatch(2),
            },
            vec![],
        )],
        vec![
            create(
                1,
                container,
                BatchCreateDestination::Root { name: "one".into() },
                vec![],
            ),
            BatchStageDirection::Delete {
                object: BatchObjectReference::EarlierBatch(1),
            },
        ],
    ] {
        let mut registry = registry();
        let revision = registry.revision();
        let usage = registry.usage();
        assert_eq!(
            registry.prepare_atomic_batch(directions).unwrap_err(),
            RegistryError::BatchInvalid
        );
        assert_eq!(registry.revision(), revision);
        assert_eq!(registry.usage(), usage);
    }
}

#[test]
fn deleting_stable_ancestor_of_an_earlier_create_is_prepublication_batch_invalid() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let stable_root = registry
        .create(
            container.type_id,
            rlvgl_core::actor::CreateDestination::Root { name: "stable" },
            &[rlvgl_core::actor::ConstructorInput {
                id: field(container, "bounds"),
                value: ValueRef::Rect {
                    x: BOUNDS.x,
                    y: BOUNDS.y,
                    width: BOUNDS.width,
                    height: BOUNDS.height,
                },
            }],
        )
        .unwrap();
    let revision = registry.revision();
    let usage = registry.usage();

    let error = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::Stable(stable_root),
                },
                vec![],
            ),
            BatchStageDirection::Delete {
                object: BatchObjectReference::Stable(stable_root),
            },
        ])
        .unwrap_err();

    assert_eq!(error, RegistryError::BatchInvalid);
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.usage(), usage);
    assert_eq!(registry.root_id("stable"), Some(stable_root));
    assert!(registry.children(stable_root).unwrap().is_empty());
}

#[test]
fn constructor_schema_errors_precede_contextual_object_resolution() {
    let container = descriptor("container::Container");
    let bounds_id = field(container, "bounds");
    let cases = [
        (
            vec![CreateField {
                id: bounds_id,
                value: OwnedValue::Object(0),
            }],
            RegistryError::TypeMismatch {
                field_id: bounds_id,
                expected: ValueTag::Rect,
                actual: ValueTag::Object,
            },
        ),
        (
            vec![CreateField {
                id: bounds_id,
                value: OwnedValue::BatchObject(0),
            }],
            RegistryError::TypeMismatch {
                field_id: bounds_id,
                expected: ValueTag::Rect,
                actual: ValueTag::BatchObject,
            },
        ),
        (
            vec![CreateField {
                id: 9_999,
                value: OwnedValue::Object(0),
            }],
            RegistryError::UnknownField { field_id: 9_999 },
        ),
        (
            vec![CreateField {
                id: 9_999,
                value: OwnedValue::BatchObject(0),
            }],
            RegistryError::UnknownField { field_id: 9_999 },
        ),
        (
            vec![
                bounds_field(container),
                CreateField {
                    id: bounds_id,
                    value: OwnedValue::Object(0),
                },
            ],
            RegistryError::DuplicateField {
                field_id: bounds_id,
            },
        ),
        (
            vec![],
            RegistryError::MissingField {
                field_id: bounds_id,
            },
        ),
    ];

    for (fields, expected) in cases {
        let mut registry = registry();
        let revision = registry.revision();
        let error = registry
            .prepare_atomic_batch(vec![raw_create(1, container, fields)])
            .unwrap_err();
        assert_eq!(error, expected);
        assert_eq!(registry.revision(), revision);
        assert_eq!(registry.usage().actors, 0);
        assert_eq!(registry.root_id("schema"), None);
    }
}

#[test]
fn augmented_shadow_enforces_actor_root_child_depth_and_text_limits() {
    let container = descriptor("container::Container");
    let label = descriptor("label::Label");
    let cases = [
        (
            RegistryLimits {
                max_actors: 1,
                ..limits()
            },
            vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root { name: "a".into() },
                    vec![],
                ),
                create(
                    2,
                    container,
                    BatchCreateDestination::Root { name: "b".into() },
                    vec![],
                ),
            ],
            CapacityKind::Actors,
        ),
        (
            RegistryLimits {
                max_roots: 1,
                ..limits()
            },
            vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root { name: "a".into() },
                    vec![],
                ),
                create(
                    2,
                    container,
                    BatchCreateDestination::Root { name: "b".into() },
                    vec![],
                ),
            ],
            CapacityKind::Roots,
        ),
        (
            RegistryLimits {
                max_children_per_actor: 1,
                ..limits()
            },
            vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root { name: "a".into() },
                    vec![],
                ),
                create(
                    2,
                    container,
                    BatchCreateDestination::Child {
                        parent: BatchObjectReference::EarlierBatch(1),
                    },
                    vec![],
                ),
                create(
                    3,
                    container,
                    BatchCreateDestination::Child {
                        parent: BatchObjectReference::EarlierBatch(1),
                    },
                    vec![],
                ),
            ],
            CapacityKind::Children,
        ),
        (
            RegistryLimits {
                max_tree_depth: 1,
                ..limits()
            },
            vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root { name: "a".into() },
                    vec![],
                ),
                create(
                    2,
                    container,
                    BatchCreateDestination::Child {
                        parent: BatchObjectReference::EarlierBatch(1),
                    },
                    vec![],
                ),
            ],
            CapacityKind::TreeDepth,
        ),
        (
            RegistryLimits {
                max_text_bytes: 2,
                ..limits()
            },
            vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root { name: "a".into() },
                    vec![],
                ),
                create(
                    2,
                    label,
                    BatchCreateDestination::Child {
                        parent: BatchObjectReference::EarlierBatch(1),
                    },
                    vec![CreateField {
                        id: field(label, "text"),
                        value: OwnedValue::Text("too long".into()),
                    }],
                ),
            ],
            CapacityKind::TextBytes,
        ),
    ];

    for (limits, directions, kind) in cases {
        let mut registry = StageRegistry::new(StageId::new(93).unwrap(), &CATALOG, limits).unwrap();
        assert_eq!(
            registry.prepare_atomic_batch(directions).unwrap_err(),
            RegistryError::Capacity { kind }
        );
        assert_eq!(registry.usage().actors, 0);
        assert_eq!(registry.revision().get(), 0);
    }
}

#[test]
fn delete_then_create_reuses_the_slot_at_the_next_generation_and_stale_commit_is_clean() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let old = registry
        .create(
            container.type_id,
            rlvgl_core::actor::CreateDestination::Root { name: "old" },
            &[rlvgl_core::actor::ConstructorInput {
                id: field(container, "bounds"),
                value: ValueRef::Rect {
                    x: BOUNDS.x,
                    y: BOUNDS.y,
                    width: BOUNDS.width,
                    height: BOUNDS.height,
                },
            }],
        )
        .unwrap();
    let prepared = registry
        .prepare_atomic_batch(vec![
            BatchStageDirection::Delete {
                object: BatchObjectReference::Stable(old),
            },
            create(
                9,
                container,
                BatchCreateDestination::Root { name: "new".into() },
                vec![],
            ),
        ])
        .unwrap();
    let mut committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(committed.create_outputs().len(), 1);
    assert_eq!(committed.create_outputs()[0].operation_index, 1);
    assert_eq!(committed.create_outputs()[0].batch_ref, 9);
    let new = committed.take_create_outputs().pop().unwrap().object_id;
    assert_eq!(new.slot(), old.slot());
    assert_eq!(new.generation(), old.generation() + 1);
    assert_eq!(registry.root_id("new"), Some(new));
    registry.release_committed_batch(committed).unwrap();

    let stale = registry
        .prepare_atomic_batch(vec![create(
            10,
            container,
            BatchCreateDestination::Root {
                name: "later".into(),
            },
            vec![],
        )])
        .unwrap();
    registry
        .apply_batch(&[rlvgl_core::direction::StageDirection::SetFlag {
            object_id: new,
            flag: rlvgl_core::direction::RuntimeFlag::Hidden,
            enabled: true,
        }])
        .unwrap();
    let revision = registry.revision();
    let error = registry.commit_prepared_batch(stale).unwrap_err();
    assert_eq!(error.cause(), RegistryError::BatchInvalid);
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.root_id("later"), None);
}

#[test]
fn stable_subtree_delete_commits_child_first_without_create_outputs() {
    let mut registry = registry();
    let root = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "main" },
    );
    let branch = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent: root },
    );
    let leaf = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent: branch },
    );
    let unrelated = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "unrelated" },
    );
    let starting_revision = registry.revision();

    let prepared = registry
        .prepare_atomic_batch(vec![BatchStageDirection::Delete {
            object: BatchObjectReference::Stable(root),
        }])
        .unwrap();
    assert_eq!(prepared.deleted_object_ids(), [leaf, branch, root]);
    assert!(prepared.prepared_creates().is_empty());

    let committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert_eq!(committed.deleted_object_ids(), [leaf, branch, root]);
    assert!(committed.create_outputs().is_empty());
    for object_id in [root, branch, leaf] {
        assert_eq!(
            registry.actor_info(object_id),
            Err(RegistryError::StaleObject { object_id })
        );
    }
    assert!(registry.actor_info(unrelated).is_ok());
    assert_eq!(registry.root_id("main"), None);
    assert_eq!(registry.root_id("unrelated"), Some(unrelated));
    assert!(registry.children(unrelated).unwrap().is_empty());
    assert_eq!(registry.usage().actors, 1);
    assert_eq!(registry.usage().roots, 1);

    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn earlier_created_reorders_use_final_indices_and_preserve_ownership() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let first = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let second = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let alpha = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "alpha" },
    );
    let omega = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "omega" },
    );
    let starting_revision = registry.revision();

    let prepared = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::Stable(parent),
                },
                vec![],
            ),
            BatchStageDirection::Reorder {
                object: BatchObjectReference::EarlierBatch(1),
                index: 1,
            },
            create(
                2,
                container,
                BatchCreateDestination::Root {
                    name: "created".into(),
                },
                vec![],
            ),
            BatchStageDirection::Reorder {
                object: BatchObjectReference::EarlierBatch(2),
                index: 1,
            },
        ])
        .unwrap();
    let mut committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert_eq!(committed.create_outputs().len(), 2);
    assert_eq!(committed.create_outputs()[0].operation_index, 0);
    assert_eq!(committed.create_outputs()[0].batch_ref, 1);
    assert_eq!(committed.create_outputs()[1].operation_index, 2);
    assert_eq!(committed.create_outputs()[1].batch_ref, 2);
    let outputs = committed.take_create_outputs();
    let created_child = outputs[0].object_id;
    let created_root = outputs[1].object_id;

    assert_eq!(
        registry.children(parent).unwrap(),
        [first, created_child, second]
    );
    assert_eq!(
        registry.actor_info(created_child).unwrap().parent,
        Some(parent)
    );
    assert_eq!(registry.root_id("created"), Some(created_root));
    let token = registry.snapshot_begin().unwrap();
    let page = registry
        .snapshot_read(
            token,
            limits().max_actors,
            usize::try_from(limits().max_text_bytes).unwrap(),
        )
        .unwrap();
    assert!(page.ended);
    assert_eq!(
        page.records
            .iter()
            .filter(|record| record.parent.is_none())
            .map(|record| record.object_id)
            .collect::<Vec<_>>(),
        [parent, created_root, alpha, omega]
    );

    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn valid_noop_reorder_commits_once_without_outputs() {
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let first = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let second = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let prepared = registry
        .prepare_atomic_batch(vec![BatchStageDirection::Reorder {
            object: BatchObjectReference::Stable(second),
            index: 1,
        }])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();

    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert!(committed.create_outputs().is_empty());
    assert_eq!(registry.children(parent).unwrap(), [first, second]);
    assert_eq!(registry.actor_info(second).unwrap().parent, Some(parent));
    assert_eq!(registry.usage(), starting_usage);
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn out_of_range_earlier_created_reorder_rejects_without_publication() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let first = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let second = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();
    let starting_order = registry.children(parent).unwrap();

    let error = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::Stable(parent),
                },
                vec![],
            ),
            BatchStageDirection::Reorder {
                object: BatchObjectReference::EarlierBatch(1),
                index: 3,
            },
        ])
        .unwrap_err();

    assert_eq!(error, RegistryError::Range { field_id: 0 });
    assert_eq!(registry.revision(), starting_revision);
    assert_eq!(registry.usage(), starting_usage);
    assert_eq!(registry.children(parent).unwrap(), starting_order);
    assert_eq!(registry.children(parent).unwrap(), [first, second]);
}

#[test]
fn earlier_created_reparents_use_detach_first_indices_and_emit_only_create_outputs() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let starting_revision = registry.revision();
    let prepared = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Root {
                    name: "left".into(),
                },
                vec![],
            ),
            create(
                2,
                container,
                BatchCreateDestination::Root {
                    name: "right".into(),
                },
                vec![],
            ),
            create(
                3,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(1),
                },
                vec![],
            ),
            create(
                4,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(2),
                },
                vec![],
            ),
            create(
                5,
                container,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(2),
                },
                vec![],
            ),
            BatchStageDirection::Reparent {
                object: BatchObjectReference::EarlierBatch(3),
                new_parent: BatchObjectReference::EarlierBatch(2),
                index: 1,
            },
            BatchStageDirection::Reparent {
                object: BatchObjectReference::EarlierBatch(4),
                new_parent: BatchObjectReference::EarlierBatch(2),
                index: 2,
            },
            create(
                6,
                container,
                BatchCreateDestination::Root {
                    name: "moving".into(),
                },
                vec![],
            ),
            BatchStageDirection::Reparent {
                object: BatchObjectReference::EarlierBatch(6),
                new_parent: BatchObjectReference::EarlierBatch(2),
                index: 0,
            },
        ])
        .unwrap();
    let mut committed = registry.commit_prepared_batch(prepared).unwrap();

    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert_eq!(committed.create_outputs().len(), 6);
    assert_eq!(
        committed
            .create_outputs()
            .iter()
            .map(|output| (output.operation_index, output.batch_ref))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (7, 6)]
    );
    let outputs = committed.take_create_outputs();
    let left = outputs[0].object_id;
    let right = outputs[1].object_id;
    let moved_cross_parent = outputs[2].object_id;
    let moved_same_parent = outputs[3].object_id;
    let retained_sibling = outputs[4].object_id;
    let former_root = outputs[5].object_id;

    assert!(registry.children(left).unwrap().is_empty());
    assert_eq!(
        registry.children(right).unwrap(),
        [
            former_root,
            moved_cross_parent,
            retained_sibling,
            moved_same_parent,
        ]
    );
    for object_id in [moved_cross_parent, moved_same_parent, former_root] {
        assert_eq!(registry.actor_info(object_id).unwrap().parent, Some(right));
    }
    assert_eq!(registry.root_id("left"), Some(left));
    assert_eq!(registry.root_id("right"), Some(right));
    assert_eq!(registry.root_id("moving"), None);

    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn valid_structural_noop_reparent_commits_one_revision() {
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let first = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let second = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let prepared = registry
        .prepare_atomic_batch(vec![BatchStageDirection::Reparent {
            object: BatchObjectReference::Stable(second),
            new_parent: BatchObjectReference::Stable(parent),
            index: 1,
        }])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();

    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert!(committed.create_outputs().is_empty());
    assert_eq!(registry.children(parent).unwrap(), [first, second]);
    assert_eq!(registry.actor_info(second).unwrap().parent, Some(parent));
    assert_eq!(registry.usage(), starting_usage);
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn reparent_resolves_target_before_new_parent_without_publication() {
    let mut registry = registry();
    let target = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "target" },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let error = registry
        .prepare_atomic_batch(vec![
            BatchStageDirection::Delete {
                object: BatchObjectReference::Stable(target),
            },
            BatchStageDirection::Reparent {
                object: BatchObjectReference::Stable(target),
                new_parent: BatchObjectReference::EarlierBatch(0),
                index: usize::MAX,
            },
        ])
        .unwrap_err();

    assert_eq!(error, RegistryError::StaleObject { object_id: target });
    assert_eq!(registry.revision(), starting_revision);
    assert_eq!(registry.usage(), starting_usage);
    assert_eq!(registry.root_id("target"), Some(target));
}

#[test]
fn reparent_self_cycle_and_policy_failures_are_prepublication() {
    let mut registry = registry();
    let root = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "root" },
    );
    let child = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Child { parent: root },
    );
    let unrelated = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "unrelated" },
    );
    let leaf_parent = create_stable_label(&mut registry, root);
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();
    let starting_children = registry.children(root).unwrap();

    for direction in [
        BatchStageDirection::Reparent {
            object: BatchObjectReference::Stable(root),
            new_parent: BatchObjectReference::Stable(root),
            index: 0,
        },
        BatchStageDirection::Reparent {
            object: BatchObjectReference::Stable(root),
            new_parent: BatchObjectReference::Stable(child),
            index: 0,
        },
        BatchStageDirection::Reparent {
            object: BatchObjectReference::Stable(unrelated),
            new_parent: BatchObjectReference::Stable(leaf_parent),
            index: 0,
        },
    ] {
        assert_eq!(
            registry.prepare_atomic_batch(vec![direction]).unwrap_err(),
            RegistryError::InvalidParent
        );
        assert_eq!(registry.revision(), starting_revision);
        assert_eq!(registry.usage(), starting_usage);
        assert_eq!(registry.children(root).unwrap(), starting_children);
        assert_eq!(registry.root_id("root"), Some(root));
        assert_eq!(registry.root_id("unrelated"), Some(unrelated));
    }
}

#[test]
fn reparent_depth_child_capacity_and_index_failures_are_prepublication() {
    let mut depth_registry = registry();
    let depth_root = create_stable_container(
        &mut depth_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "depth" },
    );
    let mut deepest = depth_root;
    for _ in 1..limits().max_tree_depth {
        deepest = create_stable_container(
            &mut depth_registry,
            rlvgl_core::actor::CreateDestination::Child { parent: deepest },
        );
    }
    let depth_target = create_stable_container(
        &mut depth_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "target" },
    );
    let depth_revision = depth_registry.revision();
    let depth_usage = depth_registry.usage();
    assert_eq!(
        depth_registry
            .prepare_atomic_batch(vec![BatchStageDirection::Reparent {
                object: BatchObjectReference::Stable(depth_target),
                new_parent: BatchObjectReference::Stable(deepest),
                index: 0,
            }])
            .unwrap_err(),
        RegistryError::Capacity {
            kind: CapacityKind::TreeDepth,
        }
    );
    assert_eq!(depth_registry.revision(), depth_revision);
    assert_eq!(depth_registry.usage(), depth_usage);
    assert_eq!(depth_registry.root_id("target"), Some(depth_target));
    assert!(depth_registry.children(deepest).unwrap().is_empty());

    let mut capacity_registry = registry();
    let capacity_parent = create_stable_container(
        &mut capacity_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    for _ in 0..limits().max_children_per_actor {
        create_stable_container(
            &mut capacity_registry,
            rlvgl_core::actor::CreateDestination::Child {
                parent: capacity_parent,
            },
        );
    }
    let capacity_target = create_stable_container(
        &mut capacity_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "target" },
    );
    let capacity_revision = capacity_registry.revision();
    let capacity_usage = capacity_registry.usage();
    let capacity_children = capacity_registry.children(capacity_parent).unwrap();
    assert_eq!(
        capacity_registry
            .prepare_atomic_batch(vec![BatchStageDirection::Reparent {
                object: BatchObjectReference::Stable(capacity_target),
                new_parent: BatchObjectReference::Stable(capacity_parent),
                index: capacity_children.len(),
            }])
            .unwrap_err(),
        RegistryError::Capacity {
            kind: CapacityKind::Children,
        }
    );
    assert_eq!(capacity_registry.revision(), capacity_revision);
    assert_eq!(capacity_registry.usage(), capacity_usage);
    assert_eq!(
        capacity_registry.children(capacity_parent).unwrap(),
        capacity_children
    );
    assert_eq!(capacity_registry.root_id("target"), Some(capacity_target));

    let mut index_registry = registry();
    let index_parent = create_stable_container(
        &mut index_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let index_child = create_stable_container(
        &mut index_registry,
        rlvgl_core::actor::CreateDestination::Child {
            parent: index_parent,
        },
    );
    let index_target = create_stable_container(
        &mut index_registry,
        rlvgl_core::actor::CreateDestination::Root { name: "target" },
    );
    let index_revision = index_registry.revision();
    let index_usage = index_registry.usage();
    assert_eq!(
        index_registry
            .prepare_atomic_batch(vec![BatchStageDirection::Reparent {
                object: BatchObjectReference::Stable(index_target),
                new_parent: BatchObjectReference::Stable(index_parent),
                index: 2,
            }])
            .unwrap_err(),
        RegistryError::Range { field_id: 0 }
    );
    assert_eq!(index_registry.revision(), index_revision);
    assert_eq!(index_registry.usage(), index_usage);
    assert_eq!(
        index_registry.children(index_parent).unwrap(),
        [index_child]
    );
    assert_eq!(index_registry.root_id("target"), Some(index_target));
}

#[test]
fn same_parent_reparent_succeeds_at_child_capacity_after_detach() {
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let mut children = Vec::with_capacity(limits().max_children_per_actor);
    for _ in 0..limits().max_children_per_actor {
        children.push(create_stable_container(
            &mut registry,
            rlvgl_core::actor::CreateDestination::Child { parent },
        ));
    }
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();
    let moved = children[0];

    let prepared = registry
        .prepare_atomic_batch(vec![BatchStageDirection::Reparent {
            object: BatchObjectReference::Stable(moved),
            new_parent: BatchObjectReference::Stable(parent),
            index: limits().max_children_per_actor - 1,
        }])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();

    children.rotate_left(1);
    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert!(committed.create_outputs().is_empty());
    assert_eq!(registry.children(parent).unwrap(), children);
    assert_eq!(registry.actor_info(moved).unwrap().parent, Some(parent));
    assert_eq!(registry.root_id("parent"), Some(parent));
    assert_eq!(registry.usage(), starting_usage);
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn root_to_child_reparent_releases_name_accounting_for_same_batch_reuse() {
    let container = descriptor("container::Container");
    let mut registry = registry();
    let parent = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "parent" },
    );
    let former_named_root = create_stable_container(
        &mut registry,
        rlvgl_core::actor::CreateDestination::Root { name: "moving" },
    );
    let starting_revision = registry.revision();
    let starting_usage = registry.usage();

    let prepared = registry
        .prepare_atomic_batch(vec![
            BatchStageDirection::Reparent {
                object: BatchObjectReference::Stable(former_named_root),
                new_parent: BatchObjectReference::Stable(parent),
                index: 0,
            },
            create(
                1,
                container,
                BatchCreateDestination::Root {
                    name: "moving".into(),
                },
                vec![],
            ),
        ])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    let replacement_root = committed.create_outputs()[0].object_id;

    assert_eq!(committed.revision().get(), starting_revision.get() + 1);
    assert_eq!(committed.create_outputs().len(), 1);
    assert_eq!(committed.create_outputs()[0].operation_index, 1);
    assert_eq!(committed.create_outputs()[0].batch_ref, 1);
    assert_ne!(replacement_root, former_named_root);
    assert_eq!(registry.root_id("moving"), Some(replacement_root));
    assert_eq!(
        registry.actor_info(former_named_root).unwrap().parent,
        Some(parent)
    );
    assert_eq!(registry.children(parent).unwrap(), [former_named_root]);
    assert_eq!(registry.usage().actors, starting_usage.actors + 1);
    assert_eq!(registry.usage().roots, starting_usage.roots);
    assert_eq!(registry.usage().text_bytes, starting_usage.text_bytes);
    registry.release_committed_batch(committed).unwrap();
}

const REFERENCE_TYPE: TypeId = TypeId::registered(0x0001_ff01);
const REFERENCE_FIELD: u32 = 77;
const REFERENCE_PROPERTIES: [PropertyDescriptor; 1] = [PropertyDescriptor {
    id: REFERENCE_FIELD,
    name: "reference",
    value_tag: ValueTag::Object,
    access: PropertyAccess::ReadOnly,
    default: PropertyDefault::Absent,
    constraint: PropertyConstraint::None,
    required_capabilities: ActorCapabilities::EMPTY,
    effects: MutationEffects::NONE,
}];

struct ReferenceActor {
    reference: u64,
}

impl Widget for ReferenceActor {
    fn bounds(&self) -> Rect {
        BOUNDS
    }

    fn draw(&self, _renderer: &mut dyn Renderer) {}

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

impl MpyActor for ReferenceActor {
    type Prepared = ();

    fn property(&self, id: u32) -> Result<OwnedValue, RegistryError> {
        if id == REFERENCE_FIELD {
            Ok(OwnedValue::Object(self.reference))
        } else {
            Err(RegistryError::UnknownProperty { property_id: id })
        }
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

fn construct_reference(args: ConstructorArgs<'_>) -> Result<ConstructedActor, RegistryError> {
    let reference = match args.get(REFERENCE_FIELD) {
        Some(ValueRef::Object(reference)) => reference,
        Some(value) => {
            return Err(RegistryError::TypeMismatch {
                field_id: REFERENCE_FIELD,
                expected: ValueTag::Object,
                actual: value.tag(),
            });
        }
        None => {
            return Err(RegistryError::MissingField {
                field_id: REFERENCE_FIELD,
            });
        }
    };
    let constructed = construct_native_actor(REFERENCE_TYPE, ReferenceActor { reference });
    LEAKED_PRECONSTRUCTED_WIDGET.with(|leaked| {
        *leaked.borrow_mut() = Some(constructed.node().widget().clone());
    });
    Ok(constructed)
}

const REFERENCE_DESCRIPTOR: TypeDescriptor = TypeDescriptor {
    type_id: REFERENCE_TYPE,
    stable_name: "tests::ReferenceActor",
    schema_revision: 1,
    constructor_fields: &[ConstructorFieldDescriptor {
        id: REFERENCE_FIELD,
        name: "reference",
        value_tag: ValueTag::Object,
        required: true,
    }],
    properties: &REFERENCE_PROPERTIES,
    actions: &[],
    events: &[],
    constructor: construct_reference,
    ..rlvgl_widgets::container::MPY_DESCRIPTOR
};

static REFERENCE_CATALOG: [TypeDescriptor; 2] = [
    rlvgl_widgets::container::MPY_DESCRIPTOR,
    REFERENCE_DESCRIPTOR,
];

fn reference_registry() -> StageRegistry {
    StageRegistry::new(StageId::new(92).unwrap(), &REFERENCE_CATALOG, limits()).unwrap()
}

#[test]
fn constructor_object_values_resolve_stable_and_earlier_batch_references() {
    let container = &REFERENCE_CATALOG[0];
    let reference = &REFERENCE_CATALOG[1];
    let mut registry = reference_registry();
    let prepared = registry
        .prepare_atomic_batch(vec![
            create(
                1,
                container,
                BatchCreateDestination::Root {
                    name: "main".into(),
                },
                vec![],
            ),
            create(
                2,
                reference,
                BatchCreateDestination::Child {
                    parent: BatchObjectReference::EarlierBatch(1),
                },
                vec![CreateField {
                    id: REFERENCE_FIELD,
                    value: OwnedValue::BatchObject(1),
                }],
            ),
        ])
        .unwrap();
    let mut committed = registry.commit_prepared_batch(prepared).unwrap();
    let outputs = committed.take_create_outputs();
    assert_eq!(
        registry
            .property(outputs[1].object_id, REFERENCE_FIELD)
            .unwrap(),
        OwnedValue::Object(outputs[0].object_id.get())
    );
    registry.release_committed_batch(committed).unwrap();

    let stable = outputs[0].object_id;
    let prepared = registry
        .prepare_atomic_batch(vec![create(
            3,
            reference,
            BatchCreateDestination::Child {
                parent: BatchObjectReference::Stable(stable),
            },
            vec![CreateField {
                id: REFERENCE_FIELD,
                value: OwnedValue::Object(stable.get()),
            }],
        )])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(
        registry
            .property(committed.create_outputs()[0].object_id, REFERENCE_FIELD)
            .unwrap(),
        OwnedValue::Object(stable.get())
    );
    registry.release_committed_batch(committed).unwrap();
}

#[test]
fn constructor_object_reference_errors_are_prepublication_failures() {
    let container = &REFERENCE_CATALOG[0];
    let reference = &REFERENCE_CATALOG[1];
    for value in [
        OwnedValue::BatchObject(0),
        OwnedValue::BatchObject(2),
        OwnedValue::Object(0),
    ] {
        let mut registry = reference_registry();
        let error = registry
            .prepare_atomic_batch(vec![
                create(
                    1,
                    container,
                    BatchCreateDestination::Root {
                        name: "main".into(),
                    },
                    vec![],
                ),
                create(
                    3,
                    reference,
                    BatchCreateDestination::Child {
                        parent: BatchObjectReference::EarlierBatch(1),
                    },
                    vec![CreateField {
                        id: REFERENCE_FIELD,
                        value,
                    }],
                ),
            ])
            .unwrap_err();
        assert_eq!(error, RegistryError::BatchInvalid);
        assert_eq!(registry.usage().actors, 0);
        assert_eq!(registry.root_id("main"), None);
    }

    let mut registry = reference_registry();
    let victim = registry
        .create(
            container.type_id,
            rlvgl_core::actor::CreateDestination::Root { name: "victim" },
            &[rlvgl_core::actor::ConstructorInput {
                id: field(container, "bounds"),
                value: ValueRef::Rect {
                    x: BOUNDS.x,
                    y: BOUNDS.y,
                    width: BOUNDS.width,
                    height: BOUNDS.height,
                },
            }],
        )
        .unwrap();
    let revision = registry.revision();
    let error = registry
        .prepare_atomic_batch(vec![
            BatchStageDirection::Delete {
                object: BatchObjectReference::Stable(victim),
            },
            create(
                4,
                reference,
                BatchCreateDestination::Root {
                    name: "reference".into(),
                },
                vec![CreateField {
                    id: REFERENCE_FIELD,
                    value: OwnedValue::Object(victim.get()),
                }],
            ),
        ])
        .unwrap_err();
    assert_eq!(error, RegistryError::StaleObject { object_id: victim });
    assert_eq!(registry.revision(), revision);
    assert_eq!(registry.root_id("victim"), Some(victim));
}

#[test]
fn retained_preconstructed_actor_borrow_rejects_without_publication_and_retries() {
    let container = &REFERENCE_CATALOG[0];
    let reference = &REFERENCE_CATALOG[1];
    let mut registry = reference_registry();
    let stable = registry
        .create(
            container.type_id,
            rlvgl_core::actor::CreateDestination::Root { name: "main" },
            &[rlvgl_core::actor::ConstructorInput {
                id: field(container, "bounds"),
                value: ValueRef::Rect {
                    x: BOUNDS.x,
                    y: BOUNDS.y,
                    width: BOUNDS.width,
                    height: BOUNDS.height,
                },
            }],
        )
        .unwrap();
    let prepared = registry
        .prepare_atomic_batch(vec![create(
            1,
            reference,
            BatchCreateDestination::Child {
                parent: BatchObjectReference::Stable(stable),
            },
            vec![CreateField {
                id: REFERENCE_FIELD,
                value: OwnedValue::Object(stable.get()),
            }],
        )])
        .unwrap();
    let widget = LEAKED_PRECONSTRUCTED_WIDGET.with(|leaked| leaked.borrow().clone().unwrap());
    let borrow = widget.borrow_mut();
    let revision = registry.revision();

    let (error, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(prepared).unwrap_err());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(error.cause(), RegistryError::DispatchBusy);
    assert_eq!(registry.revision(), revision);
    assert!(registry.children(stable).unwrap().is_empty());

    drop(borrow);
    let (committed, allocations, deallocations) =
        count_allocator_operations(|| registry.commit_prepared_batch(error.into_prepared()));
    assert_eq!((allocations, deallocations), (0, 0));
    let committed = committed.unwrap();
    assert_eq!(registry.children(stable).unwrap().len(), 1);
    registry.release_committed_batch(committed).unwrap();
}
