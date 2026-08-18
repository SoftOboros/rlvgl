<!--
CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md - Flattened frame metadata, slot lifetime, and Python buffer export contract.
-->

# CPY-05 — Frame Lease and Buffer Protocol

**Document ID:** CPY-05-FRAME-LEASE-BUFFER-PROTOCOL

**Status:** Draft 2026-08-18. Five policy PCDNs resolved 2026-08-18;
`PCDN-CPY-05-002` remains measurement-blocked. Not ratified.

**Revision:** 0.2.0

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Canonical path:** `docs/cpython/CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md`

**Parent:** [CPY-00](CPY-00-CONCEPTS.md)

**Dependencies:** CPY-01 through CPY-03; CPY-04 consumes this surface.

## 0. Authority Policy

CPY-05 owns Python-visible flattened frame metadata, immutable storage leases,
buffer export, damage projection, presentation modes, and slot backpressure.
It does not own widget rendering semantics, internal mutable `Surface`, native
display flush behavior, or WLD's private Shadow Frame/SHM ownership.

The [CPython buffer protocol](https://docs.python.org/3/c-api/buffer.html) owns
export/acquire/release behavior. CPY adapts it with an immutable frame object
whose storage remains valid for every live export.

## 1. Purpose

Make every completed rlvgl frame available to Python as a zero-copy read-only
byte buffer with exact metadata, while preserving:

- native renderer and scanout ownership;
- bounded memory and observable saturation;
- stable storage through arbitrary `memoryview` lifetime;
- deterministic damage, sequence, revision, and loss reporting;
- native-presented, Python-presented, and headless profiles; and
- a separate writable resource path for Python-authored pixels.

## 2. Problem Statement

`platform::Surface` is an internal mutable borrowed render target. Exporting
its slice directly would allow Python to retain memory after the borrow/slot is
reused, mutate a frame being scanned out, or pin every render buffer without a
defined presentation policy. Returning `bytes` avoids lifetime hazards but
copies every frame and loses an explicit damage/lease contract.

The CPython buffer protocol enables zero-copy reads only when the exporter can
guarantee stable storage until release. rlvgl therefore needs a publication
layer above internal rendering, not a direct wrapper around `Surface`.

## 3. Canonical Glossary

| Term | Definition | Owner and relationship |
|---|---|---|
| **Frame ID** | Monotonic identity of one frozen rendered frame within a Service Epoch. | Owned by CPY-05; does not exist upstream yet. |
| **Pixel Layout** | Exact byte-level channel order, component width, packing, alpha meaning, and endianness of a frame. | Owned by CPY-05; adapts but does not restate `platform::PixelFmt`. |
| **Damage Set** | Bounded clipped rectangles whose union covers every changed output byte, plus overflow/full-damage state. | As defined by LPAR invalidation/present semantics; adapted into Frame Descriptor metadata. |
| **Observer Loss** | Explicit count/range indicating frames not made available to a Python observer while native presentation continued. | Owned by CPY-05. |
| **Canvas Buffer** | Separate writable Python-exportable resource that becomes render input only after an explicit Safe Turn commit. | Owned by CPY-05; not a Frame Lease. |
| **Frame Busy** | Bounded admission outcome when the selected mode cannot acquire/recycle a slot without violating a live lease. | Owned by CPY-05. |

Frame Descriptor, Frame Lease, and Frame Slot use the definitions in CPY-00.

## 4. Source-of-Truth Map

| Surface | Canonical artifact |
|---|---|
| Frame metadata, modes, slot/lease lifecycle | This document after ratification |
| Internal mutable rendering | `platform/src/blit.rs` and renderer implementations |
| Hardware framebuffer ownership | `platform/src/hwcore/surface.rs` and owning platform docs |
| Logical invalidation/presentation | LPAR-03 and `platform/src/present.rs` |
| CPython export/release behavior | Official CPython buffer protocol |
| Python `Frame` API | CPY-04 consuming this contract |
| Native/daemon presentation | CPY-06 |
| Host capture/presentation | CPY-07 |

## 5. Frozen Decisions — Frame Descriptor

Every Frame Descriptor MUST contain:

- Service Epoch and Frame ID;
- Stage ID and Stage Revision represented by the frame;
- width and height in pixels;
- total byte length and row stride in bytes;
- exact Pixel Layout identifier;
- bounded Damage Set plus explicit full-damage/overflow state;
- logical tick and optional separately labeled monotonic timestamp;
- presentation mode and whether native presentation occurred;
- Observer Loss since the prior delivered observer frame; and
- any color-space/alpha statement required to interpret bytes truthfully.

The descriptor MUST describe memory as exported, not the renderer's conceptual
`u32` value. A name such as `Argb8888` is insufficient unless it fixes the
actual byte order on the target architecture.

The first-release Pixel Layout identifier is `BGRA8888_LE_STRAIGHT`. It is a
top-to-bottom, left-to-right, tightly packed software-reference layout with:

- four unsigned 8-bit components per pixel at byte offsets `[B, G, R, A]`;
- unassociated/straight alpha, where `0` is transparent and `255` is opaque;
- `row_stride == width * 4` and `byte_length == row_stride * height`;
- no row padding or architecture-native integer reinterpretation; and
- `color_space == "rlvgl-renderer-v1"`, meaning current renderer component
  values with no publication-time transfer/profile conversion and the existing
  sRGB-naive straight-alpha blend behavior, not a calibrated-sRGB claim.

The defining operation is
`Color(r, g, b, a).to_argb8888().to_le_bytes()`. These canonical vectors MUST
produce the same bytes on every admitted target:

| Logical channels `(R,G,B,A)` | Exported bytes |
|---|---|
| `(00,00,00,00)` | `00 00 00 00` |
| `(ff,00,00,80)` | `00 00 ff 80` |
| `(12,34,56,78)` | `56 34 12 78` |

A backend whose render/scanout memory has another layout or padded stride MUST
copy/convert once while transitioning into the canonical frozen publication
slot. The zero-copy claim begins at the frozen Frame Slot and covers all Python
views of that slot; it does not claim that every backend renders directly into
the published storage.

## 6. Frozen Decisions — Slot and Lease Lifecycle

The slot lifecycle is:

```text
Free -> Rendering -> Frozen -> Published -> Retained
  ^                                  |          |
  |                                  v          v
  +---------------------------- Recyclable <- Presented
                                      ^
                                      |
                                final export release
```

- Only native rendering owns `Rendering` storage and may mutate it.
- Transition to `Frozen` publishes immutable bytes and complete metadata.
- `Published` makes a lease/notice visible under the selected mode.
- Native presentation may occur while a slot is frozen; it cannot authorize
  mutation while a Python export remains.
- Every acquired Python buffer increments slot export retention. The final
  release, not Python object reachability alone, permits `Recyclable`.
- Service close revokes new acquisition but MUST keep exported storage valid
  until release or use an ownership form whose deallocation is independent of
  the closed service.

Adding a slot lifecycle state is **Standards Action**.

## 7. Frozen Decisions — Python Buffer Export

The initial exporter MUST expose one read-only, C-contiguous, one-dimensional
unsigned-byte view: PEP 3118 format `B`, `itemsize == 1`,
`shape == (byte_length,)`, and `strides == (1,)`. Width, height, row stride,
Pixel Layout, damage, and sequence metadata remain immutable `Frame`
attributes. The exporter MUST NOT advertise a multidimensional or structured
view in the first release.

Writable export requests MUST fail. A consumer may create its own copy. Python
libraries may construct NumPy/Pillow/Cairo views over the exported bytes, but
those integrations do not become neutral dependencies.

Every acquire/release path, including nested `memoryview`, slices, exceptions,
garbage collection, interpreter finalization, and module close, MUST preserve
slot accounting. Explicit `Frame.release()` may release the object's own hold
but cannot invalidate independent buffer exports still alive.

## 8. Frozen Decisions — Modes and Backpressure

### 8.1 Native-presented observer

Native presentation MUST continue when Python is slow. If all observer-capable
slots are pinned, rlvgl MUST use a bounded policy that preserves native cadence
and reports Observer Loss when observation resumes. It MUST NOT overwrite a
leased slot or block scanout indefinitely.

Every completed native frame receives its Frame ID before observer publication.
When no observer slot is recyclable, that frame is still presented natively,
observer publication is skipped, and the service extends one pending inclusive
loss range. The next successfully published observer Frame reports exactly:

- `lost_count`;
- `first_lost_frame_id`; and
- `last_lost_frame_id`.

The range contains every completed Frame ID after the prior delivered observer
Frame and before the current delivered Frame that was not published to that
observer. A skipped frame is never replayed later or represented as the current
frame. If close/fault occurs before another observer Frame can carry the range,
the terminal lifecycle record MUST carry the same pending loss fields. Frame
notices and egress coalescing MUST preserve this accounting under CPY-03.

### 8.2 Headless/Python-presented

When Python is the required consumer/presenter, slot exhaustion MUST return a
typed Frame Busy/capacity outcome or honor an explicit bounded wait/timeout.
No call may block forever by default.

The first release requires this mode for host-headless frame capture and
conformance. Slot exhaustion returns `CapacityError` with
`context.resource == "frame_slot"` unless the caller selected a finite wait;
expiration of that wait returns `WaitTimeoutError` without overwriting a live
lease. Python-driven physical-device scanout is optional and does not block the
embedded-Linux conformance profile, whose required path is native presentation
with optional Python observation.

### 8.3 Canvas input

Writable Canvas Buffer storage MUST be distinct from live frame/scanout slots.
Publishing it to rlvgl MUST validate size/layout and transfer or snapshot its
content at an explicit Safe Turn. Python mutation after commit cannot race the
native draw traversal.

No public Canvas Buffer ships in the first release. The preceding isolation
rule reserves the only admissible future design; adding the resource is
Standards Action with a named consumer, owned-capacity policy, and Safe Turn
evidence.

### 8.4 Capacity accounting

For the initial layout, `slot_bytes == width * height * 4` and the configured
retained-byte ceiling is `slot_bytes * slot_count` plus fixed descriptor/lease
overhead. Slot count, byte ceiling, pinned high-water mark, skipped-frame count,
and wait/capacity outcomes MUST be exposed in metrics and the CPY-09 evidence
manifest. Exact minimum/default/maximum counts remain measurement-gated by
`PCDN-CPY-05-002`; no implementation default may become normative by accident.

## 9. Phase Invariants

| Id | Invariant | Verification surface |
|---|---|---|
| **INV-CPY-05-1** | A frozen/published frame MUST remain byte-for-byte immutable until every native and Python lease releases it. | Hash-before/after and concurrent export tests |
| **INV-CPY-05-2** | Every export MUST retain valid storage until its matching release, including after Frame object release and service close. | Buffer lifetime/finalization stress suite |
| **INV-CPY-05-3** | Frame metadata MUST state exact byte layout, stride, dimensions, damage, revision, and loss without architecture-dependent ambiguity. | Descriptor schema and cross-architecture vectors |
| **INV-CPY-05-4** | Writable Python access to live frame, render, DMA, or scanout storage MUST be rejected. | Buffer flags and mutation tests |
| **INV-CPY-05-5** | Slot count and retained bytes MUST be bounded, and saturation MUST produce the mode-specific observable outcome. | Held-lease exhaustion tests |
| **INV-CPY-05-6** | Native-presented mode MUST preserve native cadence when Python observers or callbacks stall. | Cadence-under-stall measurement |
| **INV-CPY-05-7** | Observer frame loss/coalescing MUST be reported by exact count/range and MUST NOT masquerade as consecutive frames. | Sequence/loss property tests |
| **INV-CPY-05-8** | Canvas Buffer writes MUST remain isolated from active rendering until a validated Safe Turn commit. | Concurrent mutation and commit tests |

## 10. Reconciliation Decisions

| Existing surface | CPY-05 treatment |
|---|---|
| `platform::Surface<'a>` | Internal mutable rendering primitive; never exported directly. |
| `platform::PixelFmt` | Rendering classification only. CPY adds exact byte-layout metadata and proves mapping. |
| `BlitPlanner` | Source of bounded damage; overflow maps to explicit full damage. |
| Hardware `FrontBuffer`/`BackBuffer` | Preserve typestate ownership; Python lease uses separately frozen storage or a proven immutable slot. |
| WLD Shadow Frame/SHM slots | WLD-private. Shared types require later cross-family reconciliation; no CPY relocation. |
| `bytes` | Optional copying convenience, not the canonical zero-copy frame boundary. |

## 11. Non-Goals and Decisions

### 11.1 Non-goals

- Exposing a mutable framebuffer to Python.
- Promising zero-copy all the way to every display backend.
- Requiring NumPy, Pillow, Cairo, OpenCV, or GPU APIs.
- Defining HDR, indexed palettes, DMA-BUF, or compressed-frame transport in the
  first profile.
- Using Python garbage-collection timing as the slot reuse protocol.

### 11.2 Resolved Decisions

- **PCDN-CPY-05-001 — Initial Pixel Layout — Accepted as amended
  2026-08-18.** Use the exact tightly packed `BGRA8888_LE_STRAIGHT` layout,
  `rlvgl-renderer-v1` color-space label, and canonical vectors in §5. A
  backend-local conversion before freeze does not weaken Python-side zero-copy.
- **PCDN-CPY-05-003 — Initial buffer shape — Accepted as amended
  2026-08-18.** Export only a one-dimensional, read-only, C-contiguous `B`
  view. Structured or multidimensional export is a later additive profile.
- **PCDN-CPY-05-004 — Observer loss — Accepted as amended 2026-08-18.** Never
  block native presentation or overwrite a lease. Skip publication while all
  observer slots are pinned and report the exact inclusive Frame-ID range/count
  on the next observer Frame or terminal lifecycle record.
- **PCDN-CPY-05-005 — Required presentation modes — Accepted as amended
  2026-08-18.** Require native-presented embedded observation and required-
  consumer host-headless capture. Python-driven physical scanout is optional.
- **PCDN-CPY-05-006 — Canvas Buffer scope — Accepted as amended
  2026-08-18.** Defer the public Canvas Buffer from the first release while
  reserving the isolated owned-storage/Safe Turn contract for Standards Action.

### 11.3 Open Decision

| PCDN | Question | Current disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-05-002` | What are the default/minimum/maximum Frame Slot counts per mode? | Remains open. Measure retained memory, pin duration, native cadence, conversion bandwidth, observer loss, and required-consumer latency on host and the CPY-01 SBC. Record configurable bounds and defaults in CPY-09; triple buffering is a candidate, not a decision. | CPY-05 ratification and CPY-09 budgets |

## 12. Acceptance Checklist

- [ ] Every PCDN in §§11.2–11.3 is resolved; `PCDN-CPY-05-002` remains open.
- [x] Exact Pixel Layout and canonical byte vectors are recorded.
- [ ] Slot lifecycle covers render, native present, every buffer export, close,
      and final release.
- [ ] Each mode has bounded saturation/loss behavior.
- [ ] Writable live-frame access is impossible by API and tests.
- [x] Damage overflow and observer loss policy is unambiguous.
- [ ] Cross-architecture and finalization lifetime proofs are specified.
- [ ] The owner records ratification in §15.

## 13. Files Cited

| File or authority | Role |
|---|---|
| `platform/src/blit.rs` | Current Surface, PixelFmt, damage, renderer |
| `platform/src/cpu_blitter.rs` | Deterministic software reference rendering |
| `platform/src/hwcore/surface.rs` | Typed framebuffer/DMA ownership |
| `platform/src/present.rs` | LPAR presentation planning |
| CPython buffer protocol/type-object docs | External acquire/export/release contract |
| `docs/wayland/WLD-00-CONCEPTS.md` | Adjacent private Shadow Frame/SHM ownership |

## 14. Unblocks

Five policy PCDNs are resolved, but CPY-05 remains Draft. Ratification is
blocked by measured slot counts in `PCDN-CPY-05-002`, CPY-02/03, cross-target
layout vectors, held-lease/finalization proofs, and owner acceptance of the
completed phase. Ratification and a headless held-lease proof would unblock
frame exposure in CPY-04, embedded presentation in CPY-06, and host capture in
CPY-07.

## 15. Change Log

### 0.2.0 — 2026-08-18 — frame policy PCDNs accepted as amended

**Author:** Ira Abbott

**Change kind:** semantic

**Touches:** INV-CPY-05-1, INV-CPY-05-2, INV-CPY-05-3, INV-CPY-05-4,
INV-CPY-05-5, INV-CPY-05-6, INV-CPY-05-7, INV-CPY-05-8,
PCDN-CPY-05-001, PCDN-CPY-05-003, PCDN-CPY-05-004, PCDN-CPY-05-005,
PCDN-CPY-05-006, §5, §7, §8, §11, §12, §14

**Commits:** pending

**Summary:** Fixes the first flattened Pixel Layout and vectors, one-dimensional
read-only Python buffer, exact observer-loss range, required headless/native
modes, and Canvas Buffer deferral while retaining measured slot counts as the
only phase PCDN.

#### Rationale

The software renderer already serializes `0xAARRGGBB` explicitly as little-
endian bytes, giving CPY a deterministic `[B,G,R,A]` publication oracle without
redefining rendering. A tight one-dimensional lease is broadly consumable and
keeps width/stride semantics explicit. Native cadence must not inherit Python
lease duration, while required-consumer headless mode must surface bounded
capacity instead of silently losing frames.

Considered and rejected: architecture-native `u32` naming, direct export of a
mutable or padded backend surface, multidimensional PEP 3118 metadata before a
consumer requires it, blocking native present behind pinned Python views,
silent latest-frame coalescing, mandatory Python-driven device scanout, and an
unowned first-release Canvas Buffer.

What deliberately did not change: no frame slot, lease, buffer exporter, copy,
conversion, capacity, or Canvas resource is implemented. WLD SHM ownership and
internal mutable `Surface`/framebuffer typestates remain with their authorities;
CPY-05 remains Draft and slot counts remain evidence-gated.

### 0.1.0 — 2026-08-18 — drafted

**Author:** Ira Abbott / OpenAI Codex (drafting)

**Change kind:** scope

**Touches:** none — new document

**Summary:** Defines exact flattened-frame metadata, immutable slot/lease lifecycle, read-only buffer export, presentation modes, and bounded observer backpressure.

#### Rationale

Python needs a real framebuffer boundary, not a copied screenshot convenience.
The exporter lifetime and native presentation cadence must therefore be
designed together before a `memoryview` is exposed.

Considered and rejected: exporting `Surface.buf` directly and returning only
`bytes`; the first is unsound across borrow/slot reuse, while the second hides
the intended zero-copy lifetime and copies every frame.

What deliberately did not change: internal Surface mutation, framebuffer
typestates, LPAR presentation semantics, and WLD SHM ownership remain with
their existing authorities.
