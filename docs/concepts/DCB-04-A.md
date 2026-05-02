# DCB-04-A — LTDC scanout cache discipline

**Status:** Drafted 2026-05-02. Sub-letter analysis surfaced during
DCB-02c when the SD-adapter retrofit shrank the `raw_dcache`
BASELINE to a single remaining entry — the LTDC scanout pre-clean
in `freertos_entry.rs:1011, 1449`. DCB-00 §10 deferred the
typestate-vs-MPU decision to this phase. Per the parent CLAUDE.md
"Sub-letter doc convention" this analysis ratifies its resolution
into DCB-00 §3 / §5 / §6 / §10 + §15 as a Standards Action
amendment, after which this file is preserved as historical record
only.

## 1. Purpose

DCB-01b ratified the typestate primitives for one-shot DMA-read
(`DcaBuf<T, N> + DeviceReadPending`), continuous circular
double-buffer (`DcaBuf<T, N> + DeviceActiveCirc<DIR> + HalfGuard`),
and the M0AR/M1AR double-buffer-mode parallel
(`DcaDoubleBuf<T, N> + DeviceActiveDoubleBuf<DIR> + BankGuard`).
LTDC scanout fits **none** of those cleanly.

This doc:

- documents the actual STM32H747I-DISCO scanout cache pattern
  (which surprised the DCB-02c review),
- enumerates the option set that DCB-00 §10 sketched (typestate-
  on-publish vs MPU non-cacheable carve-out),
- recommends one,
- proposes the §15 amendment surface to ratify.

## 2. Problem statement

### 2a. Current state on STM32H747I-DISCO

The disco firmware's FreeRTOS render loop
(`examples/stm32h747i-disco/src/freertos_entry.rs`) drives the
LCD panel through LTDC in a **single-buffer FRONT** model:

- `FRONT_FB_ADDR` is the active scanout buffer (LTDC reads it
  continuously; pixel-clock rate ≈ 27 MHz × 4 bytes = 108 MB/s
  for 480×800 @ ARGB8888 / 30 fps).
- The render task writes pixels directly into `FRONT_FB_ADDR`
  — no double-buffer swap.
- The "present" step (lines 1009 + 1449) is a manual
  `scb.clean_dcache_by_address(FRONT_FB_ADDR, FB_BYTES)`
  followed by `cortex_m::asm::dsb()`. This is the last
  remaining `raw_dcache` BASELINE entry as of DCB-02c.

The single-buffer pattern (vs `Scanout::swap`) is documented at
`freertos_entry.rs:1453-1454`:

> No buf_ready — present retriggers the same FRONT.
> Single-buffer = zero flicker.

### 2b. SDRAM is already MPU-configured Write-Through

Surprising fact uncovered during DCB-04-A drafting:
`platform/src/stm32h747i_disco.rs:1322` (`configure_mpu_sdram_
writethrough`) maps the entire SDRAM bank
(`0xD000_0000..0xD200_0000`, 32 MiB, MPU region 6) as
**Write-Through Non-Shareable**: TEX=0, C=1, B=0, S=0.

Under Write-Through, CPU writes go simultaneously to both cache
and main memory (RAM is always up-to-date). One might therefore
conclude that the explicit `clean_dcache_by_address` at present
is redundant.

**It is not.** Cortex-M7's AXI write buffer batches Write-Through
transactions on the bus side to keep the cache from monopolising
the AXI port (per the cited `configure_mpu_sdram_writethrough`
comment, lines 1351–1357). Pending writes sit in the buffer
until the AXI is free. Without an explicit `clean + dsb` before
LTDC reads, the buffer can hold the most recent pixel writes long
enough for LTDC to scan stale memory. The clean acts as a
**barrier that drains the write buffer**, not as a cache
write-back.

This means:

- (a) Replacing Write-Through SDRAM with MPU non-cacheable
  Strongly-Ordered (or Device) memory would skip the cache
  entirely but **not** the AXI write-buffer drain — a DSB + bus
  handover delay would still be needed.
- (b) Keeping Write-Through and wrapping the existing clean+DSB
  in a typestate handle preserves the proven pattern with no
  change to runtime semantics.

This nuance reframes the §10 row's binary choice between
"cacheable + per-frame clean" (Option A) and "MPU non-cacheable
carve-out" (Option B): on this MCU both options *still need
DSB-equivalent ordering*, so the overhead delta is smaller than
DCB-00 §10 implied. Option B's win is mostly the *type system
mechanically removing the clean*, not raw cycles.

### 2c. Scanout vs single-FRONT — two distinct patterns

`platform/src/hwcore/surface.rs:399` defines a `Scanout` type
holding two `FrameBuffer`s with `swap()` semantics — the
proper double-buffer model. The current freertos path doesn't
use it; the bare-metal disco path does.

DCB-04 ratification needs to cover **both** patterns:

- Single-FRONT (freertos): one buffer, CPU writes + clean + DSB
  + DMA continuously reads. Closest fit: `DeviceReadPending`-
  per-frame, but the engine is *already armed* and doesn't
  pause between frames. None of DCB-01/01b's typestates models
  "engine is continuously reading the same buffer while CPU
  refills it in place."
- Scanout (bare-metal disco): two buffers swapped at frame
  boundaries. Closest fit: `DcaDoubleBuf<u8, FB_BYTES> +
  DeviceActiveDoubleBuf<Read>` with a single bank-guard scoped
  per-frame around CPU-side dirty-rect writes — but LTDC
  doesn't alternate banks per frame; the swap is software.

Both patterns need a new typestate or a DCB-04 amendment.

## 3. Options

### Option A — `DeviceLtdcScan<...>` typestate per buffer

A new typestate, parallel to `DeviceActiveCirc<DIR>` and
`DeviceActiveDoubleBuf<DIR>`, for buffers continuously read by a
fixed-rate pixel-stream consumer (LTDC, but also DCMI display,
parallel RGB) where the CPU writes in-place between consumer
reads. The typestate exposes:

- a `present(scb)` method that emits clean + DSB and returns
  the same handle (no transition; the engine never paused),
- per-frame access via `paint_full(&mut self) -> &mut [T]`
  returning a slice that's valid until the next `present`.

`paint_full` is single-bank (no inactive-half), hence not a
guard; it's a borrow whose lifetime ties to the typestate
handle. After painting, the caller calls `present()`. INV-D7-
analogue: there's no overrun check because the engine never
"finishes" — it reads continuously. Coherency depends on the
clean+DSB ordering, not on engine-position math.

**Pros**:

- Wraps the existing freertos pattern verbatim — runtime
  behavior unchanged. Single-FRONT and Scanout both use this
  typestate (Scanout swaps two `DeviceLtdcScan` handles in
  software; the type system stays uniform).
- INV-D9 (new DMA buffers MUST use DcaBuf) is honored end-to-
  end in `rlvgl-platform`.
- Composes with the existing surface/Scanout types.

**Cons**:

- Yet another typestate variant. The surface area grows with no
  immediate consumer beyond LTDC scanout.
- "Continuous read with in-place CPU writes" is structurally
  different from the device-active typestates (which assume
  bank/half ownership). Modeling it as a typestate is more
  ceremony than insight.

**Implementation effort**: ~150 lines in `hwcore/dca.rs` + 3
trybuild fixtures + 1–2 retrofits in
`examples/stm32h747i-disco/src/freertos_entry.rs` (single-
FRONT) and `surface.rs::Scanout` (proper double-buffer).

### Option B — `DcaBuf::in_uncached_region` + MPU non-cacheable scanout pair

Add a new constructor on the existing `DcaBuf` (and / or
`FrameBuffer`) that takes a fixed-address region the *caller*
has carved out as MPU non-cacheable, and returns a typestate-
wrapped handle whose `Cpu / DeviceXxx*` transitions emit cache
ops *that compile to nothing* (zero-cost — no SCB call). DCB-00
§6 INV-D6 already names this constructor as the prescribed
shape.

The MPU region table grows by one entry: a non-cacheable
scanout-FB region for the SDRAM range covering the front +
(optionally) back buffers.

**Pros**:

- The cache discipline is enforced *at construction*, not at
  every transition. Cleanest separation of "what the type
  system enforces" from "what the runtime emits."
- DCB-00 §6 INV-D6 already prescribes this shape; DCB-04 just
  ratifies the constructor surface.
- Removes the per-frame clean entirely from generated code.
  AXI write-buffer drain still needs a DSB but that's a single
  instruction, not a cache-line iteration.

**Cons**:

- Requires the MPU region table to be edited. On the disco the
  current scanout FB lives at `0xD180_0000` inside the
  Write-Through SDRAM region — splitting that region is
  ABI-impacting (other consumers of SDRAM at 0xD000_0000–
  0xD180_0000 expect Write-Through). The region split is a
  separate concept doc-level change.
- CPU reads of the framebuffer (screenshots, FB dumps for
  serial debugging) hit non-cacheable RAM at SDRAM latency
  (~150 ns per access vs ~3 ns cache). For diagnostic dumps of
  ~1.5 MB FB this is ~225 ms vs ~5 ms — a 45× slowdown for
  one-off operations. Acceptable in practice (dumps are rare),
  but worth flagging.
- Requires the `DcaBuf::in_uncached_region(addr)` constructor
  (and a parallel `DcaDoubleBuf::from_addrs_uncached`) — small
  but non-trivial API additions.

**Implementation effort**: ~50 lines for the constructors +
~20 lines of MPU region split in
`platform/src/stm32h747i_disco.rs` + retrofit in the two
freertos lines.

### Option C — keep the BASELINE entry permanently

The remaining `freertos_entry.rs` entries stay grandfathered
indefinitely. `RLVGL_LINT_STRICT=1` (DCB-00 §12 (c) acceptance
gate) ratifies with `BASELINE` non-empty.

**Pros**: zero new code.

**Cons**: hollows out INV-D9. DCB-00 §12 (c) cannot ratify
without modification. Future scanout consumers (Zephyr port,
BeagleBone Black LCDC, esp32-p4 DPI panel) inherit the
exception. Bad precedent.

## 4. Recommendation

**Option A** (`DeviceLtdcScan<...>` typestate).

Justification:

- **Preserves verified runtime behaviour.** The disco
  Write-Through + clean + DSB pattern is bench-validated.
  Wrapping it in a typestate doesn't change cycles emitted; it
  changes what the type system requires of callers. Option B
  introduces an MPU region split that has to be re-validated
  (CPU-read latency change for diagnostic paths,
  cross-validation that no other code in the SDRAM region
  depends on Write-Through).
- **Single typestate covers both LTDC patterns.** Single-FRONT
  (freertos) and `Scanout::swap` (bare-metal) both wrap one or
  two `DeviceLtdcScan` handles. The type system stays uniform
  across the platform's two scanout idioms.
- **Composable with `surface.rs::Scanout`.** The existing
  `BackBuffer` / `FrontBuffer` / `BorrowedForDma` /
  `InFlight<T>` chain (Register-Mashing Discipline rule #3)
  layers on top: `Scanout::back_mut()` returns a
  `DeviceLtdcScan`-wrapped `BackBuffer`; the renderer paints
  via the slice; `Scanout::swap()` plus a single `present()`
  on the new front does the per-frame clean. No new MPU
  region needed.
- **Sets up a portability story.** The Zephyr port (which
  currently uses display-driver-managed FBs) and future
  BeagleBone Black LCDC retrofit can adopt the same
  `DeviceLtdcScan` typestate without per-platform MPU
  configuration.

Option B is the more "zero-cost-cache" answer in spirit but
has a bigger hardware-config footprint and weaker portability
story. Pragmatic stance: **DCB-04 amends with Option A;
Option B remains a future amendment if a platform's CPU-FB-read
path becomes hot enough that per-frame cleans matter.**

Option C is rejected for the same reason DCB-02-A rejected its
Option D: hollows out INV-D9 and the scanner's purpose.

## 5. Proposed amendments to DCB-00

The following amendments would land in DCB-00 §15 first, in a
separate PR, before DCB-01c implementation behaviour PR rides
on them. Sketches only — exact wording ratifies in the §15
amendment commit.

### §3 glossary additions

- **DeviceLtdcScan\<T, N\>** — typestate for a buffer
  continuously read by a fixed-rate pixel-stream consumer
  (LTDC, DCMI display-out, parallel RGB) where the CPU paints
  in place between consumer reads. The engine never pauses;
  per-frame ordering between CPU writes and consumer reads is
  enforced by `present()` (cache clean + DSB, draining the
  AXI write buffer).
- **Scanout-FB region** — a `DcaBuf<T, N>` (or
  `DcaDoubleBuf<T, N>` for the double-buffer form) configured
  in `DeviceLtdcScan<...>` typestate. Lives in cacheable RAM
  with the Write-Through MPU attribute (the current disco
  default); the per-frame `present()` drains the AXI write
  buffer rather than writing back dirty cache lines.

### §5 typestate set extension

```text
DcaBuf<T, N>
   ├─ CpuOwned                   ← (existing)
   ├─ DeviceReadPending           ← (existing)
   ├─ DeviceWritePending          ← (existing)
   ├─ DeviceActiveCirc<DIR>       ← (existing)
   └─ DeviceLtdcScan<T, N>        ← NEW
         per-frame API:
            paint_full(&mut self) -> &mut [T; N]
                — slice valid until next present()
            present(&mut self, ctx: &mut DcaCacheCtx)
                — emits clean + dsb over the buffer extent;
                  no typestate transition (engine never paused)
            stop_scan(self, ctx: &mut DcaCacheCtx)
                  -> Cpu<'a, T, N>
                — caller MUST stop the LTDC engine first;
                  emits final clean + dsb before returning
                  buffer to CpuOwned.
```

`DcaDoubleBuf<T, N>` gains a parallel `DeviceLtdcScanDouble`
typestate if and when the proper double-buffer Scanout
pattern ratifies a per-bank model — deferred until a
`Scanout`-based platform actually needs it.

### §6 layout invariant additions

- **INV-D16: `DeviceLtdcScan<T, N>` ordering contract.** The
  `present()` call MUST emit *both* a `DcaCache::clean` over
  the buffer's full padded extent AND a memory barrier
  (`dsb` on Cortex-M; `__DSB()` equivalent on other targets)
  that drains the AXI / interconnect write buffer. The
  individual `clean` is insufficient on Cortex-M7 +
  Write-Through SDRAM because the AXI write buffer can hold
  pending writes past the cache write-back; the DSB is the
  load-bearing ordering primitive.

### §10 reconciliation row addition

> | LTDC scanout pre-clean (`freertos_entry.rs:1011, 1449` —
> single-FRONT model + `Scanout::swap` model) | **Replaces.**
> Wraps the buffer in `DeviceLtdcScan<T, N>` typestate.
> `paint_full` returns a `&mut [T; N]` for in-place CPU
> writes; `present()` emits the existing clean + DSB. The
> manual `scb.clean_dcache_by_address` calls disappear; the
> last remaining `raw_dcache` BASELINE entry clears. | DCB-04
> retrofit (target: examples/stm32h747i-disco/src/freertos_
> entry.rs single-FRONT path; surface.rs::Scanout
> double-buffer path; future Zephyr / BBB / esp32-p4 ports).
> Lands after DCB-01c ships the new typestate. |

### §15 ratification entry (proposed wording)

> **2026-MM-DD — DeviceLtdcScan<T, N> amendment (DCB-04-A
> resolution; Standards Action).** Adds the
> `DeviceLtdcScan<T, N>` typestate as a parallel-and-
> mutually-exclusive sibling of the existing
> `DeviceActiveCirc<DIR>` / `DeviceActiveDoubleBuf<DIR>`
> typestates. Adds INV-D16 (LTDC `present()` ordering
> contract). §10 reconciliation row added for the LTDC
> scanout pre-clean. Motivation: DCB-04-A §3 / §4 — Option A
> selected over B (MPU non-cacheable region split has bigger
> hardware-config footprint, weaker portability story) and
> C (hollows out INV-D9). Implementation lands in DCB-01c;
> first user is the disco freertos single-FRONT retrofit in
> DCB-04 itself, with the bare-metal `Scanout`-based path
> following.

## 6. Implementation plan summary (informative)

After DCB-00 §15 ratifies the amendment:

- **DCB-01c** — Land `DeviceLtdcScan<T, N>` in
  `platform/src/hwcore/dca.rs`. Add trybuild fixtures parallel
  to existing dca-typestate fixtures (use-after-paint, double-
  paint rejection, present-without-stop sound). Existing
  `DcaBuf` / `DcaDoubleBuf` typestates untouched.
- **DCB-04** — Retrofit
  `examples/stm32h747i-disco/src/freertos_entry.rs:1011, 1449`
  onto `DeviceLtdcScan<u8, FB_BYTES>` + `present()`. Removes
  the last `raw_dcache` BASELINE entry, unblocking
  `RLVGL_LINT_STRICT=1` (DCB-00 §12 (c) acceptance gate). The
  bare-metal `Scanout`-based render path can adopt the same
  typestate in the same PR or follow-up.

## 7. Change log

- **2026-05-02 — Drafted.** Surfaced during DCB-02c
  (rlvgl `9d90d81`) when the `raw_dcache` BASELINE shrank to
  a single remaining LTDC scanout entry. Recommendation:
  Option A (`DeviceLtdcScan<T, N>` typestate). Awaiting owner
  ratification via a DCB-00 §15 amendment.
