<!--
CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md - Flattened frame metadata, slot lifetime, and Python buffer export contract.
-->

# CPY-05 — Frame Lease and Buffer Protocol

**Document ID:** CPY-05-FRAME-LEASE-BUFFER-PROTOCOL

**Status:** Draft 2026-08-18. Not ratified.

**Revision:** 0.1.0

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

The initial exporter SHOULD expose one read-only, C-contiguous byte view with
`itemsize == 1`; dimensions, stride, and Pixel Layout remain explicit Frame
attributes. A multidimensional/structured export requires resolution of
`PCDN-CPY-05-003` and must not invent a misleading PEP 3118 format.

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

### 8.2 Headless/Python-presented

When Python is the required consumer/presenter, slot exhaustion MUST return a
typed Frame Busy/capacity outcome or honor an explicit bounded wait/timeout.
No call may block forever by default.

### 8.3 Canvas input

Writable Canvas Buffer storage MUST be distinct from live frame/scanout slots.
Publishing it to rlvgl MUST validate size/layout and transfer or snapshot its
content at an explicit Safe Turn. Python mutation after commit cannot race the
native draw traversal.

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

## 11. Non-Goals and Open Decisions

### 11.1 Non-goals

- Exposing a mutable framebuffer to Python.
- Promising zero-copy all the way to every display backend.
- Requiring NumPy, Pillow, Cairo, OpenCV, or GPU APIs.
- Defining HDR, indexed palettes, DMA-BUF, or compressed-frame transport in the
  first profile.
- Using Python garbage-collection timing as the slot reuse protocol.

### 11.2 Open Decisions

| PCDN | Question | Recommended disposition | Blocks |
|---|---|---|---|
| `PCDN-CPY-05-001` | What exact initial Pixel Layout is canonical? | Select one software-reference layout whose byte vector is proven on all required architectures; name bytes explicitly. | CPY-05 ratification |
| `PCDN-CPY-05-002` | What are the default/minimum/max frame-slot counts per mode? | Triple buffering for native observer as candidate; measure memory/cadence and make capacities configurable/bounded. | CPY-05 ratification and CPY-09 budgets |
| `PCDN-CPY-05-003` | Is the initial Python export 1-D bytes or multidimensional/strided? | 1-D read-only `B` view with explicit metadata; add structured views only after consumer evidence. | CPY-05 ratification |
| `PCDN-CPY-05-004` | What exact observer-loss policy applies when every exportable slot is pinned? | Preserve native present, skip/coalesce observer publication, and report exact skipped Frame IDs/count. | CPY-05 ratification |
| `PCDN-CPY-05-005` | Is Python-Presented Mode required in the first release? | Required for headless capture; optional for device scanout, where native presentation is primary. | CPY-05/07/09 claims |
| `PCDN-CPY-05-006` | Does Canvas Buffer ship in the first release? | Defer unless a named Python-drawing consumer is required; reserve the isolation contract now. | CPY-05 scope, not Frame Lease proof |

## 12. Acceptance Checklist

- [ ] Every PCDN in §11.2 is resolved.
- [ ] Exact Pixel Layout and canonical byte vectors are recorded.
- [ ] Slot lifecycle covers render, native present, every buffer export, close,
      and final release.
- [ ] Each mode has bounded saturation/loss behavior.
- [ ] Writable live-frame access is impossible by API and tests.
- [ ] Damage overflow and observer loss are unambiguous.
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

Ratification and a headless held-lease proof unblock frame exposure in CPY-04,
embedded presentation in CPY-06, and host capture in CPY-07.

## 15. Change Log

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
