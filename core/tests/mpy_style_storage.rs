//! MPY-04 local-style prerequisite transaction evidence.

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::Cell,
};

use rlvgl_core::style_cascade::{
    MpyStyleStorageError, MpyStyleUpdate, Part, Selector, StyleProperty, StylePropertyValue,
    StyleState,
};

std::thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct TrackingAllocator;

// SAFETY: every operation delegates unchanged to the process allocator; the
// thread-local counters are observational only.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: delegated with the caller-provided layout.
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACKING.with(Cell::get) {
            DEALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: delegated with the pointer and layout from System.
        unsafe { std::alloc::System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACKING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: delegated with the pointer, layout, and requested size.
        unsafe { std::alloc::System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCATIONS.with(|count| count.set(0));
    DEALLOCATIONS.with(|count| count.set(0));
    TRACKING.with(|enabled| enabled.set(true));
    let result = operation();
    TRACKING.with(|enabled| enabled.set(false));
    let allocations = ALLOCATIONS.with(Cell::get);
    let deallocations = DEALLOCATIONS.with(Cell::get);
    (result, allocations, deallocations)
}

#[test]
fn final_commit_and_stale_rejection_do_not_allocate_or_deallocate() {
    let selector = Selector::part(Part::MAIN);
    let mut state = StyleState::new();
    let seed = state
        .prepare_mpy_local_update(
            selector,
            StyleProperty::Alpha,
            MpyStyleUpdate::Set(StylePropertyValue::U32(255)),
            1,
        )
        .unwrap();
    let seeded = state.commit_mpy_local_update(seed).unwrap();
    state.release_mpy_local_update(seeded);
    let prepared = state
        .prepare_mpy_local_update(
            selector,
            StyleProperty::PaddingTop,
            MpyStyleUpdate::Set(StylePropertyValue::I32(4)),
            1,
        )
        .unwrap();
    let stale = state
        .prepare_mpy_local_update(
            selector,
            StyleProperty::PaddingTop,
            MpyStyleUpdate::Set(StylePropertyValue::I32(8)),
            1,
        )
        .unwrap();

    let (committed, allocations, deallocations) =
        measure(|| state.commit_mpy_local_update(prepared).unwrap());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(state.mpy_revision(), 2);

    let (error, allocations, deallocations) =
        measure(|| state.commit_mpy_local_update(stale).unwrap_err());
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(error.cause(), MpyStyleStorageError::Stale);
    state.release_prepared_mpy_local_update(error.into_prepared());

    let (_, allocations, deallocations) = measure(|| state.release_mpy_local_update(committed));
    assert_eq!(allocations, 0);
    assert!(deallocations > 0);
}

#[test]
fn failed_preparation_and_explicit_release_leave_durable_state_unchanged() {
    let selector = Selector::part(Part::MAIN);
    let state = StyleState::new();
    let prepared = state
        .prepare_mpy_local_update(
            selector,
            StyleProperty::Alpha,
            MpyStyleUpdate::Set(StylePropertyValue::U32(42)),
            1,
        )
        .unwrap();
    state.release_prepared_mpy_local_update(prepared);
    assert_eq!(state.mpy_revision(), 0);
    assert!(state.mpy_local_entries().is_empty());

    assert_eq!(
        state
            .prepare_mpy_local_update(
                selector,
                StyleProperty::Alpha,
                MpyStyleUpdate::Set(StylePropertyValue::U32(300)),
                1,
            )
            .unwrap_err(),
        MpyStyleStorageError::Range
    );
    assert_eq!(state.mpy_revision(), 0);
    assert!(state.mpy_local_entries().is_empty());
}
