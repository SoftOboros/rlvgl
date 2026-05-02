# DCB-00 — DMA Cacheable Buffers Concepts

**Status:** Ratified 2026-05-02 (§15); amended 2026-05-02 with
DCB-02-A resolution (DMA double-buffer-mode coverage); amended
2026-05-02 with DCB-04-A resolution (LTDC scanout typestate);
amended 2026-05-02 with DCB-03-A resolution (DMA2D destination
retrofit closed-with-deferral). DCB-01 / DCB-01b / DCB-01c
typestate APIs are shipped; the BASELINE-shrink track ends at
DCB-04. Per the parent CLAUDE.md spec-before-code discipline,
future modifications to the §5 typestate set, §6 layout
invariants, §8 engine submission contract, or §9 INV-D8..D13
require a §15 amendment **first**, in a separate PR, before any
behaviour PR rides on the change.

## 0. Authority policy

This doc is the **single normative source** for the
`rlvgl-platform` cache-coherent DMA buffer contract. It does not
redocument the M7 architecture reference; it cites it. The authority
split:

| Concern | Owner | DCB-side relationship |
|---|---|---|
| Cortex-M7 D-cache architectural semantics (line size, clean / invalidate / clean+invalidate, MPU attribute interaction) | ARM ARMv7-M Architecture Reference Manual; STM32 *Programming manual for Cortex-M7* (PM0253) | Cited; not redocumented. DCB assumes architectural defaults (32-byte cache lines, write-back write-allocate Normal memory). |
| STM32H747 cache controller wiring + per-region cacheability | RM0399 §4 (Cortex-M7 cache) and §2.3 (memory map) | Cited; not redocumented. DCB assumes the H747 cache is enabled and that buffers live in cacheable memory regions unless explicitly carved out via MPU. |
| `cortex-m::peripheral::SCB::{clean_dcache_by_address, invalidate_dcache_by_address, clean_invalidate_dcache_by_address}` and the `cortex-m-rt` interrupt model | `cortex-m` crate (≥0.7) | Consumed via published API. DCB owns the wrapper; ad-hoc callers in `rlvgl-platform` are migrated per §10. |
| `InFlight<'dma, T>` + `BorrowedForDma<'dma, T>` + `BackBuffer<'a>` aliasing/ownership lifecycle | `platform/src/hwcore/surface.rs` (Register-Mashing Discipline rule #3) | DCB **extends** these; it does not replace them. The cache-state typestate composes with the existing borrow lifecycle — see §10. |
| Per-DMA-engine submission API (DMA2D `start_*_typed`, future SAI `start_rx_dma`, SDMMC R/W) | `platform/src/dma2d.rs`, future SAI / SDMMC modules | Each engine's submission API takes a DCB-owned buffer and returns a typestate-transitioned token. The cache-clean/invalidate placement is fixed by DCB; the engine cannot opt out. |

If a phase needs to **modify** a cited authority (e.g. to assume a
different cache line size for a future MCU port, or to require an MPU
non-cacheable region for LTDC scanout), the modification ratifies in a
DCB §15 amendment **first** before the consumer phase lands.

## 1. Purpose

Make D-cache maintenance for DMA buffers **a property of the type
system**, not of the call site. Each cacheable RAM buffer used by a
DMA master has exactly one owner at any time — CPU, or device — and
the typestate transitions between those owners insert the correct
cache operation (clean before device-read, invalidate before
CPU-read-after-device-write) automatically. Application and driver
code cannot forget the cache op, cannot reorder it, and cannot apply
the wrong one.

This phase produces no executable artifacts. It establishes the
contract that DCB-01 (the typestate API) and DCB-02+ (per-engine
retrofits) build on.

## 2. Problem statement

**Recurring failure mode.** Across multiple sessions, the same class
of bug has cost real bench time:

- 2026-04-30 / 2026-05-01: SAI1 line-in/line-out loopback on the
  disco-analyzer subrepo presented as "loud bees" on the headphone
  output. Synth-generated output samples were correct; the codec
  was correct. Root cause: CPU writes to the SAI TX buffer remained
  in the M7 D-cache while the DMA controller read stale RAM. Fixed
  by adding a `clean_dcache_by_address` call after the half-buffer
  fill in `analyzer-cm7/src/main.rs`.
- Earlier sessions: equivalent cache-coherency hazards in the
  DMA2D submission path predate the `InFlight<T>` work; the existing
  `start_*_typed` API gates aliasing but not cache state.
- `platform/src/audio_player.rs:174` calls a private `clean_dcache`
  helper that bypasses the SCB ownership convention because plumbing
  `&mut SCB` through the player is intrusive (see comment at
  `audio_player.rs:248`). It works today only because the path is
  the only DMA writer to that buffer.
- `platform/src/sd_emmc_adapter.rs:39` uses
  `clean_invalidate_dcache_by_address` rather than
  `invalidate_dcache_by_slice` because the latter asserts cache-line
  alignment that the SD buffers do not guarantee. The "correct" op
  is being chosen call-by-call, by humans, at three different sites.

Every site is hand-written. Every site has been wrong at least once.
The CLAUDE.md "Register-Mashing Discipline" rule #3 already encoded
*aliasing* ownership for DMA via `InFlight<T>`, but cache state is
orthogonal to aliasing — and the H747 D-cache is the dominant hazard
on this MCU.

**Bootstrapping intent.** DCB-01 lands the typestate API in
`rlvgl-platform`. DCB-02 retrofits the SAI1 audio path on
disco-analyzer (the most active DMA consumer with the cleanest
isolation today) as the first user. DCB-03+ retrofits DMA2D, SDMMC,
audio_player, and (per §10) decides scanout policy: typestate
clean-on-publish vs MPU non-cacheable carve-out. The narrow goal is
that *new code cannot create a new cache/DMA race* — existing call
sites are grandfathered until their retrofit phase lands.

## 3. Glossary

Reserved DCB vocabulary. Capitalised use of these terms in DCB
docs MUST refer to the defined meaning; alternative phrasings
introduce drift and are forbidden in normative sections.

| Term | Meaning | Owner |
|---|---|---|
| **Cache line** | The granule at which the M7 D-cache moves data between cache and main memory. Architecturally 32 bytes on Cortex-M7. DCB buffers MUST be cache-line aligned and MUST be padded to a whole multiple of cache lines. | ARM ARMv7-M ARM. |
| **Cacheable region** | A range of physical address space mapped as Normal cacheable memory (write-back, write-allocate by default on H747). All H747 SRAMs (D1 SRAM, D2 SRAM, AXI SRAM) are cacheable from the M7's perspective unless an MPU region overrides them. | RM0399 §2 / §4. |
| **CPU-owned** | Typestate. The CPU may freely read and write the buffer. No DMA master is reading or writing it. The buffer's cache state is unconstrained — the CPU's view is authoritative. | DCB. |
| **Device-read-pending** | Typestate. A DMA master is about to read the buffer (or is reading it now). The buffer has been cleaned to RAM at the boundary so the DMA engine sees CPU-written data. The CPU MUST NOT touch it through this handle. | DCB. |
| **Device-write-pending** | Typestate. A DMA master is about to write the buffer (or is writing it now). Cache lines spanning the buffer are invalidated at the boundary so any subsequent CPU read returns DMA-written data. The CPU MUST NOT touch it through this handle. | DCB. |
| **Device-active circular** | Typestate. A DMA master is continuously reading or writing the buffer in circular / double-buffer mode. The CPU is permitted to access only the *inactive half* (or, more generally, the half not currently targeted by the DMA stream), through a guard that performs the appropriate cache op for that half. | DCB. |
| **Cache op** | One of the SCB primitives `clean_dcache_by_address` (CPU→device handoff), `invalidate_dcache_by_address` (device→CPU handoff), `clean_invalidate_dcache_by_address` (bidirectional handoff or unaligned-slice case). DCB owns *which* op is applied at *which* transition; user code does not choose. | DCB. |
| **DcaBuf\<T, N\>** | The owning, cache-line-aligned, cache-line-padded buffer type. Lives in CPU-owned typestate at construction. Type parameters: element type `T`, element count `N`. Reborrows produce typestate-transitioned guards that consume / restore ownership. | DCB. |
| **Half-guard** | The wait-free RAII handle returned when the CPU asks for the inactive half of a Device-active-circular buffer. Performs the cache op on entry (e.g. invalidate, for a device-writer ring) and revokes itself on drop / on next half toggle. | DCB. |
| **DcaDoubleBuf\<T, N\>** | The owning, cache-line-aligned, cache-line-padded buffer family for **STM32 DMA double-buffer mode** (M0AR + M1AR alternating). Holds two `[T; N]` banks that MAY be physically non-contiguous. Each bank independently satisfies INV-D1 / INV-D2 / INV-D3. Distinct from `DcaBuf<T, N>` because the engine register layout (M0AR + M1AR + `CT` bit + `MBM` flag) is different from circular mode. | DCB. |
| **Bank** | One of `M0` / `M1`; analogous to `Half` for the double-buffer-mode case. The DMA engine alternates which bank is the active write target (read target for TX-style direction); the CPU operates on the *inactive* bank. | DCB. |
| **Bank-guard** | The RAII handle returned when the CPU asks for the inactive bank of a `DeviceActiveDoubleBuf<DIR>` buffer. Performs the cache op on entry (clean for `Read`, invalidate for `Write`) and revokes itself on drop. Live-recheck on `release(current_target)` reads the engine's `CT` bit to detect bank flip during the guard's lifetime — the double-buffer-mode analogue of `HalfGuard`'s NDTR re-read. | DCB. |
| **CT bit (current target)** | Bit field in the STM32 DMA stream's `CR` register (bit 19 on H7) reporting which bank (M0 or M1) the engine is *currently* writing or reading. The single-read source-of-truth for `BankGuard::release`'s INV-D15 live-recheck. | RM0399 §15 (DMA). |
| **DeviceLtdcScan\<T, N\>** | Typestate for a buffer continuously read by a fixed-rate pixel-stream consumer (LTDC, DCMI display-out, parallel RGB) where the CPU paints in place between consumer reads. The engine never pauses; per-frame ordering between CPU writes and consumer reads is enforced by `present()` (cache clean + DSB, draining the AXI / interconnect write buffer). Distinct from `DeviceActiveCirc<DIR>` because there's no half / bank-rotation: the consumer reads the same buffer continuously and the CPU paints into it in place. | DCB. |
| **Scanout-FB region** | A `DcaBuf<T, N>` (or `DcaDoubleBuf<T, N>` for the swap-pair form) configured in `DeviceLtdcScan<...>` typestate. Lives in cacheable RAM with the Write-Through MPU attribute on STM32H7 platforms (the current disco default); the per-frame `present()` drains the AXI write buffer rather than writing back dirty cache lines. | DCB. |
| **MPU non-cacheable carve-out** | An alternative compliance path for buffers where per-frame cache maintenance is too expensive (LTDC scanout is the canonical case): an MPU region marks the buffer non-cacheable, and DCB's typestate becomes a no-op on that buffer. Covered in §10; not the default — DCB-04-A §3 / §4 ratified `DeviceLtdcScan` over MPU carve-out for the LTDC scanout class. | DCB. |

## 4. Source-of-truth map

The crates and APIs DCB depends on. **`rlvgl-platform` MUST NOT
reach inside these surfaces outside the cited entry points.** When a
cited entry point needs to grow, the vendor's spec lineage ratifies
the change first.

| Component | Pinned form | Used API surface | Vendor spec ref |
|---|---|---|---|
| `cortex-m` | `^0.7` (already a workspace dep) | `peripheral::SCB::{clean_dcache_by_address, invalidate_dcache_by_address, clean_invalidate_dcache_by_address}`; `peripheral::Peripherals::steal` (in `unsafe` blocks only, per Register-Mashing Discipline rule #7). | `cortex-m` 0.7 docs. |
| `rlvgl-platform` `hwcore::surface` | This crate | `BorrowedForDma<'dma, T>`, `InFlight<'dma, T>`, `BackBuffer<'a>`. DCB composes with these — see §10 #R-S1, #R-S2. | `platform/src/hwcore/surface.rs`. |
| `rlvgl-platform` `hwcore::addr` | This crate | `DmaAddr`, `PhysAddr`, `MmioAddr<T>` (Register-Mashing Discipline rule #4). DCB buffers expose `dma_addr()` returning `DmaAddr`. | `platform/src/hwcore/addr.rs`. |
| `rlvgl-platform` discipline scanner | This crate | The lint test at `platform/tests/discipline.rs` gains a new rule (DCB-01 amendment) that flags raw `clean_dcache_by_*` / `invalidate_dcache_by_*` calls outside DCB's owning module, with the same `BASELINE` exemption mechanism the existing rules use. Migration follows the staged-baseline pattern. | `platform/tests/discipline.rs`. |

Path-internal references that are NOT authoritative (DCB owns
their replacement):

- `platform/src/audio_player.rs` private `clean_dcache(ptr, len)`
  (lines 174, 253) — superseded by DCB-02b retrofit.
- `platform/src/stm32h747i_disco_sd.rs` direct `Sd::clean_dcache_by_slice`
  / `invalidate_dcache_by_slice` calls (lines 35, 46) — superseded by
  DCB-02c retrofit.
- `platform/src/sd_emmc_adapter.rs` direct
  `clean_invalidate_dcache_by_address` call (line 47) — superseded by
  DCB-02c retrofit. The unaligned-slice rationale (line 39 comment)
  becomes a §6 alignment invariant on DCB buffers; SDMMC retrofit
  uses cache-line-padded `DcaBuf` and drops the bidirectional op.

## 5. Frozen typestates

The DCB typestate set is **Standards Action** (per CLAUDE.md
registration policy). Adding, renaming, or removing a typestate
requires a §15 amendment to this doc and explicit go-ahead from the
owner. Demoting a typestate to a non-typestate runtime check is also
Standards Action.

```text
DcaBuf<T, N>            ─ owning storage; typestate is its current owner
   │
   ├─ CpuOwned          ─ CPU may read/write; cache state irrelevant
   │
   ├─ DeviceReadPending ─ DMA reader holds it; CPU blocked
   │     (entry: clean_dcache_by_address(buf, padded_bytes))
   │
   ├─ DeviceWritePending ─ DMA writer holds it; CPU blocked
   │     (entry: invalidate_dcache_by_address(buf, padded_bytes))
   │     (exit returning to CpuOwned: optional re-invalidate per §6 INV-D5)
   │
   └─ DeviceActiveCirc<DIR> ─ DMA in continuous transfer
         DIR ∈ {Read, Write}
         Half-guard API: half_guard(stream_pos) → HalfGuard<DIR>
            (HalfGuard entry: clean (Read) or invalidate (Write)
             on the inactive half)
            (HalfGuard drop: state revalidated against current
             stream_pos; if the DMA has crossed into this half,
             panic in debug, return-error in release per §9 INV-D7)
```

`DeviceActiveCirc<DIR>` deliberately omits a `ReadWrite` direction: no
`rlvgl-platform` consumer today drives a circular DMA that both reads
from and writes to the same buffer. If a future consumer needs that
shape (e.g. a chained M2M pipeline), the direction is added via §15
amendment with a named first user.

### Parallel family: `DcaDoubleBuf<T, N>` (DMA double-buffer mode)

For consumers using **STM32 DMA double-buffer mode** (M0AR + M1AR
alternating; engine never stops between banks), DCB exposes a
parallel storage family with a parallel typestate set. Selection
between `DcaBuf<T, N>` (circular) and `DcaDoubleBuf<T, N>`
(double-buffer) is at construction and is mutually exclusive — a
buffer family chooses one based on the engine's DMA mode and cannot
switch.

```text
DcaDoubleBuf<T, N>            ─ owning storage; two physically-disjoint
   │                            [T; N] banks; typestate is its current
   │                            owner.
   │
   ├─ CpuOwned                ─ same shape as DcaBuf::CpuOwned. CPU may
   │                            freely read and write either bank.
   │
   └─ DeviceActiveDoubleBuf<DIR> ─ DMA in continuous double-buffer
         DIR ∈ {Read, Write}      transfer; engine alternates between
                                  M0 and M1 on each TC.
         Bank-guard API: bank_guard(current_target: Bank)
                                  → BankGuard<DIR>
            (BankGuard entry: clean (Read), invalidate (Write)
             on the inactive bank — i.e. the bank the engine is
             *not* currently servicing per the CT bit)
            (BankGuard drop / release: re-read the engine's CT bit;
             if it names the same bank the guard exposed, the
             post-condition observation is a fault per §6 INV-D15:
             panic in debug, return-error in release)
```

The transitions, with ownership rules, are frozen as:

| From | To | Triggered by | Cache op inserted |
|---|---|---|---|
| `CpuOwned` | `DeviceActiveDoubleBuf<DIR>` | `buf.start_double_buffer(engine, DIR)` returning `DcaDoubleBuf<DeviceActiveDoubleBuf<DIR>>` | per-DIR initial op (clean for `Read`, invalidate for `Write`) over **both** banks |
| `DeviceActiveDoubleBuf<DIR>` | `DeviceActiveDoubleBuf<DIR>` (BankGuard scope) | `buf.bank_guard(current_target)` returning `BankGuard<DIR>` | per-DIR op on the *inactive bank only* (`clean` for `Read`, `invalidate` for `Write`) |
| `DeviceActiveDoubleBuf<DIR>` | `CpuOwned` | `buf.stop_double_buffer()` after engine-stop handshake | engine-DIR-dependent op over both banks (`clean` for `Read`, `invalidate` for `Write`) |

`DcaDoubleBuf<T, N>` storage layout: each bank is independently
`#[repr(C, align(32))]` with `[T; N_padded]` payload satisfying
INV-D1 / INV-D2 / INV-D3 (per §6 INV-D14). The two banks MAY live
at arbitrary disjoint addresses — wrapping pre-existing fixed-
address buffers (e.g. `0x3000_0000` + `0x3000_1000` for SAI1 RX) is
the supported construction path via an `unsafe fn from_addrs(m0,
m1)` constructor.

Like `DeviceActiveCirc<DIR>`, the `ReadWrite` direction is
deliberately omitted; no engine drives M0/M1 with both directions
simultaneously, and adding it would require a §15 amendment with a
named first user.

The transitions, with ownership rules, are frozen as:

| From | To | Triggered by | Cache op inserted |
|---|---|---|---|
| `CpuOwned` | `DeviceReadPending` | `buf.lend_for_read(engine)` returning `(DcaBuf<DeviceReadPending>, DmaAddr)` | `clean_dcache_by_address` over the padded extent |
| `CpuOwned` | `DeviceWritePending` | `buf.lend_for_write(engine)` returning `(DcaBuf<DeviceWritePending>, DmaAddr)` | `invalidate_dcache_by_address` over the padded extent |
| `CpuOwned` | `DeviceActiveCirc<DIR>` | `buf.start_circular(engine, DIR)` returning `DcaBuf<DeviceActiveCirc<DIR>>` | per-DIR initial op (clean, invalidate, or clean+invalidate) |
| `DeviceReadPending` | `CpuOwned` | DMA-engine completion handler returning the borrow | none (the device only *read* RAM; the CPU's cached copy is unchanged from before the transfer and remains authoritative) |
| `DeviceWritePending` | `CpuOwned` | DMA-engine completion handler returning the borrow | none at exit (the entry-side invalidate evicted the buffer's cache lines; INV-D3 forbids cache-line sharing with adjacent state, so no stale prefetch can reintroduce them during the transfer; the CPU's first read after exit therefore hits RAM and observes the DMA-written data) |
| `DeviceActiveCirc<DIR>` | `DeviceActiveCirc<DIR>` (HalfGuard scope) | `buf.half_guard(stream_pos)` returning `HalfGuard<DIR>` | per-DIR op on the *inactive half only*: `clean` for `Read`, `invalidate` for `Write` |
| `DeviceActiveCirc<DIR>` | `CpuOwned` | `buf.stop_circular()` after engine-stop handshake | engine-DIR-dependent op over the full padded extent (`clean` for `Read`, `invalidate` for `Write`) |

`DcaBuf<T, N>` itself is a `#[repr(C, align(32))]` newtype around
`[T; N_padded]` where `N_padded = round_up(N * size_of::<T>(),
CACHE_LINE) / size_of::<T>()`. Padding bytes are not exposed through
the CPU view (`as_slice` / `as_mut_slice` return `&[T; N]` / `&mut
[T; N]`).

### Continuous-read variant: `DeviceLtdcScan<T, N>` (LTDC scanout)

For consumers continuously read by a fixed-rate pixel-stream engine
(LTDC, DCMI display-out, parallel RGB) where the CPU paints in
place between consumer reads, DCB exposes a fifth typestate variant
on `DcaBuf<T, N>`: `DeviceLtdcScan<T, N>`. Selection between the
existing engine-driven typestates (`DeviceReadPending` /
`DeviceWritePending` / `DeviceActiveCirc<DIR>`) and this one is at
construction; a buffer family chooses one based on the consumer's
ownership model.

```text
DcaBuf<T, N>
   │ ... (existing typestates from the ASCII tree above)
   │
   └─ DeviceLtdcScan<T, N>      ─ engine reads continuously; never
                                   pauses. CPU paints in place.
         per-frame API:
            paint_full(&mut self) → &mut [T; N]
                — slice valid until the next present()
            present(&mut self, ctx: &mut DcaCacheCtx)
                — emits clean + dsb over the full padded extent;
                  no typestate transition (the engine is still
                  reading, did not pause). Per §6 INV-D16 BOTH
                  the cache clean AND the memory barrier are
                  required: the cache op alone does not flush
                  the AXI / interconnect write buffer.
            stop_scan(self, ctx: &mut DcaCacheCtx) → Cpu<'a, T, N>
                — caller MUST stop the consumer engine first;
                  emits a final clean + dsb before returning the
                  buffer to CpuOwned typestate.
```

The transitions, with ownership rules, are frozen as:

| From | To | Triggered by | Cache op inserted |
|---|---|---|---|
| `CpuOwned` | `DeviceLtdcScan<T, N>` | `buf.start_ltdc_scan(engine_handle, ctx)` returning `DcaBuf<DeviceLtdcScan<T, N>>` | `clean_dcache_by_address` + `dsb` over the padded extent (initial publish so the consumer's first scan reads CPU-written data) |
| `DeviceLtdcScan<T, N>` | `DeviceLtdcScan<T, N>` (paint scope) | `buf.paint_full() -> &mut [T; N]` returning a borrow valid until the next `present()` | none (paint accumulates in cache; ordering is enforced at `present`) |
| `DeviceLtdcScan<T, N>` | `DeviceLtdcScan<T, N>` (present checkpoint) | `buf.present(ctx)` | `clean_dcache_by_address` + `dsb` over the padded extent (drains the AXI write buffer per §6 INV-D16) |
| `DeviceLtdcScan<T, N>` | `CpuOwned` | `buf.stop_scan(ctx)` after engine-stop handshake | `clean_dcache_by_address` + `dsb` over the padded extent (final publish so the next phase's CPU read sees the buffer's terminal state) |

`DeviceLtdcScan<T, N>` deliberately **does not have a `DIR`
parameter**. The CPU is always the producer and the consumer
always reads — the inverse direction is a different typestate
family altogether (it would be a `DeviceLtdcGrab` for camera
input via DCMI; not in scope here, would ratify in a future §15
amendment). Likewise it does not have a `HalfGuard` /
`BankGuard`-style scope guard because the consumer reads the same
buffer continuously: there is no inactive-half / inactive-bank
to protect.

For the **swap-pair pattern** (the existing
`platform/src/hwcore/surface.rs::Scanout` type), the two
`FrameBuffer`s are independently wrapped as `DcaBuf<T,
N>::DeviceLtdcScan` handles. `Scanout::swap()` is software-driven
(it changes which handle the LTDC engine reads from); each
frame's `present()` is on the back-becoming-front. A parallel
`DcaDoubleBuf<T, N> + DeviceLtdcScanDouble<T, N>` typestate is
deferred until a hardware-managed swap (DBM / DSI command-mode
swap) ratifies a per-bank model.

## 6. Layout & alignment invariants

These are properties of `DcaBuf<T, N>` and of any region a DCB-owned
buffer overlaps. They are normative.

- **INV-D1: Cache-line alignment.** `DcaBuf<T, N>` MUST be aligned to
  the cache line size (32 bytes on M7). Encoded via
  `#[repr(C, align(32))]`.
- **INV-D2: Cache-line padding.** `DcaBuf<T, N>`'s storage MUST be
  padded so its byte length is a whole multiple of the cache line
  size. The padding bytes MUST NOT be reachable through the
  CPU-side `as_slice` / `as_mut_slice` accessors.
- **INV-D3: No cache-line sharing.** Two distinct `DcaBuf` instances
  MUST NOT share a cache line. INV-D1 + INV-D2 together imply this.
  Composite structs containing multiple `DcaBuf`s satisfy INV-D3 by
  construction; bare `[u8; N]` arrays of cache-aware sub-buffers
  do not, and are not permitted as DCB storage.
- **INV-D4: Single owner.** At any instant a `DcaBuf` is in exactly
  one of {`CpuOwned`, `DeviceReadPending`, `DeviceWritePending`,
  `DeviceActiveCirc<DIR>`}. The typestate makes this a compile-time
  property.
- **INV-D5: Spurious invalidate is safe; spurious clean is safe;
  spurious clean+invalidate is safe.** All three cache ops are
  idempotent over an aligned, padded extent. DCB MAY insert a
  cache op on a transition where it is strictly redundant, but it
  MUST NOT omit a required op.
- **INV-D6: Cacheable region required.** DCB typestate transitions
  emit cache ops unconditionally. If a buffer lives in an MPU
  non-cacheable region (per §10 MPU carve-out), the cache ops are
  no-ops at the hardware level but DCB MUST still emit them — the
  driver MUST NOT branch on cacheability at runtime. Carve-out is
  encoded by a different *constructor* (e.g. `DcaBuf::in_uncached_region`)
  whose returned type elides the cache ops at the typestate-transition
  boundary, not by a runtime flag.
- **INV-D7: Circular DMA half-ownership is enforced by stream
  position.** A `HalfGuard<DIR>` for the inactive half is constructed
  from the *current* stream position (`NDTR` for STM32 DMA, an
  equivalent on other engines). If the DMA crosses into the
  half during the guard's lifetime, the guard's `Drop`
  observation is a hard error: panic in `debug_assertions`,
  return-an-error / set-fault-flag in release builds (the exact
  release-mode policy is settled in DCB-01a).
- **INV-D14: `DcaDoubleBuf<T, N>` per-bank alignment / padding.**
  Each of the two banks of a `DcaDoubleBuf<T, N>` MUST
  independently satisfy INV-D1 (cache-line alignment) and INV-D2
  (`size_of::<T>() * N` is a multiple of `CACHE_LINE`). The two
  banks MAY live at arbitrary disjoint addresses; INV-D3 (no
  cache-line sharing) applies independently within each bank,
  and across the two banks even when they are non-adjacent. A
  `DcaDoubleBuf::from_addrs(m0, m1)` constructor for fixed-
  address banks asserts both bank addresses are cache-line
  aligned; a `[m0_addr, m0_addr + bank_bytes)` /
  `[m1_addr, m1_addr + bank_bytes)` extent must be cleanly
  separated (no overlap), but the *gap between banks* is
  unconstrained.
- **INV-D15: Double-buffer DMA bank-ownership is enforced by the
  engine's CT bit.** A `BankGuard<DIR>` for the inactive bank is
  constructed from the *current* `CT` value read from the DMA
  stream's `CR` register (bit 19 on STM32H7). The CT-based check
  is the double-buffer-mode analogue of INV-D7's NDTR check, and
  is strictly simpler: a single 32-bit register read disambiguates
  active vs inactive bank with no half-mark math. If the engine
  flips CT into the bank exposed by the guard during the guard's
  lifetime, `BankGuard::release(current_target)` observation is a
  hard error per the same release-mode policy as INV-D7
  (`debug_assertions` → panic; release → return error).
- **INV-D16: `DeviceLtdcScan<T, N>` ordering contract.** The
  `present()` call MUST emit *both* a `DcaCache::clean` over the
  buffer's full padded extent **AND** a memory barrier (`dsb` on
  Cortex-M; `__DSB()` equivalent on other targets) that drains
  the AXI / interconnect write buffer. The individual `clean` is
  insufficient on Cortex-M7 + Write-Through SDRAM because the
  AXI write buffer can hold pending writes past the cache
  write-back; the DSB is the load-bearing ordering primitive.
  `start_ltdc_scan` and `stop_scan` carry the same contract: a
  `clean` *plus* a `dsb` are both required at the typestate
  boundaries. Implementations of `DeviceLtdcScan<T, N>` MUST
  invoke the DSB through the platform's typed barrier wrapper
  rather than via a raw `cortex_m::asm::dsb()` call site —
  consolidating the DSB through DCB's owning module preserves
  the discipline-scanner whitelist for the underlying
  architecturally-mandated barrier intrinsic.

## 7. Cross-core / multi-master scope

DCB is **single-master per buffer**. A `DcaBuf` has one DMA engine
on the device side at any time. Multi-master scenarios — for example,
a buffer simultaneously read by SAI TX-DMA and written by another
DMA engine, or a buffer shared across CM7 and CM4 — are out of scope
for the DCB-00 contract.

The CM7/CM4 cross-core case is governed by a separate concept (the
DAA subrepo's DAA-03 §7 / INV-D14: "no D-cache maintenance on
cross-core shared regions; the regions live in D3 SRAM4 which is not
cached from CM7"). DCB and the cross-core convention compose: a
buffer owned by DCB MUST live in a cacheable region; a buffer in D3
SRAM4 is not a DCB buffer.

This is a **non-goal**, not a deferred goal. The single-master
scope is what makes the typestate sound. If a future use case needs
multi-master cache coherency, a DCB-NN amendment ratifies the
extended typestate first.

## 8. Engine submission API contract

A DMA engine's submission API consumes a `DcaBuf` in `CpuOwned`
state plus its own borrow on the destination handle (e.g.
`BorrowedForDma<'dma, BackBuffer<'fb>>` for DMA2D), and returns:

```text
fn start_<engine>_<verb>(
    engine: &mut Engine,
    buf:    DcaBuf<T, N, CpuOwned>,
    /* engine-specific config */,
) -> InFlight<'dma, EngineToken<DcaBuf<T, N, DeviceXxxPending>>>
```

The key contract:

- The submission API MUST take `DcaBuf<..., CpuOwned>` by value.
  Typestate transition consumes the CPU-owned token; the buffer
  cannot be used by another caller during the transfer.
- The `InFlight` payload type MUST be the *transitioned* `DcaBuf`,
  not the original. Completion handlers consume the `InFlight`,
  observe the engine's "DMA done" condition, and return the
  buffer in `CpuOwned` again.
- The submission API MUST call `clean_dcache_by_address` (read
  direction) or `invalidate_dcache_by_address` (write direction)
  *before* arming the engine. The DCB typestate constructor
  enforces this — the engine never sees an un-cleaned `DcaBuf` in
  `DeviceReadPending` state, because that constructor is the only
  way to get one.
- Circular submission returns `DcaBuf<..., DeviceActiveCirc<DIR>>`,
  not an `InFlight`. The caller stops the engine via a separate
  `stop_circular` method that consumes the transitioned `DcaBuf`
  and returns it to `CpuOwned`.

The existing `BackBuffer<'a>` / `BorrowedForDma<'dma, T>` /
`InFlight<'dma, T>` chain is *the model* for this contract; DCB adds
the cache typestate on the inside of the DMA-side payload `T`. See
§10 reconciliation entries.

## 9. Discipline invariants

These are normative for `rlvgl-platform` consumers once DCB-01 lands.

- **INV-D8: Forbid raw cache ops in cacheable-DMA paths.** The
  discipline scanner (`platform/tests/discipline.rs`) gains a rule
  `raw_dcache` that flags any direct call to
  `SCB::{clean,invalidate,clean_invalidate}_dcache_by_{address,slice}`
  outside DCB's owning module. The rule respects the existing
  `BASELINE` exemption mechanism; the migration follows the same
  staged-baseline pattern as Register-Mashing Discipline rules
  #1–#7.
- **INV-D9: New DMA buffers MUST use `DcaBuf`.** Any new DMA
  destination/source buffer in cacheable RAM that lands after
  DCB-01 ratifies MUST be a `DcaBuf<T, N>` (or live in an MPU
  non-cacheable carve-out per §10). Bare `[T; N]` DMA buffers are
  a discipline violation.
- **INV-D10: Existing call sites are grandfathered.** The three
  pre-existing manual cache ops named in §4 (`audio_player.rs`,
  `stm32h747i_disco_sd.rs`, `sd_emmc_adapter.rs`) are exempt from
  INV-D8 via `BASELINE` until DCB-02b / DCB-02c retrofits land.
  No new `BASELINE` entries MAY be added after DCB-01 ratifies.
- **INV-D11: HalfGuard observation is mandatory.** A
  `HalfGuard<DIR>` whose lifetime ends without the user observing
  the post-condition (i.e. by reading the half slice the guard
  exposed) is not an error in itself. But a HalfGuard whose
  `Drop` detects the DMA stream has crossed into the guarded half
  during the guard's lifetime is a fault per §6 INV-D7. This MUST
  be implemented as a runtime check; making it a compile-time
  property would require linear types.
- **INV-D12: `unsafe` containment.** Every `unsafe { ... }` block
  inside DCB's implementation MUST carry the `// SAFETY:` comment
  required by Register-Mashing Discipline rule #7. DCB extends
  the existing convention; it does not relax it.
- **INV-D13: SCB ownership consolidation.** The `&mut SCB` borrow
  required to drive `clean_dcache_by_*` / `invalidate_dcache_by_*`
  MUST be plumbed through DCB construction sites only. New code in
  `rlvgl-platform` and downstream consumers MUST NOT take a
  `&mut SCB` borrow outside of (a) constructing a `DcaBuf` family,
  (b) constructing a DCB-owning engine driver (e.g. `AudioPlayer`,
  `SdmmcEngine`), or (c) the existing pre-DCB call sites
  grandfathered by INV-D10. This is the inverse of §11's "DCB does
  not change the `cortex-m` SCB ownership convention": DCB does
  consolidate *where* the convention is invoked, so that
  `&mut SCB` plumbing through application code (the
  `audio_player.rs:248` rationale) does not recur. New
  `&mut SCB` borrow sites added after DCB-01 ratifies are a
  discipline violation; the scanner SHOULD grow a follow-on rule
  `raw_scb_for_cache` to make this enforceable, with `BASELINE`
  initialised from the same three sites as INV-D10.

## 10. Reconciliation with adjacent primitives

DCB does not exist in isolation. Each row below names how DCB
composes with — or replaces — an existing `rlvgl-platform`
primitive. Consumer code follows the *Composition* column.

| Adjacent primitive | DCB relationship | Composition |
|---|---|---|
| `BackBuffer<'a>` / `BorrowedForDma<'dma, T>` / `InFlight<'dma, T>` (Register-Mashing rule #3) | **Closed-with-deferral (DCB-03-A 2026-05-02).** The originally-prescribed shape — "DMA2D destination becomes a `DcaBuf` *containing* the framebuffer pixels, with `BackBuffer` as the format/geometry view; `start_*_typed` takes `DcaBuf<u8, FB_BYTES, CpuOwned>` and returns an `InFlight` whose Drop releases the cache typestate" — remains the correct shape *if and when* a non-Write-Through DMA2D destination materializes. On the only platform that currently uses DMA2D (STM32H7 disco), every destination buffer lives in MPU-Write-Through SDRAM (`platform/src/stm32h747i_disco.rs:1322`); the `lend_for_write` runtime cache op would be a functional no-op. The §10 prescription is preserved here as design hygiene; until a consumer needs it the existing `BackBuffer` / `BorrowedForDma` / `InFlight` chain (Register-Mashing rule #3) is grandfathered, and INV-D9 ("new DMA buffers MUST use DcaBuf") is read as "new DMA buffers in *new* DMA paths". **Reopen triggers** — any of the following warrants a DCB-03-B sub-letter analysis with a named first user: (a) a new DMA2D destination in cacheable RAM that is NOT MPU-Write-Through (D1 SRAM scratch, AXI SRAM tile cache, glyph atlas in cacheable region, M2M-only buffer pairs that stay off-screen); (b) a port to a chip / board where SDRAM is not pre-MPU-configured Write-Through (any non-disco platform, or a future H7 bring-up that disables Write-Through for bus-utilization reasons); (c) the SDRAM Write-Through configuration in `configure_mpu_sdram_writethrough` is itself revised (e.g. switched to Normal Write-Back). | Existing DMA2D consumers continue to call `start_fill_typed` / `start_blit_typed` unchanged. Future consumers in non-Write-Through cacheable RAM route through `DcaCacheCtx::cache.invalidate(...)` per the DCB-02c / DCB-04 trait-dispatch pattern, *or* land DCB-03-B with the named first user. |
| `Scanout` (LTDC double-buffer) | **Decision deferred to DCB-04.** LTDC continuously reads the front buffer at frame rate; per-frame cache ops are expensive. Two options, **structurally distinct, not equivalent alternatives**: (a) keep the front buffer cacheable, wrap it in `DcaBuf<..., DeviceActiveCirc<Read>>`, and clean only the dirty rectangle on present — DCB ownership and cache discipline apply at runtime; (b) MPU-mark the scanout pair non-cacheable and use `DcaBuf::in_uncached_region` — DCB ownership still applies but cache ops elide *at construction*, and the MPU table becomes part of the platform's frozen memory map. Option (b) trades runtime cost for static-config rigidity (every consumer of the front buffer must accept non-cacheable read latency); option (a) keeps the buffer cacheable for incidental CPU reads (screenshots, FB dumps) at the cost of per-frame maintenance. DCB-00 §11 lists this as a non-goal at this phase; DCB-04 ratifies the choice — and DCB-04 MUST NOT default to (a) on the assumption that the typestate-everywhere story is uniform. | Pending. Code continues to use the existing `Scanout` API verbatim until DCB-04. |
| `audio_player.rs` private `clean_dcache(ptr, len)` (line 253) | **Replaces.** The audio-player TX buffer is a single-master DMA-read scenario with no special cache geometry. Becomes a `DcaBuf<i16, PLAYER_BUF_SAMPLES>` produced by an `AudioPlayer::lend_chunk_for_tx()` API that internally returns `DeviceReadPending`. The `static SCB`-bypass comment at `audio_player.rs:248` is resolved by the typestate API owning a single `&mut SCB` borrow at construction. | DCB-02b retrofit. Removes the private helper. |
| `stm32h747i_disco_sd.rs` `Sd::clean_dcache_by_slice` / `invalidate_dcache_by_slice` (lines 35, 46) | **Replaces.** SDMMC R/W buffers become `DcaBuf<u8, BLOCK_BYTES>`. The W path lends `DeviceReadPending`; the R path lends `DeviceWritePending`. Cache-line-padded storage means the unaligned-slice rationale at `sd_emmc_adapter.rs:39` no longer applies (INV-D2 forces the multiple-of-line size), and the bidirectional `clean_invalidate` collapses to the directional op. | DCB-02c retrofit. |
| `sd_emmc_adapter.rs` `clean_invalidate_dcache_by_address` (line 47) | **Replaces.** Same lifecycle as the `_disco_sd.rs` retrofit. The aligned-padded storage removes the original justification for the bidirectional op. | DCB-02c retrofit. |
| DAA `analyzer-cm7/src/main.rs` SAI1 TX clean (current bench fix) | **Replaces.** The SAI1 line-out TX ring becomes a `DcaBuf<i16, SAI1_TX_BUF_HALFWORDS, DeviceActiveCirc<Read>>`, and the per-half-fill code path becomes a `HalfGuard<Read>` whose construction takes the current `NDTR` and whose existence enforces "DMA is on the other half". The manual `clean_dcache_by_address` call disappears. | DCB-02 retrofit, **shipped 2026-05-02** (disco-analyzer `c117a20`). The `rlvgl-platform` API surface lands in DCB-01 (`a56987b`). Bench-validation gate (§12 (b)) is hardware-flash + audio reproduction; pending. |
| DAA `analyzer-audio/src/sai1_linein.rs` SAI1 RX invalidate (line 279) and SAI4 PDM RX (`sai4_pdm.rs`) | **Replaces.** Both RX paths use **STM32 DMA double-buffer mode** (M0AR + M1AR alternating) with non-contiguous physical banks. They become `DcaDoubleBuf<i16, SAI1_DMA_HALFWORDS>` (and equivalent for PDM) with `DeviceActiveDoubleBuf<Write>` + `BankGuard<Write>` per the parallel typestate family added by the DCB-02-A resolution. The manual `scb.invalidate_dcache_by_address` calls disappear; the bank-guard's entry op performs the invalidate and INV-D15's CT-bit re-check enforces "engine is on the other bank". | DCB-02-R retrofit. Lands after DCB-01b ships `DcaDoubleBuf` / `BankGuard<DIR>` / `DeviceActiveDoubleBuf<DIR>`. Future users: SDMMC streaming reads, USB HS bulk endpoints, DCMI camera frame-grab. |
| LTDC scanout pre-clean (`examples/stm32h747i-disco/src/freertos_entry.rs:1011, 1449` — single-FRONT model; and the `platform/src/hwcore/surface.rs::Scanout` swap-pair pattern) | **Replaces.** Wraps the buffer in the `DeviceLtdcScan<T, N>` typestate added by the DCB-04-A resolution. `paint_full()` returns a `&mut [T; N]` for in-place CPU writes; `present()` emits the existing clean + DSB pattern (which on STM32H7 + Write-Through SDRAM acts as an AXI-write-buffer drain per §6 INV-D16, *not* as a cache write-back). The manual `scb.clean_dcache_by_address` calls disappear; the last `raw_dcache` BASELINE entry clears, unblocking `RLVGL_LINT_STRICT=1` (§12 (c)). | DCB-04 retrofit. Targets: `freertos_entry.rs` single-FRONT path; `surface.rs::Scanout` swap-pair path (a parallel `DeviceLtdcScanDouble` for hardware-managed swap is deferred until needed). Lands after DCB-01c ships the new typestate. |
| DAA `analyzer-cm4` shared-memory cross-core regions (DAA-03 D3 SRAM4 pool) | **Out of scope.** Per §7. Cross-core blocks live in D3 SRAM4 which CM7 does not cache; DCB does not own them. | DAA-03 §7 / INV-D14 governs. |
| Future SDMMC DMA, USB endpoint buffers, QSPI memory-mapped writes | **In scope; later phases.** Each peripheral's submission API gains a `DcaBuf` parameter when its DCB-NN retrofit phase lands. | Pending. |

The composition with the existing `InFlight` chain is the load-bearing
design choice. DCB does not introduce a parallel ownership system; it
adds a cache-state dimension to the payload type the existing
`InFlight` already carries.

## 11. Non-goals

- **DCB does not mandate MPU non-cacheable regions.** §10 leaves the
  scanout-vs-MPU choice to DCB-04. DCB-00 only ensures that *if* a
  buffer is in cacheable memory, its cache state is type-managed.
- **DCB does not address cross-core coherency.** Per §7. CM7↔CM4
  shared regions are governed by the DAA-03 (or successor) cross-core
  contract.
- **DCB does not address coherency between two DMA masters.** Per §7.
  Single-master per buffer is the spec's safety boundary.
- **DCB does not retrofit existing call sites in DCB-01.** DCB-01
  ratifies the API and lands the type. DCB-02 + DCB-02b + DCB-02c
  perform the per-site migrations. The discipline scanner
  `BASELINE` mechanism makes the staged migration visible.
- **DCB does not promise zero overhead for circular DMA.** A
  `HalfGuard<DIR>` performs a cache op on entry over the inactive
  half (16 KB of audio TX ring → ~512 cache lines → microseconds at
  CM7 cache speed). Callers that can tolerate batched / coalesced
  cache ops MAY queue multiple half-fills before guard release in a
  later optimisation phase; that optimisation is not DCB-00 scope.
- **DCB does not change the `cortex-m` SCB ownership convention.**
  The `&mut SCB` borrow plumbing remains owner-side. DCB owns one
  `&mut SCB` per `DcaBuf` family at construction (typically held by a
  `DcaCacheCtx` token or the engine driver itself).

## 12. Acceptance checklist (initiative-level)

A conforming `rlvgl-platform` deployment satisfies DCB once each of
the following lands and is marked "ratified" in §15. DCB is
considered "in flight" until all of (a)–(c) hold.

- (a) `DcaBuf<T, N>`, the typestate set from §5, the `HalfGuard<DIR>`,
  the discipline-scanner rule `raw_dcache` (with starting `BASELINE`
  populated by the three call sites in §4) all land in
  `rlvgl-platform`. Consumers can construct and use a `DcaBuf` with
  no manual cache calls. (DCB-01 phase.)
- (b) The disco-analyzer SAI1 line-in / line-out path on the
  `analyzer-cm7` binary uses `DcaBuf<..., DeviceActiveCirc<DIR>>` +
  `HalfGuard<DIR>` for both RX and TX rings, with no remaining
  manual `clean_dcache_by_address` / `invalidate_dcache_by_address`
  calls in the audio path. Bench-flash on the H747I-DISCO board
  reproduces the current "synth tone clean / live-mic clean" result
  with the typestate API. (DCB-02 phase.)
- (c) `audio_player.rs`, `stm32h747i_disco_sd.rs`, `sd_emmc_adapter.rs`
  retrofits land. The `BASELINE` exemption list for `raw_dcache`
  shrinks to empty, and the `RLVGL_LINT_STRICT=1` mode added by
  DCB-01 enforces it. (DCB-02b + DCB-02c phases.)

A conforming deployment MAY additionally satisfy:

- (d) `Scanout` / LTDC scanout policy ratified per DCB-04 (typestate
  vs MPU carve-out). Independently conformant either way.
- (e) Future DMA2D / SDMMC / USB / QSPI retrofits per DCB-03 / DCB-05
  / etc.

## 13. Files cited

Existing rlvgl-platform code that DCB extends or replaces:

- `platform/src/hwcore/surface.rs:280-395` — `BackBuffer`,
  `BorrowedForDma`, `InFlight` (the existing aliasing-ownership
  chain DCB composes with).
- `platform/src/hwcore/addr.rs` — `DmaAddr`, `PhysAddr`,
  `MmioAddr<T>` (Register-Mashing rule #4 address-domain types).
- `platform/src/dma2d.rs:495-572` — DMA2D `start_*_typed` API
  returning `InFlight<'b, BackBuffer<'fb>>` (the model for DCB
  engine-submission contracts).
- `platform/src/audio_player.rs:174,248,253` — private
  `clean_dcache` helper (DCB-02b replacement target).
- `platform/src/stm32h747i_disco_sd.rs:35,46` — direct
  `Sd::clean_dcache_by_slice` / `invalidate_dcache_by_slice`
  (DCB-02c replacement targets).
- `platform/src/sd_emmc_adapter.rs:39-57` — `clean_invalidate_dcache_by_address`
  + the unaligned-slice rationale that DCB-02 §6 INV-D2 obsoletes.
- `platform/tests/discipline.rs` — discipline scanner; DCB-01 adds
  the `raw_dcache` rule.

Cross-repo references (read-only):

- `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs` —
  current SAI1 TX `clean_dcache_by_address` site (DCB-02 first-user
  retrofit).
- `streamz/submodules/disco-analyzer/analyzer-audio/src/sai1_linein.rs` —
  SAI1 RX/TX ring buffer constants `SAI1_TX_BUF_HALFWORDS`,
  `SAI1_TX_BUF_HALF`, `SAI1_DMA_HALFWORDS` (the geometry the first
  `DcaBuf` consumer uses).
- `streamz/submodules/disco-analyzer/docs/concepts/DAA-03-CONCEPTS.md`
  §7 / INV-D14 — cross-core D3 SRAM4 convention that bounds DCB's
  scope per §7 here.

Authority documents:

- ARM ARMv7-M Architecture Reference Manual — D-cache semantics.
- STM32 PM0253 *Cortex-M7 programming manual* — cache controller
  behaviour for STM32 parts.
- RM0399 §4 (Cortex-M7 cache) and §2.3 (memory map) — H747-specific
  cacheability and memory map.

## 14. Unblocks

Ratifying DCB-00 unblocks the following work:

- **DCB-01** — Land `DcaBuf<T, N>`, `CpuOwned`, `DeviceReadPending`,
  `DeviceWritePending`, `DeviceActiveCirc<DIR>`, `HalfGuard<DIR>` in
  `rlvgl-platform`. Add the `raw_dcache` discipline rule with starting
  `BASELINE`. Compile-fail trybuild fixtures for the typestate
  transitions.
- **DCB-02** — First user: disco-analyzer SAI1 **TX** ring on the
  bench. **Shipped 2026-05-02** as disco-analyzer commit `c117a20`
  (combined with bench-9l foundation work). The manual
  `scb.clean_dcache_by_address` in the SAI1 TX drain loop is
  replaced by `HalfGuard<Read>`. Consumes the DCB-01 API surface
  via a temporary local-path patch on `rlvgl-platform`; reverts to
  the published version once DCB-01 publishes. Bench-validation
  gate (§12 (b) — synth tone / live-mic reproduction) is
  hardware-dependent and tracked separately.
- **DCB-01b** — Land `DcaDoubleBuf<T, N>`, `Bank`, `BankGuard<DIR>`,
  and the `DeviceActiveDoubleBuf<DIR>` typestate in
  `platform/src/hwcore/dca.rs`. Add trybuild compile-fail fixtures
  parallel to `dca_use_after_lend.rs` / `dca_double_lend.rs` /
  `dca_half_guard_double.rs` covering the new typestate. No changes
  to existing `DcaBuf` typestates. Unblocked by the 2026-05-02
  DCB-02-A resolution amendment in §15.
- **DCB-02-R** — Retrofit `Sai1LineInSource` (analyzer-audio) and
  `Sai4PdmSource` onto `DcaDoubleBuf` + `BankGuard<Write>`.
  Removes the manual `scb.invalidate_dcache_by_address` at
  `sai1_linein.rs:279` and the equivalent in `sai4_pdm.rs`.
  Bench-flash reproduces the current FFT / spectrum result with
  the typestate API. Unblocked by DCB-01b.
- **DCB-02b** — `audio_player.rs` retrofit; remove private
  `clean_dcache` helper.
- **DCB-02c** — `stm32h747i_disco_sd.rs` + `sd_emmc_adapter.rs`
  retrofit; collapse the bidirectional `clean_invalidate` to
  directional ops via padded storage.
- **DCB-03** — **Closed (deferred), 2026-05-02.** DMA2D
  destination retrofit. Closure rationale and reopen path:
  DCB-03-A. The originally-prescribed shape (FrameBuffer
  composes `DcaBuf<u8, FB_BYTES>`; `start_*_typed` threads the
  typestate) is the correct design when a consumer needs it,
  but on the only current platform with DMA2D (the disco) every
  destination buffer is in MPU-Write-Through SDRAM where the
  cache op is a no-op. The §10 row preserves the prescription
  with explicit reopen triggers; `DcaCacheCtx`-route is
  available for ad-hoc future consumers without reopening.
  **Reopen with DCB-03-B** when any of: (a) a non-Write-Through
  DMA2D destination is needed, (b) a port without H7-style
  Write-Through SDRAM defaults adopts DMA2D, (c) the disco's
  own SDRAM Write-Through config changes. Any of those would
  warrant a fresh DCB-03-B sub-letter naming the first user
  before the FrameBuffer rearchitect lands.
- **DCB-01c** — Land `DeviceLtdcScan<T, N>` in
  `platform/src/hwcore/dca.rs`. Add trybuild compile-fail fixtures
  parallel to existing dca-typestate fixtures (use-after-paint,
  double-paint rejection, present-without-stop sound). No changes
  to existing `DcaBuf` / `DcaDoubleBuf` typestates. Unblocked by
  the 2026-05-02 DCB-04-A resolution amendment in §15.
- **DCB-04** — Retrofit
  `examples/stm32h747i-disco/src/freertos_entry.rs:1011, 1449`
  onto `DeviceLtdcScan<u8, FB_BYTES>` + `present()`. Removes the
  last `raw_dcache` BASELINE entry, unblocking
  `RLVGL_LINT_STRICT=1` (§12 (c) acceptance gate). The bare-metal
  `Scanout`-based render path can adopt the same typestate in
  the same PR or follow-up. Sub-letter analysis: DCB-04-A
  (resolved 2026-05-02 — Option A: typestate-on-publish over MPU
  non-cacheable carve-out, on bench-behaviour-preservation and
  portability grounds).

The following work remains *blocked* by DCB-00 ratification (no
behaviour MAY land touching these surfaces until DCB-00 ratifies):

- Any new DMA buffer added to `rlvgl-platform` cacheable-RAM paths.
- Any modification to the existing manual `clean_dcache` /
  `invalidate_dcache` call sites named in §4 (other than removal as
  part of a DCB-NN retrofit).
- Any change to `InFlight<'dma, T>` / `BorrowedForDma` / `BackBuffer`
  that would alter their composition with `DcaBuf` per §10 #R-S1.

## 15. Change log

- **2026-05-02 — Drafted.** Initial draft written following the
  GPT-suggested typestate pattern and the bench-driven motivation
  in §2 (SAI1 TX cache-coherency bees on disco-analyzer 2026-04-30
  / 2026-05-01).
- **2026-05-02 — Pre-ratification clarifications (non-substantive).**
  Tightening pass before §15 ratification entry. Changes:
  - §5 typestate ASCII tree: dropped `ReadWrite` from
    `DeviceActiveCirc<DIR>`'s direction set; restricted to {Read,
    Write} with a note that future directions ratify via §15
    amendment with a named first user. Rationale: no current
    `rlvgl-platform` consumer drives a circular DMA that both reads
    from and writes to the same buffer; the spec surface was
    unmotivated. (Standards Action subtraction; per-policy this
    requires §15 documentation, which this entry provides.)
  - §5 transition table: expanded the cache-state justification on
    both `DeviceXxxPending → CpuOwned` rows so the "no cache op"
    cells are self-explanatory rather than relying on the reader to
    re-derive the symmetry. Cited INV-D3 in the `DeviceWritePending`
    row as the load-bearing reason adjacent-line refill cannot
    reintroduce stale lines during transfer.
  - §5 transition table: spelled out per-DIR cache ops on the
    `DeviceActiveCirc<DIR>` HalfGuard and `stop_circular` rows
    (`clean` for `Read`, `invalidate` for `Write`) so the table is
    closed under the {Read, Write} set without the reader looking
    up the ASCII tree.
  - §10 `Scanout` row: relabelled options (a) and (b) as
    "structurally distinct, not equivalent alternatives" and
    explained the trade — (a) keeps cacheable + per-frame cache ops
    + incidental CPU reads cheap; (b) MPU-non-cacheable + static
    map + non-cacheable-read latency for any CPU consumer. Added a
    DCB-04 MUST that forbids defaulting to (a) on uniformity
    grounds.
  - §9 INV-D13: added. Consolidates the `&mut SCB` borrow into DCB
    construction sites; new code MUST NOT take `&mut SCB` outside
    of DCB construction or grandfathered call sites. Notes a
    follow-on scanner rule `raw_scb_for_cache` SHOULD grow once
    DCB-01 lands. This is the inverse of §11's "DCB does not change
    the SCB ownership convention" — DCB consolidates *where* the
    convention is invoked.
  - §14 DCB-02 bullet: added the cross-repo timing dependency note.
    DCB-02 lands in `streamz/submodules/disco-analyzer`, consumes
    DCB-01 as a published `rlvgl-platform` release, and does not
    block DCB-01's §12 (a) acceptance gate. §12 (b) ratifies
    separately on the DAA owner's bring-up schedule.
  - §7 / §10 citation normalisation: both now cite "DAA-03 §7 /
    INV-D14" in the same form. No semantic change; prevents drift
    if DAA-03 renumbers.

  None of the above modify the frozen typestate set (subtraction of
  `ReadWrite` is the only typestate-set change, and it is documented
  here per Standards Action policy), §6 layout invariants, §8 engine
  submission contract, §9 INV-D8..D12, or §11 non-goals.
- **2026-05-02 — Ratified.** §0–§14 ratified at the form above. DCB-01
  (typestate API + `raw_dcache` discipline rule + starting `BASELINE`)
  is now unblocked; subsequent phases follow the §14 unblocks list.
  Future modifications to the §5 typestate set, §6 layout invariants,
  §8 engine submission contract, or §9 INV-D8..D13 require a §15
  amendment **first**, in a separate PR, before any behaviour PR
  rides on the change.
- **2026-05-02 — `DeviceActiveDoubleBuf<DIR>` amendment (DCB-02-A
  resolution; Standards Action).** Adds the `DcaDoubleBuf<T, N>`
  storage family, the `DeviceActiveDoubleBuf<DIR>` typestate (with
  `DIR ∈ {Read, Write}`), and the `Bank` / `BankGuard<DIR>` types as
  a parallel-and-mutually-exclusive sibling of `DcaBuf<T, N>` /
  `DeviceActiveCirc<DIR>` / `HalfGuard<DIR>`. The two families
  cover the two STM32 DMA continuous-transfer modes
  (circular vs double-buffer M0AR+M1AR); a buffer chooses one at
  construction and cannot switch.

  Sections amended:
  - §3 glossary: `DcaDoubleBuf<T, N>`, `Bank`, `Bank-guard`, `CT
    bit (current target)` added. `Half-guard` row left unchanged.
  - §5: new "Parallel family: `DcaDoubleBuf<T, N>`" subsection
    after the `DeviceActiveCirc<DIR>` ASCII tree, with the
    typestate diagram and a transition table parallel to the
    `DcaBuf` table. The ratified `DcaBuf` transitions are
    untouched.
  - §6: INV-D14 (per-bank alignment / padding) and INV-D15
    (CT-bit live-recheck) added. INV-D1..D13 untouched.
  - §10: SAI1 RX + SAI4 PDM RX reconciliation row added; the
    existing SAI1 TX row reworded to remove the obsolete
    "RX similarly becomes DeviceActiveCirc<Write>" sentence and
    to mark DCB-02 as shipped.
  - §14: DCB-02 entry updated to reflect 2026-05-02 ship; new
    DCB-01b (typestate API for DoubleBuf family) and DCB-02-R
    (RX-side retrofit consuming DCB-01b) bullets added.

  Motivation (DCB-02-A §3 / §4): SAI1 RX (and SAI4 PDM RX) use
  STM32 DMA double-buffer mode with non-contiguous physical banks
  (BUF0 at 0x3000_0000 + BUF1 at 0x3000_1000, 3 KiB gap), which
  doesn't fit `HalfGuard<DIR>`'s contiguous-halves assumption.
  `BankGuard<DIR>` mirrors `HalfGuard<DIR>` but reads the engine's
  `CT` bit directly (RM0399 §15) for the live-recheck, which is
  strictly simpler than NDTR-based half-mark math and unambiguous
  in double-buffer mode. Multiple known consumers (SAI1 RX, SAI4
  PDM RX today; SDMMC streaming, USB HS bulk, DCMI camera
  frame-grab likely) amortise the new API surface (~200
  implementation lines).

  Options enumerated and rejected in DCB-02-A §3: (B) linker-
  coerce buffers contiguous → doesn't generalise, leaves CT-vs-
  NDTR mismatch; (C) one-shot DeviceWritePending per bank → typestate
  would falsely claim engine-off during Cpu-owned phases of a
  continuous transfer; (D) permanent BASELINE entry → hollows out
  INV-D9.

  This is a Standards Action *addition* to §5; no existing
  typestate or invariant is modified. INV-D9 ("New DMA buffers
  MUST use `DcaBuf`") is read by reference as also-applies-to
  `DcaDoubleBuf<T, N>` for the double-buffer-mode case; the
  scanner rule `raw_dcache` already covers both code paths
  because the matched calls (SCB d-cache APIs) are direction-
  independent. No `BASELINE` change is required by this
  amendment.

  DCB-01b is now unblocked. DCB-02-R is unblocked once DCB-01b
  ships. The DCB-02-A sub-letter analysis at
  `docs/concepts/DCB-02-A.md` is preserved as historical record;
  its decision has folded into this entry.
- **2026-05-02 — `DeviceLtdcScan<T, N>` amendment (DCB-04-A
  resolution; Standards Action).** Adds the
  `DeviceLtdcScan<T, N>` typestate as a fifth variant on the
  `DcaBuf<T, N>` family for buffers continuously read by a
  fixed-rate pixel-stream consumer (LTDC, DCMI display-out,
  parallel RGB). Selection between `DeviceLtdcScan<T, N>` and the
  existing engine-driven typestates
  (`DeviceReadPending` / `DeviceWritePending` /
  `DeviceActiveCirc<DIR>`) is at construction; consumer ownership
  shape determines which.

  Sections amended:
  - §3 glossary: `DeviceLtdcScan<T, N>`, "Scanout-FB region", and
    a clarifying note on the `MPU non-cacheable carve-out` row
    (the carve-out alternative is rejected for the LTDC scanout
    class per DCB-04-A §3 / §4).
  - §5: new "Continuous-read variant: `DeviceLtdcScan<T, N>`"
    subsection at the end of §5, with the typestate diagram and
    a four-row transition table covering
    `start_ltdc_scan` / `paint_full` / `present` / `stop_scan`.
    Existing typestate sets (DcaBuf and DcaDoubleBuf) are
    unchanged. The `DIR` parameter is *deliberately* omitted (the
    CPU is always producer; the inverse direction would be a
    different family — `DeviceLtdcGrab` for DCMI camera input —
    not in scope here).
  - §6: INV-D16 (`DeviceLtdcScan<T, N>` ordering contract). Per
    DCB-04-A §2b, `present()` MUST emit *both* the cache clean
    AND a memory barrier (DSB on Cortex-M); on STM32H7 +
    Write-Through SDRAM the DSB is the load-bearing primitive
    (drains the AXI write buffer that would otherwise hold pixel
    writes past the cache write-back). The DSB also flows
    through DCB's owning module rather than via raw
    `cortex_m::asm::dsb()` call sites — preserving the
    discipline-scanner's ability to keep raw barrier-intrinsic
    calls out of consumer code.
  - §10: new reconciliation row covering both LTDC scanout
    patterns — single-FRONT (the
    `examples/stm32h747i-disco/src/freertos_entry.rs:1011, 1449`
    BASELINE entry) and the swap-pair `Scanout::swap` pattern
    in `platform/src/hwcore/surface.rs:399`. Both wrap their
    buffer(s) as `DeviceLtdcScan<T, N>` handles. The
    parallel `DeviceLtdcScanDouble<T, N>` for hardware-managed
    swap is deferred until a consumer needs it.
  - §14: DCB-01c (typestate API for the LTDC family) and DCB-04
    (the freertos / Scanout retrofit) bullets added. DCB-01c
    unblocks DCB-04; both are unblocked once this amendment
    ratifies.

  Motivation (DCB-04-A §3 / §4): the disco firmware's existing
  `clean_dcache_by_address(FRONT_FB, FB_BYTES)` + `dsb()` pre-
  scanout pattern is bench-validated. On STM32H7 + Write-Through
  SDRAM (already configured at
  `platform/src/stm32h747i_disco.rs:1322`), the cache clean is
  *cache-redundant* but the DSB drains the AXI write buffer —
  load-bearing for LTDC's continuous-read access pattern. Option
  A (typestate-on-publish) preserves this verified runtime; Option
  B (MPU non-cacheable carve-out) was rejected because the SDRAM
  region split has bigger hardware-config footprint, weaker
  portability story (each port MPU-configures independently), and
  imposes ~45× CPU-FB-read latency penalty on diagnostic dumps.
  Option B remains a future amendment if a platform's CPU-FB-read
  path becomes hot enough to justify it. Option C (permanent
  BASELINE) was rejected because it hollows out INV-D9 and
  prevents §12 (c) STRICT-mode ratification.

  This is a Standards Action *addition* to §5 (a new typestate
  variant on the existing `DcaBuf<T, N>` family); no existing
  typestate or invariant is modified. The `raw_dcache` scanner
  rule does not need a change — it already matches direction-
  independent SCB calls; the DCB-04 retrofit removes the last
  BASELINE entry rather than adding a new exemption.

  DCB-01c is now unblocked. DCB-04 is unblocked once DCB-01c
  ships. The DCB-04-A sub-letter analysis at
  `docs/concepts/DCB-04-A.md` is preserved as historical record;
  its decision has folded into this entry.
- **2026-05-02 — DCB-03-A resolution: Option C
  (closure-with-deferral).** DCB-03 is closed without a
  retrofit. The DMA2D destination cache discipline that the
  §10 row prescribed is not currently load-bearing because every
  DMA2D destination on the disco target lives in
  MPU-Write-Through SDRAM (`platform/src/stm32h747i_disco.rs:
  1322`); the prescribed `lend_for_write` runtime cache op
  would be a functional no-op on the only platform that
  currently uses DMA2D.

  Sections amended:
  - §10: the `BackBuffer` / `BorrowedForDma` / `InFlight` row
    is reworded to mark the prescription as "closed-with-
    deferral" rather than "concrete refactor lands in DCB-03",
    and gains a normative **Reopen triggers** clause
    enumerating (a) a new non-Write-Through DMA2D destination,
    (b) a port without H7-style Write-Through SDRAM defaults,
    (c) a change to the disco's SDRAM Write-Through config.
    Any of those triggers a DCB-03-B sub-letter analysis with a
    named first user before the FrameBuffer rearchitect lands.
  - §14: DCB-03 marked **Closed (deferred), 2026-05-02** with
    a back-pointer to DCB-03-A and the same reopen-trigger set
    documented in §10. The DCB-03-B reopen path is preserved
    explicitly so future maintainers don't need to re-derive
    the analysis.

  Motivation (DCB-03-A §3 / §4): DCB has delivered observable
  runtime safety where it mattered (DCB-02 SAI1 "loud bees" fix;
  DCB-02b/c removal of pre-DCB cache hacks; DCB-04 BASELINE
  clearance). DMA2D's destination boundary doesn't have an
  analogous load-bearing target on the current MCU + MPU
  configuration; forcing the §10 rearchitect now (~300-line
  cross-cutting change with bench-validation risk in the hot
  rendering path) trades real refactor budget against
  imagined future requirements. The infrastructure for a soft
  retrofit (`DcaCacheCtx`-route per DCB-02c / DCB-04) is
  already in place for ad-hoc future consumers without
  reopening DCB-03; only a structurally new requirement
  (typestate-tracked DMA2D destination ownership) warrants
  reopening.

  This is **not** a Standards Action change to the typestate
  set, layout invariants, engine submission contract, or
  scanner rule. It is a closure-with-deferral on the §10
  reconciliation prescription only. INV-D9 wording remains
  unchanged but is read forward-looking — "new DMA buffers in
  *new* DMA paths MUST use DcaBuf"; existing
  pre-DCB DMA2D destinations are grandfathered alongside the
  `FrameBuffer` / `BackBuffer` chain. No `BASELINE` change is
  required; the `raw_dcache` BASELINE is already empty after
  DCB-04.

  The DCB-03-A sub-letter analysis at
  `docs/concepts/DCB-03-A.md` is preserved as historical
  record; its decision has folded into this entry. The
  initiative's BASELINE-shrink track formally ends at DCB-04.
  Future DCB-NN phases (DCB-03-B if reopened, future port-
  specific phases, future engine-specific phases) ratify into
  §15 here on first need.
