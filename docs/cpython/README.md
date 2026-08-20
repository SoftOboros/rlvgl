<!--
README.md - Informative index for the CPython embedded-Linux and host initiative.
-->

# CPY — CPython Director for Embedded Linux and Host

**Status:** CPY-00 through CPY-02 ratified 2026-08-18. All CPY-03 policy PCDNs
are resolved; its bounded native service, Unix readiness, and retained v1/v2
host capacity matrices are complete, with typed restart/stale-epoch and native
close-fence stress validation. Owner destruction and deterministic bounded-turn
traces are complete. Its representative v3 native workload and clean-source
host matrix are complete; physical BBB qualification remains open. CPY-03
through CPY-09 remain Draft and separately evidence-gated.

CPY specifies a full CPython binding over rlvgl's Stage-and-Actors runtime,
with embedded Linux as the primary deployment and full-host headless/windowed
operation as the development and conformance companion. It consumes the
language-neutral MPY contracts without making CPython their semantic authority.

The public distribution is `rlvgl` and must be publishable to PyPI as truthful
prebuilt wheels plus a self-contained source distribution that carries/builds
the pinned native rlvgl graph. Python startup configuration selects packaged
backends. For Wayland, Python requests logical window area and public metadata;
the WLD-owned native session retains compositor/event-loop authority and reports
the actual configured geometry.

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
| [CPY-01](CPY-01-BASELINE-TARGET-PROFILES.md) | Repository baseline, CPython/PyO3 pins, target profiles, and capability matrix | Ratified 2026-08-18; exact baseline retained; runtime qualification remains later-phase evidence |
| [CPY-02](CPY-02-UNIFY-PARTITION-CRATES.md) | Unify neutral contracts and partition interpreter/runtime/platform crates | Ratified 2026-08-18; graph/firewall/handoff complete; first `runtime-std` slice authorized |
| [CPY-03](CPY-03-NATIVE-RUNTIME-SERVICE.md) | Native runtime thread, bounded queues, lifecycle, and callback isolation | Draft; lifecycle/turn proof and representative host matrix retained; record classes and physical BBB qualification open |
| [CPY-04](CPY-04-CPYTHON-DIRECTOR-BINDING.md) | PyO3 module, Python objects, runtime/backend configuration, transactions, exceptions, callbacks, and typing | Draft; seven binding-policy PCDNs resolved; dependency/implementation evidence open |
| [CPY-05](CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md) | Flattened frames, immutable leases, buffer protocol, damage, and backpressure | Draft; five frame-policy PCDNs resolved; measured slot capacities and proof open |
| [CPY-06](CPY-06-EMBEDDED-LINUX-RUNTIME.md) | Primary fbdev/evdev or admitted native backend, copied Python startup configuration, device lifecycle, and privilege profiles | Draft; six embedded policy PCDNs resolved; rootfs/board/runtime proof open |
| [CPY-07](CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md) | Headless and configured windowed host profiles, asyncio readiness, and launcher boundary | Draft; three policies resolved; window topology and drain count open |
| [CPY-08](CPY-08-PACKAGING-CROSS-DEPLOYMENT.md) | PyPI sdist/wheels, target-rootfs cross-builds, services, versioning, and artifact manifests | Draft; six packaging policies resolved; isolated build/import/reproducibility proof open |
| [CPY-09](CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md) | Cross-driver parity, frame/thread/lifetime evidence, budgets, PyPI gates, docs, and release closure | Draft; three policies resolved; budgets/version/retention open |

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
  artifact baselines. The first conforming instance and its provenance are in
  [`evidence/`](evidence/README.md).
- [CPY-CAPACITY-EVIDENCE.schema.json](CPY-CAPACITY-EVIDENCE.schema.json) is the
  authored diagnostic grammar for CPY-03 queue/turn candidates. Its instances
  cannot select normative defaults.

Generated object-index JSON is not edited by hand. After authored CPY and any
concurrent WLD/MPY documentation settle, regenerate the combined projection
with `make spec-index` and verify it with `make spec-test spec-index-check`.
