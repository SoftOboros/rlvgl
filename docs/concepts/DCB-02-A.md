# DCB-02-A — RX double-buffer-mode coverage

**Status:** Drafted 2026-05-02. Sub-letter analysis surfaced during
DCB-02 (SAI1 TX retrofit). Awaiting resolution before DCB-02 RX-side
work proceeds. Per DCB-00 README "Sub-letter doc convention" this
doc is *scoped to one decision* and *transient*: the chosen option
folds into DCB-00 §5 / §6 / §10 + §15 as a Standards Action
amendment, after which this file is preserved as historical
analysis only.

## 1. Purpose

DCB-01 ratified `DeviceActiveCirc<DIR>` (one circular buffer with
two halves; `HalfGuard<DIR>` reborrows the parent for inactive-half
access). The DCB-02 first-user retrofit on disco-analyzer's SAI1
**TX** path used this typestate cleanly. The SAI1 **RX** path does
not fit. This doc names the mismatch, enumerates the resolution
options, and recommends one.

## 2. Problem statement

The SAI1 line-in RX path on disco-analyzer
(`analyzer-audio/src/sai1_linein.rs`) uses **STM32 DMA double-buffer
mode** (M0AR + M1AR alternating), not circular mode. The two
physical buffers are:

| Symbol | Address | Size |
|---|---|---|
| `SAI1_DMA_BUF0_ADDR` | `0x3000_0000` | `SAI1_DMA_HALFWORDS * 2` = 1024 bytes |
| `SAI1_DMA_BUF1_ADDR` | `0x3000_1000` | 1024 bytes |

The two buffers are **physically non-contiguous** — there is a
3 KiB gap between BUF0's end (`0x3000_0400`) and BUF1's start
(`0x3000_1000`). The DMA engine alternates between M0AR and M1AR on
each TC interrupt; the CPU consumes whichever buffer just completed
(`buf_idx`).

This shape violates two assumptions of `DeviceActiveCirc<DIR>`:

1. **Contiguous halves.** `HalfGuard` computes the inactive-half
   extent as `base + N/2 * sizeof(T)`. With non-contiguous physical
   buffers, the second half's address is *not* `base + half_bytes`.
2. **Single base address.** `CircRead` / `CircWrite` carry one
   `&'a mut DcaBuf` borrow with one `dma_addr()`. Double-buffer
   DMA needs two distinct DMA addresses (one per buffer) the engine
   can program independently.

INV-D11 (HalfGuard observation against `NDTR`) also weakens because
DMA double-buffer mode reports completion via TC interrupt + the
`CT` (current target) bit in the stream's `CR` register, not via a
single decrementing `NDTR`. The "DMA crossed into the guarded half"
check needs to read `CT`, not `NDTR < N/2`.

## 3. Options

### Option A — Add `DeviceActiveDoubleBuf<DIR>` typestate

A new fifth typestate variant alongside the existing four:

```text
DcaDoubleBuf<T, N>      ─ owning storage; TWO `[T; N]` regions
   │                      that may be physically non-contiguous.
   ├─ CpuOwned          ─ same as DcaBuf
   ├─ DeviceReadPending ─ (n/a; DMA double-buffer mode is for
   │                       continuous transfers, not one-shots)
   ├─ DeviceWritePending ─ (n/a; same reason)
   ├─ DeviceActiveCirc<DIR>      ─ (n/a; different DMA mode)
   └─ DeviceActiveDoubleBuf<DIR> ─ DMA in continuous double-buffer
         transfer. Two DmaAddr values exposed: m0_addr() / m1_addr().
         Bank-guard API: bank_guard(current_target_bit) -> BankGuard<DIR>
            (BankGuard entry: clean (Read), invalidate (Write) on
             the inactive bank — i.e. opposite of CT bit)
            (BankGuard drop / release: re-read CT; fault if it
             flipped during the guard's lifetime — INV-D7 analogue)
```

**New types**: `DcaDoubleBuf<T, const N: usize>` storage owner
(holds two `DcaBuf<T, N>`-shaped regions, possibly at fixed
addresses via `unsafe fn from_addrs`); `Bank` enum (`{ M0, M1 }`)
analogous to `Half`; `BankGuard<DIR>` analogous to `HalfGuard<DIR>`.

**Pros**: clean type-system match for the actual hardware DMA mode.
Live-recheck reads CT bit (single 32-bit register read) which is
simpler than NDTR math. Composes with `DcaCacheCtx` exactly like the
existing typestates.

**Cons**: Standards Action surface area — adds typestate + supporting
types + scanner hooks. Two consumers known today (SAI1 RX,
SAI4 PDM RX in `analyzer-audio/src/sai4_pdm.rs`); future SDMMC and
USB high-speed bulk paths may also use double-buffer DMA. Worth
naming users to justify the surface.

**Implementation effort**: ~150–200 lines added to
`platform/src/hwcore/dca.rs`, ~50 lines in tests, a few lines in
the discipline scanner whitelist. No changes to existing
typestates.

### Option B — Linker-coerce RX buffers contiguous; reuse `DeviceActiveCirc<DIR>`

Move `SAI1_DMA_BUF1_ADDR` from `0x3000_1000` to `0x3000_0400`
(immediately following BUF0). The two physical buffers become
adjacent, can be reinterpreted as a single
`DcaBuf<i16, 2 * SAI1_DMA_HALFWORDS>`, and `HalfGuard` works
unchanged.

**Pros**: zero DCB API surface. Pure linker / address-constant
change in disco-analyzer.

**Cons**: doesn't generalise. SAI4 PDM RX, future SDMMC R/W, USB
EP buffers that are double-buffer DMA still need ad-hoc
manual-cache fallbacks because the buffer layout is an *engine*
constraint (M0AR + M1AR are independent registers, the engine
doesn't require contiguity), and other consumers may have hard
reasons to keep them separated (e.g. MPU sub-region attributes).
Also: the 3 KiB gap currently in the layout might be intentional
for cache-line-isolation reasons that weren't documented; coercing
contiguity could re-introduce the line-sharing class of bug
INV-D3 forbids.

Additionally the live-recheck mismatch (NDTR vs CT bit) remains —
even with contiguous buffers, `DeviceActiveCirc<DIR>::release`
expects to compare against `Half`, but in double-buffer mode the
engine's "current target" is reported via the CT bit, not via NDTR
crossing the half-mark. Reusing `DeviceActiveCirc` here would
require shoehorning the CT bit into a `Half` synthetic value, which
loses the type-system safety guarantee.

### Option C — Two independent `DeviceWritePending` buffers with TC-driven swap

Treat each RX bank as a separate `DcaBuf<i16, SAI1_DMA_HALFWORDS>`.
On each TC interrupt: completion handler transitions the just-filled
buffer from `DeviceWritePending` to `Cpu`, hands the slice to the
consumer, then transitions back to `DeviceWritePending` to lend
again.

**Pros**: uses only existing DCB primitives.

**Cons**: misrepresents the hardware. The DMA engine never *stops*
between banks — the `DeviceWritePending` typestate models a one-shot
transfer, not a continuous double-buffered stream. The cache op
fires twice per TC (entry-side invalidate on each lend) where once
would suffice. Subtler: the brief window between completion-handler
entry and the bank's re-lend is unrepresented; in reality the
engine has already started writing the *other* bank by the time
the handler runs, so the "Cpu owns the just-completed bank" claim
is sound but skipping the lend-back drops a bank's worth of audio.

The biggest issue: the typestate encodes **engine off** during
`Cpu`-owned phases, but for double-buffer DMA the engine is always
on. The model would lie about the hardware.

### Option D — Leave RX manual; document gap

Keep the existing `scb.invalidate_dcache_by_address(...)` call in
`Sai1LineInSource::tick()`. Add a permanent BASELINE entry for
`raw_dcache` on `analyzer-audio/src/sai1_linein.rs` (or its
equivalent in `rlvgl-platform` once promoted) and document the
exception in DCB-00 §10 / §11.

**Pros**: zero work, no API surface.

**Cons**: leaves a class of cache/DMA race outside the type-system
guarantee, exactly the failure mode DCB exists to eliminate. Each
new double-buffer-mode consumer (SAI4 PDM, future SDMMC, USB)
inherits the same exception. INV-D9 ("New DMA buffers MUST use
DcaBuf") is hollowed out.

## 4. Recommendation

**Option A** (`DeviceActiveDoubleBuf<DIR>`).

Justification:

- **Hardware fidelity.** STM32 DMA double-buffer mode is a
  first-class engine mode with its own register layout (`CT`
  bit, M0AR + M1AR, separate `MBM` flag in `CR`). The typestate
  set should mirror the engine's mode set, not approximate it
  with a misfit.
- **Multiple consumers in scope.** SAI1 RX *and* SAI4 PDM RX both
  use double-buffer mode today. Future SDMMC streaming reads,
  USB high-speed bulk endpoints, and any DCMI camera frame-grab
  path on H7-class parts will likely use it too. The new
  typestate amortises across the family.
- **CT-based live-recheck is simpler than NDTR.** A single 32-bit
  read of the stream's `CR` register's `CT` bit is unambiguous
  and atomic; NDTR-based "did DMA cross the half" checks have a
  small race window that double-buffer-mode CT does not.
- **Standards Action cost is modest.** ~200 lines of
  implementation, fully composable with existing primitives, no
  changes to ratified §5 typestates (only an addition).

Options B and D are rejected for the reasons in §3. Option C is
rejected because misrepresenting the engine state in the
typestate breaks the "type system as fault-prevention" thesis.

## 5. Proposed amendments to DCB-00

The following amendments would land in DCB-00 §15 *first*, in a
separate PR, before any DCB-01b implementation behaviour PR rides
on them. Sketches only — exact wording ratifies in the §15
amendment commit.

### §3 glossary additions

- **DcaDoubleBuf\<T, N\>** — owning, cache-line-aligned storage
  for a *pair* of cacheable DMA banks of `[T; N]` each. The two
  banks may be physically non-contiguous; alignment and padding
  invariants apply per-bank.
- **Bank** — `M0` or `M1`; analogous to `Half` for the
  double-buffer-mode case.
- **BankGuard\<DIR\>** — RAII guard over the inactive bank,
  analogous to `HalfGuard<DIR>` but driven by the engine's `CT`
  bit rather than `NDTR`.

### §5 typestate set extension

```text
DcaDoubleBuf<T, N>
   ├─ CpuOwned                       ─ same shape as DcaBuf
   └─ DeviceActiveDoubleBuf<DIR>     ─ M0AR + M1AR both armed
         DIR ∈ {Read, Write}
         Bank-guard API: bank_guard(current_target: Bank) ->
                          BankGuard<DIR>
            (BankGuard entry: per-DIR op on inactive bank only)
            (BankGuard drop/release: re-read CT bit; fault if
             current_target flipped during the guard's lifetime —
             INV-D7 analogue)
```

`DeviceActiveCirc<DIR>` and `DeviceActiveDoubleBuf<DIR>` are
parallel, mutually exclusive: a buffer family chooses one based
on the engine's DMA mode at construction.

### §6 layout invariant additions

- **INV-D14: Per-bank alignment / padding.** For `DcaDoubleBuf<T,
  N>` each of the two banks MUST independently satisfy INV-D1 +
  INV-D2 (cache-line aligned, byte size a whole multiple of
  CACHE_LINE). The two banks MAY be at arbitrary disjoint
  addresses; INV-D3 (no cache-line sharing) applies independently
  to each bank.
- **INV-D15: CT-bit live-recheck.** `BankGuard<DIR>::release(ct)`
  semantics mirror `HalfGuard::release(half)`: if `ct` names the
  same bank the guard exposed, the guard observes an INV-D7
  analogue overrun.

### §10 reconciliation row addition

> | SAI1 RX (`Sai1LineInSource` BUF0/BUF1) — and SAI4 PDM RX,
> SDMMC streaming, USB HS bulk (future) | **Replaces.**
> Becomes `DcaDoubleBuf<i16, SAI1_DMA_HALFWORDS>`-style storage
> with `DeviceActiveDoubleBuf<Write>` + `BankGuard<Write>`. The
> manual `scb.invalidate_dcache_by_address` in
> `sai1_linein.rs:279` is removed; the bank-guard's entry op
> performs the invalidate. | DCB-02-R retrofit (RX side).
> Lands after DCB-01b ships the new typestate. |

### §15 ratification entry (proposed wording)

> **2026-MM-DD — DeviceActiveDoubleBuf<DIR> amendment (DCB-02-A
> resolution).** Adds the `DcaDoubleBuf<T, N>` storage family,
> the `DeviceActiveDoubleBuf<DIR>` typestate, the `Bank` /
> `BankGuard<DIR>` types, INV-D14 / INV-D15. Ratification
> motivated by DCB-02-A §3 / §4 — Option A selected over B (no
> generalisation), C (misrepresents continuous engine state),
> and D (hollows out INV-D9). Implementation lands in DCB-01b;
> first user is the SAI1 RX retrofit in DCB-02-R. Future users
> named in §10: SAI4 PDM RX, SDMMC streaming, USB HS bulk.

## 6. Implementation plan summary (informative)

After DCB-00 §15 ratifies the amendment:

- **DCB-01b** — Land `DcaDoubleBuf<T, N>`, `Bank`, `BankGuard<DIR>`,
  and the `DeviceActiveDoubleBuf<DIR>` typestate in
  `platform/src/hwcore/dca.rs`. Add trybuild compile-fail
  fixtures parallel to `dca_use_after_lend.rs` /
  `dca_double_lend.rs` / `dca_half_guard_double.rs`. Extend
  the scanner whitelist if any new SCB call site emerges (none
  expected — the existing `DcaCache` trait suffices).
- **DCB-02-R** — Retrofit `Sai1LineInSource` (and, in the same or
  follow-on PR, SAI4 PDM RX) onto `DcaDoubleBuf` +
  `BankGuard<Write>`. Remove
  `analyzer-audio/src/sai1_linein.rs:279` manual invalidate.
  Bench-flash the spectrum/meter path to confirm no audio
  regression vs the current bench-9l result.

## 7. Change log

- **2026-05-02 — Drafted.** Surfaced during DCB-02 SAI1 TX
  retrofit (rlvgl `a56987b`, disco-analyzer `c117a20`); the
  RX-side mismatch with `DeviceActiveCirc<DIR>` blocked a
  unified retrofit. Recommendation: Option A
  (`DeviceActiveDoubleBuf<DIR>`). Awaiting owner ratification
  via a DCB-00 §15 amendment.
