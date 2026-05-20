# DPR-01-A — Migration Outline (Phase 1, Step-by-Step Playbook)

**Status:** Draft 2026-05-19. Sub-letter to
[`DPR-01-A.md`](DPR-01-A.md) §4. Resolves PCDN-DPR-006
(2026-05-19) into a line-by-line elaboration of the six-step
phase-1 sequence. Unblocks the DPR-01a code work.

This document is the *executable* outline of DPR-01-A §4 phase 1.
It does **not** restate the rationale — DPR-01-A §3 (the four-op
grouping) and DPR-01 §5.4..§5.6 (the ratified types) remain
authoritative. Numbers cited here are line offsets in `v0.2.0`
HEAD `01c23b8` unless explicitly marked otherwise.

## 0. Resolution of DPR-01-A §4 design choices

Before the line-by-line edits, four open design choices from the
parent §4 narrative are resolved here. Each is named in
DPR-01-A's §11 carry-over or surfaces from the surrounding code
shape.

### Choice A — Where do `DSI_W` and `LTDC` typed singletons live?

**Decision (recommend):** Add `pub static DSI_W: DsiWrapper` and
`pub static LTDC: Ltdc` directly inside
`platform/src/frame_scheduler.rs`, alongside the existing
`FrameScheduler<S, P>` definition. The bare-metal DSI ISR calls
`frame_scheduler::consume_erif_static()` (a free function added
in Step 1) which reads `DSI_W.regs().wifcr.write(0x02)`.

**Coexistence:** `platform/src/dsi_cmd_mode.rs:33..36` already
declares its own `static DSI_HOST: Dsi`, `static DSI_W:
DsiWrapper`, `static LTDC_PERIPH: Ltdc`, `static GPIOJ: Gpio`.
These are **kept** through DPR-01a — `dsi_cmd_mode.rs` continues
to drive init (`stop_dsi`, `start_dsi`,
`configure_adapted_cmd_mode`, `enable_lp_cmd_overrides`,
`disable_lp_cmd_overrides`, `send_set_tear_on`,
`configure_te_gpio`) and its module-local `present` and
`handle_erif_isr` (used by the Zephyr path) remain live.

Two `DsiWrapper` singletons in the program are admissible
**because the `unsafe const fn DsiWrapper::new()` contract is
"at most one active at a time"**, and bare-metal vs. Zephyr are
mutually exclusive at build time via the `zephyr` cargo
feature. The DPR-01a step 6 audits this: the bare-metal `_dsi_isr`
calls the new `frame_scheduler::consume_erif_static`, never
`dsi_cmd_mode::handle_erif_isr`, so only one `static DSI_W` is
reachable in any single binary.

**Alternative (rejected for DPR-01a):** Aggregator at
`platform/src/hwcore/singletons.rs`. Reasonable goal but premature
— it asks the consolidation question one phase before the
DPR-01b FreeRTOS migration would expose the second consumer. The
aggregator becomes obvious work for DPR-01c or a later
consolidation phase.

### Choice B — Scheduler ownership at `Stm32h747iDiscoDisplay`

**Decision (recommend):** By-value field on
`Stm32h747iDiscoDisplay<B, BL, RST>`. Construction happens at the
*end* of `Stm32h747iDiscoDisplay::new`, after every init-time
MMIO write has landed (the AR=1 enable at line 916, the post-init
delay at 918). The scheduler is **only invoked from within
`swap` / `present` / `wait_frame_done` after `new` returns**, so
there is no aliasing concern with the init code that uses raw
casts and the typed `DsiWrapper::new()` singleton inside the
scheduler.

**Alternative (rejected for DPR-01a):** Hold a reference to a
program-`static` scheduler. This is what the analyzer-side
(DPR-03) pattern will eventually need so the scheduler can be
read from the DSI ISR. For DPR-01a, the ISR clears `WIFCR` via
the *separate* free function `consume_erif_static()` and the
`IsrFlag` / `IsrCounter` / `AtomicU32` triple, so the scheduler
itself does not need to be reachable from ISR context. By-value
keeps construction obvious; the `static` upgrade is left for
DPR-01b or DPR-03 if the analyzer wires a scheduler-aware ISR.

### Choice C — `wait_frame_done` semantics

**Finding:** `Stm32h747iDiscoDisplay::wait_frame_done`
(`platform/src/stm32h747i_disco.rs:1673..1692`) is a
**polling-side** helper, not an ISR. It busy-polls `WISR` at
`0x5000_040C` until ERIF fires, then clears `WIFCR` at line
1684. **No production caller invokes it** — the bare-metal main
loop at `examples/stm32h747i-disco/src/main.rs:4534..4628` uses
the `ERIF_FLAG` / `ERIF_CYCCNT` atomics fed by the DSI ISR at
`main.rs:478..514` instead. `wait_frame_done` is reachable only
by dead code or future telemetry.

**Decision:** Migrate `wait_frame_done`'s body to delegate to
`self.scheduler.consume_erif()` (the existing scheduler method
at `frame_scheduler.rs:342..344`) but *keep the public signature*
returning DWT cycle delta. The `WIFCR ← 0x02` write at line
1684 was the BASELINE-tracked entry; `consume_erif()` is the
typed equivalent. The polling loop body itself uses
`DsiWrapper.regs().wisr.read()` rather than the raw `WISR`
cast at line 1674.

The actual ISR-side `WIFCR ← 0x02` clear migrates separately —
Step 6 also rewrites the bare-metal `_dsi_isr::DSI` body in
`main.rs:478..514` to call the new
`frame_scheduler::consume_erif_static()` free function.

### Choice D — Coexistence with `dsi_cmd_mode::present` / `handle_erif_isr`

**Decision:** Both stay live through DPR-01a. They are still
called by the Zephyr path (`examples/stm32h747i-disco/src/zephyr_entry.rs`),
which is **out of scope** for DPR-01a (DPR-01c, deferred). The
bare-metal binary builds with `--features cm7,...` *without*
`zephyr`, so `dsi_cmd_mode::handle_erif_isr` is unreachable
from a bare-metal binary even though the symbol is compiled.

`dsi_cmd_mode::present` at lines 235..258 is a different
function from `Stm32h747iDiscoDisplay::present` — DPR-01a does
not touch it. DPR-01b (FreeRTOS) and a future DPR-01c (Zephyr)
will route Zephyr/FreeRTOS through `FrameScheduler::present` and
delete `dsi_cmd_mode::present`.

The two-`DSI_W`-singletons situation is resolved by the
"only-one-binary-at-a-time" cfg-feature exclusivity: in the
bare-metal binary, only the `frame_scheduler::DSI_W` is touched
after `new` returns; in a Zephyr binary, only
`dsi_cmd_mode::DSI_W` is touched. No simultaneous access.

## 1. Step 1 — Promote `frame_scheduler.rs` to typed-singleton pattern

**Scope:** `platform/src/frame_scheduler.rs` (currently 401
lines).

**Goal:** Add module-level typed singletons (`DSI_W`, `LTDC`)
matching the `dsi_cmd_mode.rs:33..36` precedent, plus a free
function `consume_erif_static()` callable from the bare-metal
DSI ISR. The scheduler `struct` retains its own field-held
copies (so `Stm32h747iDiscoDisplay` can own a scheduler by
value); the singletons are an *additional* surface for ISR-side
register access. This is the same "two paths to the same MMIO,
but typed handles enforce unaliasing" pattern.

**Edits:**

1. `platform/src/frame_scheduler.rs:30` (after
   `pub(crate) mod sealed { ... }`) — insert two
   `pub(crate) static` declarations matching the
   `dsi_cmd_mode.rs:33..36` pattern:
   - `pub(crate) static DSI_W: DsiWrapper = unsafe {
     DsiWrapper::new() };`
   - `pub(crate) static LTDC: Ltdc = unsafe { Ltdc::new() };`

   The SAFETY comment names the aliasing argument: this module
   owns the DSI wrapper + LTDC after `Stm32h747iDiscoDisplay::new`
   returns; cfg-feature exclusivity between bare-metal and
   Zephyr binaries ensures `dsi_cmd_mode.rs`'s parallel statics
   are not reachable concurrently.

2. Below the statics, add a free function:
   `pub unsafe fn consume_erif_static() {
   DSI_W.regs().wifcr.write(0x02); }`. Doc comment names the
   ISR-context precondition (called from DSI ISR or with DSI
   interrupts masked).

3. No other edits in this step. The `FrameScheduler` struct
   and its `swap` / `present` / `consume_erif` methods are
   untouched.

**New types/symbols introduced (this step):**
- `pub(crate) static DSI_W: DsiWrapper`
- `pub(crate) static LTDC:  Ltdc`
- `pub unsafe fn consume_erif_static()`

**Compile gate:**
```bash
RUSTFLAGS="" cargo test -p rlvgl-platform --lib frame_scheduler
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
```

**Discipline scanner effect:** None — Step 1 only adds new code,
removes no opt-out markers. BASELINE shape unchanged. Run the
scanner to confirm:
```bash
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
```

**Bench validation (user-driven, post-commit):** No bench;
scaffold-only change. CI green is sufficient.

**Rollback:** `git revert <commit>` cleanly reverts — Step 1
adds three new items, touches no existing code.

## 2. Step 2 — Add `BareMetalErifSignals` + `with_signals` constructor

**Scope:** `platform/src/frame_scheduler.rs` (the
`BareMetalLoopPacing` impl at lines 168..220).

**Goal:** Replace the scaffold `wait_erif` body with the
PCDN-DPR-006-ratified `IsrFlag + IsrCounter + AtomicU32`
pattern. Keep the existing `BareMetalLoopPacing::new()` as
a *scaffold-only* constructor — it constructs a pacing without
signals, and `wait_erif` panics or returns a stub. The
production path uses the new `with_signals` constructor.

**Edits:**

1. `platform/src/frame_scheduler.rs:25..29` — extend imports:
   add `use core::sync::atomic::{AtomicU32, Ordering};` and
   `use crate::hwcore::isr::{IsrCounter, IsrFlag};`.

2. `platform/src/frame_scheduler.rs:115` (after the `ErifInfo`
   block, before the `Pacing` trait at line 136) — insert a new
   `#[derive(Copy, Clone)] pub struct BareMetalErifSignals`
   with three fields: `flag: &'static IsrFlag`,
   `cyccnt: &'static AtomicU32`, `count: &'static IsrCounter`.
   Each field carries a doc comment per the PCDN-DPR-006
   pattern.

3. `platform/src/frame_scheduler.rs:168..170` — extend the
   `BareMetalLoopPacing` struct with `signals:
   Option<BareMetalErifSignals>`. Keep `erif_count: u32`.

4. `platform/src/frame_scheduler.rs:174..180` — keep existing
   `pub const fn new()` (now returns `Self { erif_count: 0,
   signals: None }`); add `pub const fn with_signals(signals:
   BareMetalErifSignals) -> Self { Self { erif_count: 0,
   signals: Some(signals) } }`.

5. `platform/src/frame_scheduler.rs:188..200` — rewrite the
   `wait_erif` body to branch on `self.signals`:
   - `Some(s)` path: `while !s.flag.take() {
     core::hint::spin_loop(); }` then return `ErifInfo {
     cyccnt: s.cyccnt.load(Ordering::Acquire), erif_count:
     s.count.read() }`.
   - `None` path: preserve the existing stub (host-test
     compatibility — `erif_count = erif_count.wrapping_add(1);
     ErifInfo { cyccnt: 0, erif_count }`).

   `compute_holdoff_us`, `wait_holdoff`, `signal_buf_ready`,
   `wait_render_gate` are unchanged from scaffold.

**New types/symbols introduced:**
- `pub struct BareMetalErifSignals` (with three `&'static` fields)
- `pub const fn BareMetalLoopPacing::with_signals(...)`
- `signals: Option<BareMetalErifSignals>` field on
  `BareMetalLoopPacing`

**Compile gate:**
```bash
RUSTFLAGS="" cargo test -p rlvgl-platform --lib frame_scheduler
```
The unit tests at `frame_scheduler.rs:349..401` continue to use
`BareMetalLoopPacing::new()` (the scaffold path); they remain
green because `signals: None` preserves the old stub semantics.

**Discipline scanner effect:** None.

**Bench validation:** None — type-level change.

**Rollback:** Step 2 only extends data structures and adds a new
constructor. Revert leaves Step 1's singletons intact.

## 3. Step 3 — Add scheduler field to `Stm32h747iDiscoDisplay`

**Scope:**
- `platform/src/stm32h747i_disco.rs` struct definition at
  lines 93..133.
- `platform/src/stm32h747i_disco.rs` constructor at line
  197 (`pub fn new`).

**Goal:** Embed a
`FrameScheduler<AdaptedCommand, BareMetalLoopPacing>` field on
`Stm32h747iDiscoDisplay`. Construct it at the end of
`new`, after every init-time MMIO write has landed (post the
auto-refresh enable + delay at lines 916..918, *before* the
function returns).

**Edits:**

1. `platform/src/stm32h747i_disco.rs:93..133` — extend the struct
   field set inside the existing cfg-gated block:
   ```rust
   #[cfg(all(
       feature = "stm32h747i_disco",
       any(target_arch = "arm", target_arch = "aarch64")
   ))]
   scheduler: crate::frame_scheduler::FrameScheduler<
       crate::frame_scheduler::AdaptedCommand,
       crate::frame_scheduler::BareMetalLoopPacing,
   >,
   ```
   Place this after `fb_addr_back` at line 132 to preserve the
   existing field ordering for Drop / Debug consistency.

2. `platform/src/stm32h747i_disco.rs:223..237` — extend the
   `Self { ... }` initializer in `new` with a new `scheduler`
   field. Construct via `unsafe { FrameScheduler::new(
   DsiWrapper::new(), Ltdc::new(), BareMetalLoopPacing::new()
   ) }`. SAFETY comment names the aliasing argument:
   `frame_scheduler::DSI_W` and `LTDC` statics also reference
   the same MMIO regions, but only one `FrameScheduler` is
   ever constructed and the singletons are accessed only from
   ISR / free-function context after Step 6, never
   concurrently with `&mut self` methods.

   The scheduler is constructed here with the scaffold
   `BareMetalLoopPacing::new()` (no signals). Step 6 swaps to
   `with_signals` via the `wire_erif_signals` setter.

**New types/symbols introduced:** None (uses Step 1-2 surface).

**Compile gate:**
```bash
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
```
Bare-metal build only; FreeRTOS path is unchanged in DPR-01a.

**Discipline scanner effect:** None — only adds a field and a
construction call.

**Bench validation:** Flash bare-metal binary, confirm splash +
desktop still render. No behavior change is intended — the
scheduler field is constructed but no `swap`/`present`/etc.
delegates to it yet.

**Rollback:** Field-only revert. Two-line removal in struct
def + one block removal in `new`.

## 4. Step 4 — Migrate Op A (`swap`)

**Scope:** `platform/src/stm32h747i_disco.rs:1612..1632`
(`pub fn swap`).

**Goal:** Replace the raw `0x5000_10AC` / `0x5000_1024` writes at
lines 1626..1627 with a delegation to
`self.scheduler.swap(PhysAddr::new(next))`. Keep the
`cortex_m::interrupt::free` wrapper — interrupt safety is a
caller-side invariant per `FrameScheduler::swap`'s doc.

**Edits:**

1. `platform/src/stm32h747i_disco.rs:1622..1631` — inside the
   `cortex_m::interrupt::free(|_| { ... })` block, replace the
   two-write `unsafe { ... }` body (lines 1625..1628) with a
   single `self.scheduler.swap(PhysAddr::new_unchecked(next as
   usize))` call wrapped in `unsafe` for the `PhysAddr`
   constructor's preconditions. (At v0.2.0 HEAD `01c23b8` the
   API is `PhysAddr::new_unchecked` per
   `platform/src/hwcore/addr.rs`; verify at PR base SHA.) The
   surrounding `cortex_m::asm::dsb()` calls, the
   `core::mem::swap(&mut self.fb_addr, &mut self.fb_addr_back)`,
   and the `let next = self.fb_addr_back` capture are
   preserved.

2. **Removed lines (1626, 1627):** the two raw casts to
   `0x5000_10AC` (L1CFBAR) and `0x5000_1024` (SRCR.IMR),
   including their `// rlvgl-discipline: allow(...)` opt-out
   markers.

3. The inner `unsafe { ... }` block at line 1625 is removed
   along with its body — `scheduler.swap` is a safe call. The
   PhysAddr constructor's `unsafe` envelope is the only
   remaining `unsafe` in this method's body.

**New types/symbols introduced:** None.

**Compile gate:**
```bash
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
```

**Discipline scanner effect:** Two opt-out markers shed (lines
1626, 1627). BASELINE comment text in `discipline.rs:161..165`
("bulk per-line opt-out markers cover ... DSI/LTDC bring-up")
narrows; no BASELINE *array entries* change.

**Bench validation (user-driven, post-commit):** `make
flash-disco` and confirm:
- Splash boot screen visible.
- Desktop widget rendering visible.
- No new flicker on a 60-second soak.
The `swap` path is currently used at boot for the very first
frame before the ERIF ISR is armed (see `swap`'s doc comment at
line 1606..1611). The animation path uses `present`. So this
step's behavior change is mostly invisible — golden frame
captured pre-DPR-01a should be pixel-identical.

**Rollback:** Restore the two-line `unsafe { ... }` block and
revert the `scheduler.swap(...)` call.

## 5. Step 5 — Migrate Op B (`present`)

**Scope:** `platform/src/stm32h747i_disco.rs:1638..1662`
(`pub fn present`).

**Goal:** Replace the five raw writes at lines 1646, 1650, 1651,
1656, 1659 with a single delegation to
`self.scheduler.present(PhysAddr)`. The `S::PULSED_LTDCEN`
const-generic gate inside `FrameScheduler::present` already
handles the AdaptedCommand-only LTDCEN pulse + trailing CERIF
clear, so the disco's `AdaptedCommand` instantiation gets the
full five-write sequence.

**Edits:**

1. `platform/src/stm32h747i_disco.rs:1638..1662` — rewrite the
   `pub fn present(&mut self)` body. Preserve the prefix
   `cortex_m::asm::dsb()` (line 1641 — cache drain before LTDC
   reads) and the `let next = self.fb_addr_back` capture. Then
   call `self.scheduler.present(unsafe {
   PhysAddr::new_unchecked(next as usize) })`. Then
   `core::mem::swap(&mut self.fb_addr, &mut self.fb_addr_back)`.

2. **Removed lines (1644..1660):**
   - 1646: `WIFCR ← 0x02` (pre-retarget CERIF clear)
   - 1647, 1648, 1658: intermediate `cortex_m::asm::dsb()` —
     no longer needed; the scheduler's writer body issues a
     single barrier
   - 1650: `L1CFBAR ← next`
   - 1651: `SRCR ← 1`
   - 1656: `WCR ← 0x0C` (LTDCEN pulse)
   - 1659: `WIFCR ← 0x02` (post-LTDCEN CERIF clear)
   - All five raw-cast writes' opt-out markers are deleted
     with their lines.

3. The two `unsafe { ... }` blocks at 1643..1652 and
   1654..1660 are removed; `scheduler.present(...)` is a safe
   call (the inner `unsafe` is contained in the `PhysAddr`
   constructor).

**New types/symbols introduced:** None.

**Compile gate:**
```bash
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
```

**Discipline scanner effect:** Five opt-out markers shed (lines
1646, 1650, 1651, 1656, 1659). Comment narrowing in
`discipline.rs:161..165` continues.

**Bench validation (user-driven, post-commit):** Highest-risk
step. `make flash-disco`, confirm:
- Splash visible.
- Desktop visible, widget tree rendering at ~30 fps.
- Star crawl animation runs without tearing.
- 5-minute soak with no flicker, no scan-time regression.
The five writes are the hot path — any off-by-one in ordering
inside `FrameScheduler::present` surfaces here as flicker, snow,
or freeze. Compare against the pre-DPR-01a golden frame.

**Rollback:** Revert to the original `unsafe { ... }` block. The
five writes are sequential and order-dependent — restore them
exactly as in v0.2.0 HEAD.

## 6. Step 6 — Migrate Op C (`wait_frame_done` + bare-metal DSI ISR)

**Scope:**
- `platform/src/stm32h747i_disco.rs:1673..1692`
  (`pub fn wait_frame_done`)
- `examples/stm32h747i-disco/src/main.rs:466..515` (the
  `_dsi_isr::DSI` interrupt body)
- `examples/stm32h747i-disco/src/main.rs:440..454` (existing
  `ERIF_FLAG: AtomicBool` and `ERIF_CYCCNT: AtomicU32`)

**Goal:** Three coupled migrations:
1. Replace `ERIF_FLAG: AtomicBool` with `IsrFlag`; add a new
   `ERIF_COUNT: IsrCounter`. (`ERIF_CYCCNT: AtomicU32` stays —
   API already matches PCDN-DPR-006.)
2. Rewrite `_dsi_isr::DSI` body to use a module-local
   `DsiWrapper` for the WISR read + bulk WIFCR clear + WCR
   write, and bump the three signals at the end.
3. Wire the scheduler's pacing to the signal triple via a new
   `Stm32h747iDiscoDisplay::wire_erif_signals(...)` setter
   called once from `main` after statics are addressable.
4. Migrate `wait_frame_done`'s body to typed handles
   (`frame_scheduler::DSI_W` for WISR poll;
   `self.scheduler.consume_erif()` for the clear;
   `cortex_m::peripheral::DWT::cycle_count()` for timing).

**Edits:**

1. `examples/stm32h747i-disco/src/main.rs:440..454` — change
   `ERIF_FLAG` type from `core::sync::atomic::AtomicBool` to
   `rlvgl_platform::IsrFlag`; add a parallel
   `pub(crate) static ERIF_COUNT: rlvgl_platform::IsrCounter =
   rlvgl_platform::IsrCounter::new();` block with the same
   cfg-gate. `IsrFlag` / `IsrCounter` are already re-exported
   at `platform/src/lib.rs:213`.

2. `examples/stm32h747i-disco/src/main.rs:478..514` — rewrite
   the DSI ISR body:
   - Declare a local `static DSI_W_LOCAL: DsiWrapper = unsafe {
     DsiWrapper::new() };` at the top of the `unsafe fn DSI`.
   - Read `wisr = DSI_W_LOCAL.regs().wisr.read()`.
   - Clear all wrapper flags via
     `DSI_W_LOCAL.regs().wifcr.write(wisr & 0x3FFF)`.
   - On `wisr & 0x02 != 0`: capture
     `cortex_m::peripheral::DWT::cycle_count()`; clear LTDCEN
     via `DSI_W_LOCAL.regs().wcr.write(0x08)`; then publish the
     three signals in order: `ERIF_CYCCNT.store(cyc, Release)`,
     `ERIF_COUNT.increment()`, `ERIF_FLAG.set()`.
   - Host-level FIR0/FIR1 clears at lines 504..512 keep their
     opt-out markers pending a typed DSI-host accessor.
   - **Removed:** const WISR (480), const WIFCR (481), const
     DSI_WCR (498), raw `DSI_WCR.write_volatile(0x08)` (499).
     Line 495 (PJ0 scope-probe BSRR write) is **kept with
     opt-out marker** — typed GPIOJ migration is deferred to a
     follow-up.

3. `examples/stm32h747i-disco/src/main.rs:523..525` — keep
   `take_erif()` signature; redirect to `ERIF_FLAG.take()`.
   Existing call sites at line 4547 / 4564 / 4622 are
   unchanged because `IsrFlag::take` matches the previous
   `AtomicBool::swap(false, AcqRel)` semantics exactly.
   Line 4564's explicit `ERIF_FLAG.store(false, Release)`
   becomes `ERIF_FLAG.clear()`.

4. `platform/src/stm32h747i_disco.rs:1673..1692` — rewrite
   `wait_frame_done`:
   - Replace raw `DWT_CYCCNT.read_volatile()` with
     `cortex_m::peripheral::DWT::cycle_count()`.
   - Replace raw `WISR.read_volatile()` with
     `crate::frame_scheduler::DSI_W.regs().wisr.read()`.
   - Replace raw `WIFCR.write_volatile(0x02)` with
     `unsafe { self.scheduler.consume_erif(); }` — the
     `consume_erif`'s `unsafe` envelope is satisfied because
     the caller is in a polling context with DSI interrupts
     armed but the ISR is by definition not actively running
     (we are *waiting* for it to fire).
   - **Removed:** const declarations at 1674, 1675, 1676 +
     four `read_volatile`/`write_volatile` calls.

5. `platform/src/stm32h747i_disco.rs` (after the existing
   `swap` / `present` / `wait_frame_done` block, ~line 1700) —
   add `pub fn wire_erif_signals(&mut self, signals:
   crate::frame_scheduler::BareMetalErifSignals)` that
   replaces the scheduler's pacing field via `*self.scheduler.pacing_mut()
   = BareMetalLoopPacing::with_signals(signals);`. Scheduler's
   `dsi_wrapper` / `ltdc` fields are untouched.

6. `examples/stm32h747i-disco/src/main.rs` — locate the
   `let mut display = Stm32h747iDiscoDisplay::new(...)` call
   site (upstream of the existing scope_probe / DSI IRQ
   enable block at lines 3247..3275, approximately line
   3050..3200). Insert a `display.wire_erif_signals(
   BareMetalErifSignals { flag: &ERIF_FLAG, cyccnt: &ERIF_CYCCNT,
   count: &ERIF_COUNT })` call.

   **Ordering invariant:** `wire_erif_signals` MUST land
   before `NVIC::unmask(Interrupt::DSI)` at lines 3273..3274,
   so no edge fires into a pacing with `signals: None`.

**New types/symbols introduced:**
- `pub fn Stm32h747iDiscoDisplay::wire_erif_signals(...)`
- New static `ERIF_COUNT: IsrCounter` in `main.rs`
- `ERIF_FLAG` type changes from `AtomicBool` to `IsrFlag`

**Compile gate:**
```bash
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --lib frame_scheduler
```

**Discipline scanner effect:**
- `stm32h747i_disco.rs:1674,1675,1676,1684` — four opt-out
  markers shed.
- `main.rs:480,481,498,499` — four opt-out markers shed in the
  ISR body (the FIR0/FIR1 / ISR0/ISR1 access keeps its markers
  pending a typed DSI-host accessor, deferred).
- `main.rs:495` (PJ0 scope probe) keeps its marker pending a
  typed GPIOJ migration (in scope for a discipline-cleanup
  follow-up, not DPR-01a).

**Bench validation (user-driven, post-commit):** Coupled with
Step 5 — the present/wait_frame_done pair is the hot path. `make
flash-disco`, confirm:
- ERIF ISR is firing (visible as star-crawl animation advancing
  smoothly at ~30 fps).
- `take_erif()` callers at `main.rs:4547`/`4622` see the same
  edge semantics (the swap-and-clear on `IsrFlag::take()` is
  identical to the old `AtomicBool::swap`).
- `take_erif()` returns true at most once per ISR fire.
- 24-hour soak with star crawl on: no flicker regression vs.
  pre-DPR-01a golden capture.

**Rollback:** Three coupled files — `stm32h747i_disco.rs`,
`main.rs`, `frame_scheduler.rs`. Revert as one commit. The
`ERIF_FLAG` type change is the only API-shape change for
existing call sites (the `take_erif()` shim absorbs it).

## 7. Step 7 — Remove the DWT probe (stm32h747i_disco.rs:934..993)

**Scope:** `platform/src/stm32h747i_disco.rs:920..995` (the DWT
EoR probe block).

**Goal:** Delete the dev-only diagnostic block per DPR-01-A §2.1
(lines 934, 952, 955, 964, 992, 993). This is a one-shot
post-init measurement that has no production role.

**Edits:**

1. `platform/src/stm32h747i_disco.rs:920..995` — delete the
   entire `unsafe { ... }` block from "DWT-timed EoR probe"
   (currently around line 920) through the final `cortex_m::asm::dsb()`
   at ~995. Specifically:
   - Lines 922..995 (the entire `unsafe { ... }` block).
   - The preceding two-line comment block at 920..921.
2. Preserve the `wisr` read at ~996..997 only if downstream
   semihosting prints reference it; otherwise delete through to
   the next `dbg(...)` call.

**Removed register writes (per DPR-01-A §2.1):**
- 934: `WIFCR ← 0x03` (probe-only)
- 952: `WCFGR ← wcfgr & !(1<<6)` (AR=0 for probe)
- 955: `WIFCR ← 0x03`
- 964: `WCR ← 0x0C` (single LTDCEN pulse)
- 992: `WCFGR ← wcfgr` (restore AR=1)
- 993: `WIFCR ← 0x03`

Plus the DWT_CTRL/DEMCR/LAR setup writes at 925..930, which are
**kept** if downstream code (the bare-metal main loop's
`cycles_since_erif`, etc.) depends on DWT being enabled. Review
at PR base SHA: confirm whether DWT init is duplicated in
`main.rs` or relied on solely from `new`. If duplicated, delete
the `new`-side block; if not, preserve only lines 925..930.

**New types/symbols introduced:** None.

**Compile gate:**
```bash
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
```

**Discipline scanner effect:** Eight opt-out markers shed
(lines 934, 952, 955, 964, 992, 993, plus the two DWT_CYCCNT
reads at 938, 940 if DWT init code is fully deleted, or
preserved otherwise). Marker count is dependent on the DWT-init
preservation decision above; conservatively, six markers shed.

**Bench validation (user-driven, post-commit):** `make
flash-disco`, confirm:
- Splash + desktop boot.
- No regression in boot-time DWT-driven timing (e.g.
  `cycles_since_erif` at `main.rs:533` still returns sensible
  values).
- 60-second soak.
The probe is dev-only; its removal should be invisible.

**Rollback:** Pure deletion. Revert restores the block exactly.

## 8. Step 8 — Final compile/test gate + bench validation

**Scope:** All files touched by Steps 1..7.

**Goal:** Run the full pre-publish sequence (Phase 0..6 from
CLAUDE.md) to confirm the consolidated change is publishable.

**Edits:** None.

**Compile gates:**

```bash
# Phase 0: format
cargo fmt --all -- --check

# Phase 1: clippy
RUSTFLAGS="" cargo clippy --workspace -- -D warnings

# Phase 2: workspace tests
RUSTFLAGS="" cargo test --workspace

# Phase 2.5: discipline (baseline mode — confirm no NEW violations)
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile

# Phase 3-4.9 per CLAUDE.md as scoped to this change.

# Phase 6: embedded build
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf \
    -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
    --features cm7,splash,desktop,dma2d
make build-disco
```

**Discipline scanner effect:** Cumulative — across Steps 4..7
the scanner sheds at least 15 opt-out markers in
`stm32h747i_disco.rs` and 4 in `main.rs`. BASELINE *array
entries* in `discipline.rs:123..` are unchanged (the
DPR-01-A-tracked registers were never in BASELINE — they were
covered by per-line opt-out markers).

**Bench validation (user-driven, post-commit):**

1. `make flash-disco` — bare-metal binary boots, splash visible,
   desktop renders.
2. Star crawl animation: smooth, no tearing, ~30 fps.
3. 24-hour soak with `examples/apps/disco-demo` running:
   no flicker regression, no scan-time drift, no DSI ISR
   missed-fire.
4. Golden frame: capture `D<x>,<y>,<w>,<h>,1` via playit, diff
   against pre-DPR-01a golden.

**Rollback:** Coupled — Steps 4, 5, 6 form the atomic migration.
If any bench validation fails, revert all three together; Step 1
(scaffold), 2 (signals struct), 3 (field add), 7 (probe deletion)
can each be retained independently if Steps 4..6 are reverted.

## 9. Files Cited

- `docs/concepts/DPR-00-CONCEPTS.md` §6 INV-DPR-3 (consolidated-
  MMIO invariant), §6 INV-DPR-8 (typed register coverage).
- `docs/concepts/DPR-01-CONCEPTS.md` §5.4 (FrameScheduler
  signature), §5.6 (Pacing trait), §10 (reconciliation
  decisions).
- `docs/concepts/DPR-01-A.md` §2 (per-site inventory at v0.2.0
  HEAD), §3 (Op A/B/C/D grouping), §4 phase-1 (six-step
  sequence).
- `platform/src/frame_scheduler.rs:25..401` (scaffold,
  scan-mode, pacing, scheduler types).
- `platform/src/dsi_cmd_mode.rs:33..36` (typed-singleton
  precedent), :219..258 (its own `present`), :260..305
  (`handle_erif_isr`).
- `platform/src/stm32h747i_disco.rs:93..133` (struct),
  :197..1100 (init), :920..995 (DWT probe), :1612..1632
  (`swap`), :1638..1662 (`present`), :1673..1692
  (`wait_frame_done`).
- `platform/src/hwcore/regs/dsi.rs:221..251` (`Dsi`),
  :254..279 (`DsiWrapper`).
- `platform/src/hwcore/regs/ltdc.rs:148..187` (`Ltdc`).
- `platform/src/hwcore/isr.rs:169..200` (`IsrFlag`),
  :214..257 (`IsrCounter`).
- `platform/src/hwcore/addr.rs` (`PhysAddr`, `MmioAddr`).
- `platform/src/lib.rs:213` (re-export of `IsrChannel`,
  `IsrCounter`, `IsrFlag`), :261 (re-export of
  `Stm32h747iDiscoDisplay`).
- `examples/stm32h747i-disco/src/main.rs:440..454` (existing
  `ERIF_FLAG: AtomicBool` + `ERIF_CYCCNT: AtomicU32`),
  :466..515 (the `_dsi_isr::DSI` body), :517..525
  (`take_erif`), :3247..3275 (init-time DSI IRQ enable),
  :4547/4622 (`take_erif` call sites).
- `platform/tests/discipline.rs:123..277` (BASELINE array and
  shape).

## 10. Unblocks

This outline unblocks:
- The DPR-01a code PR (steps 1..7 each as a separate commit, or
  steps 1..3 + steps 4..7 as two commits).
- DPR-01b's design conversation — the `BareMetalErifSignals` /
  `with_signals` pattern generalizes to FreeRTOS as a similar
  `FreeRtosErifSignals` taking semaphore handles.
- The `dsi_cmd_mode.rs` / `frame_scheduler.rs` consolidation
  question. DPR-01a leaves both `static DSI_W` declarations in
  place; a future phase (DPR-01c or a discipline-cleanup
  follow-up) collapses them into a single owner once the Zephyr
  path also routes through `FrameScheduler`.

## 11. Change Log

- **2026-05-19** — Initial draft. Resolves PCDN-DPR-006 via
  the `IsrFlag + IsrCounter + AtomicU32` signal triple. Elaborates
  DPR-01-A §4 phase-1 into eight executable steps with
  per-step compile gates and bench-validation checkpoints.
  Resolves design choices A (singletons live in
  `frame_scheduler.rs`), B (scheduler is a by-value field on
  `Stm32h747iDiscoDisplay`), C (`wait_frame_done` is polling,
  routes through `consume_erif`), D (Zephyr/FreeRTOS coexistence
  via cfg-feature exclusivity). Companion sub-letter to
  `DPR-01-A.md` §4.
