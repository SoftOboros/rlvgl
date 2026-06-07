<!--
  STM_BSP_GENERATION.md — STM32 BSP generator behavior and flags
  Covers inputs, outputs, environment overrides, split‑core support,
  current feature set, and a roadmap for enhancements.
-->

# STM32 BSP Generation

This document explains how the rlvgl‑creator BSP generator consumes CubeMX `.ioc` files to produce STM32 board support code (PAC and HAL styles), how environment overrides work, what flags are supported, and the current behavior for dual‑core parts like STM32H747. It also outlines a plan to enhance the generator to cover additional, commonly used STM features.

## Inputs

- CubeMX `.ioc` file for the target board/MCU. The generator parses:
  - MCU, package, pins (function/AF), user labels
  - Peripheral instances and kernel clock selections
  - Power settings: supply (SMPS/LDO), VOS/SDLEVEL
  - Clock intent: SYSCLK source, PLL source, HSE value, PLL1..3 M/N/P/Q/R, prescalers (D1/D2/D3)

- Environment overrides (uppercase “STM32_SECTION_KEY”):
  - `STM32_PWR_SUPPLY=SMPS|LDO` → overrides `.ioc` PWR.Supply
  - `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3` → overrides `.ioc` PWR.SDLEVEL (VOS)
  - Reserved for future use (not yet applied):
    - `STM32_RCC_HSE_HZ=<Hz>` → overrides/defines HSE frequency when absent
    - Generic “Camel→SCREAM” mapping: keys of the form `STM32_<SECTION>_<KEY>` will be recognized as we add support

## Outputs

- PAC style BSP (register‑level): `pac.rs`
- HAL style BSP (stm32xx‑hal): `hal.rs`
- One‑file by default, or split into per‑core outputs under `cm7/` and `cm4/` for dual‑core parts
- Optional label constants module based on `GPIO_Label` entries

## Generator Behavior (H7 focus)

- Pins
  - Configures GPIO mode/AF/pull/OD/speed based on `.ioc`
  - Emits grouped register writes when requested
  - Optionally emits label constants and/or uses label‑based identifiers

- Power (PWR) on STM32H7
  - Enables supply/VOS updates via SCUEN bit (gated writes)
  - Selects supply (SMPS/LDO) and, if SMPS, sets SDLEVEL from VOS
  - Programs target VOS to `PWR.D3CR.VOS[15:14]` via raw bits
  - Waits for `ACTVOSRDY` before proceeding

- Clocks (RCC)
  - Parses `.ioc` clock intent: SYSCLK source, PLL source, HSE, PLL1 params, D1/D2/D3 prescalers
  - CM7‑only `init_clocks(&dp)` hook:
    - HSI/HSE SYSCLK: enables source and switches `CFGR.SW`
    - PLL1 SYSCLK: configures `PLLCKSELR` (PLLSRC/DIVM1), `PLL1DIVR` (N/P/Q/R), enables DIVP1, enables PLL1, switches SYSCLK to PLL1 and waits
    - Applies D1 CPU prescaler (D1CFGR.D1CPRE) and APB prescalers (D1PPRE/APB3, D2PPRE1/APB1, D2PPRE2/APB2, D3PPRE/APB4) from `.ioc` tokens
  - Lightweight logging (feature `bsp_log`): emits a summary of SYSCLK/PLLSRC/HSE/prescalers via a weak `_bsp_log` sink

- Dual‑core (H747)
  - Auto‑detects split when both CM7 and CM4 projects are present in the `.ioc`
  - Emits per‑core PAC/HAL with correct PAC modules (`stm32h747cm7` / `stm32h747cm4`)
  - Mailbox reserved (1 KB at `0x3004_7000` in D2 SRAM3) for inter‑core sync/handoff
  - Helpers:
    - `signal_clocks_ready()` (primary core) sets semaphore and SEVs
    - `wait_for_clocks()` (secondary core) WFE‑waits on the semaphore

- Memory/linker (example project)
  - CM7 `memory.x`: DTCM `RAM`, D1 split regions for future placement, shared `MAILBOX`, D3 retention region declared
  - CM4 `memory_cm4.x`: D2 `RAM`, same `MAILBOX`, D1 split and D3 retention declared
  - Top‑level `build.rs` stages the correct `memory*.x` based on the binary name and passes `-Tlink.x`

## CLI Flags (bsp from-ioc)

- Layout and content
  - `--emit-pac` / `--emit-hal` — render one or both BSP styles
  - `--grouped-writes` — collapse GPIO/RCC writes by register for compactness
  - `--with-deinit` — emit basic deinit helpers
  - `--use-label-names` — prefer label‑based identifiers in HAL BSP
  - `--emit-label-consts` — emit `pins` module with label constants in PAC BSP
  - `--label-prefix <str>` — prefix for labels that start with digits/underscores

- Core ownership
  - `--split-cores` — emit `cm7/` and `cm4/` when dual‑core; auto‑enabled if both cores are present in `.ioc`
  - `--core cm7|cm4` — restrict unified output to a single core
  - `--clock-init-core cm7|cm4` — override which core owns system clock init (default: CM7 for H7x7)
  - `--periph-core name=core,...` — assign specific peripheral ownership

## Environment Flags (affect template context)

- `STM32_PWR_SUPPLY=SMPS|LDO`
- `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3`
- Reserved (planned): `STM32_RCC_HSE_HZ=<Hz>`

## Example Helper

The `examples/stm32h747i-disco/gen-bsp.sh` script sets defaults for SMPS/VOS1 and regenerates only when needed. It honors:

- `STM32_PWR_SUPPLY`, `STM32_PWR_SDLEVEL` — default to `SMPS`, `VOS1`
- `FORCE_BSP=1` — force regeneration

## Current Limitations

- PLL configuration is tailored for H7 and assumes integer N/P/Q/R. Fractional configs and VCO range tuning are not yet emitted.
- Clock prescaler token mapping covers common CubeMX encodings but may miss variant strings.
- Dual‑core sync uses a simple mailbox; no HSEM or EXTI wake yet.
- Kernel clocks (peripheral muxes) are partially emitted; expansion is ongoing.

## Enhancement Plan (Roadmap)

1) Clocking
   - Add fractional PLL support (FRACN) and VCO range selection from `.ioc`
   - Emit complete D1/D2/D3 prescaler mapping for variant tokens
   - Expand kernel clock (CCIPx) mappings for more peripherals
   - Provide HAL clock init path mirroring PAC intent for users favoring HAL

2) Dual‑core infrastructure
   - Optional HSEM‑based handshake as an alternative to mailbox
   - Extend mailbox to a structured protocol (versioned header, commands, acks)
   - Provide example CM4 startup flow that enables domain clocks post‑signal

3) Power and low‑power
   - Emit low‑power domain management helpers (D3 retention, STOP/Standby entry/exit)
   - Support BOR/PVD/voltage monitor configuration from `.ioc`

4) Pins and peripherals
   - Emit pin summaries/pin‑report with label cross‑refs
   - Generate peripheral init helpers for common blocks (I2C/SPI/UART) from `.ioc` roles

5) Cross‑family coverage
   - Extend PWR/RCC templates for STM32H5, G4/L4/L5 families with family‑specific registers
   - Auto‑select PAC modules per sub‑family (already done for H747 cm7/cm4)

6) DX and validation
   - Add `--verbose` mode that logs applied power/clock settings via `bsp_log!`
   - Unit tests for token → bitfield mappings across families

## Contributing

Please open issues or PRs with `.ioc` samples and desired behaviors. Include the MCU, package, external clock source (HSE), and any required peripheral muxing so we can extend the generator safely.

