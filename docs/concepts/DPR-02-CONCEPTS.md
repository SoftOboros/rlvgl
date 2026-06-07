# DPR-02 — Warm-Reset Safe Stop, Boot Sentinels, Telemetry Slot Layout

**Status:** Draft 2026-05-19. Not ratified. This document ratifies the
warm-reset cleanup vocabulary, boot-sentinel registry, and telemetry
slot layout deferred from DPR-00 §5.3 / §8 / §10 (INV-DPR-5 implementation
plan and the reserved `0x3800_0500..0x3800_0600` Board Runtime range).

DPR-02 ratification unblocks DPR-02a (SafeStop scaffold + demo audio-
service validation) and DPR-02b (boot-sentinel migration + analyzer
adoption prep). It does not itself migrate the analyzer — that gate
remains in DPR-03 per INV-DPR-15.

## 0. Authority Policy

This doc is the normative source for the **warm-reset safe stop, boot
sentinel registry, and telemetry slot layout** sub-phase of the DPR
initiative. It ratifies the Board Runtime services that implement
INV-DPR-5 (warm-reset cleanup is platform-owned) and freezes the
SRAM4 byte-offset layout inside the DPR-00 §5.3 reserved range.

Vocabulary and invariants from DPR-00 §3 / §6 and DPR-01 §3 are binding
here without restatement. Where DPR-02 adds new vocabulary
(`SafeStopSequence`, `BootSentinel`, `TelemetrySlot`,
`PeripheralServiceSet`, `IrqClearMask`), the additions are recorded in
§3 below.

The authority split:

| Concern | Owner | DPR-02 relationship |
|---|---|---|
| STM32H747 peripheral-disable mechanisms (DMA stream EN, SAI block EN, NVIC ICER/ICPR, I2C peripheral disable, SDMMC disable, QUADSPI disable, BDMA channel EN) | ST RM0399 §16 (DMA), §54 (SAI), §47 (I2C), §57 (SDMMC), §25 (QUADSPI), ARMv7-M Architecture Reference Manual B3.4 (NVIC ICER/ICPR) | Cited; DPR-02 does NOT redocument peripheral disable semantics. Frozen ordering and timeout budgets in §5 cite the relevant RM chapters by number only. |
| Reference safe-stop sequence | `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs::peripheral_safe_stop()` (lines 263..368) | Evidence only. The analyzer's open-coded sequence is the reference for ordering and timeout budgets; DPR-02 reshapes it into a profile-driven Board Runtime service. The analyzer's literal becomes a deletion candidate under DPR-03. |
| Existing demo boot sentinels | `examples/stm32h747i-disco/src/main.rs` (lines 1496, 1508, 1513, 1756, 1770, 1860, 2003, 2017, 2144, 2149, 2154) | Evidence only. Demo writes 11 `0xA11C_xxxx` literals to `0x3800_0300`. This address overlaps the analyzer's `SAFE_STOP_TELEMETRY_ADDR` (`0x3800_0304`) by proximity; the collision is the immediate motivation for §5.3's strict offset layout. |
| Existing analyzer boot sentinels | `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs::boot_sentinel` (lines 812..871) | Evidence only. Six `0xA11C_xxxx` constants `PRE_GPIO_SPLIT`..`POST_FIRST_RENDER` at `ADDR = 0x3800_0300`. Migrated under DPR-02b. |
| Telemetry range table | `DPR-00-CONCEPTS.md` §5.3 | Binding. DPR-02 ratifies the byte-offset *layout* inside the `0x3800_0500..0x3800_0600` Board Runtime range reserved there; moving the outer range remains a DPR-00 §5.3 Standards-Action change. |
| INV-DPR-5 warm-reset cleanup is platform-owned | `DPR-00-CONCEPTS.md` §6 | Binding. DPR-02 is the implementation plan for INV-DPR-5. |
| ServiceSet → safe-stop mapping policy | `DPR-01-CONCEPTS.md` §5.1 (`services: ServiceSet`) and §5.6 (Pacing) | Binding. DPR-02 §5.4 maps each registered service to its safe-stop steps; new services added under DPR-01 Specification Required MUST §5.4-amend with the corresponding mapping. |
| Cross-repo analyzer adoption | `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md` and the planned `DAA-01-B-2` | Out of scope for DPR-02. DPR-02 publishes the surface; ratification of analyzer consumption is a DPR-03 gate. |

If a DPR-02 sub-phase (a/b) changes a frozen byte-offset in §5.3, a
boot-sentinel constant in §5.2, or the safe-stop ordering in §5.1, this
doc's §15 MUST be amended first in a separate change.

## 1. Purpose

Make warm-reset cleanup a Board Runtime service rather than a copy of
the analyzer's open-coded sequence. Specifically:

- **Peripheral safe-stop.** A bounded, ServiceSet-driven sequence that
  stops autonomous peripherals left running across CPU reset
  (`SYSRESETREQ`, probe-rs reset, NRST pulse), clears pending IRQ
  state, and records per-peripheral timeout telemetry — before the new
  boot's init code touches the same peripherals. Modeled on
  `peripheral_safe_stop()` in analyzer-cm7 `bsp.rs:263..368`.
- **Boot sentinel registry.** A named, public set of `0xA11C_xxxx`-
  style sentinels written at canonical boot milestones, migrated from
  the demo's and analyzer's open-coded literals into a single public
  Rust constant set under DPR-02b. Sentinels MUST live inside the DPR-
  00 §5.3 `0x3800_0500..0x3800_0600` Board Runtime range; the existing
  `0x3800_0300` write site is reconciled under §10.
- **Telemetry slot layout.** A frozen byte-offset table inside the
  reserved range, so future Board Runtime telemetry (safe-stop report,
  boot sentinels, ServiceSet activation log) cannot accidentally
  collide.

Out of scope for DPR-02 (see §9): the actual safe-stop *code* (lands
under DPR-02a), HSEM (DPR-01b), clock tree (DPR-01a), analyzer
migration (DPR-03).

## 2. Problem Statement

Three concrete failure modes motivate DPR-02:

- **Analyzer carries open-coded safe-stop with raw MMIO casts and a
  hard-coded SRAM4 address.** `analyzer-cm7/src/bsp.rs:263..368`
  implements `peripheral_safe_stop()` using raw `*mut u32` writes to
  `DMA1_LIFCR` (`0x4002_0008`), `DMA1_S0CR` (`0x4002_0010`),
  `DMA1_S1CR` (`0x4002_0028`), `SAI1_ACR1`/`SAI1_BCR1`/`SAI1_ACLRFR`/
  `SAI1_BCLRFR` (`0x4001_5804..0x4001_583C`), and NVIC ICER0/ICPR0
  (`0xE000_E180`, `0xE000_E280`), with telemetry at
  `SAFE_STOP_TELEMETRY_ADDR = 0x3800_0304`. The sequence has six
  `// rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)`
  markers and four ~1 ms busy-wait loops. It exists because the
  alternative — copying half of `rlvgl-platform` into the analyzer —
  was rejected by DAA-01-B Option C (mirror only).
- **Boot sentinel addresses already collide in practice.** The demo
  writes 11 sentinels to `0x3800_0300` (`main.rs:1496..2154`); the
  analyzer's `boot_sentinel::ADDR` is also `0x3800_0300`
  (`bsp.rs:828`). The analyzer's `SAFE_STOP_TELEMETRY_ADDR` is
  `0x3800_0304` — adjacent. If a future demo run with the analyzer's
  build flags pre-boots the analyzer, the second slot is clobbered by
  the demo's sentinel writes. Both sit *outside* the DPR-00 §5.3
  `0x3800_0500..0x3800_0600` Board Runtime range that is supposed to
  own them.
- **Demo has no warm-reset cleanup today.** `examples/stm32h747i-disco/`
  (both `main.rs` and `freertos_entry.rs`) does not run any
  peripheral safe-stop on cold or warm boot. If the demo is built with
  `audio` enabled (per the CLAUDE.md cached-release feature set
  `cm7,dma2d,splash,desktop,audio`) and the CPU is reset while SAI1
  + DMA1 streams are mid-transfer, the next boot inherits running
  peripherals. This is the `RESET-WEDGE-ANALYSIS.md` failure mode the
  analyzer hit and that the demo will hit as soon as a user power-
  cycles a unit with audio active. INV-DPR-5 requires this to be a
  Board Runtime service, not an app-copy job.

The failure mode is not any one app being wrong. It is that the
analyzer learned the safe-stop sequence the hard way and codified it
inline; the demo will eventually need the same sequence; and neither
the safe-stop telemetry nor the boot-sentinel literals live in the
range DPR-00 reserved for them.

## 3. Glossary (Additions to DPR-00 §3 and DPR-01 §3)

Capitalized use of these terms in DPR docs MUST refer to the
definitions below. DPR-00 §3 and DPR-01 §3 entries remain authoritative
and are not restated here.

| Term | Meaning | Owner |
|---|---|---|
| **SafeStopSequence** | The ordered, timeout-bounded sequence run during early boot to halt autonomous peripherals left running from the previous boot, clear their pending IRQ state, and record telemetry. Decomposed into 5 frozen steps per §5.1. | DPR-02. |
| **BootSentinel** | A named 32-bit constant written to a known offset inside the §5.3 telemetry range at a canonical boot milestone. The presence of a BootSentinel value in SRAM4 is positive proof that the corresponding boot step completed. Adding sentinels is Standards Action. | DPR-02. |
| **TelemetrySlot** | A frozen byte range inside the DPR-00 §5.3 reserved Board Runtime range (`0x3800_0500..0x3800_0600`), assigned to a single owner (boot sentinels, safe-stop report, future ServiceSet activation log). Slots MUST NOT overlap; the layout is in §5.3. | DPR-02. |
| **PeripheralServiceSet** | The subset of DPR-01 §5.1 `ServiceSet` values that have a corresponding SafeStopSequence step. Currently `{ audio, mems_mic, sd, qspi, codec_reset }`. `scope_probes` is excluded because GPIO output pins do not require disable sequencing. Adding entries is Specification Required and MUST come with a §5.4 mapping. | DPR-02. |
| **IrqClearMask** | A 32-bit mask passed to `NVIC_ICERn` and `NVIC_ICPRn` per peripheral, naming the IRQ lines that the SafeStopSequence's step 4 (clear pending) MUST clear. One mask per peripheral, registered in §5.4. | DPR-02. |
| **SafeStopReport** | The structured output of a SafeStopSequence execution. Contains the per-peripheral timeout bitmask, total elapsed microseconds, and the entry/exit telemetry sentinel pair. Persisted to the SRAM4 TelemetrySlot at `0x3800_0510..0x3800_0520`. | DPR-02. |

## 4. Source-of-Truth Map (Additions)

DPR-00 §4 and DPR-01 §4 remain authoritative for existing surface.
DPR-02 adds:

| Surface | New location | DPR-02 treatment |
|---|---|---|
| `SafeStop` service struct, `SafeStopReport`, `run` entry point | `platform/src/board_runtime/safe_stop.rs` (new) | Public module behind `pub mod board_runtime` (DPR-01 §7). Chosen over a top-level `platform/src/warm_reset.rs` because every entry point in this file is consumed by `BoardRuntime::init` and is meaningless outside the runtime composition — co-locating with the runtime keeps the ownership boundary visible. The shorter `warm_reset.rs` name is rejected because future Board Runtime services (clock-tree validation, heap allocation, HSEM init, telemetry slot allocation) will also live under `board_runtime/`, and a flat top-level file fragments the module tree. |
| `BootSentinel` constants + `write_sentinel(stage)` helper | `platform/src/board_runtime/boot_sentinel.rs` (new) | Public sub-module of `board_runtime`. Exposed as `rlvgl_platform::board_runtime::boot_sentinel::*`. The analyzer's existing `pub mod boot_sentinel` (`analyzer-cm7/src/bsp.rs:826..871`) becomes a thin re-export shim under DPR-03; the demo's open-coded literals become deletion candidates under DPR-02b. |
| `TelemetryHandles` / `TelemetrySlot` typed accessors | `platform/src/board_runtime/telemetry.rs` (new) | Public sub-module. Slot allocation is compile-time-checked via const offset constants; the layout in §5.3 is the source of truth. |
| Analyzer's `peripheral_safe_stop` (`bsp.rs:263..368`) | (evidence only) | Reference behavior. The DPR-02a SafeStop implementation MUST produce equivalent register effects for the `audio` service subset on hardware. Validation captures: warm-reset wedge symptom from `docs/AUDIO-BEES-DEBUG-LOG.md` (analyzer-side, evidence-only). |
| Analyzer's `boot_sentinel::ADDR = 0x3800_0300` | (evidence only) | Reconciled under DPR-02b: the new public address is `0x3800_0500` (DPR-00 §5.3 reserved range). Migration sequence in §8 DPR-02b. |
| Demo's `0x3800_0300` raw sentinel writes (`main.rs:1496..2154`) | (evidence only) | 11 raw writes, all reconciled under DPR-02b. Replace with calls to `boot_sentinel::write(stage)` at the same boot milestones. |

## 5. Frozen Decisions

### 5.1 SafeStopSequence Ordering

Registration policy: **Standards Action**.

A SafeStopSequence for one peripheral MUST execute the following five
steps in order. The ordering is non-negotiable; each step depends on
the previous one's effect (e.g. clearing NVIC pending after disabling
the DMA stream prevents a stale TC ISR from being re-raised by the
EN-clear write):

| Step | Action | Timeout budget | RM0399 reference |
|---|---|---|---|
| **1. NVIC mask** | Write the peripheral's `IrqClearMask` to `NVIC_ICERn` to mask its IRQ lines. | ~1 µs (single store). | ARMv7-M B3.4.4 (ICER) |
| **2. DMA channel disable** | If the peripheral has a DMA channel, clear the channel `EN` bit (DMA stream `CR.EN`, BDMA channel `CCR.EN`) and busy-wait until the bit reads 0 with a ~1 ms budget (400 000 nop-poll iterations at sys_ck = 400 MHz). | ~1 ms per channel. | RM0399 §16.5.5 (DMA), §16.7.5 (BDMA) |
| **3. Peripheral disable** | Clear the peripheral's `EN` bit (`SAI_xCR1.SAIxEN`, `I2C_CR1.PE`, `SDMMC_POWER` if applicable, `QUADSPI_CR.EN`) and busy-wait with the same ~1 ms budget. | ~1 ms per peripheral block. | RM0399 §54 (SAI), §47 (I2C), §57 (SDMMC), §25 (QUADSPI) |
| **4. NVIC pending clear** | Write the peripheral's `IrqClearMask` to `NVIC_ICPRn` so any TC / TE / HT event that was pending at reset is dropped before the new boot unmasks the IRQ. | ~1 µs (single store). | ARMv7-M B3.4.5 (ICPR) |
| **5. Telemetry record** | Update the SafeStopReport: OR the per-peripheral timeout bits accumulated in steps 2/3 into the report's `timeouts` field; advance `elapsed_us` from the DWT cycle counter. | ~1 µs (in-RAM update). | (no MMIO; in-RAM TelemetrySlot at `0x3800_0510..0x3800_0520`) |

Total budget across all peripherals in a single SafeStopSequence run:
**~5 ms wall-clock** (DPR-02 ceiling; INV-DPR-2-2 below makes this
hard). If a peripheral times out at step 2 or step 3, the corresponding
bit in `SafeStopReport.timeouts` is set and the sequence proceeds to
step 4 for that peripheral; the SafeStopSequence MUST NOT block boot
on a single peripheral's timeout. RM-cited disable semantics complete
in microseconds when working correctly, so 1 ms per step is
~1000× margin.

The exact register sequences are vendor-specific. DPR-02a's
implementation MUST use `hwcore::regs::*` typed accessors where they
exist (DMA, NVIC) and add typed accessors where they don't (SAI block,
I2C, SDMMC, QUADSPI) — raw `*mut u32` casts in `safe_stop.rs` are an
INV-DPR-8 violation and a discipline scanner regression.

### 5.2 BootSentinelSet

Registration policy: **Standards Action**.

The following five named boot sentinels are frozen as the DPR-02
initial set. Each is a 32-bit value with the upper 16 bits = `0xA11C`
("ALIC" — preserved from the demo/analyzer convention) and the lower
16 bits encoding the boot stage. Values are chosen sparsely so that an
SRAM4 hex dump distinguishes them at a glance and so leaving room for
future stages is trivial.

| Sentinel | Value | Boot milestone |
|---|---|---|
| `PRE_CLOCK_INIT` | `0xA11C_0010` | Earliest Rust execution path entered; pre clock-tree programming. Equivalent to the demo's `0xA11C_0001` write at `main.rs:1496`. |
| `POST_CLOCK_INIT` | `0xA11C_0020` | Clock tree (PLL1/PLL2/PLL3) configured per INV-DPR-12. Heap not yet initialized. |
| `POST_SAFE_STOP` | `0xA11C_0030` | SafeStopSequence completed for every peripheral in the active PeripheralServiceSet. SafeStopReport is committed to its TelemetrySlot. |
| `POST_SDRAM_INIT` | `0xA11C_0040` | FMC SDRAM bring-up complete; Bank 2 at `0xD000_0000` is readable + writable. Equivalent to analyzer `POST_SDRAM_INIT` (`0xA11C_0007`). |
| `POST_DISPLAY_INIT` | `0xA11C_0050` | `Stm32h747iDiscoDisplay::new` returned successfully. Equivalent to analyzer `POST_DISPLAY_INIT` (`0xA11C_0008`) and demo write `0xA11C_0011` at `main.rs:2017`. |

The sentinel values are **renumbered** vs. the demo's and analyzer's
existing literals — the legacy `0xA11C_0001..0xA11C_0022` sequence is
unstructured (gaps reflect debugger-attach delays, not boot semantics).
The DPR-02 numbering uses 0x10 spacing so DPR-02b or later can slot
sub-stages (`0xA11C_0011` "POST_CLOCK_INIT_PLL1_LOCKED",
`0xA11C_0012` "POST_CLOCK_INIT_PLL3_LOCKED") without renumbering the
named set. Adding a new top-level sentinel between two existing ones
is Standards Action; adding a sub-stage inside the gap is Specification
Required.

Sentinels MUST be written *after* the corresponding boot step
completes (INV-DPR-2-3 below). Writing a sentinel before the step
defeats the "positive proof of progress" property.

### 5.3 TelemetrySlot Layout

Registration policy: **Expert Review** for additions; **Standards
Action** for moving an existing slot.

Inside DPR-00 §5.3's reserved `0x3800_0500..0x3800_0600` Board Runtime
range (256 bytes), the following byte-offset layout is frozen:

| Range | Slot | Owner | Layout |
|---|---|---|---|
| `0x3800_0500..0x3800_0510` | **boot_sentinels** | `boot_sentinel::write(stage)` | 4 × u32. Slot 0 = most-recent sentinel value. Slots 1..3 reserved for future "ring buffer of last-N stages" (DPR-02b decides whether to use them; default = slot 0 only). |
| `0x3800_0510..0x3800_0520` | **safe_stop_report** | `SafeStop::run()` | 4 × u32: `[0] = entry sentinel` (`0xB007_5A5E`), `[1] = exit sentinel \| timeout_mask` (`0xB007_D000 \| (timeouts & 0xFFFF)`), `[2] = elapsed_us`, `[3] = reserved`. |
| `0x3800_0520..0x3800_0530` | **service_set_active** | `BoardRuntime::init` | 4 × u32: `[0] = bitmask of activated services` (bit positions per DPR-01 §5.1 ServiceSet), `[1..3] = reserved` for activation timestamps under DPR-02b. |
| `0x3800_0530..0x3800_0600` | **reserved** | (unallocated) | Future Board Runtime telemetry. Claiming a slot is Expert Review and MUST §15-amend this table. |

The legacy `0x3800_0300..0x3800_0310` range (analyzer
`SAFE_STOP_TELEMETRY_ADDR` + demo `0xA11C_xxxx` writes) remains
described in DPR-00 §5.3 as the analyzer's safe-stop slot. DPR-02b
migrates the demo's writes into the new `0x3800_0500` range; the
analyzer's writes remain at `0x3800_0304` until DPR-03 retires them.
After DPR-03 lands, `0x3800_0300..0x3800_0310` reverts to "unallocated"
in DPR-00 §5.3 via a §15 amendment to that doc.

### 5.4 PeripheralServiceSet → SafeStopSequence Mapping

Registration policy: **Specification Required** for additions.

For each service in DPR-01 §5.1's `ServiceSet`, the following table
freezes which peripherals SafeStopSequence MUST stop and which
`IrqClearMask` it MUST use. Services not in this table (currently
`scope_probes`) have no SafeStopSequence and contribute no work to a
SafeStop run.

| Service | Peripherals stopped | `IrqClearMask` (NVIC vectors) | Timeout bit in `SafeStopReport.timeouts` |
|---|---|---|---|
| **audio** | DMA1 stream 0 (SAI1_A RX), DMA1 stream 1 (SAI1_B TX), SAI1 Block A, SAI1 Block B | IRQ 11 (DMA1_STR0), IRQ 12 (DMA1_STR1), IRQ 87 (SAI1) | bits 0..3 (matches analyzer `peripheral_safe_stop` semantics) |
| **mems_mic** | SAI4 Block A, BDMA channel for SAI4 PDM RX | IRQ 146 (SAI4), IRQ 129 (BDMA_CH for SAI4) | bits 4..5 |
| **sd** | SDMMC1, DMA channel for SDMMC1 (built-in IDMA, not DMA1/2) | IRQ 49 (SDMMC1) | bit 6 |
| **qspi** | QUADSPI, DMA channel for QUADSPI (MDMA on H747 — IRQ 122) | IRQ 92 (QUADSPI), IRQ 122 (MDMA) | bit 7 |
| **codec_reset** | I2C4 (no DMA today; I2C4 transactions are blocking) | IRQ 95 (I2C4_EV), IRQ 96 (I2C4_ER) | bit 8 |

Bits 9..15 in `SafeStopReport.timeouts` are reserved for future
services. Bits 16..31 are reserved for future per-peripheral
sub-step timeouts (e.g. distinguishing "DMA channel did not disable"
from "DMA channel disabled but stream-flag clear took too long").

A profile whose `ServiceSet` includes a service from this table MUST
either activate the corresponding SafeStopSequence step or explicitly
opt out via `RuntimeConfig::safe_stop = SafeStopPolicy::ColdBootOnly`
and document why in the profile's narrative. Mismatched ServiceSet vs.
SafeStopSequence is an INV-DPR-2-4 violation.

`SafeStopPolicy::ColdBootOnly` is meaningful only on platforms that
can distinguish cold from warm reset (POR vs. SYSRESETREQ); on
STM32H747 the distinction is read from `RCC_RSR` per RM0399 §8.7.21,
deferred to DPR-02a for typed handling.

### 5.5 SafeStop Public Surface Shape (informative)

The DPR-02a Rust API ratifies the following shape (final field names
and visibility ratify in code review):

- `SafeStop::run(services: ServiceSet, telemetry: &mut TelemetryHandles)
  -> SafeStopReport` — entry point called once during `BoardRuntime::
  init`, before clock-tree reprogramming has changed peripheral kernel
  clocks. Returns a copy of the persisted SafeStopReport for the
  caller's convenience.
- `SafeStopReport { timeouts: u32, elapsed_us: u32 }` — opaque struct
  exposing the timeout bitmask and total elapsed wall-clock from the
  DWT cycle counter. Display formatting is informative; the persisted
  TelemetrySlot is the canonical record.
- `SafeStopPolicy { ColdBootOnly, Always }` — profile-supplied switch.
  Demo and analyzer profiles default to `Always`; future
  power-management-aware profiles may opt in to `ColdBootOnly`.

Full Rust signatures land in DPR-02a §7.

## 6. Runtime Invariants

DPR-00 §6 INV-DPR-1..15 and DPR-01 §6 remain binding. DPR-02 adds
four invariants specific to the warm-reset cleanup surface:

- **INV-DPR-2-1: Safe-stop precedes clock-tree reprogramming.** A
  Board Runtime profile that runs SafeStopSequence MUST run it
  **before** the new boot reprograms RCC dividers, PLL configuration,
  or peripheral kernel-clock muxes. Rationale: changing the SAI1
  kernel clock from PLL3 (previous boot) to its reset default (current
  boot) while SAI1 is enabled with an active DMA transfer can latch
  the DMA peripheral into a partially-disabled state that the new
  boot's SAI init then cannot recover. The analyzer's reference
  sequence runs safe-stop after clock-tree config; DPR-02 reverses
  this for the demo and analyzer-via-DPR-03 alike.
- **INV-DPR-2-2: Total safe-stop budget is bounded.** SafeStopSequence
  for every peripheral in the active PeripheralServiceSet MUST
  complete within ~5 ms wall-clock total (1 ms per peripheral × 5
  peripheral families × ~1 timeout margin). If a peripheral times out
  at step 2 or 3 of §5.1, the corresponding bit in
  `SafeStopReport.timeouts` is set and the sequence proceeds. Safe-
  stop MUST NOT block boot on a stuck peripheral. The 5 ms ceiling is
  a hard upper bound for the DPR-02 ServiceSet; adding a service
  whose safe-stop budget would push the total over 5 ms is a §15
  Specification Required change and MUST update this invariant.
- **INV-DPR-2-3: Boot sentinels are positive-progress markers.** A
  BootSentinel value MUST be written to its TelemetrySlot **after** the
  corresponding boot step completes, never before. The presence of a
  sentinel in SRAM4 is positive proof the named step succeeded; writing
  the sentinel speculatively before the step defeats this property and
  is an INV-DPR-2-3 violation. Probe-rs dumps of SRAM4 are the primary
  bench-validation surface for cold/warm boot sequencing.
- **INV-DPR-2-4: ServiceSet and SafeStopSequence must agree.** A
  RuntimeProfile whose `ServiceSet` includes a service listed in §5.4
  MUST either (a) include the corresponding SafeStopSequence step in
  its safe-stop activation, or (b) opt out via
  `RuntimeConfig::safe_stop = SafeStopPolicy::ColdBootOnly` and
  document the rationale in the profile's narrative. Silent mismatch
  (service active but no safe-stop) is the failure mode that produces
  the analyzer's "warm reset bricked the audio path" symptom and is a
  discipline violation. DPR-02a CI MUST include a compile-time check
  that named demo/analyzer presets satisfy this invariant.

## 7. API Sketch — Informative, Deferred to DPR-02a

This section is informative. Exact Rust signatures ratify in DPR-02a.
The intent is to surface the ownership boundaries implied by §5 / §6,
not to commit to names.

```rust
// Pseudo-Rust. Ratifies in DPR-02a.

pub struct SafeStop {
    // opaque — owns the typed register handles for the peripherals it
    // stops, plus a reference to the DWT cycle counter for the elapsed_us
    // accumulator.
}

impl SafeStop {
    /// Run the safe-stop sequence for every peripheral implied by
    /// `services`, in the §5.1 frozen order. Records §5.3
    /// safe_stop_report telemetry and returns a SafeStopReport for the
    /// caller.
    ///
    /// SAFETY: MUST be called once during early boot, before any
    /// peripheral in the active PeripheralServiceSet is enabled by the
    /// new boot's init code. Specifically, it MUST precede clock-tree
    /// reprogramming (INV-DPR-2-1).
    pub unsafe fn run(
        services: ServiceSet,
        telemetry: &mut TelemetryHandles,
    ) -> SafeStopReport;
}

pub struct SafeStopReport {
    pub timeouts: u32,     // bitmask, one bit per peripheral per §5.4
    pub elapsed_us: u32,   // total wall-clock from DWT cycle counter
}

pub enum SafeStopPolicy {
    /// Always run safe-stop on every boot.
    Always,
    /// Run safe-stop only on detected warm reset (RCC_RSR per RM0399
    /// §8.7.21). Cold (POR) boots skip; saves ~5 ms at startup.
    ColdBootOnly,
}

pub mod boot_sentinel {
    pub const PRE_CLOCK_INIT:    u32 = 0xA11C_0010;
    pub const POST_CLOCK_INIT:   u32 = 0xA11C_0020;
    pub const POST_SAFE_STOP:    u32 = 0xA11C_0030;
    pub const POST_SDRAM_INIT:   u32 = 0xA11C_0040;
    pub const POST_DISPLAY_INIT: u32 = 0xA11C_0050;

    /// Write a boot-stage sentinel to the §5.3 boot_sentinels slot.
    /// Per INV-DPR-2-3, MUST be called *after* the named step.
    pub fn write(stage: u32);
}
```

The actual constructor signature and how `SafeStop` integrates into
`BoardRuntime::init` ratifies in DPR-02a. The `BoardRuntime::init`
boot ordering is: `PRE_CLOCK_INIT` sentinel → SafeStopSequence →
clock-tree init → `POST_CLOCK_INIT` sentinel → `POST_SAFE_STOP`
sentinel → heap init → SDRAM init → `POST_SDRAM_INIT` sentinel →
display init → `POST_DISPLAY_INIT` sentinel.

Note that `POST_SAFE_STOP` is written **after** `POST_CLOCK_INIT`,
not adjacent to safe-stop completion, so that the sentinel sequence
remains monotonic with the boot-stage numbering. SafeStopReport is
the per-peripheral detail; the sentinel is just the "we got past
safe-stop" marker.

## 8. Phase Plan

### DPR-02 (this doc) — Concept doc

Acceptance:

- [ ] §3 vocabulary additions accepted.
- [ ] §5.1..§5.5 frozen decisions accepted (or amended).
- [ ] §6 INV-DPR-2-1..4 accepted.
- [ ] §7 API sketch accepted as the target for DPR-02a/b code.

### DPR-02a — SafeStop scaffold + demo audio-service validation

Land `platform/src/board_runtime/safe_stop.rs` with the `SafeStop`
struct, `SafeStopReport`, and `SafeStopPolicy` per §5.5 / §7. Add
typed `hwcore::regs::*` accessors for SAI block disable, BDMA channel
disable, and NVIC ICER/ICPR (extending the existing 85%-typed
coverage cited in DPR-01 §2). Wire `SafeStop::run` into
`BoardRuntime::init` before clock-tree reprogramming.

Validate the demo with the `cm7,desktop,dma2d,audio` feature set:
cold boot, probe-rs warm reset, repeated power-cycle (10 cycles
minimum). SRAM4 telemetry at `0x3800_0510..0x3800_0520` MUST show
`entry = 0xB007_5A5E`, `exit = 0xB007_D000` (zero timeouts) on a
clean run; bench captures of intentional mid-transfer reset MUST
show the corresponding timeout bit set without blocking boot.

Acceptance:

- All raw `*mut u32` casts in `safe_stop.rs` are eliminated; the
  module passes the discipline scanner with no new exemptions.
- Demo `cm7,desktop,dma2d,audio` build flashes and boots through 10
  consecutive power-cycles with no audio-path wedge.
- SafeStopReport telemetry at `0x3800_0510..0x3800_0520` is
  observable via probe-rs and matches the expected pattern.
- INV-DPR-2-1..4 are verified: safe-stop precedes clock-tree
  reprogramming (manual code-path review), total budget under 5 ms
  (DWT measurement in the report), sentinels written after steps
  (manual code-path review), ServiceSet ↔ SafeStopSequence agreement
  (compile-time check per INV-DPR-2-4).
- PCDN-DPR-2-001..003 are resolved or explicitly deferred to DPR-02b.

### DPR-02b — BootSentinel migration + analyzer adoption prep

Land `platform/src/board_runtime/boot_sentinel.rs` with the §5.2
public constants and `write(stage)` helper. Migrate the demo's 11
`0xA11C_xxxx` raw writes at `main.rs:1496..2154` to call
`boot_sentinel::write(...)`. Re-publish the constants as
`rlvgl_platform::board_runtime::boot_sentinel::*` so the analyzer can
delete its local module under DPR-03.

Verify with probe-rs SRAM4 dump:

- `0x3800_0500..0x3800_0510` shows the named sentinels in §5.2 order
  as the demo boots through clock init → safe-stop → SDRAM → display.
- The legacy `0x3800_0300..0x3800_0310` range is no longer written
  by the demo (analyzer still writes there until DPR-03).

Acceptance:

- Demo `cm7,desktop,dma2d,audio` build emits sentinels in the §5.3
  `0x3800_0500..0x3800_0510` slot in the §5.2-frozen sequence on a
  successful boot.
- No remaining `0xA11C_xxxx` literals in `examples/stm32h747i-disco/
  src/`; all routed through `boot_sentinel::write`.
- DPR-00 §5.3 amended via a §15 entry to reflect that the
  `0x3800_0300..0x3800_0310` range is now analyzer-only and slated
  for deprecation under DPR-03.

### DPR-03 (out of scope here)

Cross-repo analyzer adoption. Gated on DPR-02a/b landing and on
DAA-01-B-2 §15 ratifying analyzer consumption per INV-DPR-15.

## 9. Non-Goals

- **DPR-02 does not migrate or rewrite the analyzer's
  `peripheral_safe_stop`.** That is the analyzer's job under DPR-03,
  after the platform-side surface has stabilized on the demo.
- **DPR-02 does not extend safe-stop to peripherals not in any
  current service.** USB OTG, Ethernet, CRYP, RNG, FDCAN, etc. are
  out of scope. If/when a service is added for any of those under
  DPR-01 §5.1 Specification Required, DPR-02's §5.4 mapping MUST be
  amended in the same PR.
- **DPR-02 does not redocument peripheral disable semantics.** RM0399
  is cited by chapter; DPR-02 does not duplicate register bit
  positions or sequence narrative from the manual.
- **DPR-02 does not handle CM4-side safe-stop.** Dual-core profiles
  (`Cores::Cm7Cm4`) require CM4 to either be unhalted *after* CM7's
  safe-stop completes (the analyzer's current shape per INV-DPR-11)
  or to run its own per-core safe-stop sequence. Per-core CM4 safe-
  stop is deferred to a future DPR phase if/when an analyzer use
  case requires it.
- **DPR-02 does not commit to a specific cold/warm reset detection
  mechanism.** `SafeStopPolicy::ColdBootOnly` reserves the *policy*
  axis; the `RCC_RSR` decode lands in DPR-02a per §5.4.

## 10. Reconciliation Decisions

| Existing concept | DPR-02 decision |
|---|---|
| Analyzer `peripheral_safe_stop` (`bsp.rs:263..368`) | Evidence only. Equivalent behavior moves into `platform/src/board_runtime/safe_stop.rs` under DPR-02a. First validator is `Demo::*` with `audio` in `ServiceSet`. Analyzer-side `peripheral_safe_stop` becomes a deletion candidate under DPR-03; the analyzer's `// Adapted from rlvgl@d99f793:...` header on the function block is removed in the same PR. |
| Analyzer `boot_sentinel` module (`bsp.rs:826..871`) | Evidence only. The DPR-02b public module under `rlvgl_platform::board_runtime::boot_sentinel::*` becomes the source of truth. Analyzer-side `pub mod boot_sentinel` becomes a re-export shim or is deleted under DPR-03, depending on whether the analyzer's CM4 polling code still consumes the old values during the migration. |
| Analyzer `SAFE_STOP_TELEMETRY_ADDR = 0x3800_0304` | Evidence only. The new DPR-02 `safe_stop_report` slot is `0x3800_0510..0x3800_0520` (§5.3). The analyzer retains `0x3800_0304` until DPR-03 lands. Both addresses are simultaneously written during the DPR-02a→DPR-03 transition window; the analyzer's `SAFE_STOP_TELEMETRY_ADDR` constant is deprecated in DPR-03 and removed from the analyzer source in DAA-01-B-2. |
| Demo's `0x3800_0300` raw sentinel writes (`main.rs:1496, 1508, 1513, 1756, 1770, 1860, 2003, 2017, 2144, 2149, 2154`) | Migrated to `boot_sentinel::write(...)` under DPR-02b. All 11 raw `*mut u32` casts become discipline-scanner removals. The 11 demo-side sentinel *values* (`0xA11C_0001`..`0xA11C_0022`) compress to the 5 §5.2 named sentinels; the intermediate values (debugger-attach delay markers, MPU-config checkpoints) become DPR-02b sub-stage extensions under Specification Required. |
| Demo's missing safe-stop | Added under DPR-02a. The `Demo::BareMetal` and `Demo::FreeRtos` profile expansions (DPR-01 §5.1) gain `safe_stop: SafeStopPolicy::Always` per INV-DPR-2-4 when the `audio` service is in their `ServiceSet`. |
| DPR-01 §5.1 `ServiceSet` | Binding. DPR-02's §5.4 maps each entry to its SafeStopSequence step. Adding a new service to DPR-01 §5.1 under Specification Required MUST add a §5.4 row in the same PR per INV-DPR-2-4. |
| DPR-01 §5.6 `Pacing` trait | Orthogonal. SafeStopSequence runs before any Pacing instance is constructed; the Pacing implementation does not participate in safe-stop. |
| `RCC_RSR` reset-cause register (RM0399 §8.7.21) | Decoded by DPR-02a into a typed `ResetCause { Cold, Warm, Watchdog, BorrowDeep, ... }` enum. The decode lands in `platform/src/board_runtime/reset_cause.rs` (a new module sibling to `safe_stop.rs`). `SafeStopPolicy::ColdBootOnly` consumes the decoded `ResetCause`. |

## 11. Open Questions Carried into DPR-02a/b

These do not block DPR-02 ratification; they are deferred to DPR-02a
or DPR-02b PRs. Each MUST be resolved in the named sub-phase's §15
entry (or re-deferred with a named later phase).

- **PCDN-DPR-2-001:** Should `SafeStopReport` be retained across boots
  via SRAM4 persistence (so the *previous* boot's report is observable
  on the next boot's startup), or only in-RAM during boot? The
  analyzer's current shape persists implicitly because SRAM4 is not
  zeroed on warm reset; DPR-02b decides whether the public API
  surface treats this as a contract or a coincidence. Telemetry
  recovery (debugging "what was wrong with boot N-1") vs. SRAM4
  budget (256 bytes total in the reserved range) is the tradeoff.
  Resolve under DPR-02a.
- **PCDN-DPR-2-002:** Should boot sentinels be published as a Rust
  `enum BootSentinel { PreClockInit = 0xA11C_0010, ... }` so the
  compiler enforces use of named values, or as bare `const` items
  matching the analyzer's existing convention? The const-set shape is
  more flexible (sub-stages between named milestones are easy to add);
  the enum shape catches typos at compile time. Resolve under DPR-02b.
- **PCDN-DPR-2-003:** How does SafeStopSequence interact with DCB
  (DMA Cacheable Buffers, `docs/concepts/DCB-00-CONCEPTS.md`)? If a
  DMA transfer was in flight at reset, the cache lines for the
  destination buffer may be dirty (TX direction) or stale (RX
  direction). Step 2 of §5.1 disables the DMA channel but does NOT
  perform cache maintenance. Question: does DPR-02a need a step 2.5
  that runs `SCB::clean_dcache_by_address` / `invalidate_dcache_by_
  address` on the destination buffer range, or does the new boot's
  DCB-typestate-driven init handle this implicitly via
  `DcaCacheCtx`? Resolve under DPR-02a after a literature review of
  the DCB-00 §5 typestate transitions for the RX-direction
  `HalfGuard`.
- **PCDN-DPR-2-004:** Should `ResetCause` decoding (§10
  reconciliation row) live in `safe_stop.rs` or a sibling
  `reset_cause.rs` module? The current §4 source-of-truth map
  proposes a sibling module; the alternative (folding into
  `safe_stop.rs`) keeps the file count smaller but couples two
  semi-orthogonal concerns. Resolve under DPR-02a.

## 12. Acceptance Checklist

DPR-02 (this concept doc) is **ratified 2026-05-20**:

- [x] §3 vocabulary additions are accepted.
- [x] §5.1 (SafeStopSequence ordering) is accepted.
- [x] §5.2 (BootSentinelSet) is accepted.
- [x] §5.3 (TelemetrySlot layout) is accepted.
- [x] §5.4 (PeripheralServiceSet mapping) is accepted.
- [x] §5.5 (SafeStop public surface shape) is accepted.
- [x] §6 invariants INV-DPR-2-1..4 are accepted.
- [x] §8 phase plan (DPR-02a, DPR-02b) is accepted.
- [x] §10 reconciliation rows are accepted (additions allowed via §15
      amendment).
- [x] §11 PCDN-DPR-2-001..004 are explicitly deferred to DPR-02a or
      DPR-02b.

DPR-02a and DPR-02b code PRs have their own acceptance gates per §8.

## 13. Files Cited

- `docs/concepts/README.md`
- `docs/concepts/DPR-00-CONCEPTS.md` (especially §5.3 telemetry range
  table, §6 INV-DPR-5, §8 DPR-02 phase plan, §10 reconciliation row
  "Analyzer peripheral_safe_stop")
- `docs/concepts/DPR-01-CONCEPTS.md` (especially §5.1 ServiceSet, §5.6
  Pacing trait, §8 DPR-01b acceptance)
- `docs/concepts/DCB-00-CONCEPTS.md` (PCDN-DPR-2-003 cross-reference)
- `platform/src/hwcore/regs/dsi.rs`, `ltdc.rs`, `tim.rs` (existing
  typed-accessor coverage; DPR-02a extends with SAI block / NVIC /
  I2C / SDMMC / QUADSPI)
- `platform/src/hwcore/isr.rs` (IsrChannel / NVIC primitives DPR-02a
  builds on)
- `examples/stm32h747i-disco/src/main.rs` (lines 1490..1514,
  1756..1770, 1860, 2003..2017, 2144..2154 — the 11 demo-side
  `0xA11C_xxxx` raw sentinel writes targeted by DPR-02b migration)
- `examples/stm32h747i-disco/src/freertos_entry.rs` (no current
  safe-stop or sentinel writes; DPR-02a integrates via `BoardRuntime
  ::init` which both bare-metal and FreeRTOS entry paths call)
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs` (lines
  212..232 telemetry constants, 263..368 `peripheral_safe_stop` body,
  812..871 `boot_sentinel` module)
- Parent workspace: `streamz/submodules/disco-analyzer/docs/concepts/
  DAA-01-B-RLVGL-INTEGRATION.md` (DPR-03 cross-repo gating reference)
- RM0399 (cited by chapter only):
  §8.7.21 RCC_RSR, §16.5.3 / §16.5.5 DMA, §16.7.5 BDMA, §25 QUADSPI,
  §47 I2C, §54 SAI, §57 SDMMC.
- ARMv7-M Architecture Reference Manual: B3.4.4 NVIC ICER, B3.4.5
  NVIC ICPR.

## 14. Unblocks

DPR-02 unblocks DPR-02a (SafeStop scaffold + demo audio-service
validation) and DPR-02b (BootSentinel migration). DPR-03 (cross-repo
analyzer adoption) remains gated on DPR-02a landing AND on DAA-01-B-2
§15 ratifying analyzer consumption per INV-DPR-15.

## 15. Change Log

- **2026-05-20** — **Ratified.** All §12 acceptance gates checked.
  Scaffold validated by execution: SafeStop scaffold + BootSentinel
  registry + TelemetrySlot byte-offset table all landed in commit
  `fbaf54d` without surfacing structural objections. 16/16 new unit
  tests pass; disco target compile clean. Three signature
  refinements applied during scaffolding (PostMemoryInit →
  PostSdramInit per §5.2, RESERVED split into ServiceSetActive + tail
  per §5.3, repr(C) + Eq/PartialEq derives) align the executed code
  with the doc; no spec amendment needed. DPR-02a (demo audio-service
  warm-reset validation) and DPR-02b (BootSentinel migration of the
  demo's 11 raw `0x3800_0300` writes + analyzer adoption prep) remain
  binding sub-phase gates per §8. PCDN-DPR-2-001..004 remain deferred
  per §11.
- **2026-05-19** — Initial draft. Ratifies §3 vocabulary additions
  (`SafeStopSequence`, `BootSentinel`, `TelemetrySlot`,
  `PeripheralServiceSet`, `IrqClearMask`, `SafeStopReport`);
  freezes §5.1 SafeStopSequence ordering, §5.2 BootSentinelSet, §5.3
  TelemetrySlot byte-offset layout, §5.4 PeripheralServiceSet →
  SafeStopSequence mapping; adds §6 invariants INV-DPR-2-1..4; lays
  out the DPR-02a (SafeStop scaffold + demo audio-service validation)
  and DPR-02b (BootSentinel migration) phase split. Defers
  PCDN-DPR-2-001..004 to DPR-02a/b. Reconciles the demo's open-coded
  `0x3800_0300` raw sentinel writes (`main.rs:1496..2154`, 11 sites)
  and the analyzer's `peripheral_safe_stop` + `boot_sentinel`
  modules under §10.
