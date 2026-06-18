//! Value-binding `Subject<T>` for deterministic observer notifications.
//!
//! This module provides a typed publish/subscribe primitive that is **separate
//! from** the LPAR-04 [`ObjectEvent`](crate::object::ObjectEvent) dispatch
//! system.  It carries scalar data values (`i32`, `bool`, [`Color`](crate::widget::Color),
//! `String`) between arbitrary owners through explicit subscribe/notify
//! callbacks, without routing through `ObjectNode` or emitting `ObjectEvent`.
//!
//! # Relationship to the LPAR-04 event system
//!
//! | Dimension | LPAR-04 `ObjectEvent` | `Subject<T>` |
//! |---|---|---|
//! | What it carries | Semantic gestures, lifecycle signals | Scalar data values |
//! | Routing | Tree dispatch through `ObjectNode` | Direct callback invocation |
//! | Coupling | Requires `ObjectNode` hierarchy | Caller holds both ends |
//! | v1 integration | Not altered | Added orthogonally |
//!
//! A widget MAY both receive `ObjectEvent::Clicked` (dispatched by the tree
//! router) **and** update a `Subject<bool>` it owns.  No framework mechanism
//! connects them; wiring is the caller's responsibility.
//!
//! # `no_std` notes
//!
//! [`Subject<T>`] requires `alloc` (it owns a `Vec<Box<dyn FnMut>>` observer
//! list).  Targets without a heap allocator MUST NOT instantiate subjects;
//! the type compiles under `no_std + alloc` but panics if the allocator
//! aborts.
//!
//! # Reentrancy
//!
//! Observers receive `&T`, not `&mut Subject<T>`, so direct recursive
//! mutation of the *same* subject from within its own observer is
//! structurally prevented.  A second `Subject` may be mutated from within an
//! observer — this is the intended cross-subject binding pattern (see
//! [`tests::cross_subject_observer`](crate::observer) below).
//!
//! A lightweight `notifying: bool` sentinel is included as a defensive guard.
//! If a re-entrant [`Subject::notify`] call somehow occurs (e.g. via unsafe
//! aliasing outside this module), the inner call is silently skipped rather
//! than potentially overflowing the stack or double-borrowing.  This is
//! documented and testable; it does **not** imply that re-entrant use is
//! supported.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;

/// Heap-allocated, mutable observer callback for a value of type `T`.
///
/// Factored out to satisfy `clippy::type_complexity`; the underlying type is
/// `Box<dyn FnMut(&T)>`.
pub type ObserverFn<T> = Box<dyn FnMut(&T)>;

/// A typed value holder with an observer callback list.
///
/// `Subject<T>` stores a current and previous value and a list of callbacks.
/// Calling [`set`](Subject::set) atomically updates the value and notifies
/// all registered observers synchronously, in subscription order.
///
/// ## Type coverage in v1
///
/// `Subject<i32>`, `Subject<bool>`, `Subject<Color>`, and `Subject<String>`
/// cover the four [`PropertyValue`](crate::property::PropertyValue) variants.
/// Subjects are intentionally typed — callers subscribe to a `Subject<i32>`,
/// not to a `Subject<PropertyValue>`.  Bridging the two is application-level
/// code.
///
/// ## Object-identity-free
///
/// `Subject<T>` does not require object ids, `WidgetId`, or `ObjectNode`
/// references.  Subjects are owned values; callers hold them directly.  This
/// satisfies the LPAR-15 §5.I binding invariant.
pub struct Subject<T: Clone> {
    /// Current value.
    value: T,
    /// Value before the most recent [`set`](Subject::set) call.  Equals
    /// `value` on construction (no previous state exists yet).
    prev_value: T,
    /// Registered observer callbacks, called in subscription order.
    observers: Vec<ObserverFn<T>>,
    /// Reentrancy sentinel: `true` while [`notify`](Subject::notify) is
    /// dispatching.  A re-entrant `notify` call skips execution rather than
    /// recursing.  Callers MUST NOT rely on re-entrant notifications being
    /// delivered; this guard is purely defensive.
    notifying: bool,
}

impl<T: Clone> Subject<T> {
    /// Create a new `Subject` with the given initial value.
    ///
    /// `prev_value` is initialised to a clone of `initial`; no observers are
    /// registered.
    ///
    /// ```rust
    /// use rlvgl_core::observer::Subject;
    /// let s: Subject<i32> = Subject::new(0);
    /// assert_eq!(*s.get(), 0);
    /// assert_eq!(*s.prev(), 0);
    /// ```
    pub fn new(initial: T) -> Self {
        Self {
            prev_value: initial.clone(),
            value: initial,
            observers: Vec::new(),
            notifying: false,
        }
    }

    /// Borrow the current value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Borrow the value from before the most recent [`set`](Subject::set) call.
    ///
    /// On a freshly constructed subject `prev() == get()`.
    pub fn prev(&self) -> &T {
        &self.prev_value
    }

    /// Store a new value, copy the old value to `prev`, then notify all
    /// observers synchronously in subscription order.
    ///
    /// Observers receive a shared reference `&T` to the **new** value.  They
    /// MUST NOT attempt to call `set` on *this same* subject; that is
    /// structurally prevented because they do not hold a `&mut Subject`.
    /// Cross-subject `set` (observer of A mutates B) is the intended
    /// composition pattern and works correctly.
    pub fn set(&mut self, value: T) {
        self.prev_value = self.value.clone();
        self.value = value;
        self.notify();
    }

    /// Notify all observers with the current value without changing it.
    ///
    /// `prev_value` is also left unchanged.  Useful for re-broadcasting after
    /// an external mutation or initialisation.
    ///
    /// If this method is already executing on the call stack (re-entrant call),
    /// the inner invocation is silently skipped.  See the module-level
    /// reentrancy note for rationale.
    pub fn notify(&mut self) {
        if self.notifying {
            return;
        }
        self.notifying = true;
        // Snapshot the current value so that observers see a consistent view.
        let current = self.value.clone();
        for cb in &mut self.observers {
            cb(&current);
        }
        self.notifying = false;
    }

    /// Register an observer callback.
    ///
    /// Callbacks are invoked in subscription order on every [`set`] or
    /// explicit [`notify`](Subject::notify) call.  There is no unsubscribe
    /// mechanism in v1; adding one is a Specification Required amendment that
    /// does not break existing call sites.
    ///
    /// `F: 'static` is required because the closure is heap-allocated behind a
    /// `Box<dyn FnMut>` and may outlive the calling scope.
    pub fn subscribe<F: FnMut(&T) + 'static>(&mut self, cb: F) {
        self.observers.push(Box::new(cb));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::string::String;
    use alloc::vec;
    use core::cell::Cell;
    use core::cell::RefCell;

    // -----------------------------------------------------------------------
    // Basic get / prev after construction
    // -----------------------------------------------------------------------

    #[test]
    fn new_get_and_prev_equal_initial() {
        let s: Subject<i32> = Subject::new(42);
        assert_eq!(*s.get(), 42);
        assert_eq!(*s.prev(), 42);
    }

    // -----------------------------------------------------------------------
    // set() notifies observers and updates prev
    // -----------------------------------------------------------------------

    #[test]
    fn set_updates_value_and_prev() {
        let mut s: Subject<i32> = Subject::new(10);
        s.set(20);
        assert_eq!(*s.get(), 20);
        assert_eq!(*s.prev(), 10);
    }

    #[test]
    fn set_notifies_single_observer() {
        let mut s: Subject<i32> = Subject::new(0);
        let received = Rc::new(Cell::new(-1_i32));
        let received_clone = received.clone();
        s.subscribe(move |v| received_clone.set(*v));
        s.set(7);
        assert_eq!(received.get(), 7);
    }

    #[test]
    fn set_notifies_multiple_observers_in_order() {
        let mut s: Subject<i32> = Subject::new(0);
        let log = Rc::new(RefCell::new(Vec::<(usize, i32)>::new()));

        for idx in 0..3 {
            let log_clone = log.clone();
            s.subscribe(move |v| log_clone.borrow_mut().push((idx, *v)));
        }

        s.set(5);

        let entries = log.borrow();
        assert_eq!(*entries, vec![(0, 5), (1, 5), (2, 5)]);
    }

    // -----------------------------------------------------------------------
    // notify() fires without changing value or prev
    // -----------------------------------------------------------------------

    #[test]
    fn notify_fires_without_changing_value() {
        let mut s: Subject<i32> = Subject::new(3);
        s.set(9); // prev = 3, value = 9
        let call_count = Rc::new(Cell::new(0_u32));
        let call_count_clone = call_count.clone();
        s.subscribe(move |_| call_count_clone.set(call_count_clone.get() + 1));

        s.notify();
        assert_eq!(*s.get(), 9, "value unchanged by notify");
        assert_eq!(*s.prev(), 3, "prev unchanged by notify");
        assert_eq!(call_count.get(), 1, "observer fired once");
    }

    // -----------------------------------------------------------------------
    // Multiple sequential set() calls
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_set_calls_track_prev_correctly() {
        let mut s: Subject<i32> = Subject::new(1);
        s.set(2);
        assert_eq!(*s.prev(), 1);
        assert_eq!(*s.get(), 2);
        s.set(3);
        assert_eq!(*s.prev(), 2);
        assert_eq!(*s.get(), 3);
    }

    // -----------------------------------------------------------------------
    // Cross-subject observer: observer of A updates B
    // -----------------------------------------------------------------------

    #[test]
    fn cross_subject_observer() {
        let mut a: Subject<i32> = Subject::new(0);
        let b = Rc::new(RefCell::new(Subject::<i32>::new(0)));

        let b_clone = b.clone();
        a.subscribe(move |v| {
            b_clone.borrow_mut().set(*v * 2);
        });

        a.set(5);

        assert_eq!(*a.get(), 5);
        assert_eq!(*b.borrow().get(), 10);
        assert_eq!(*b.borrow().prev(), 0);
    }

    // -----------------------------------------------------------------------
    // Reentrancy sentinel: re-entrant notify is skipped, no panic/recursion
    // -----------------------------------------------------------------------

    #[test]
    fn reentrant_notify_is_skipped_no_panic() {
        // We use a raw pointer trick to demonstrate the sentinel: we capture a
        // pointer to the subject and attempt a notify inside the observer.  The
        // sentinel `notifying = true` prevents recursion.  This is intentionally
        // unsafe in the test only to exercise the guard; production code cannot
        // trigger this path because observers receive `&T`, not `&mut Subject`.
        let mut s: Subject<i32> = Subject::new(0);
        let call_count = Rc::new(Cell::new(0_u32));
        let call_count_clone = call_count.clone();

        // SAFETY (test-only): we hold a raw pointer to `s` solely to invoke
        // `notify` inside the observer and exercise the reentrancy guard.
        // `s` outlives the closure; the closure is dropped before `s`.
        // No other aliasing occurs; the sentinel prevents actual re-entry.
        let s_ptr = &mut s as *mut Subject<i32>;
        s.subscribe(move |_| {
            call_count_clone.set(call_count_clone.get() + 1);
            // Attempt a re-entrant notify
            // SAFETY: see above.
            unsafe { (*s_ptr).notify() };
        });

        s.set(1);

        // The outer notify fires the observer once.  The inner notify is
        // skipped by the sentinel, so the observer is NOT called a second time.
        assert_eq!(call_count.get(), 1, "re-entrant notify must be skipped");
        assert_eq!(*s.get(), 1);
    }

    // -----------------------------------------------------------------------
    // Subject<bool>
    // -----------------------------------------------------------------------

    #[test]
    fn bool_subject_set_and_notify() {
        let mut s: Subject<bool> = Subject::new(false);
        let seen = Rc::new(Cell::new(false));
        let seen_clone = seen.clone();
        s.subscribe(move |v| seen_clone.set(*v));
        s.set(true);
        assert!(seen.get());
    }

    // -----------------------------------------------------------------------
    // Subject<String>
    // -----------------------------------------------------------------------

    #[test]
    fn string_subject_tracks_value() {
        let mut s: Subject<String> = Subject::new("hello".into());
        s.set("world".into());
        assert_eq!(s.get().as_str(), "world");
        assert_eq!(s.prev().as_str(), "hello");
    }

    // -----------------------------------------------------------------------
    // Zero observers — set is a no-op for notifications
    // -----------------------------------------------------------------------

    #[test]
    fn set_with_no_observers_does_not_panic() {
        let mut s: Subject<i32> = Subject::new(0);
        s.set(42); // must not panic
        assert_eq!(*s.get(), 42);
    }
}
