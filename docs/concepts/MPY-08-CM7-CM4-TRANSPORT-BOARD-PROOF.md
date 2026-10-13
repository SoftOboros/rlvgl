<!--
MPY-08-CM7-CM4-TRANSPORT-BOARD-PROOF.md - Dual-core transport and board-proof contract.
-->

# MPY-08 — CM7/CM4 Transport and Board Proof

**Status:** Draft 2026-08-09; MPY-05 backpressure proof obligation adopted
2026-08-16. Not ratified. Shared-memory placement, cache policy, signaling,
ready/enable flow control, and reset mechanics require platform evidence before
behavior implementation.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependency:
MPY-07 must prove the same scenarios in-process before this phase may add a
dual-core transport.

## 0. Authority Policy

| Concern | Owner | MPY-08 relationship |
|---|---|---|
| Logical IDs, values, frames, errors, correlation, and version negotiation | MPY-02 | Used without reinterpretation. |
| Stage behavior, snapshots, cues, safe turns, and MicroPython API | MPY-03 through MPY-06 | Transported without changing semantics. |
| Canonical scenarios and trace comparison | MPY-07 | Replayed as the board oracle. |
| STM32H747 memory, cache, barriers, HSEM/interrupt, reset, and boot mechanics | Applicable platform initiative, ST RM0399, and Arm architecture references | Platform authority must ratify the implementation choices; MPY does not restate register semantics. |
| Existing demonstration mailbox | `examples/stm32h747i-disco/src/ipc.rs` and paired linker scripts | Evidence of a working starting point, not the MPY protocol authority. |
| Logical channel roles, admission behavior, transport observability, and board acceptance | This document after ratification | MPY-08 is canonical. |

## 1. Purpose

Carry the already-proven MPY protocol between MicroPython on the STM32H747
CM7 and the native rlvgl runtime on CM4. The board proof must show that Python
can set the stage, create and direct actors, receive input cues, and orchestrate
later mutations while rlvgl performs layout, event dispatch, and rendering on
the runtime core.

## 2. Problem Statement

The in-tree demonstration IPC is not the MPY transport. It currently allocates
a 1 KiB D2 SRAM3 mailbox, sends fixed 20-byte application commands from CM4 to
a CM7 display server, returns fixed 20-byte UI events from CM7 to CM4, and uses
data-memory barriers around raw `#[repr(C)]` queues. MPY assigns the application
director to CM7 and the display/input runtime to CM4, so both logical directions
are reversed. Its typed, variable-length frames, results, cue guarantees, reset
identity, and discoverable capacity also exceed the existing format.

Silently reversing the current queue functions or treating a Rust structure's
memory layout as the wire protocol would preserve neither application behavior
nor compatibility. Likewise, a barrier alone cannot be assumed to resolve a
CM7 cacheability decision that has not yet been made and tested.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Director Core** | CM7, running the MicroPython VM and MPY binding; produces Commands and consumes Results, Cues, and Runtime Notices. | Owned by MPY-08. |
| **Runtime Core** | CM4, owning the Stage Registry, rlvgl runtime, display, and input; consumes Commands and produces Results, Cues, and Runtime Notices. | Owned by MPY-08. |
| **Command Ring** | Single-producer/single-consumer channel from Director Core to Runtime Core. | Transport of MPY-02 Commands/Batches. |
| **Return Ring** | Single-producer/single-consumer channel from Runtime Core to Director Core for Results, Cues, and Runtime Notices. | Transport of MPY-02/05 output frames. |
| **Critical Reserve** | Capacity that cannot be consumed by lossy/coalescible traffic and is sufficient to report an admitted request's result or a transport fault. | Owned by MPY-08. |
| **Boot Epoch** | Handshake identity that distinguishes one initialized channel lifetime from stale frames and handles left by a prior or partial reset. | Transport-local; reflected in diagnostics. |
| **Transport Profile** | Negotiated capacities, frame/fragment limits, capabilities, and board implementation revision. | Extends MPY-02 capability discovery. |
| **Transport Fault** | Structured, observable failure in framing, compatibility, coherency, overflow, progress, or peer lifecycle. | Maps to MPY-02 errors/notices. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Logical frame encoding and protocol version | MPY-02 |
| Cue class, coalescing, and loss metadata | MPY-05 |
| Public Python behavior | MPY-06 |
| Scenario inputs and expected semantic traces | MPY-07 |
| Current 1 KiB mailbox implementation | `examples/stm32h747i-disco/src/ipc.rs` |
| Current mailbox reservation | `examples/stm32h747i-disco/memory.x` and `memory_cm4.x` |
| Current board IPC limitations | `docs/bsp/STM32.md` |
| Historical CM7-director/CM4-runtime intent | `docs/future/MICROPYTHON-INTEGRATION.md` |
| Exact shared-memory/cache/signaling/boot implementation | Ratified platform phase plus linker and board code |
| Board trace and evidence manifest | This document and MPY-09 |

## 5. Frozen Decisions — Core Roles and Channels

The first board profile has exactly these ownership directions:

```text
CM7 Director Core                            CM4 Runtime Core
MicroPython + rlvgl module                   Stage + actors + display/input
        |                                               |
        +---- Command Ring: Command / Batch ----------->+
        |                                               |
        +<--- Return Ring: Result / Cue / Notice --------+
```

Each ring has one producer and one consumer. An interrupt, HSEM notification,
or polling loop MAY announce progress but MUST NOT transfer ownership outside
that rule. The Runtime Core is the only core allowed to mutate or traverse the
live stage and the only core allowed to perform actor layout, native dispatch,
or drawing. The Director Core owns Python objects and callable references; no
VM object enters shared memory.

The logical payload is the canonical MPY-02 byte encoding. Fixed-size transport
slots MAY fragment a frame, but reassembly must finish and validate before the
protocol accepts it. Fragment metadata is transport-local and cannot replace,
reinterpret, or expose a Rust/C structure layout as the logical frame format.

## 6. Frozen Decisions — Admission, Progress, and Backpressure

Commands and Batches are never silently dropped. Before the Runtime Core
accepts a request, the transport/runtime combination reserves enough return
capacity to satisfy MPY-02's exactly-one-Result rule. If that guarantee cannot
be made, the request remains unaccepted and the Director observes bounded
backpressure or an explicit capacity error.

The Return Ring applies the MPY-05 queue classes:

- Results, lifecycle cues, subscription changes, compatibility failures, and
  Transport Faults use protected capacity and are never silently coalesced.
- Ordered cues retain sequence order or report an explicit loss range.
- Coalescible input motion and scroll updates may merge only when their event
  descriptors permit it and the resulting cue carries merge/loss metadata.
- Critical Reserve cannot be consumed by coalescible traffic.

Both cores publish queue fill level, high-water mark, rejected admission,
coalesced/lost cue counts, malformed/incomplete frame counts, peer stalls, and
epoch changes through the Transport Profile or Runtime Notices. Blocking waits
must have a documented progress owner and timeout/watchdog policy; an
unbounded spin loop is not an MPY release behavior.

An enhanced board profile may advertise MPY-05 input pause or raw-event
retention only after proving a ready-and-enable handshake. CM7 publishes Return
Ring receive readiness or credits; CM4 derives a local task-level input-
admission enable. Physical input is dequeued and dispatched only while both
conditions hold, unless a separately bounded raw-input retention slot is
reserved. The proof must cover queue capacity, bounded reaction latency,
cache/barrier visibility, wraparound, peer stalls, saturation, and Boot Epoch
reset. No interrupt may block or spin on the handshake.

Until that proof exists, the profile uses MPY-05's conservative minimum:
reserve worst-case non-coalescible cue capacity before native input dispatch,
or reject the raw event before actor mutation and report Critical
`CueOverflow`. A hardware FIFO is not an implicit retention guarantee.

## 7. Frozen Decisions — Memory and Coherency Contract

MPY-08 does not preselect D2 SRAM3, D3 SRAM4, or another shared region. The
ratified platform decision must establish:

- one linker-owned base, size, and alignment visible identically to both
  images;
- proof that the range does not overlap either core's data, heap, stack,
  framebuffer, retained data, or another mailbox;
- CM7 MPU/cache attributes and any CM4 attributes that affect visibility;
- the exact initialization, publication, consumption, and wraparound ordering
  sequence, including required cache maintenance and architectural barriers;
- which core initializes each header/ring and when the peer may access it; and
- compile-time or startup assertions that the code and linked range agree.

Code must consume linker symbols or a generated board-memory descriptor; an
independently repeated address constant is insufficient. If the selected
region is cacheable on either core, the platform proof must specify and test
clean/invalidate granularity and ownership. If it is non-cacheable, the MPU and
linker evidence must prove that property. The current implementation's DMB
sequence is evidence to review, not a declaration that coherency is solved.

## 8. Frozen Decisions — Boot, Compatibility, and Recovery

Before either ring carries an application frame, the peers exchange or inspect
a versioned channel header containing at least:

- magic and transport-header revision;
- Boot Epoch and per-core ready state;
- supported MPY protocol range and selected version;
- descriptor/schema identity or compatible capability digest;
- slot, frame, fragment, text, actor, subscription, and snapshot capacities;
- required and optional capabilities; and
- diagnostic counters initialized for the new epoch.

No Stage opens until protocol/schema compatibility and required capabilities
succeed. A mismatch produces a stable diagnostic and leaves the runtime closed.
Full reset, either-core reset, firmware replacement, or channel reinitialization
must establish a new Boot Epoch, discard incomplete frames, reset ring
ownership, invalidate prior Stage/Object/Subscription handles, and repeat the
handshake. The mechanism that safely detects and sequences a single-core reset
belongs to the platform decision; treating retained queue bytes as current is
forbidden.

Signaling is an optimization over the ring state, not the source of truth. A
lost notification must not corrupt ownership or ordering; a bounded polling or
re-notification path must recover progress. Exact HSEM, interrupt, or polling
selection remains open until measured on the board.

## 9. Frozen Decisions — Invariants and Evidence

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-08-1** | CM7 MUST remain the sole MicroPython Director Core and Command producer, while CM4 MUST remain the sole rlvgl Runtime Core and Result/Cue producer. | Dual-image role audit plus forbidden-call/link tests. |
| **INV-MPY-08-2** | The board transport MUST carry canonical MPY-02 bytes and MUST NOT use native Rust/C object layout, pointers, or reversed legacy message meaning as its logical protocol. | Cross-core golden vectors, malformed-frame tests, and pointer-pattern scan. |
| **INV-MPY-08-3** | Shared-memory base, extent, alignment, non-overlap, cache attributes, cache maintenance, and barrier sequence MUST be owned by one ratified platform description and verified with both linked images. | Link-map audit, startup assertions, and cache-enabled board stress. |
| **INV-MPY-08-4** | Each ring MUST preserve single-producer/single-consumer ownership and the platform's ratified publication/consumption ordering across wraparound, notification loss, and peer stalls. | Ring model checks and board wrap/stall stress. |
| **INV-MPY-08-5** | Every accepted request MUST retain capacity for exactly one Result, and critical Results/Cues/Faults MUST NOT be silently displaced by coalescible traffic. | Saturation, credit/reserve, and intentional-overflow fixtures. |
| **INV-MPY-08-6** | A reset, compatibility failure, or Boot Epoch change MUST prevent stale frames and Stage/Object/Subscription handles from affecting the new runtime lifetime. | CM7-only, CM4-only, full-reset, and firmware-mismatch recovery tests. |
| **INV-MPY-08-7** | The board proof MUST replay the MPY-07 canonical scenarios with equivalent logical traces while reporting all target-specific capacities and transport-only timing separately. | Board trace comparator and checksummed evidence manifest. |

### 9.1 Required board proof

The initial STM32H747I-DISCO record includes:

1. a CM7 MicroPython REPL imports the public module, inspects the catalog, and
   creates the Container, Label, Button, Slider, and List proof actors;
2. Python commits requested flex layout, while CM4 computes geometry, draws,
   and returns an equivalent stage snapshot;
3. physical touch/button input becomes a queued Python callback cue, whose
   mutation appears on the following Safe Turn;
4. at least 1,000 mixed create/set/get/action/subscription operations exercise
   wraparound, fragmentation, backpressure, and correlation;
5. each core is deliberately stalled and reset, and the peer reports/recoveries
   match the Boot Epoch contract;
6. queue saturation proves Critical Reserve, declared cue coalescing, loss
   metadata, and eventual progress, including ready/credit withdrawal, CM4
   input-admission disable, bounded reaction, and the explicit-drop fallback;
   and
7. the relevant MPY-07 traces compare equal, with board timings and rendered
   capture retained as additional evidence rather than normalized semantics.

## 10. Reconciliation Decisions

| Existing surface | MPY-08 decision |
|---|---|
| `ipc.rs` `CmdQueue` (CM4 to CM7) | Legacy demo channel with the opposite application role. Replace through a versioned transport or retain under a separate legacy namespace; do not silently reverse it. |
| `ipc.rs` `EvtQueue` (CM7 to CM4) | Same reconciliation rule; it is not the MPY Return Ring. |
| Fixed 20-byte `CommandRaw`/`EventRaw` | Cannot encode the canonical actor protocol by native layout. Slots may be redesigned as explicit canonical-byte fragments. |
| 1 KiB `MAILBOX` at `0x3004_7000` | Candidate evidence only. Capacity and location require platform ownership and measured sizing. |
| DMB-only publication | Retained only if the selected memory attributes and architecture proof show it is sufficient; otherwise the ratified coherency sequence replaces it. |
| `cmd_push_blocking` spin | Test/demo behavior only unless a bounded progress and watchdog policy is ratified. |
| Historical HSEM/shared-SRAM sketch | Direction is adopted; exact signaling and memory placement remain platform decisions. |

## 11. Non-Goals and Open Decisions

1. **No protocol redesign for the board.** Transport headers/fragments are
   allowed, but command, result, cue, error, and version semantics remain MPY-02.
2. **No second stage replica on CM7.** Python wrappers and optional cached
   descriptor data are not a live object tree.
3. **No direct Python draw/input interrupt callbacks.** MPY-05 Safe Turns still
   govern delivery.
4. **No claim that the current mailbox is coherent or large enough.** Those are
   evidence-backed platform decisions.

- **PCDN-MPY-08-001:** Which shared-memory region, size, slot geometry, and
  linker section are selected after link-map, framebuffer, heap, and cache
  review?
- **PCDN-MPY-08-002:** Is the CM7 mapping non-cacheable, write-through, or
  explicitly maintained, and what exact publication/consumption sequence does
  the platform authority require?
- **PCDN-MPY-08-003:** Does the first profile signal through HSEM/interrupt,
  bounded polling, or a measured hybrid, and what recovers a lost signal?
- **PCDN-MPY-08-004:** Which MicroPython STM32 source revision, board port, boot
  order, and paired CM4 image form the reproducible board profile?
- **PCDN-MPY-08-005:** Is the current demo IPC replaced in place after its
  consumers migrate, or retained as a separately versioned legacy channel?

## 12. Acceptance Checklist

- [ ] `INV-MPY-08-1` fixes the CM7-director/CM4-runtime ownership direction.
- [ ] `INV-MPY-08-2` canonical-byte framing and legacy-format separation are accepted.
- [ ] `INV-MPY-08-3` leaves placement/coherency to a named ratified platform contract.
- [ ] `INV-MPY-08-4` SPSC ownership and progress recovery are accepted.
- [ ] `INV-MPY-08-5` admission reserve and no-silent-critical-loss rules are accepted.
- [ ] `INV-MPY-08-6` Boot Epoch reset and stale-handle behavior are accepted.
- [ ] `INV-MPY-08-7` reuses MPY-07 traces as the board semantic oracle.
- [ ] PCDN-MPY-08-001 through PCDN-MPY-08-005 are resolved with board evidence.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md`
- `docs/concepts/MPY-06-MICROPYTHON-DIRECTOR-BINDING.md`
- `docs/concepts/MPY-07-SAME-CORE-SIMULATOR-CONFORMANCE.md`
- `docs/concepts/DPR-00-CONCEPTS.md`
- `docs/concepts/DCB-00-CONCEPTS.md`
- `docs/future/MICROPYTHON-INTEGRATION.md`
- `docs/bsp/STM32.md`
- `examples/stm32h747i-disco/src/ipc.rs`
- `examples/stm32h747i-disco/memory.x`
- `examples/stm32h747i-disco/memory_cm4.x`
- ST RM0399 and the applicable Arm Cortex-M architecture references

## 14. Unblocks

After ratification, implementation, and a green board record, MPY-08 unblocks
MPY-09's board, capacity, transport-equivalence, and release-closure gates. It
does not by itself authorize an introspection-parity claim that remains open in
the MPY-01 matrix.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-08-1, INV-MPY-08-2, INV-MPY-08-3, INV-MPY-08-4, INV-MPY-08-5, INV-MPY-08-6, INV-MPY-08-7, INV-MPY-7, INV-MPY-8, PCDN-MPY-08-001, PCDN-MPY-08-002, PCDN-MPY-08-003, PCDN-MPY-08-004, PCDN-MPY-08-005, §0–§14

**Commits:** pending

**Summary:** Drafts the CM7 MicroPython director and CM4 rlvgl runtime roles,
canonical-frame shared-memory transport, admission reserve, boot epoch,
platform-owned coherency gate, legacy IPC reconciliation, and board proof.

#### Rationale

The existing board mailbox proves that the cores can communicate but assigns
the opposite application/display roles and exposes only small native-layout
messages. MPY requires a versioned replacement whose semantics have already
been proven in-process and whose memory visibility, reset behavior, and
capacity limits are explicit rather than incidental.

### 0.1.1 — 2026-08-16 — Reconciled

**Author:** OpenAI Codex with owner direction

**Change kind:** clarification

**Touches:** §0, §6, §9.1, §15

**Commits:** pending

**Summary:** Adopts MPY-05's evidence-gated ready-and-enable enhancement. The
board profile must prove Return Ring readiness/credits and CM4 task-level input
admission before advertising pause or raw-event retention; otherwise it uses
the observable pre-dispatch-loss baseline. MPY-08 remains Draft with all five
board PCDNs open.
