<!--
README.md - Informative index for the CPython embedded-Linux and host initiative.
-->

# CPY — CPython Director for Embedded Linux and Host

**Status:** CPY-00 ratified 2026-08-18. CPY-01 through CPY-09 remain Draft;
no later-phase implementation is authorized by this index.

CPY specifies a full CPython binding over rlvgl's Stage-and-Actors runtime,
with embedded Linux as the primary deployment and full-host headless/windowed
operation as the development and conformance companion. It consumes the
language-neutral MPY contracts without making CPython their semantic authority.

The initiative also includes a dedicated crate-topology phase. That phase
unifies protocol, runtime, rendering, and conformance assets at their neutral
owners while partitioning CPython, MicroPython, threaded-host, platform, and
privileged-device responsibilities behind explicit dependency boundaries.

## Conformance targets

- A conforming **CPY embedded-Linux deployment** MUST satisfy the ratified and
  executed CPY-01 through CPY-06, CPY-08, and applicable CPY-09 gates. It uses
  native rlvgl presentation and input; Python directs application policy.
- A conforming **CPY host deployment** MUST satisfy CPY-01 through CPY-05,
  CPY-07, CPY-08, and applicable CPY-09 gates. Headless and windowed artifacts
  expose the same Python object model.
- A conforming **CPY hardened deployment** additionally satisfies the
  separately ratified daemon/privilege gates in CPY-06 and CPY-09.
- Free-threaded CPython is a separately qualified runtime variant. Evidence for
  a conventional GIL-enabled build MUST NOT be relabeled as free-threaded
  evidence.

These statements name intended conformance artifacts. They become binding only
through ratified phase documents and their acceptance gates.

## Phase map

| Phase | Scope | Status |
|---|---|---|
| [CPY-00](CPY-00-CONCEPTS.md) | Authority, vocabulary, profiles, invariants, and phase order | Ratified 2026-08-18; three root PCDNs accepted as amended |
| [CPY-01](CPY-01-BASELINE-TARGET-PROFILES.md) | Repository baseline, CPython/PyO3 pins, target profiles, and capability matrix | Draft; six selections resolved; manifest/rootfs/board evidence open |
| [CPY-02](CPY-02-UNIFY-PARTITION-CRATES.md) | Unify neutral contracts and partition interpreter/runtime/platform crates | Draft; six topology PCDNs resolved; blocked by CPY-01 and an actual MPY Handoff Record |
| [CPY-03](CPY-03-NATIVE-RUNTIME-SERVICE.md) | Native runtime thread, bounded queues, lifecycle, and callback isolation | Draft; four policy PCDNs resolved; capacity measurement and CPY-02 open |
| [CPY-04](CPY-04-CPYTHON-DIRECTOR-BINDING.md) | PyO3 module, Python objects, transactions, exceptions, callbacks, and typing | Draft; six binding-policy PCDNs resolved; dependency/implementation evidence open |
| [CPY-05](CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md) | Flattened frames, immutable leases, buffer protocol, damage, and backpressure | Draft; five frame-policy PCDNs resolved; measured slot capacities and proof open |
| [CPY-06](CPY-06-EMBEDDED-LINUX-RUNTIME.md) | Primary fbdev/evdev or admitted native backend, device lifecycle, and privilege profiles | Draft; blocked by CPY-03/04/05 |
| [CPY-07](CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md) | Headless and windowed host profiles, asyncio readiness, and launcher boundary | Draft; blocked by CPY-03/04/05 |
| [CPY-08](CPY-08-PACKAGING-CROSS-DEPLOYMENT.md) | Wheels, target-rootfs cross-builds, services, versioning, and artifact manifests | Draft; blocked by CPY-02/04 and target decisions |
| [CPY-09](CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md) | Cross-driver parity, frame/thread/lifetime evidence, budgets, docs, and release closure | Draft; blocked by CPY-01 through CPY-08 |

## Coordination boundaries

- [MPY-00](../concepts/MPY-00-CONCEPTS.md) and its separately gated phases own
  the language-neutral Stage, Actor, direction, result, cue, descriptor, and
  Safe Turn semantics CPY consumes. CPY MUST NOT normalize unfinished MPY work
  or make PyO3 the semantic oracle.
- [LPAR](../concepts/LPAR-00-CONCEPTS.md) owns LVGL-parity widget, layout,
  event, display, and rendering semantics. CPY projects those capabilities.
- [WLD](../wayland/README.md) owns any native Wayland backend. CPY may consume a
  ratified WLD backend but does not relocate or redefine it during crate work.
- [ERRATA.md](ERRATA.md) is the family-local permanent deviation log.
- [CPY-BASELINE-MANIFEST.schema.json](CPY-BASELINE-MANIFEST.schema.json) is the
  authored CPY-01 grammar for exact source, interpreter, rootfs, board, and
  artifact baselines; no conforming manifest instance exists yet.

Generated object-index JSON is not edited by hand. After authored CPY and any
concurrent WLD/MPY documentation settle, regenerate the combined projection
with `make spec-index` and verify it with `make spec-test spec-index-check`.
