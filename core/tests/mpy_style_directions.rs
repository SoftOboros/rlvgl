//! MPY-04 SetLocalStyle and bounded sparse-style snapshot conformance.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    ptr,
};

use rlvgl_core::{
    actor::{
        CapacityKind, ConstructorInput, CreateDestination, MutationEffects, RegistryError,
        RegistryLimits, STYLE_PROPERTIES, StageId, StageRegistry, StyleValueConstraint,
        TypeDescriptor, ValueRef,
    },
    direction::{
        BatchCreateDestination, BatchObjectReference, BatchStageDirection, CreateDirection,
        CreateField, OwnedValue, SnapshotError, StageDirection,
    },
    object::ObjectStates,
    widget::Rect,
};
use rlvgl_widgets::mpy::CATALOG;

struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static FAIL_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
}

fn allocation_must_fail() -> bool {
    FAIL_AFTER.with(|remaining| match remaining.get() {
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
        None => false,
    })
}

// SAFETY: successful operations delegate unchanged pointers and layouts to
// System. The opt-in failure seam returns null only to fallible Vec reservation
// exercised by one test thread.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if allocation_must_fail() {
            return ptr::null_mut();
        }
        if TRACKING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the caller-provided layout is forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACKING.with(Cell::get) {
            DEALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the pointer and layout came from System.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if allocation_must_fail() {
            return ptr::null_mut();
        }
        if TRACKING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the caller-provided layout is forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if allocation_must_fail() {
            return ptr::null_mut();
        }
        if TRACKING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: all arguments are forwarded under GlobalAlloc's contract.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }

    ALLOCATIONS.with(|count| count.set(0));
    DEALLOCATIONS.with(|count| count.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    let guard = Guard;
    let result = operation();
    drop(guard);
    (
        result,
        ALLOCATIONS.with(Cell::get),
        DEALLOCATIONS.with(Cell::get),
    )
}

const BOUNDS: Rect = Rect {
    x: 3,
    y: 5,
    width: 80,
    height: 40,
};

fn limits() -> RegistryLimits {
    RegistryLimits {
        max_roots: 4,
        max_actors: 8,
        max_tree_depth: 4,
        max_children_per_actor: 4,
        max_text_bytes: 256,
        max_resources: 8,
    }
}

fn registry() -> StageRegistry {
    StageRegistry::new(StageId::new(151).unwrap(), &CATALOG, limits()).unwrap()
}

fn descriptor(suffix: &str) -> &'static TypeDescriptor {
    CATALOG
        .iter()
        .find(|descriptor| descriptor.stable_name.ends_with(suffix))
        .unwrap()
}

fn bounds_field(descriptor: &TypeDescriptor) -> u32 {
    descriptor
        .constructor_fields
        .iter()
        .find(|field| field.name == "bounds")
        .unwrap()
        .id
}

fn create_root(
    registry: &mut StageRegistry,
    suffix: &str,
    name: &str,
) -> rlvgl_core::actor::ObjectId {
    let descriptor = descriptor(suffix);
    registry
        .create(
            descriptor.type_id,
            CreateDestination::Root { name },
            &[ConstructorInput {
                id: bounds_field(descriptor),
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

fn create_direction(batch_ref: u16, suffix: &str, name: &str) -> BatchStageDirection {
    let descriptor = descriptor(suffix);
    BatchStageDirection::Create(CreateDirection {
        batch_ref,
        type_id: descriptor.type_id,
        destination: BatchCreateDestination::Root { name: name.into() },
        fields: vec![CreateField {
            id: bounds_field(descriptor),
            value: OwnedValue::Rect {
                x: BOUNDS.x,
                y: BOUNDS.y,
                width: BOUNDS.width,
                height: BOUNDS.height,
            },
        }],
    })
}

fn style_value(property_id: u32) -> OwnedValue {
    let property = &STYLE_PROPERTIES[property_id as usize - 1];
    match property.constraint {
        StyleValueConstraint::None => OwnedValue::Color(0xff10_0000 | property_id),
        StyleValueConstraint::U32 { min, .. } => OwnedValue::U32(min),
        StyleValueConstraint::I32 { min, max } => {
            OwnedValue::I32(if min <= 0 && max >= 0 { 0 } else { min })
        }
        StyleValueConstraint::Enum { domain_id, values } => OwnedValue::Enum {
            domain: domain_id,
            value: values[0],
        },
    }
}

fn stable_style(
    object_id: rlvgl_core::actor::ObjectId,
    state_mask: u32,
    property_id: u32,
    value: OwnedValue,
) -> BatchStageDirection {
    BatchStageDirection::SetLocalStyle {
        object: BatchObjectReference::Stable(object_id),
        part_id: 0,
        state_mask,
        property_id,
        value,
    }
}

fn snapshot(
    registry: &mut StageRegistry,
    style_limit: usize,
) -> rlvgl_core::direction::SnapshotRecord {
    let token = registry.snapshot_begin().unwrap();
    let page = registry
        .snapshot_read_with_style_limit(token, 8, 1024, style_limit)
        .unwrap();
    assert!(page.ended);
    assert_eq!(page.records.len(), 1);
    page.records.into_iter().next().unwrap()
}

#[test]
fn all_properties_commit_in_one_group_and_snapshot_in_property_order() {
    let mut registry = registry();
    let root = create_root(&mut registry, "Container", "main");
    let before = registry.revision();
    let directions = (1..=20)
        .rev()
        .map(|property_id| stable_style(root, 0, property_id, style_value(property_id)))
        .collect();
    let prepared = registry.prepare_atomic_batch(directions).unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(committed.revision().get(), before.get() + 1);
    assert!(committed.create_outputs().is_empty());
    assert!(committed.requested_layout_outputs().is_empty());
    registry.release_committed_batch(committed).unwrap();
    assert_eq!(
        registry.last_commit_effects(),
        MutationEffects::DRAW
            .union(MutationEffects::LAYOUT)
            .union(MutationEffects::SNAPSHOT)
    );
    assert_eq!(registry.last_invalidations(), &[BOUNDS]);

    let record = snapshot(&mut registry, 20);
    assert_eq!(record.total_style_values, 20);
    assert!(!record.styles_truncated);
    assert!(!record.truncated);
    assert_eq!(
        record
            .styles
            .iter()
            .map(|style| style.property_id)
            .collect::<Vec<_>>(),
        (1..=20).collect::<Vec<_>>()
    );
    assert!(
        record
            .styles
            .iter()
            .all(|style| style.part_id == 0 && style.state_mask == 0)
    );
    for style in &record.styles {
        assert_eq!(style.value, style_value(style.property_id));
    }
}

#[test]
fn exact_selectors_preserve_zero_and_remove_only_one_sparse_property() {
    let mut registry = registry();
    let root = create_root(&mut registry, "Container", "container");
    let directions = vec![
        stable_style(
            root,
            ObjectStates::DISABLED.bits(),
            2,
            OwnedValue::Color(0xff00_0022),
        ),
        stable_style(root, 0, 4, OwnedValue::U32(0)),
        stable_style(root, 0, 1, OwnedValue::Color(0xff00_0011)),
        stable_style(root, 0, 1, OwnedValue::Color(0xff00_0033)),
        stable_style(root, 0, 2, OwnedValue::None),
    ];
    let prepared = registry.prepare_atomic_batch(directions).unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    registry.release_committed_batch(committed).unwrap();

    let record = snapshot(&mut registry, 8);
    assert_eq!(record.total_style_values, 3);
    assert_eq!(record.styles[0].state_mask, ObjectStates::DISABLED.bits());
    assert_eq!(record.styles[0].property_id, 2);
    assert_eq!(record.styles[1].state_mask, 0);
    assert_eq!(record.styles[1].property_id, 1);
    assert_eq!(record.styles[1].value, OwnedValue::Color(0xff00_0033));
    assert_eq!(record.styles[2].property_id, 4);
    assert_eq!(record.styles[2].value, OwnedValue::U32(0));

    let prepared = registry
        .prepare_batch(vec![StageDirection::SetLocalStyle {
            object_id: root,
            part_id: 0,
            state_mask: 0,
            property_id: 1,
            value: OwnedValue::None,
        }])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    registry.release_committed_batch(committed).unwrap();
    let record = snapshot(&mut registry, 8);
    assert_eq!(record.total_style_values, 2);
    assert_eq!(record.styles[0].state_mask, ObjectStates::DISABLED.bits());
    assert_eq!(record.styles[1].value, OwnedValue::U32(0));
}

#[test]
fn earlier_create_style_is_published_atomically_and_only_create_has_output() {
    let mut registry = registry();
    let prepared = registry
        .prepare_atomic_batch(vec![
            create_direction(7, "Container", "created"),
            BatchStageDirection::SetLocalStyle {
                object: BatchObjectReference::EarlierBatch(7),
                part_id: 0,
                state_mask: ObjectStates::DISABLED.bits(),
                property_id: 1,
                value: OwnedValue::Color(0xff12_3456),
            },
        ])
        .unwrap();
    assert!(prepared.prepared_creates().len() == 1);
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    assert_eq!(committed.create_outputs().len(), 1);
    assert_eq!(committed.create_outputs()[0].operation_index, 0);
    assert!(committed.requested_layout_outputs().is_empty());
    let object_id = committed.create_outputs()[0].object_id;
    registry.release_committed_batch(committed).unwrap();

    let record = snapshot(&mut registry, 4);
    assert_eq!(record.object_id, object_id);
    assert_eq!(record.styles.len(), 1);
    assert_eq!(record.styles[0].state_mask, ObjectStates::DISABLED.bits());
    assert_eq!(record.styles[0].value, OwnedValue::Color(0xff12_3456));
}

#[test]
fn target_selector_property_and_value_errors_precede_without_partial_publication() {
    let mut registry = registry();
    let root = create_root(&mut registry, "Container", "main");
    let starting = registry.revision();
    let stale = rlvgl_core::actor::ObjectId::new(u64::MAX).unwrap();
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![stable_style(stale, 1 << 31, 0, OwnedValue::I32(1),)])
            .unwrap_err(),
        RegistryError::StaleObject { object_id: stale }
    );
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![stable_style(root, 1 << 31, 0, OwnedValue::I32(1),)])
            .unwrap_err(),
        RegistryError::Unsupported
    );
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![stable_style(root, 0, 99, OwnedValue::I32(1))])
            .unwrap_err(),
        RegistryError::UnknownProperty { property_id: 99 }
    );
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![stable_style(root, 0, 4, OwnedValue::I32(1))])
            .unwrap_err(),
        RegistryError::TypeMismatch {
            field_id: 4,
            expected: rlvgl_core::actor::ValueTag::U32,
            actual: rlvgl_core::actor::ValueTag::I32,
        }
    );
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![stable_style(root, 0, 4, OwnedValue::U32(256))])
            .unwrap_err(),
        RegistryError::Range { field_id: 4 }
    );
    assert_eq!(
        registry
            .prepare_atomic_batch(vec![
                stable_style(root, 0, 1, OwnedValue::Color(0xffaa_bbcc)),
                stable_style(root, 0, 4, OwnedValue::U32(256)),
            ])
            .unwrap_err(),
        RegistryError::Range { field_id: 4 }
    );
    assert_eq!(registry.revision(), starting);
    assert_eq!(snapshot(&mut registry, 8).total_style_values, 0);
}

#[test]
fn no_op_and_changed_style_commits_are_allocation_free_and_advance_once() {
    let mut registry = registry();
    let root = create_root(&mut registry, "Container", "main");

    for value in [OwnedValue::None, OwnedValue::U32(0), OwnedValue::U32(0)] {
        let starting = registry.revision();
        let prepared = registry
            .prepare_batch(vec![StageDirection::SetLocalStyle {
                object_id: root,
                part_id: 0,
                state_mask: 0,
                property_id: 4,
                value,
            }])
            .unwrap();
        let (result, allocations, deallocations) =
            measure(|| registry.commit_prepared_batch(prepared));
        assert_eq!((allocations, deallocations), (0, 0));
        let committed = result.unwrap();
        assert_eq!(committed.revision().get(), starting.get() + 1);
        registry.release_committed_batch(committed).unwrap();
        assert_eq!(
            registry.last_commit_effects(),
            MutationEffects::DRAW.union(MutationEffects::SNAPSHOT)
        );
        assert_eq!(registry.last_invalidations(), &[BOUNDS]);
    }
    let record = snapshot(&mut registry, 4);
    assert_eq!(record.total_style_values, 1);
    assert_eq!(record.styles[0].value, OwnedValue::U32(0));
}

#[test]
fn bounded_snapshot_reports_prefix_legacy_zero_staleness_and_retryable_allocation_failure() {
    let mut registry = registry();
    let root = create_root(&mut registry, "Container", "main");
    let prepared = registry
        .prepare_atomic_batch(vec![
            stable_style(root, 0, 3, OwnedValue::U32(1)),
            stable_style(root, 0, 1, OwnedValue::Color(0xff00_0001)),
            stable_style(root, 0, 4, OwnedValue::U32(2)),
        ])
        .unwrap();
    let committed = registry.commit_prepared_batch(prepared).unwrap();
    registry.release_committed_batch(committed).unwrap();

    let legacy_token = registry.snapshot_begin().unwrap();
    let legacy = registry.snapshot_read(legacy_token, 1, 1024).unwrap();
    assert_eq!(legacy.records[0].total_style_values, 3);
    assert!(legacy.records[0].styles.is_empty());
    assert!(legacy.records[0].styles_truncated);

    let record = snapshot(&mut registry, 2);
    assert_eq!(record.total_style_values, 3);
    assert_eq!(
        record
            .styles
            .iter()
            .map(|style| style.property_id)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert!(record.styles_truncated);

    let failure_token = registry.snapshot_begin().unwrap();
    FAIL_AFTER.with(|remaining| remaining.set(Some(2)));
    assert_eq!(
        registry.snapshot_read_with_style_limit(failure_token, 1, 1024, 3),
        Err(SnapshotError::Registry(RegistryError::Capacity {
            kind: CapacityKind::SnapshotValues,
        }))
    );
    FAIL_AFTER.with(|remaining| remaining.set(None));
    let retry = registry
        .snapshot_read_with_style_limit(failure_token, 1, 1024, 3)
        .unwrap();
    assert_eq!(retry.sequence, 0);
    assert_eq!(retry.records[0].styles.len(), 3);

    let stale_token = registry.snapshot_begin().unwrap();
    registry
        .apply_batch(&[StageDirection::SetLocalStyle {
            object_id: root,
            part_id: 0,
            state_mask: 0,
            property_id: 3,
            value: OwnedValue::U32(1),
        }])
        .unwrap();
    assert!(matches!(
        registry.snapshot_read_with_style_limit(stale_token, 1, 1024, 3),
        Err(SnapshotError::Stale { .. })
    ));
}
