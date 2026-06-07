# DPR-03 — Dual-App Validation (Cross-Repo Analyzer Adoption)

**Status:** Draft 2026-05-19. Not ratified. This document plans the
cross-repo gate that retires the disco analyzer's ~1200 lines of mirrored
bring-up code by routing the analyzer onto the rlvgl-platform
`BoardRuntime::init` + `FrameScheduler<VideoMode, BareMetalLoopPacing>` +
`SafeStop::run` surface ratified in DPR-01 / DPR-02.

DPR-03 ratification closes the "second-app proof" gate named in DPR-00
INV-DPR-2 and INV-DPR-15. It does not move analyzer code into
`rlvgl-platform`; it ratifies the analyzer's adoption of the
already-published platform surface, and records the cross-repo
coordination contract with `DAA-01-B-2` (the planned successor to the
existing `DAA-01-B-RLVGL-INTEGRATION.md` Option C ratification).

## 0. Authority Policy

DPR-03 is unusual within the DPR family: it is the only phase that
crosses the `rlvgl` ↔ `streamz/submodules/disco-analyzer` repo boundary.
The authority split below is the central planning question — owning
which decision lives on which side keeps the two repos from silently
forking the runtime contract.

| Concern | Owner | DPR-03 relationship |
|---|---|---|
| Frozen scan-mode axes / runtime-profile axes / invariants | `DPR-00-CONCEPTS.md` §5 / §6 | Binding on analyzer-side adoption. The analyzer MUST consume `RuntimeProfile::Analyzer` per DPR-01 §5.2; INV-DPR-3 (apps do not own display MMIO), INV-DPR-11 (HSEM-before-CM4-unhalt), INV-DPR-12 (clock-tree audio invariants), and INV-DPR-15 (cross-repo gating) are non-negotiable. |
| Concrete Rust surface (`BoardRuntime::init`, `FrameScheduler<S,P>`, `Pacing`, `HsemSet::LINE_IN_RX`) | `DPR-01-CONCEPTS.md` §5 / §7 | Binding. The analyzer adopts the public re-exports under `rlvgl_platform::{BoardRuntime, FrameScheduler, ...}` as-is; analyzer-side wrappers MUST NOT shadow or re-shape these types. |
| Warm-reset SafeStop, BootSentinelSet, TelemetrySlot layout | `DPR-02-CONCEPTS.md` §5 | Binding. The analyzer's open-coded `peripheral_safe_stop` + `boot_sentinel` + `SAFE_STOP_TELEMETRY_ADDR = 0x3800_0304` retire in favor of `SafeStop::run(services, telemetry)` and the §5.3 `0x3800_0500..0x3800_0600` slot layout. |
| Analyzer audio DSP graph, FFT, widget composition, render loop body | `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs` (post-line-~1990 application layer) | Stays analyzer-side. DPR-03 explicitly does **not** migrate audio DSP, spectrum FFT, meter rendering, scope-view composition, or the render-dirty gating logic into `rlvgl-platform`. See §9. |
| Analyzer HSEM[6] receive ISR body (post-init audio mailbox wake) | `streamz/submodules/disco-analyzer/analyzer-cm7/src/hsem.rs` (the ISR body) | Stays analyzer-side. DPR-01 §5.5 reserves `HsemSet::LINE_IN_RX` as the open-registration entry point; `BoardRuntime::init` performs the **enable** sequence per INV-DPR-11, but the ISR's audio-mailbox wake semantics remain an analyzer concern. |
| Analyzer-side `AdaptedCodeOrigin` header retirement | DAA-01-B-2 (planned) | Owned by the analyzer subrepo. DAA-01-B-2 §15 records the §6 transition from Option C (mirror) to Option C-Retired (consume from platform); rlvgl-side DPR-03 acceptance MUST cite the DAA-01-B-2 §15 commit hash per INV-DPR-15. |
| embedded-hal 0.2 ↔ 1.0 pin/I2C adapters (`HalGpioBacklight`, `HalResetPin`, `I2c4Adapter`) | Analyzer subrepo `bsp.rs` | Retained analyzer-side under the §5.2 ShimAllowlist. These adapters wrap `stm32h7xx-hal 0.16` (which ships eh-0.2 only) into the eh-1.0 surface that `Stm32h747iDiscoDisplay::new` and the `rlvgl-platform::wm8994` driver require; they are app-level glue, not bring-up code. |

If a DPR-03 sub-phase (a/b/c) changes a binding row above, this doc's §15
MUST be amended first in a separate change, and the corresponding
DAA-01-B-2 amendment MUST be cited.

## 1. Purpose

Retire the ~1200 lines of bring-up code that the analyzer mirrored from
`rlvgl@d99f793:examples/stm32h747i-disco/` per `DAA-01-B-RLVGL-INTEGRATION.md`
§6 (2026-04-27 ratification of Option C — mirror), by routing the
analyzer onto the published `BoardRuntime::init` +
`FrameScheduler<VideoMode, BareMetalLoopPacing>` + `SafeStop::run`
surface ratified in DPR-01 and DPR-02.

After DPR-03c closes:

- The analyzer subrepo contains no `// Adapted from rlvgl@<hash>:...`
  headers except for the §5.2 ShimAllowlist entries.
- `analyzer-cm7/src/bsp.rs` is reduced to the §5.2 ShimAllowlist content
  (`display_pins`, `audio_i2c`); `init_clocks`, `peripheral_safe_stop`,
  `init_fmc_sdram`, `boot_sentinel`, and `SAFE_STOP_TELEMETRY_ADDR`
  are deleted.
- `analyzer-cm7/src/main.rs` boot stages 1–9 collapse into a single
  `BoardRuntime::init(display_pset, memory_pset, clock_pset,
  RuntimeProfile::Analyzer)?` call; the remaining 4 stages (widget tree,
  active-effect bookkeeping, joystick state, render loop) stay
  unchanged.
- `analyzer-cm7/src/hsem.rs` retains its ISR body and ICR/MISR access
  but deletes `init_lineiin_receive` — the enable sequence is subsumed
  by `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4 { hsem_lines:
  HsemSet::LINE_IN_RX }` expansion.

The goal is **not** to make analyzer-side code public, canonical, or
upstream-able. The analyzer remains a second-app validation surface;
DPR-03 ratifies that the platform's runtime contract can serve that
second app without copy-paste.

## 2. Problem Statement

`DAA-01-B-RLVGL-INTEGRATION.md` §6 (2026-04-27) ratified Option C
(mirror `rlvgl@d99f793:examples/stm32h747i-disco/` linker / BSP /
FreeRTOS-glue / IPC offsets) **explicitly because** the BSP generator
path was out of scope and the rlvgl-side example→library extraction was
deferred. The user's recorded rationale (DAA-01-B §6): *"the BSP for
this board does not fall out of the BSP generator cleanly — and
backport to rlvgl from this effort would be to backport from example to
BSP generator via SVD / PAC / template fixes which is out of scope and
should be its own effort, thus example alignment should be sufficient
for this to be picked up later in that effort."*

DPR is that "later in that effort." Concrete evidence (paths absolute
for cross-repo clarity):

- `/Users/iraabbott/softoboros/streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs:1-15`
  carries the `// Adapted from rlvgl@d99f793:examples/stm32h747i-disco/src/main.rs`
  header that declares the mirror. The mirror covers ~871 lines of
  clock-tree, FMC SDRAM, peripheral safe-stop, boot-sentinel, and pin-
  adapter code.
- `analyzer-cm7/src/bsp.rs:105-198` manually reconfigures PLL3
  (fractional VCO 393.216 MHz, `DIVM3=5`, `DIVN3=78`, `FRACN3=5270`,
  `DIVP3=32`, `DIVR3=12`) to land PLL3_P = 12.288 MHz exactly and
  PLL3_R = 32.768 MHz. DPR-01 §6 / INV-DPR-12 makes these values
  Board Runtime guarantees: any profile with `audio` or `mems_mic` in
  its `ServiceSet` MUST receive a clock tree where `BoardRuntime`
  publishes those exact values.
- `analyzer-cm7/src/bsp.rs:263-368` is the canonical
  `peripheral_safe_stop()` body: DMA1 stream 0/1 disable + busy-wait,
  SAI1 Block A/B disable + busy-wait, `DMA1_LIFCR` W1C, `NVIC_ICER0`/
  `NVIC_ICPR0` writes for IRQ 11/12, telemetry at
  `0x3800_0304`. DPR-02 §5.1 / §5.4 ratifies the equivalent
  SafeStopSequence under `SafeStop::run`, with the `audio` service
  expansion covering DMA1 streams 0/1 + SAI1 Block A/B (matching the
  analyzer's reference semantics exactly).
- `analyzer-cm7/src/bsp.rs:407-593` is the FMC SDRAM bring-up:
  GPIO D/E/F/G/H/I AF12 pinmux for 7+11+11+7+11+10 pins, AHB3
  + C1_AHB3 FMC clock enables, SDCR/SDTR programming for the
  IS42S32400F-6BL on Bank 2 at `0xD000_0000`. DPR-01 §5.3 routes this
  through `MemoryPeripheralSet { fmc, gpio_d..gpio_i }` consumed by
  `BoardRuntime::init`.
- `analyzer-cm7/src/main.rs:1431-1796` is the entry-point boot
  sequence. Annotated with the 13 stages and the DPR-03 disposition
  of each:

  | Stage | Lines (approx) | Action | DPR-03 fate |
  |---|---|---|---|
  | 1 | 1532-1534 | Heap init at D1 AXI `HEAP_BASE` | Subsumed by `BoardRuntime::init` per INV-DPR-14 |
  | 2 | 1540 | `ipc::init()` — zero IPC ring buffers in D2 SRAM3 | **Stays analyzer-side** (IPC is analyzer concern, not platform) |
  | 3 | 1548-1587 | `Peripherals::take()` + destructure | Replaced by three `*PeripheralSet` struct constructions |
  | 4 | 1591 | `bsp::init_clocks(PWR, RCC, SYSCFG)` | Subsumed by `BoardRuntime::init` via `ClockPeripheralSet` + INV-DPR-12 |
  | 5 | 1597-1616 | LTDC/DMA2D/DSI/FMC/SAI1/DMA1/SPI2/DMA2 clock enables | Display/memory subset subsumed; analyzer-specific SAI1/DMA1/SPI2/DMA2 enables stay (service activation per DPR-01 §5.1 `ServiceSet`) |
  | 6 | 1625-1641 | HSEM AHB4 clock + `hsem::init_lineiin_receive()` | Subsumed by `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4` expansion per INV-DPR-11 |
  | 7 | 1673 | `bsp::peripheral_safe_stop()` | Subsumed by `SafeStop::run(services, telemetry)` per DPR-02 §5.1 / §5.4 |
  | 8 | 1690-1695 | `RCC_D2CCIP1R` SAI1SEL = PLL3_P (raw MMIO) | Subsumed by `BoardRuntime::init` clock-tree contract (PLL3 selection is part of INV-DPR-12) |
  | 9 | 1700 | `boot_sentinel::write(PRE_GPIO_SPLIT)` | Migrates to `boot_sentinel::write(PRE_CLOCK_INIT)` per DPR-02 §5.2 / §5.3 |
  | 10 | 1714-1744 | `GPIOx.split(ccdr.peripheral.GPIOx)` for A/B/C/D/E/F/G/H/I/J/K | Analyzer-owned GPIO splits stay; SDRAM-pin AF12 pinmux moves to `MemoryPeripheralSet` consumer |
  | 11 | 1746-1814 | SAI4 / SAI1 / I2C4 pin AF setup | **Stays analyzer-side** (codec routing + PDM pins are app-level) |
  | 12 | 1828-1990 | I2C4 init, codec reset, FMC SDRAM, `Stm32h747iDiscoDisplay::new`, sentinel writes | Display constructor call stays (unchanged signature); BSP scaffolding around it disappears |
  | 13 | 2004- | `DiscoController::new`, render loop, active-effect, joystick state | **Stays analyzer-side** (application layer; out of DPR scope per §9) |

  Stages 1, 3, 4, 6 (HSEM enable), 7, 8, and 9 retire under DPR-03.
  Stages 2, 5 (analyzer-specific subset), 10, 11, 12 (the constructor
  call itself), and 13 stay analyzer-side.

The failure mode is not that any one mirrored block is wrong; the
failure mode is that DAA-01-B-RLVGL-INTEGRATION.md §6 explicitly
deferred this question until rlvgl could ship a clean platform surface,
and that surface (DPR-01 + DPR-02) is now ready. Keeping the mirror
beyond DPR-02b causes the analyzer to silently fork from any DPR-01/02
amendment — for example, an INV-DPR-12 change to PLL3 dividers would
propagate to the demo but not to the analyzer's `bsp.rs:136-198` raw
PLL3 reconfiguration.

## 3. Glossary (Additions to DPR-00 §3, DPR-01 §3, DPR-02 §3)

Capitalized use of these terms in DPR docs MUST refer to the
definitions below. DPR-00 / DPR-01 / DPR-02 §3 entries remain
authoritative and are not restated here.

| Term | Meaning | Owner |
|---|---|---|
| **AnalyzerKernel** | The analyzer's application-layer composition: audio DSP graph (line-in capture + line-out playback), spectrum FFT, meter rendering, scope-view widget composition, and the render-dirty gating loop. Lives in `analyzer-cm7/src/main.rs` post-line-1990 and the analyzer-graph / analyzer-audio crates. **DPR-03 does NOT modify AnalyzerKernel.** | DPR-03. |
| **AdaptedCodeOrigin** | The `// Adapted from rlvgl@<commit-hash>:<file-path>` header convention introduced by `DAA-01-B-RLVGL-INTEGRATION.md` §6 (2026-04-27). One header per file or function block declaring the rlvgl source provenance. DPR-03 retires every such header in the analyzer subrepo except those covering §5.2 ShimAllowlist entries. | DPR-03. |
| **AnalyzerProfile** | The concrete expansion of `RuntimeProfile::Analyzer` registered in DPR-01 §5.2: `(scan_mode = VideoMode, services = { audio, codec_reset, mems_mic, scope_probes }, pacing = BareMetalLoop, cores = Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX }, holdoff = None, telemetry = TelemetryProfile::analyzer())`. The analyzer subrepo consumes this preset by literal name; `RuntimeProfile::Custom { ... }` adoption is forbidden per INV-DPR-3-2. | DPR-03. |
| **ShimAllowlist** | The §5.2 frozen set of analyzer-side adapter types and modules that may persist with `AdaptedCodeOrigin` headers after DPR-03c closes. Currently `{ bsp::display_pins::HalGpioBacklight, bsp::display_pins::HalResetPin, bsp::audio_i2c::I2c4Adapter }`. Adding entries is Specification Required and MUST cite the embedded-hal version constraint that justifies the shim. | DPR-03. |
| **CrossRepoGate** | The acceptance-gate pattern named in INV-DPR-15: a DPR-03 sub-phase closes only when its corresponding DAA-01-B-2 §15 amendment lands. RLVGL-side PRs MAY merge in advance, but the §15 entry of the DPR-03 sub-phase MUST cite the DAA-01-B-2 §15 commit hash to claim the gate is closed. | DPR-03. |

## 4. Source-of-Truth Map

Two columns: every analyzer-side surface DPR-03 retires (left) maps to
its rlvgl-platform replacement (right). The pattern follows DPR-02 §4
(which mapped analyzer evidence onto platform surfaces); DPR-03 is the
adoption side of the same boundary.

### 4.1 `analyzer-cm7/src/bsp.rs` functions

| Analyzer surface | rlvgl-platform replacement | DPR-03 sub-phase |
|---|---|---|
| `init_clocks(pwr, rcc, syscfg) -> Ccdr` (`bsp.rs:43-201`) | `BoardRuntime::init(display, memory, clock, profile)` via `ClockPeripheralSet { pwr, rcc, syscfg }` per DPR-01 §5.3. INV-DPR-12 guarantees PLL3_P = 12.288 MHz / PLL3_R = 32.768 MHz. | DPR-03a |
| `peripheral_safe_stop()` (`bsp.rs:263-368`) | `SafeStop::run(services, telemetry)` per DPR-02 §5.5 / §7. The `audio` service in `RuntimeProfile::Analyzer.services` expands to the §5.4 mapping (DMA1 streams 0/1, SAI1 Block A/B, IRQ 11/12/87). | DPR-03a |
| `init_fmc_sdram()` (`bsp.rs:407-473`) | `BoardRuntime::init` via `MemoryPeripheralSet { fmc, gpio_d, gpio_e, gpio_f, gpio_g, gpio_h, gpio_i }` per DPR-01 §5.3. SDRAM geometry is a board fact per INV-DPR-9. | DPR-03a |
| `SAFE_STOP_TELEMETRY_ADDR = 0x3800_0304` (`bsp.rs:230`) | DPR-02 §5.3 `safe_stop_report` slot at `0x3800_0510..0x3800_0520`. SafeStopReport entry/exit sentinels (`0xB007_5A5E` / `0xB007_D000 \| timeout_mask`) move with the address. | DPR-03a |
| `pub mod boot_sentinel { ADDR=0x3800_0300; PRE_GPIO_SPLIT=0xA11C_0005; POST_GPIO_SPLIT=0xA11C_0006; POST_SDRAM_INIT=0xA11C_0007; POST_DISPLAY_INIT=0xA11C_0008; POST_FIRST_RENDER=0xA11C_0009; write(stage) }` (`bsp.rs:826-871`) | `rlvgl_platform::board_runtime::boot_sentinel::*` per DPR-02 §5.2. See §5.3 below for the value-mapping table. | DPR-03a |
| `mod display_pins::{HalGpioBacklight, HalResetPin}` (`bsp.rs:651-706`) | **Retained** under §5.2 ShimAllowlist. embedded-hal 0.2 ↔ 1.0 adapter; not bring-up code. | (no migration) |
| `mod audio_i2c::I2c4Adapter` (`bsp.rs:722-810`) | **Retained** under §5.2 ShimAllowlist. embedded-hal 0.2 ↔ 1.0 I²C adapter wrapping `stm32h7xx-hal 0.16`'s `I2c<I2C4>`; not bring-up code. | (no migration) |

### 4.2 `analyzer-cm7/src/main.rs` boot stages

| Analyzer line range | Action | rlvgl-platform replacement | DPR-03 sub-phase |
|---|---|---|---|
| 1532-1534 | Heap init at D1 AXI `HEAP_BASE = 0x2402_0000` | `BoardRuntime::init` returns published `(heap_base, heap_size)` per INV-DPR-14; the analyzer's `ALLOC.init` call reads from there. | DPR-03a |
| 1540 | `ipc::init()` (IPC ring zero) | **Stays analyzer-side** — IPC is an analyzer concern, not Board Runtime. | (no migration) |
| 1548-1587 | `Peripherals::take()` + destructure | Replaced by three `*PeripheralSet` struct constructions, then `BoardRuntime::init`. App still destructures app-owned GPIOs (`GPIOC`, `GPIOJ`, `GPIOK`) for codec / scope-probes / joystick. | DPR-03a |
| 1591 | `bsp::init_clocks(...)` | Subsumed by `BoardRuntime::init`. | DPR-03a |
| 1597-1600 | `ccdr.peripheral.{LTDC,DMA2D,DSI,FMC}.enable()` | Subsumed by `BoardRuntime::init` (these are the display/memory subset). | DPR-03a |
| 1606-1607 | `ccdr.peripheral.{SAI1,DMA1}.enable()` | **Stays analyzer-side**, but activated through `RuntimeProfile::Analyzer.services.contains(audio)` per DPR-01 §5.1. The analyzer still owns the SAI1/DMA1 peripheral instances; the platform tracks activation in `0x3800_0520` `service_set_active` slot per DPR-02 §5.3. |  DPR-03a |
| 1615-1616 | `ccdr.peripheral.{SPI2,DMA2}.enable()` (CM4-side audio bank probe) | **Stays analyzer-side**. Not in any registered service set; analyzer-specific. | (no migration) |
| 1625-1630 | Raw HSEM AHB4 clock enable (`RCC_AHB4ENR \|= 1<<25`) | Subsumed by `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4` expansion — `BoardRuntime::init` enables HSEM AHB4EN before HSEM register access per INV-DPR-11. | DPR-03a |
| 1640 | `hsem::init_lineiin_receive()` (KEYR, ICR clear, IER1, NVIC unmask) | Subsumed by `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX }` expansion. The **enable** sequence (KEYR cookie, ICR clear, IER1, NVIC unmask for IRQ 125) moves into `BoardRuntime::init`; the **ISR body** (MISR read, ICR clear, counter increment, `HSEM6_FRESH` set) stays in `analyzer-cm7/src/hsem.rs`. | DPR-03a |
| 1673 | `bsp::peripheral_safe_stop()` | `SafeStop::run(services, telemetry)` called inside `BoardRuntime::init` before clock-tree reprogramming per INV-DPR-2-1. | DPR-03a |
| 1690-1695 | Raw `RCC_D2CCIP1R` SAI1SEL = PLL3_P | Subsumed by `BoardRuntime::init`. INV-DPR-12 guarantees SAI1 kernel = PLL3_P; the SAI1SEL mux write is part of the clock-tree contract. | DPR-03a |
| 1700 | `bsp::boot_sentinel::write(PRE_GPIO_SPLIT)` (`0xA11C_0005` @ `0x3800_0300`) | `boot_sentinel::write(PRE_CLOCK_INIT)` (`0xA11C_0010` @ `0x3800_0500`) per DPR-02 §5.2 / §5.3. **Note**: the canonical placement of this sentinel changes — `BoardRuntime::init` writes it internally as its first action; the analyzer-side call site disappears. | DPR-03a |
| 1714-1744 | GPIO splits for A/B/C/D/E/F/G/H/I/J/K | App keeps splits for GPIOs the platform doesn't consume (A/B/C/J/K). MemoryPeripheralSet consumes D/E/F/G/H/I (those splits move inside `BoardRuntime::init`). | DPR-03a |
| 1746-1814 | SAI4 PDM pins (PE2/PC1 AF10), SAI1 line-in pins (PE3-PE6 AF6), I2C4 pins (PD12/PD13 AF4) | **Stays analyzer-side**. These are app-level pin AF moves on GPIOs the platform does not own. | (no migration) |
| 1828-1829 | `gpiog.pg3.into_push_pull_output()` (panel reset), `gpioj.pj6.into_push_pull_output()` (backlight) | **Stays analyzer-side as call sites**, but the resulting `PG3 / PJ6` typed handles flow into `DisplayPeripheralSet { panel_reset, backlight, ... }` per DPR-01 §5.3 rather than directly into `Stm32h747iDiscoDisplay::new`. The app passes pins to the platform *after* the split. | DPR-03a |
| 1887-1937 | I2C4 init, codec reset (`Wm8994::reset`), 50 ms settle, `verify_id` | **Stays analyzer-side**. I2C4 instance handed to the WM8994 driver via the `audio_i2c::I2c4Adapter` shim (ShimAllowlist). Codec reset is app behavior; the `codec_reset` service in `RuntimeProfile::Analyzer.services` tracks that the codec was reset for SafeStop telemetry purposes only. | (no migration) |
| 1956 | `bsp::init_fmc_sdram()` | Subsumed by `BoardRuntime::init` via `MemoryPeripheralSet`. | DPR-03a |
| 1959 | `boot_sentinel::write(POST_SDRAM_INIT)` | Subsumed — `BoardRuntime::init` writes `POST_SDRAM_INIT` (`0xA11C_0040`) internally after FMC init. | DPR-03a |
| 1980-1987 | `rlvgl_platform::Stm32h747iDiscoDisplay::new(CpuBlitter, HalGpioBacklight(...), HalResetPin(...), LTDC, DSIHOST, DMA2D)` | **Constructor call shape stays unchanged**, but the call site moves into `BoardRuntime::init`'s display assembly. The analyzer accesses the constructed display via `runtime.display()` per DPR-01 §7. The eh-1.0 adapter shims (`HalGpioBacklight` / `HalResetPin`) flow through `DisplayPeripheralSet` to the constructor unchanged. | DPR-03a |
| 1990 | `boot_sentinel::write(POST_DISPLAY_INIT)` | Subsumed — `BoardRuntime::init` writes `POST_DISPLAY_INIT` (`0xA11C_0050`) internally after the display constructor returns. | DPR-03a |
| 2004+ | `DiscoController::new`, `active_effect`, `joystick_state`, render loop, present path | **Stays analyzer-side**. AnalyzerKernel territory; out of DPR scope per §9. The render loop's `display.swap()` call routes through `FrameScheduler<VideoMode, BareMetalLoopPacing>` per INV-DPR-3 (the writer consolidation already happened on the platform side under DPR-01a; the analyzer's `display.swap()` call site doesn't change). | (no migration) |

### 4.3 `analyzer-cm7/src/hsem.rs`

| Analyzer surface | rlvgl-platform replacement | DPR-03 sub-phase |
|---|---|---|
| `init_lineiin_receive()` (enable sequence — KEYR, ICR clear, IER1, NVIC IRQ 125 unmask) | Subsumed by `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX }` expansion in `BoardRuntime::init`. | DPR-03a |
| `KEY` constant, `HSEM6_ISR_COUNT_ADDR`, `HSEM6_PASSTHROUGH_COUNT_ADDR`, `HSEM6_FRESH`, `HSEM_LINEIN_READY` / `HSEM_LINEIN_RETURNED` bit positions | **Stays analyzer-side**. These name the *content* of the HSEM[6] channel (audio mailbox semantics) which is an app concern, not the *enable mechanism* (which is platform). | (no migration) |
| `HSEM0()` ISR body (MISR1 read, ICR1 clear, counter increment, `HSEM6_FRESH` set) | **Stays analyzer-side**. DPR-01 §5.5 reserves `LineInRx` as a platform-supported HSEM line, but the ISR body's audio-wake semantics remain analyzer-specific. The platform MAY publish a generic `dispatch_to(handler)` helper in a future phase; for DPR-03 the ISR body keeps its current shape. | (no migration) |

### 4.4 `AdaptedCodeOrigin` headers

| Header location | Source citation | DPR-03 fate |
|---|---|---|
| `analyzer-cm7/src/bsp.rs:3-9` (module-level) | `rlvgl@d99f793:examples/stm32h747i-disco/src/main.rs` (lines ~1640-1735 — fn main clock-tree block) | Removed under DPR-03c. The covered function (`init_clocks`) is deleted under DPR-03a. |
| `analyzer-cm7/src/bsp.rs:210` (inline) | `rlvgl@d99f793:src/main.rs:1724-1727` (LTDC/DMA2D/DSI/FMC enables) | Removed under DPR-03c. The covered code (`ccdr.peripheral.*.enable()` block) is deleted under DPR-03a. |
| `analyzer-cm7/src/bsp.rs:373-378` (`init_fmc_sdram` doc comment) | `rlvgl@d99f793:examples/stm32h747i-disco/src/main.rs:1297-1360` (`early_fmc_setup` + supporting helpers) | Removed under DPR-03c. The covered function (`init_fmc_sdram`) is deleted under DPR-03a. |
| `analyzer-cm7/src/bsp.rs:641-650` (`display_pins` doc comment) | `rlvgl@d99f793:src/main.rs:1517-1538` / `:1766-1781` | **Retained** under §5.2 ShimAllowlist. The header stays but its narrative MUST be updated by DAA-01-B-2 to cite the eh-0.2 ↔ 1.0 adapter purpose rather than "v1 mirror." |
| `analyzer-cm7/src/bsp.rs:812-823` (`boot_sentinel` doc comment) | `rlvgl@d99f793:src/main.rs` boot sequence | Removed under DPR-03c. The covered module is deleted under DPR-03a. |
| `analyzer-cm7/src/main.rs:1590` (inline citation for `bsp::init_clocks`) | `rlvgl@d99f793 — see bsp.rs` | Removed under DPR-03c. The covered call site is replaced under DPR-03a. |

After DPR-03c closes, the only `// Adapted from rlvgl@...` headers in
the analyzer subrepo MUST be the ShimAllowlist entries at
`bsp.rs:641-650` (`display_pins`) and a parallel header on
`bsp.rs:707-721` (`audio_i2c` — currently not headered but
ShimAllowlist membership MAY add a header for consistency under
DAA-01-B-2).

## 5. Frozen Decisions

### 5.1 Adoption Sequence

Registration policy: **Standards Action**.

The analyzer subrepo migrates onto the platform surface in a fixed
order. The order matters because compile-time dependencies between the
migration steps mean an out-of-order PR can fail to type-check even
when each step is individually correct.

1. **`analyzer-cm7/Cargo.toml`** — bump the `rlvgl-platform` dependency
   to a version that re-exports `BoardRuntime`, `RuntimeProfile`,
   `FrameScheduler`, `Pacing`, `SafeStop`, and `boot_sentinel`. Required
   features: `["stm32h747i_disco", "dma2d", "audio"]`. The `freertos`
   feature is **not** required — `RuntimeProfile::Analyzer.pacing =
   BareMetalLoop` per DPR-01 §5.2. The `compile-verify` and `regression`
   features are tests-only and stay out of analyzer's runtime
   dependency set.

2. **`analyzer-cm7/src/main.rs`** — replace boot stages 1-9 (per §4.2)
   with a single `let runtime = BoardRuntime::init(display_pset,
   memory_pset, clock_pset, RuntimeProfile::Analyzer)?;` call. The
   three `*PeripheralSet` constructions sit between
   `Peripherals::take()` and the `BoardRuntime::init` call; the app
   keeps the GPIO destructure for `GPIOA/B/C/J/K` (codec / scope-probes
   / joystick). The widget tree, render loop, and audio path
   (stages 11-13) remain unchanged.

3. **`analyzer-cm7/src/bsp.rs`** — delete `init_clocks`,
   `peripheral_safe_stop`, `init_fmc_sdram`, the
   `SAFE_STOP_TELEMETRY_ADDR` / `SAFE_STOP_ENTRY_MAGIC` /
   `SAFE_STOP_EXIT_BASE` constants, the FMC SDRAM helper functions
   (`wait_for_sdram_ready`, `issue_sdram_command`, `configure_fmc_sdram`,
   `configure_pin_alt12`), the `SDRAM_REFRESH_COUNT` / `SDRAM_MODE_REGISTER`
   constants, and the `boot_sentinel` module. The file shrinks from ~871
   lines to ~170 lines (`display_pins` + `audio_i2c` shim modules only).

4. **`analyzer-cm7/src/hsem.rs`** — delete `init_lineiin_receive`
   (the enable function). Retain `KEY`, `HSEM6_ISR_COUNT_ADDR`,
   `HSEM6_PASSTHROUGH_COUNT_ADDR`, `HSEM6_FRESH`, the bit-position
   constants (`HSEM_LINEIN_READY`, `HSEM_LINEIN_RETURNED`), and the
   `HSEM0()` ISR body. The file shrinks by ~50 lines.

The order is enforced by build constraints: step 2 cannot compile
until step 1 is committed; step 3 deletes code that step 2's
replacement makes unreachable; step 4 deletes code subsumed by step 2's
`RuntimeProfile::Analyzer` `Cores::Cm7Cm4` expansion. Reordering
(e.g. deleting `init_clocks` before adding the `BoardRuntime::init`
call) leaves the analyzer in an uncompilable state.

The four steps land in a single DPR-03a PR — the migration is one
atomic logical change, even if review proceeds step-by-step.

### 5.2 ShimAllowlist

Registration policy: **Specification Required**.

The following analyzer-side adapter types and modules MAY persist with
`AdaptedCodeOrigin` headers after DPR-03c closes:

| Shim | File:lines | Purpose | Removal trigger |
|---|---|---|---|
| `bsp::display_pins::HalGpioBacklight` | `bsp.rs:662-681` | Wraps `stm32h7xx-hal 0.16` GPIO output pin (eh-0.2) as an eh-1.0 `SetDutyCycle` impl for `Stm32h747iDiscoDisplay::new`'s `BL` type parameter. | When `stm32h7xx-hal` ships an eh-1.0 `embedded-hal::pwm::SetDutyCycle` impl on its `Pin<...Output<PushPull>>` type, or when the analyzer migrates the backlight to TIM-driven PWM. |
| `bsp::display_pins::HalResetPin<P>` | `bsp.rs:686-706` | Wraps `stm32h7xx-hal 0.16` GPIO output pin (eh-0.2) as an eh-1.0 `OutputPin` impl for `Stm32h747iDiscoDisplay::new`'s `RST` type parameter. | When `stm32h7xx-hal` ships eh-1.0 `embedded_hal::digital::OutputPin` impls on its `Pin` types. |
| `bsp::audio_i2c::I2c4Adapter` | `bsp.rs:769-809` | Wraps `stm32h7xx-hal 0.16` `I2c<I2C4>` (eh-0.2 traits) as an eh-1.0 `embedded_hal::i2c::I2c` impl for `rlvgl_platform::wm8994::Wm8994`'s generic `I2C` parameter. | When `stm32h7xx-hal` ships eh-1.0 `embedded_hal::i2c::I2c` impls on its `I2c<...>` types. |

Adding a new ShimAllowlist entry requires a §15 amendment with the
embedded-hal version constraint that justifies the shim, the file
`:line` range, the wrapped HAL type, and the removal trigger. Adding
shims that wrap *rlvgl-platform* types (rather than third-party HAL
types) is forbidden — that would be a different INV-DPR-3-1 violation
(see §6 below).

### 5.3 BootSentinel Migration

Registration policy: **Specification Required**.

The analyzer's existing five `0xA11C_xxxx` sentinels at
`bsp.rs:826-871` map onto the DPR-02 §5.2 named set. The value mapping
is deliberately non-1-to-1: DPR-02 §5.2 chose `0xA11C_00x0` spacing
(0x10 increments) over the analyzer's 0x01 spacing to leave room for
sub-stages, so several analyzer sentinels collapse to the same
canonical DPR-02 milestone.

| Analyzer sentinel (`0x3800_0300`) | DPR-02 sentinel (`0x3800_0500`) | Rationale |
|---|---|---|
| `PRE_GPIO_SPLIT` (`0xA11C_0005`) | `PRE_CLOCK_INIT` (`0xA11C_0010`) | The analyzer wrote this *after* clock tree but *before* GPIO splits; DPR-02's `PRE_CLOCK_INIT` is `BoardRuntime::init`'s first action. The semantic shift (pre-clock vs. pre-GPIO) is intentional: DPR-02 puts the boundary at the platform entry, which is earlier in execution and corresponds to the first sentinel the platform itself can write. The analyzer's `PRE_GPIO_SPLIT` semantics are preserved as a DPR-02b sub-stage (`0xA11C_0011 POST_CLOCK_INIT_PRE_GPIO`) if the analyzer needs them re-exposed; for DPR-03 the canonical map is `PRE_GPIO_SPLIT → PRE_CLOCK_INIT`. |
| `POST_GPIO_SPLIT` (`0xA11C_0006`) | `POST_CLOCK_INIT` (`0xA11C_0020`) | After GPIO splits and clock-tree completion are observationally equivalent for the analyzer's CM4 poll — both mark "CM7 ready for application bring-up." The DPR-02 placement is slightly earlier but the cross-core polling tolerates it. |
| (no analyzer sentinel) | `POST_SAFE_STOP` (`0xA11C_0030`) | New under DPR-02 §5.2. Not previously emitted by the analyzer; `BoardRuntime::init` writes it after `SafeStop::run` returns. |
| `POST_SDRAM_INIT` (`0xA11C_0007`) | `POST_SDRAM_INIT` (`0xA11C_0040`) | Direct semantic match. Both mark FMC SDRAM Bank 2 ready. Value differs (DPR-02's 0x40 vs. analyzer's 0x07) but the named milestone is identical. |
| `POST_DISPLAY_INIT` (`0xA11C_0008`) | `POST_DISPLAY_INIT` (`0xA11C_0050`) | Direct semantic match. Both mark `Stm32h747iDiscoDisplay::new` returned. |
| `POST_FIRST_RENDER` (`0xA11C_0009`) | (no DPR-02 sentinel) | **Stays analyzer-side** as an app-level sentinel. Written by AnalyzerKernel after `DiscoController::new` + first `render_frame` + `display.swap()`; not part of `BoardRuntime::init`. The analyzer keeps writing this at its existing address (`0x3800_0300`) or migrates it to an app-owned slot — DAA-01-B-2 decides. |

The analyzer-side write to address `0x3800_0300` (one word, the
most-recent sentinel value) is **superseded** by `BoardRuntime::init`
writing the canonical sentinels to `0x3800_0500..0x3800_0510` per DPR-02
§5.3. The `0x3800_0300` slot reverts to "unallocated" in DPR-00 §5.3
per the DPR-02 §10 reconciliation row; if the analyzer needs to keep
emitting `POST_FIRST_RENDER` somewhere, DAA-01-B-2 §15 MUST claim an
app-owned slot under DPR-00 §5.3 Expert Review.

## 6. Runtime Invariants Specific to DPR-03

DPR-00 §6 INV-DPR-1..15, DPR-01 §6, and DPR-02 §6 remain binding.
DPR-03 adds two invariants specific to the cross-repo adoption surface:

- **INV-DPR-3-1: No mirrored bring-up code in the analyzer.** After
  DPR-03c closes, the analyzer subrepo (`streamz/submodules/disco-
  analyzer/`) MUST NOT contain `// Adapted from rlvgl@<commit-hash>:...`
  headers except for entries in the §5.2 ShimAllowlist. A `git grep
  "Adapted from rlvgl@"` over the analyzer subrepo MUST return only
  ShimAllowlist matches. Adding a new `AdaptedCodeOrigin` header
  outside the ShimAllowlist is an INV-DPR-3-1 violation; the
  appropriate response is to either land the migration into
  rlvgl-platform via a DPR-NN amendment or to extend the ShimAllowlist
  via §5.2 Specification Required.

- **INV-DPR-3-2: Canonical Analyzer profile uses the registered preset.**
  The canonical analyzer (the binary at `analyzer-cm7/`) MUST consume
  `RuntimeProfile::Analyzer` by literal name. `RuntimeProfile::Custom { ... }`
  adoption is forbidden for the canonical analyzer. Rationale: DPR-01
  §5.2 made `Analyzer` a registered preset specifically so that
  changes to its expansion (services, cores, scan_mode) propagate
  automatically across the rlvgl ↔ analyzer boundary. A `Custom` literal
  in the analyzer would silently fork from the platform's intent the
  next time the preset's expansion changes (e.g. a future amendment
  adding `scope_probes_v2` to the analyzer's service set). Future
  second-app variants that need analyzer-like shape without being the
  canonical analyzer MAY use `Custom`; adding a third registered preset
  is Standards Action per DPR-01 §5.2.

These two invariants are the load-bearing checks that prevent the
analyzer from silently re-forking the bring-up surface after DPR-03c
closes.

## 7. API Surface the Analyzer Consumes

This section catalogs (informatively) the rlvgl-platform entry points
the analyzer adopts under DPR-03. None of these are new APIs — they
are already ratified by DPR-01 and DPR-02. The list exists so DAA-01-B-2
§15 can cite the specific symbols the analyzer takes a runtime
dependency on.

```text
// Re-exported from rlvgl-platform under the stm32h747i_disco feature gate.

rlvgl_platform::BoardRuntime
rlvgl_platform::BoardRuntime::init(
    display: DisplayPeripheralSet<RST, BL>,
    memory:  MemoryPeripheralSet,
    clock:   ClockPeripheralSet,
    profile: RuntimeProfile,
) -> Result<BoardRuntime, InitError>

rlvgl_platform::RuntimeProfile::Analyzer        // DPR-01 §5.2 preset
rlvgl_platform::DisplayPeripheralSet<RST, BL>   // DPR-01 §5.3
rlvgl_platform::MemoryPeripheralSet             // DPR-01 §5.3
rlvgl_platform::ClockPeripheralSet              // DPR-01 §5.3

rlvgl_platform::FrameScheduler<VideoMode, BareMetalLoopPacing>
                                                 // DPR-01 §5.4 — accessed
                                                 // via runtime.frame_scheduler()
rlvgl_platform::VideoMode                       // DPR-01 §5.4 ScanMode marker
rlvgl_platform::BareMetalLoopPacing             // DPR-01 §5.6 Pacing impl

rlvgl_platform::board_runtime::SafeStop         // DPR-02 §5.5
rlvgl_platform::board_runtime::SafeStop::run    // Called inside BoardRuntime::init;
                                                 // the analyzer does NOT call directly
rlvgl_platform::board_runtime::boot_sentinel::* // DPR-02 §5.2 constants

rlvgl_platform::Stm32h747iDiscoDisplay          // Already publicly re-exported
                                                 // pre-DPR (platform/src/lib.rs:256)
rlvgl_platform::CpuBlitter                      // Already public
rlvgl_platform::Screen                          // Already public
rlvgl_platform::wm8994::Wm8994                  // Already public

rlvgl_platform::hwcore::regs::*                 // Already public (typed regs).
                                                 // Analyzer's HSEM[6] ISR body may
                                                 // continue to consume these directly
                                                 // for MISR1 / ICR1 access.

// The Cores::Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX } expansion is consumed
// transitively via RuntimeProfile::Analyzer — the analyzer does not write
// HsemSet::LINE_IN_RX literally.
```

The analyzer's existing direct consumers (`CpuBlitter`, `Screen`,
`Stm32h747iDiscoDisplay::new`, `wm8994::Wm8994`) keep their existing
call shapes. The new consumers are `BoardRuntime`, `RuntimeProfile`,
and the three `*PeripheralSet` types — these replace the boot-stage
scaffolding only.

## 8. Phase Plan

DPR-03 splits into three sub-phases with the §0 CrossRepoGate applying
to each.

### DPR-03 (this doc) — Concept doc

Acceptance:

- [ ] §3 vocabulary additions (`AnalyzerKernel`, `AdaptedCodeOrigin`,
      `AnalyzerProfile`, `ShimAllowlist`, `CrossRepoGate`) accepted.
- [ ] §4 source-of-truth map (per-file / per-stage migration) accepted.
- [ ] §5 frozen decisions (§5.1 adoption sequence, §5.2 ShimAllowlist,
      §5.3 BootSentinel migration) accepted.
- [ ] §6 invariants INV-DPR-3-1 and INV-DPR-3-2 accepted.
- [ ] §8 phase plan (DPR-03a / b / c) accepted.
- [ ] §11 open questions (PCDN-DPR-3-001..003) explicitly deferred.

### DPR-03a — Compile-only adoption

The analyzer compiles against the rlvgl-platform surface without any
behavior change in its outward shape (no audio-path regression, no
display-pipeline regression, no IPC contract change). The §5.1 four-step
sequence lands as a single analyzer-side PR.

Validation:

- `analyzer-cm7/Cargo.toml` depends on the published rlvgl-platform
  version that re-exports `BoardRuntime` + `RuntimeProfile::Analyzer`
  + `SafeStop` + `boot_sentinel`.
- `analyzer-cm7/src/main.rs` boot stages 1-9 replaced with
  `BoardRuntime::init(display_pset, memory_pset, clock_pset,
  RuntimeProfile::Analyzer)?`.
- `analyzer-cm7/src/bsp.rs` reduced to §5.2 ShimAllowlist content only.
- `analyzer-cm7/src/hsem.rs` retains ISR body but deletes
  `init_lineiin_receive`.
- `cargo check --target thumbv7em-none-eabihf -p analyzer-cm7`
  succeeds.
- `cargo clippy --target thumbv7em-none-eabihf -p analyzer-cm7 --
  -D warnings` succeeds.
- The discipline scanner over the analyzer subrepo (analyzer's own
  scanner, not rlvgl-platform's) shows the analyzer-side
  `// rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)`
  count drops from its DPR-03-pre baseline (≥18 markers across
  `init_clocks`, `peripheral_safe_stop`, `init_fmc_sdram`,
  `SAI1SEL` mux, `RCC_AHB4ENR HSEMEN`, `boot_sentinel::write`) to
  the ShimAllowlist-only baseline (zero markers; ShimAllowlist
  shims are eh-trait adapters, not MMIO).
- `git grep "Adapted from rlvgl@" -- :^/* :!**/Cargo.lock` over the
  analyzer subrepo returns at most the §5.2 ShimAllowlist matches.

Cross-repo: DAA-01-B-2 §15 records the compile-only adoption with the
DPR-03a PR commit hash.

This phase is **compile-only** by design — hardware validation is
DPR-03b. A green `cargo check` does not prove `BoardRuntime::init`
runs correctly on hardware; it proves the boundary is correctly
shaped.

### DPR-03b — Hardware validation

The analyzer runs end-to-end on H747I-DISCO with no behavior regression
vs. the pre-DPR-03 baseline. This is the gate that proves the platform
surface actually serves the analyzer, not just that the analyzer
compiles against it.

Validation:

- Bench-flash + boot + 24-hour soak on H747I-DISCO.
- Audio path bench captures match pre-DPR baselines: line-in capture,
  line-out playback, PDM mic, codec MCLK on PG7, SAI1 Block A FS at
  48 kHz exact, FFT spectrum publish rate at expected Hz.
- Scope probe traces on PJ0 / PJ4 / PJ7 match pre-DPR shapes.
- HSEM[6] cross-core wake-up still fires CM7's `HSEM6_ISR_COUNT_ADDR`
  counter on the same cadence pre-DPR captures showed.
- Display pipeline: VideoMode scan + L1CFBAR shadow reload still
  produces 30 Hz frame rate; no visible regression in spectrum overlay
  or widget tree rendering.
- SafeStopReport at `0x3800_0510..0x3800_0520` shows
  `entry = 0xB007_5A5E`, `exit = 0xB007_D000` (zero timeouts) on a
  clean run; warm-reset wedge symptom from analyzer
  `docs/AUDIO-BEES-DEBUG-LOG.md` does not reappear.
- BootSentinel sequence at `0x3800_0500..0x3800_0510` advances
  through `PRE_CLOCK_INIT → POST_CLOCK_INIT → POST_SAFE_STOP →
  POST_SDRAM_INIT → POST_DISPLAY_INIT` on every boot.
- Legacy `0x3800_0300` analyzer-sentinel slot reads `POST_FIRST_RENDER`
  (`0xA11C_0009`) or whatever app-level sentinel DAA-01-B-2 §15
  ratifies for AnalyzerKernel's "first frame rendered" milestone.

Cross-repo: DAA-01-B-2 §15 records hardware-validation pass with the
DPR-03b bench capture references.

### DPR-03c — AdaptedCodeOrigin retirement + initiative close

The analyzer subrepo deletes every `// Adapted from rlvgl@<hash>:...`
header outside the §5.2 ShimAllowlist. DAA-01-B-RLVGL-INTEGRATION.md
§6 is amended to "Option C-Retired" status with a §15 entry pointing
at DAA-01-B-2 as the successor doc. The DPR initiative reaches its
acceptance gate for INV-DPR-2 (analyzer as second-app proof) and
INV-DPR-15 (cross-repo gating named).

Validation:

- `git grep "Adapted from rlvgl@" -- :^/* :!**/Cargo.lock` over the
  analyzer subrepo returns only `bsp::display_pins` and
  `bsp::audio_i2c` ShimAllowlist matches.
- DAA-01-B-RLVGL-INTEGRATION.md §15 amended to declare Option C
  retired; DAA-01-B-2 ratification logged in DPR-03 §15.
- DPR-00 §5.3 amended via a §15 entry: `0x3800_0300..0x3800_0310`
  reverts to "unallocated" (or "analyzer-owned `POST_FIRST_RENDER`
  app-level sentinel" if DAA-01-B-2 §15 claims it under Expert
  Review).

Cross-repo: DAA-01-B-2 §15 records the initiative close.

### DPR-04 (out of scope here)

The BSP-generator reopen gate per DPR-00 §8. DPR-03c closing supplies
the concrete evidence base (demo + analyzer both running on the same
platform surface) that DPR-04 uses to decide whether the runtime
surface should feed a generator.

## 9. Non-Goals

- **DPR-03 does not move analyzer's audio DSP, FFT, meter, or scope-
  view composition into `rlvgl-platform`.** AnalyzerKernel stays
  analyzer-side. The rationale is DPR-00 §10 reconciliation "Demo
  widget tree and capabilities" — application UI is app-owned, not
  platform-owned. The analyzer's spectrum FFT and scope rendering are
  the analyzer's `DiscoEffect::Spectrum` extension and are not part of
  any registered service.
- **DPR-03 does not make `RuntimeProfile::Analyzer` mutable in shape.**
  Changes to the preset's expansion (scan_mode, services, pacing,
  cores, holdoff, telemetry) require a DPR-00 §5.2 Standards-Action
  amendment first, per DPR-01 §5.2. The canonical analyzer adopts the
  preset by literal name per INV-DPR-3-2; changing the preset changes
  the analyzer's runtime behavior implicitly, which is the *intended*
  cross-repo coupling, not a defect.
- **DPR-03 does not block on rlvgl-platform shipping a 1.0 release.**
  The analyzer can adopt against v0.2.0; cross-repo Cargo
  compatibility is via the existing `[patch.crates-io]` setup in the
  analyzer workspace (per DAA-01-B §6 implementation split).
- **DPR-03 does not handle CM4-side adoption.** `analyzer-cm4/` mirrors
  rlvgl example pieces independently (CM4 linker, panic, FreeRTOS
  bindings); a DPR-NN follow-up may extend the platform surface to the
  CM4 side, but DPR-03 is CM7-only. The HSEM[6] cross-core handshake
  spans both cores, but DPR-03 only ratifies the CM7-side enable
  sequence; CM4-side adoption is deferred.
- **DPR-03 does not migrate the analyzer's IPC ring buffers
  (`0x3004_7000` D2 SRAM3 `CmdQueue`/`EvtQueue`).** IPC is an analyzer
  concern, not a Board Runtime concern. The DPR-04 BSP-generator
  decision MAY revisit this; DPR-03 keeps `ipc::init()` and the IPC
  module structurally analyzer-owned.

## 10. Reconciliation Decisions

| Existing concept | DPR-03 decision |
|---|---|
| `analyzer-cm7/src/bsp.rs::init_clocks` | Deleted under DPR-03a. The HAL `Ccdr` it returned was consumed by analyzer-side `ccdr.peripheral.SAI1.enable()` etc.; under DPR-03a the analyzer accesses the published clock-tree handle via `runtime.clock_handle()` (DPR-01 §7 — final getter name ratifies in code review) for SAI1/DMA1/SPI2/DMA2 service activation. Display peripheral enables (LTDC/DMA2D/DSI/FMC) happen inside `BoardRuntime::init` and are no longer analyzer-visible. |
| `analyzer-cm7/src/bsp.rs::peripheral_safe_stop` | Deleted under DPR-03a. `SafeStop::run` per DPR-02 §5.1 / §5.4 is called by `BoardRuntime::init` automatically for the `audio` service in `RuntimeProfile::Analyzer.services`; the analyzer's open-coded sequence is replaced verbatim. The telemetry slot moves from `0x3800_0304` to `0x3800_0510..0x3800_0520`. |
| `analyzer-cm7/src/bsp.rs::init_fmc_sdram` | Deleted under DPR-03a. `BoardRuntime::init` via `MemoryPeripheralSet` runs the equivalent. The PAC-bypass pattern (raw `&*GPIOX::ptr()` for AF12 pinmux) moves inside the platform — INV-DPR-9 (profiles choose policy, not silicon facts) means the SDRAM geometry stays platform-owned regardless of which app instantiates it. |
| `analyzer-cm7/src/bsp.rs::boot_sentinel` (module + `ADDR=0x3800_0300` + five constants + `write`) | Deleted under DPR-03a. `rlvgl_platform::board_runtime::boot_sentinel::*` per DPR-02 §5.2 / §5.3 is the source of truth. Value-mapping in §5.3 above. `POST_FIRST_RENDER` (analyzer-only) becomes an app-owned sentinel; DAA-01-B-2 §15 claims its slot. |
| `analyzer-cm7/src/bsp.rs::SAFE_STOP_TELEMETRY_ADDR` constants | Deleted under DPR-03a. `SafeStopReport` at `0x3800_0510..0x3800_0520` per DPR-02 §5.3. Entry/exit sentinel values (`0xB007_5A5E` / `0xB007_D000 \| timeout_mask`) preserved by DPR-02 §5.3 layout `[0]`/`[1]`. |
| `analyzer-cm7/src/bsp.rs::display_pins` | **Kept** under §5.2 ShimAllowlist. Removal trigger: `stm32h7xx-hal` ships eh-1.0 trait impls. |
| `analyzer-cm7/src/bsp.rs::audio_i2c::I2c4Adapter` | **Kept** under §5.2 ShimAllowlist. Removal trigger: same as above. |
| `analyzer-cm7/src/hsem.rs::init_lineiin_receive` | Deleted under DPR-03a. The KEYR cookie + ICR clear + IER1 enable + NVIC unmask sequence is part of `RuntimeProfile::Analyzer`'s `Cores::Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX }` expansion executed by `BoardRuntime::init` per INV-DPR-11. |
| `analyzer-cm7/src/hsem.rs::HSEM0()` ISR body, `HSEM6_FRESH`, `HSEM6_ISR_COUNT_ADDR`, `HSEM6_PASSTHROUGH_COUNT_ADDR`, `KEY`, bit-position constants | **Kept** analyzer-side. The platform sets up the channel; the ISR body's audio-mailbox semantics (counter increment, fresh-data flag, passthrough conditional) are an analyzer concern. The `KEY` constant is shared between CM7 (`KEYR`) and CM4 (`CR` release) — it stays in `analyzer-cm7/src/hsem.rs` because analyzer-cm4 also imports it; promoting it to rlvgl-platform would require an unrelated CM4-side adoption. |
| `analyzer-cm7/src/main.rs:1431-1796` boot sequence | Stages 1, 3-9 deleted; replaced by `BoardRuntime::init` + three `*PeripheralSet` constructions. Stages 2 (`ipc::init`), 5 (analyzer-specific clock enables), 10-13 (GPIO splits for app pins, AF moves, codec, display constructor call, AnalyzerKernel) unchanged. |
| `analyzer-cm7/src/main.rs` display construction at lines 1980-1987 | The `Stm32h747iDiscoDisplay::new(...)` call moves *inside* `BoardRuntime::init`'s display assembly. The analyzer obtains a `&mut Display` via `runtime.display()` rather than holding `Stm32h747iDiscoDisplay` directly. The `display.swap()` call in the render loop unchanged in shape; it now routes through `runtime.frame_scheduler::<VideoMode, BareMetalLoopPacing>().swap(fb)` per INV-DPR-3 (the writer consolidation is platform-side, transparent to the analyzer). |
| `DAA-01-B-RLVGL-INTEGRATION.md` §6 (2026-04-27 Option C ratification) | Amended under DPR-03c to "Option C-Retired" status. The §6 narrative gains a closing paragraph citing DPR-00..03 as the "later in that effort" the original §6 deferred to. DAA-01-B-2 §15 is the operational successor; DAA-01-B keeps its place in the historical record. |
| Cross-repo `[patch.crates-io]` setup (workspace patch block) | Unchanged by DPR-03. The patch points the analyzer workspace at the rlvgl submodule's `rlvgl-platform` crate; whether the patch tracks a v0.2.x tag or the v0.2.0 branch HEAD is DAA-01-B-2's call. DPR-03 only requires the patched version re-exports the §7 API surface. |

## 11. Open Questions Carried into DPR-03a/b/c

These do not block DPR-03 (this doc) ratification; they are deferred to
the named sub-phase PRs. Each MUST be resolved in the named sub-phase's
§15 entry (or re-deferred with a named later phase).

- **PCDN-DPR-3-001:** When DPR-01a (FrameScheduler scaffold +
  bare-metal demo migration), DPR-01b (FreeRtosPacing + FreeRTOS demo
  migration), and DPR-02a (SafeStop scaffold) land, should DPR-03a
  start as soon as DPR-01a + DPR-02a are green, or wait for DPR-01b
  too? The analyzer is `BareMetalLoop` only, so DPR-01b's FreeRTOS
  work is technically not on the analyzer's critical path. Argument
  for waiting: bench-flash time on H747I-DISCO is scarce, and
  validating DPR-01b's bare-metal regressions before DPR-03a starts
  avoids re-flashing twice. Argument for starting early: DPR-03a is
  compile-only, so flash-time isn't its bottleneck. Resolve under
  DPR-03a after the dependencies' acceptance dates are known.

- **PCDN-DPR-3-002:** Should DAA-01-B-2 be a fresh sub-letter doc or a
  §15 amendment to the existing DAA-01-B? The §15-amendment shape is
  lighter (no new file in the analyzer subrepo) but DAA-01-B's §6
  ratification of "Option C (mirror)" makes a closure-by-fresh-doc
  shape cleaner — DAA-01-B-2's job is to ratify "Option C-Retired" and
  the precedent across DAA / DPR is fresh-doc-per-decision. Argument
  for fresh doc: DAA-01-B's §6 narrative is two years of historical
  rationale that DAA-01-B-2 should not displace by editing; a closure
  doc gives reviewers a single artifact citing both states. Argument
  for §15 amendment: smaller scope, no risk of vocabulary drift between
  the two docs. Resolve under DPR-03a (the answer is needed before the
  first DPR-03 PR lands; both shapes satisfy INV-DPR-15).

- **PCDN-DPR-3-003:** Does the DPR-03b hardware-bench capture set live
  in `rlvgl` or in the `streamz/submodules/disco-analyzer` subrepo?
  The bench captures are the validation artifact for DPR-03b; storing
  them analyzer-side keeps the validation surface co-located with the
  analyzer-specific signal (audio captures only matter if you're
  validating the analyzer). Argument for rlvgl-side: DPR-03 is a DPR
  initiative, and the rlvgl initiative's validation evidence belongs
  with the initiative. Argument for analyzer-side: bench captures are
  large (multi-MB scope traces, FFT plots) and the analyzer subrepo
  already has a `docs/AUDIO-BEES-DEBUG-LOG.md` precedent for this
  shape. Resolve under DPR-03b before the first bench-flash session.

## 12. Acceptance Checklist

DPR-03 (this concept doc) acceptance gates:

- [x] §3 vocabulary additions (`AnalyzerKernel`, `AdaptedCodeOrigin`,
      `AnalyzerProfile`, `ShimAllowlist`, `CrossRepoGate`) accepted.
- [x] §4 source-of-truth map accepted: §4.1 `bsp.rs` function migrations,
      §4.2 `main.rs` boot-stage migrations, §4.3 `hsem.rs` split,
      §4.4 `AdaptedCodeOrigin` header retirement plan.
- [x] §5.1 (Adoption sequence: Cargo.toml → main.rs → bsp.rs →
      hsem.rs) accepted.
- [x] §5.2 (ShimAllowlist: `HalGpioBacklight`, `HalResetPin`,
      `I2c4Adapter`) accepted.
- [x] §5.3 (BootSentinel value-mapping table) accepted.
- [x] §6 invariants INV-DPR-3-1 (no mirrored bring-up) and
      INV-DPR-3-2 (canonical Analyzer profile uses registered preset)
      accepted.
- [x] §8 phase plan (DPR-03a compile-only, DPR-03b hardware validation,
      DPR-03c AdaptedCodeOrigin retirement) accepted.
- [x] §10 reconciliation rows accepted (additions allowed via §15
      amendment).
- [x] §11 PCDN-DPR-3-001..003 explicitly deferred to DPR-03a/b/c.

DPR-03 (this concept doc) is **ratified 2026-05-20**. DPR-03a/b/c
sub-phase PRs have their own acceptance gates per §8 and each rides
a CrossRepoGate (§0) tied to a DAA-01-B-2 §15 amendment.

## 13. Files Cited

### rlvgl-platform (this repo)

- `docs/concepts/README.md`
- `docs/concepts/DPR-00-CONCEPTS.md` (especially §0 cross-repo authority
  row, §5.2 RuntimeProfile registrations, §5.3 telemetry range table,
  §6 INV-DPR-2 / INV-DPR-3 / INV-DPR-11 / INV-DPR-12 / INV-DPR-13 /
  INV-DPR-14 / INV-DPR-15, §8 DPR-03 phase entry, §10 reconciliation
  rows for analyzer evidence)
- `docs/concepts/DPR-01-CONCEPTS.md` (especially §5.1 Demo presets,
  §5.2 Analyzer as registered preset, §5.3 three peripheral-sets,
  §5.4 FrameScheduler generic, §5.5 HsemSet open registration,
  §5.6 Pacing trait surface, §7 API sketch)
- `docs/concepts/DPR-02-CONCEPTS.md` (especially §5.1 SafeStopSequence
  ordering, §5.2 BootSentinelSet, §5.3 TelemetrySlot layout, §5.4
  PeripheralServiceSet mapping for `audio`/`mems_mic`/`codec_reset`,
  §5.5 SafeStop public surface, §6 INV-DPR-2-1..4, §10 analyzer
  reconciliation rows)
- `platform/src/lib.rs` (re-export site for `BoardRuntime`,
  `RuntimeProfile`, `FrameScheduler`, `Pacing`,
  `Stm32h747iDiscoDisplay`)

### Analyzer subrepo (absolute paths for cross-repo clarity)

- `/Users/iraabbott/softoboros/streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md`
  (especially §6 Option C ratification 2026-04-27 — the gate DPR-03
  retires)
- `/Users/iraabbott/softoboros/streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs`
  - lines 1-15 (module header, `AdaptedCodeOrigin`)
  - lines 43-201 (`init_clocks`, PLL3 reconfiguration; covers
    INV-DPR-12)
  - line 210 (inline `AdaptedCodeOrigin` for the LTDC/DMA2D/DSI/FMC
    enables block)
  - lines 230-232 (`SAFE_STOP_TELEMETRY_ADDR`, `SAFE_STOP_ENTRY_MAGIC`,
    `SAFE_STOP_EXIT_BASE`)
  - lines 263-368 (`peripheral_safe_stop` body — DPR-02 §5.1 reference)
  - lines 407-473 (`init_fmc_sdram`)
  - lines 475-638 (FMC SDRAM helpers: `SDRAM_REFRESH_COUNT`,
    `SDRAM_MODE_REGISTER`, `wait_for_sdram_ready`, `issue_sdram_command`,
    `configure_fmc_sdram`, `configure_pin_alt12`)
  - lines 641-706 (`display_pins` module — §5.2 ShimAllowlist)
  - lines 707-810 (`audio_i2c` module — §5.2 ShimAllowlist)
  - lines 812-871 (`boot_sentinel` module — DPR-02 §5.2 reference)
- `/Users/iraabbott/softoboros/streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs`
  - lines 1431-1522 (render-dirty loop, kept analyzer-side per §9)
  - lines 1524-1796 (boot sequence stages 1-9, retired under DPR-03a)
  - lines 1797-1990 (boot sequence stages 10-12, partially retired)
  - lines 2004+ (AnalyzerKernel composition, untouched)
- `/Users/iraabbott/softoboros/streamz/submodules/disco-analyzer/analyzer-cm7/src/hsem.rs`
  - lines 1-90 (module documentation, constants, `HSEM6_FRESH`)
  - lines 92+ (`init_lineiin_receive` — retired under DPR-03a)
  - `HSEM0()` ISR body (kept analyzer-side per §4.3)

### External references

- RM0399 §11 (HSEM) — INV-DPR-11 reference (HSEM[6] enable sequence)
- ARMv7-M B3.4 (NVIC) — for the HSEM IRQ 125 unmask referenced by
  `init_lineiin_receive`

## 14. Unblocks

DPR-03 ratification unblocks DPR-03a (compile-only adoption), DPR-03b
(hardware validation), and DPR-03c (`AdaptedCodeOrigin` retirement +
initiative close).

DPR-03c closing **closes the DPR initiative's core invariants**:
INV-DPR-2 (analyzer as second-app proof) and INV-DPR-15 (cross-repo
gating named) are both satisfied once the analyzer runs end-to-end on
the platform surface with no `AdaptedCodeOrigin` headers outside the
ShimAllowlist. DPR-04 (BSP-generator reopen gate) is then unblocked
with concrete demo + analyzer evidence informing its decision.

A future `DAA-01-B-2` doc in the analyzer subrepo ratifies the
analyzer-side perspective on the same migration; its §15 entries are
the CrossRepoGate evidence DPR-03a/b/c cite to claim acceptance.

## 15. Change Log

- **2026-05-20** — **Ratified.** All §12 acceptance gates checked.
  No code-side scaffold lives yet (DPR-03 is cross-repo coordination;
  the rlvgl-side surface it consumes — `RuntimeProfile::Analyzer`,
  `BoardRuntime::init`, `FrameScheduler<VideoMode, ...>`,
  `SafeStop::run` — is partially scaffolded under DPR-01a/02a but
  not yet consumer-wired). Ratification locks in the §4 source-of-
  truth map, §5 adoption sequence + ShimAllowlist + BootSentinel
  mapping, and §6 INV-DPR-3-1/INV-DPR-3-2 against the corresponding
  analyzer-subrepo state at `0f038c7` (the worktree-isolated agents'
  base commit). DPR-03a/b/c sub-phase gates remain binding per §8;
  each rides a CrossRepoGate tied to a DAA-01-B-2 §15 amendment that
  the analyzer subrepo will produce when its side of the migration
  is ready. PCDN-DPR-3-001..003 remain deferred per §11.
- **2026-05-19** — Initial draft. Captures the cross-repo adoption
  plan for retiring the analyzer's mirrored bring-up code per
  DAA-01-B §6 (Option C, 2026-04-27). Frozen decisions: §5.1 adoption
  sequence (Cargo.toml → main.rs → bsp.rs → hsem.rs); §5.2
  ShimAllowlist (`HalGpioBacklight`, `HalResetPin`, `I2c4Adapter`
  retained, all justified by `stm32h7xx-hal 0.16` shipping eh-0.2
  traits only); §5.3 BootSentinel value-mapping
  (`PRE_GPIO_SPLIT`→`PRE_CLOCK_INIT`, `POST_GPIO_SPLIT`→`POST_CLOCK_INIT`,
  `POST_SDRAM_INIT`→`POST_SDRAM_INIT`, `POST_DISPLAY_INIT`→`POST_DISPLAY_INIT`,
  `POST_FIRST_RENDER` stays analyzer-side). New invariants: INV-DPR-3-1
  (no `AdaptedCodeOrigin` headers outside ShimAllowlist) and
  INV-DPR-3-2 (canonical analyzer uses `RuntimeProfile::Analyzer`
  by name, not `Custom`). Three-sub-phase split: DPR-03a (compile-only),
  DPR-03b (hardware validation), DPR-03c (header retirement +
  initiative close). Defers PCDN-DPR-3-001..003 to DPR-03a/b. Cites
  every file:line targeted for retirement under §13 absolute paths.
