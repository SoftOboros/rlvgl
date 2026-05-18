//! Compile-fail fixtures locking in the typed-API contracts of the
//! Register-Mashing Discipline.
//!
//! Each fixture in `tests/ui/` is a `main.rs`-shaped Rust file that
//! deliberately *fails to compile*. The expected `rustc` error is
//! recorded in the matching `*.stderr` file. `trybuild` re-runs the
//! fixture and asserts the produced diagnostic matches.
//!
//! ## Updating the stderr files
//!
//! When `rust-toolchain.toml` is bumped the diagnostic wording may
//! change. Regenerate the snapshots with:
//!
//! ```bash
//! TRYBUILD=overwrite cargo test -p rlvgl-platform --test discipline_compile
//! ```
//!
//! and review the diff in the same PR as the toolchain bump.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/inflight_*.rs");
    t.compile_fail("tests/ui/scanout_*.rs");
    t.compile_fail("tests/ui/mmio_*.rs");
    t.compile_fail("tests/ui/dca_*.rs");
}
