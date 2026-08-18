<!--
README.md - Scope and use of the native-only rlvgl std runtime scaffold.
-->

# rlvgl-runtime-std

`rlvgl-runtime-std` is the interpreter-neutral `std` ownership boundary for
the CPY initiative. Its eventual consumers are the CPython adapter, a native
daemon, and headless/native host tools. It must never contain PyO3,
MicroPython ABI objects, or callbacks into a language runtime.

The initial `0.0.0` crate is deliberately a non-publishable scaffold. It
proves that non-`Send` neutral runtime state can be constructed, used, and
destroyed entirely on one native thread while only a `Send` result crosses the
join boundary. The placeholder version does not resolve
`PCDN-CPY-09-004`; publication stays blocked until CPY selects a truthful
release line.

This slice does not define the CPY-03 service lifecycle, ingress/egress
capacities, readiness primitive, frame slots, cadence, or close policy. Those
surfaces remain gated by CPY-03/05 measurements and ratification.

```rust
use rlvgl_runtime_std::spawn_owned_thread_task;

let task = spawn_owned_thread_task(
    "rlvgl-headless",
    || String::from("native state"),
    |state| state.len(),
)?;
assert_eq!(task.join()?, 12);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The integration test uses an actual non-`Send` `rlvgl_core::endpoint::Endpoint`
as the owned state. This is a native headless proof, not a Python or display
test.

## Capacity probe

The `cpy_capacity_probe` example is a diagnostic CPY-03 measurement target. It
uses bounded Crossbeam channels around an owner-thread `Endpoint`, exercises
cold bursts, sustained admission, and a stalled egress observer, and emits one
JSON result. Its candidate values are inputs, not runtime defaults:

```bash
cargo run --release -p rlvgl-runtime-std \
  --example cpy_capacity_probe -- \
  --scenario observer-stall \
  --ingress-capacity 32 \
  --egress-capacity 64 \
  --turn-budget 16 \
  --messages 1024 \
  --ingress-payload-bytes 256 \
  --egress-payload-bytes 128 \
  --observer-stall-us 50000
```

Use `scripts/cpy_capacity_probe.py` for a reproducible matrix and evidence
bundle. Host output is diagnostic and cannot select embedded-Linux capacities;
the same committed probe must also run on the CPY-01 reference board.
