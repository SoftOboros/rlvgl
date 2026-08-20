<!--
README.md - Scope and use of the native-only rlvgl std runtime boundary.
-->

# rlvgl-runtime-std

`rlvgl-runtime-std` is the interpreter-neutral `std` ownership boundary for
the CPY initiative. Its eventual consumers are the CPython adapter, a native
daemon, and headless/native host tools. It must never contain PyO3,
MicroPython ABI objects, or callbacks into a language runtime.

The `0.0.0` crate is deliberately non-publishable. It proves that non-`Send`
neutral runtime state can be constructed, used, and destroyed entirely on one
native thread. Its first CPY-03 service also provides explicit bounded
ingress/egress, deterministic turn batches, typed admission outcomes,
process-local epochs, ordered close/fault records, metrics, and a pollable
readiness signal. Owner state is destroyed before `Closed` becomes observable,
and the stable-backlog proof requires FIFO batches to stay within the explicit
per-turn budget. The placeholder version does not resolve
`PCDN-CPY-09-004`; publication stays blocked until CPY selects a truthful
release line.

Linux readiness uses a nonblocking close-on-exec `eventfd`; other selected Unix
hosts use a nonblocking close-on-exec self-pipe. Crossbeam and Rustix stay
behind CPY-owned types. The service contains no PyO3 or MicroPython object and
does not call a language runtime.

Capacities remain required constructor inputs, not defaults. This slice does
not yet define semantic record loss classes, frame slots, platform cadence,
render/present work, or Python finalization behavior. Those surfaces remain
gated by CPY-03/05/06 measurements and integration.

Every restarted service receives a strictly newer process-local epoch.
Adapter-owned handles, request tickets, and retained records must be checked
with `validate_epoch`, `validate_ticket`, or `validate_record` before reuse;
the service returns a typed `ServiceEpochMismatch` instead of binding a stale
identity to the new owner. Request IDs may repeat only under a different
service epoch.

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

The bounded service takes one result per admitted request and faults rather
than silently losing an incomplete turn:

```rust
use rlvgl_runtime_std::{ServiceConfig, spawn_native_service};

let service = spawn_native_service(
    "rlvgl-native-service",
    ServiceConfig::new(16, 32, 8)?,
    || String::from("owner-thread state"),
    |state, requests: Vec<usize>| {
        requests.into_iter().map(|value| Ok::<_, ()>(value + state.len())).collect()
    },
)?;
let epoch = service.epoch();
service.validate_epoch(epoch)?;
let _ticket = service.try_submit(1).expect("explicit capacity admits request");
let records = service.shutdown()?;
assert!(!records.is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Capacity probe

The `cpy_capacity_probe` example is a diagnostic CPY-03 measurement target. Its
v3 workload drives the production `NativeService` around an owner-thread
`Endpoint`: real Stage/Slider mutations and completions, native pointer input
and Cue drain/acknowledgment, fixed render cadence, a private non-exported
320×240 RGBA frame, OS readiness, bounded admission, exact terminal records,
and ordered shutdown. It exercises cold bursts, sustained admission, and a
stalled egress observer, then emits one JSON result. The private frame is not a
Frame Lease or Python buffer. Candidate values are inputs, not runtime defaults:

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
  --observer-stall-us 50000 \
  --frame-period-us 16667
```

Use `scripts/cpy_capacity_probe.py` for a reproducible matrix and evidence
bundle. Host output is diagnostic and cannot select embedded-Linux capacities;
the same committed probe must also run on the CPY-01 reference board.
