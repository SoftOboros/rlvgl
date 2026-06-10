//! Integration test for `hwcore::isr` SPSC primitives.
//!
//! Drives [`IsrChannel`] from a producer thread and a consumer thread to
//! exercise the volatile + atomic ordering paths the production ISR ↔
//! main-loop usage relies on. Not a true loom-style exhaustive
//! exploration — the host's `std::thread` scheduler gives us best-effort
//! coverage; the discipline rule itself (single writer, single reader)
//! is encoded in the `unsafe` push/pop signatures and documented in the
//! module.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use rlvgl_platform::{IsrChannel, IsrCounter, IsrFlag};

#[test]
fn channel_transfers_all_items_between_threads() {
    static CH: IsrChannel<u32, 32> = IsrChannel::new();
    const N: u32 = 10_000;

    let producer = thread::spawn(|| {
        // SAFETY: this is the sole writer for the duration of the test.
        for i in 0..N {
            loop {
                if unsafe { CH.try_push(i) }.is_ok() {
                    break;
                }
                thread::yield_now();
            }
        }
    });

    let consumer = thread::spawn(|| {
        // SAFETY: this is the sole reader for the duration of the test.
        let mut received = Vec::with_capacity(N as usize);
        while received.len() < N as usize {
            if let Some(v) = unsafe { CH.try_pop() } {
                received.push(v);
            } else {
                thread::yield_now();
            }
        }
        received
    });

    producer.join().expect("producer joined");
    let received = consumer.join().expect("consumer joined");

    assert_eq!(received.len(), N as usize);
    for (i, &v) in received.iter().enumerate() {
        assert_eq!(v, i as u32, "out-of-order item at index {i}");
    }
}

#[test]
fn flag_signals_across_threads() {
    let flag = Arc::new(IsrFlag::new());
    let observed = Arc::new(AtomicBool::new(true));
    let setter = {
        let f = Arc::clone(&flag);
        let seen = Arc::clone(&observed);
        thread::spawn(move || {
            for _ in 0..50 {
                while !seen.swap(false, Ordering::AcqRel) {
                    thread::yield_now();
                }
                f.set();
                thread::yield_now();
            }
        })
    };
    let watcher = {
        let f = Arc::clone(&flag);
        let seen = Arc::clone(&observed);
        thread::spawn(move || {
            let mut takes = 0u32;
            while takes < 50 {
                if f.take() {
                    takes += 1;
                    seen.store(true, Ordering::Release);
                }
                thread::yield_now();
            }
            takes
        })
    };
    setter.join().unwrap();
    let takes = watcher.join().unwrap();
    assert_eq!(takes, 50);
}

#[test]
fn counter_increments_from_multiple_writers() {
    // IsrCounter is documented as one-writer for high-level semantics
    // but the underlying atomic is correct under contention; this test
    // proves the atomic guarantee.
    let counter = Arc::new(IsrCounter::new());
    let mut handles = Vec::new();
    for _ in 0..4 {
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                c.increment();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter.read(), 4_000);
    assert_eq!(counter.reset(), 4_000);
    assert_eq!(counter.read(), 0);
}
