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
