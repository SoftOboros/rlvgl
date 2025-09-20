# STM32H747I-DISCO Bring‑Up Notes (CM7)

This file summarizes the essential state, decisions, and next steps for the CM7 bring‑up of the STM32H747I‑DISCO target in this repository.

## Target, Build, Debug
- Target: STM32H747I‑DISCO (CM7 core)
- Build task: `build-disco (cm7)` with features:
  - `stm32h747i_disco_cm7,dma2d,backlight_pwm,pac_sdram_init,sdram_ramtest`
- VSCode launch: “CM7 attach (external OpenOCD)”
  - `runToEntryPoint: main`
  - No semihosting post‑launch commands

## Boot Sequence Snapshot
The CM7 path now consistently completes the early bring-up steps:

1. Short busy-wait so debuggers can attach before peripheral side effects.
2. MPU regions (DTCM/ITCM/SRAM/SRAM4/SRAM3/SRAM2/SRAM1 + SDRAM) are programmed before any RCC or FMC register access.
3. SDRAM GPIOs are forced to AF12/VeryHigh and the FMC kernel clock is enabled; the SDRAM command sequence executes directly through the PAC while the device is still on the reset clock tree.
4. PWR SMPS + VOS1 are configured once SDRAM is stable.
5. HAL RCC configures PLL1/PLL3 and re-enables FMC for normal runtime, followed by panel/backlight init.

## Clocks (Current)
- HSE = 25 MHz; SYSCLK = 400 MHz (PLL1)
- LTDC: PLL3R ≈ 32 MHz (pixel clock path)
- FMC kernel clock (post-HAL) = PLL2R 150 MHz → SDCLK ≈ 75 MHz (divider /2)
- AHB (HCLK) = 200 MHz via HPRE=/2

## SDRAM (FMC) Status
- Device profile: IS42S32800G (32 MiB, x32) with SDCLK now driven at ~75 MHz (FMC kernel 150 MHz / 2).
- PAC-driven init now happens before the HAL touches RCC/PWR:
  - GPIO D/E/F/G/H/I routed to AF12 + VeryHigh speed prior to any clock changes.
  - FMC clocks enabled via `AHB3ENR` and `C1_AHB3ENR`.
  - SDCR1/SDTR1/SDCMR/SDRTR written directly; BUSY polling verifies each command completes at the higher clock rate.
- Verified: MPU instrumentation still records region state (`MPU_TRACE`/`MPU_DUMP`); with the 75 MHz SDCLK (PLL2R 150 MHz with /2 divider) the SDRAM allocator/tests remain stable after bring-up.
- Logging: semihosting remains disabled on the SDRAM hot path to avoid SWD stalls.

## Optimization / Stepping
- Dev profile: `opt-level = 0` for workspace; `stm32h7xx-hal` remains unoptimized for debugging.
- Startup attach spin removed (no initial long delay).

## Known Observations
- Stepping must land after each PAC `SDCMR` write; halting before BUSY clears can leave the controller waiting forever.
- Peripheral viewer polling can still contend with the SDRAM init; keep it closed while single-stepping the sequence.
- VS Code’s pause button may issue `reset halt`; use `monitor halt` instead and rely on `.noinit` breadcrumbs for post-reset reconstruction.
- Disassembly view remains invaluable when sanity-checking the raw PAC register writes.

## Minimal, Reliable Stepping Strategy
1) Place breakpoints on the PAC helper right after each `SDCMR` write in `configure_fmc_sdram`.
2) After the breakpoint hits, read:
   - `SDCMR @ 0x5200_4140` to confirm MODE/CTBx.
   - `SDSR  @ 0x5200_4158` and wait for BUSY=0 before continuing.
3) Continue through ClkEnable → delay → PALL → AR(8) → LoadMode → SDRTR programming.
4) If MPU faults still appear, temporarily build with `--features skip_sdram_mpu`; once MPU completes, inspect `MPU_TRACE`/`MPU_DUMP` for final region state.

## Proposals to Move Forward
- Fault trapping: add CM7 exception handlers to break in place on faults
  - Implement `HardFault`, `BusFault`, `UsageFault` with `bkpt()` loops so faults never fall back to reset
- DWT cycle counter delay (optional): use CYCCNT for deterministic, debugger-immune delays instead of asm spin (both are OK; CYCCNT eases tuning)
- Inter-command waits: if any `SDSR.BUSY` doesn’t clear, extend the busy loop or add a short delay between `SDCMR` writes in `configure_fmc_sdram()`
- CubeIDE cross-check: generate FMC/SDRAM init for H747I-DISCO and mirror the exact SDCR/SDTR/SDCMR/SDRTR values for an apples-to-apples compare
- Scope check (hardware): verify SDCLK on PG8 and that SDCKE1 is asserted prior to PALL (PH7)
- Memory allocation: keep the default heap/stack in DTCM and introduce a second allocator backed by `SDRAM`
  - Define a `.sdram_heap` output section in `memory.x`
  - Back it with a Rust static using `#[link_section = ".sdram_heap"]` and initialise a dedicated `Heap`/bump allocator
  - Guard the SDRAM allocator behind a mutex so high-footprint components opt-in explicitly

### Exception handlers
Still recommended: add `HardFault`, `BusFault`, `UsageFault` handlers that loop on `bkpt()` so faults don’t reset CM7. When they hit, read `0xE000_ED28`, `0xE000_ED2C`, `0xE000_ED34`, `0xE000_ED38` to identify the cause.

## Quick Reference (Addresses)
- FMC base: `0x5200_4000`
  - `BCR1..`:      `0x5200_4000`
  - `SDCR1/2`:     `0x5200_4080`
  - `SDTR1/2`:     `0x5200_4104`
- `SDCMR`:       `0x5200_4140`
- `SDSR`:        `0x5200_4158`
- `MPU_TRACE`:   `0x2001_0030`
- `MPU_DUMP`:    `0x2001_0034` (pairs of RBAR/RASR written during MPU bring-up)

Typical checks while stepping:
- After each `SDCMR` write, read `SDSR` and confirm BUSY clears (command completed) before issuing the next.
- Verify `BCR1.FMCEN` set by the HAL (`memory_controller_enable()`).
- Confirm `SDCR` fields match desired CAS/width/banks/col/row; `SDTR` meets timing at SDCLK.

## Current Defaults in Code
- FMCSEL: `PLL2R` @ `100 MHz`
- `max_sd_clock_hz: 75_000_000`
- `hclk: 200_000_000` (HPRE=/2)
- Pin speeds: VeryHigh for all FMC pins
- No SDRAM semihost logging; no initial startup spin

## What Codex Needs (Essentials)
- Confirmation at which `configure_fmc_sdram` stage `SDSR.BUSY` might stick (after ClkEnable, PALL, AR(8), or LoadMode).
- Single-shot register snapshots around each command:
  - `p/x *(u32*)0x52004140` (SDCMR) and `p/x *(u32*)0x52004158` (SDSR).
  - `p/x *(u32*)0x52004080`/`0x52004084` (SDCR1/2) and `0x52004104`/`0x52004108` (SDTR1/2).
- Fault context if anything trips: CFSR/HFSR/MMFAR/BFAR + `.noinit` breadcrumbs.
- Scope captures of PG8 (SDCLK) and PH7 (SDCKE1) remain useful for timing validation.
- CubeIDE register dump (if available) for cross-checking SDCR/SDTR/SDCMR/SDRTR values.

## GDB Stepping Cheatsheet
- Break just after each `SDCMR` write inside `configure_fmc_sdram`.
  1) Let the STR execute, then `p/x $lr` / `tbreak *$lr` as needed to hop back to the caller.
  2) Inspect `SDCMR`/`SDSR`; wait for BUSY to clear before continuing.
- If you must single-step the STR, `x/8i $pc`, `ni` through the store, then `finish` back to the helper.
- Prefer `monitor halt` to pause; `interrupt` often issues a reset. If a reset occurs, recover context from `MPU_TRACE`/`MPU_DUMP`.

## Hardware Pin Summary (FMC SDRAM)
- Clock/Enable: PG8 (SDCLK), PH7 (SDCKE1), PH6 (SDNE1)
- Control: PF11 (SDNRAS), PG15 (SDNCAS), PH5 (SDNWE)
- Bank Address: PG4 (BA0), PG5 (BA1)
- Address: PF0..PF5 (A0..A5), PF12..PF15 (A6..A9), PG0..PG2 (A10..A12)
- Byte Lanes: PE0 (NBL0), PE1 (NBL1), PI4 (NBL2), PI5 (NBL3)
- Data: PD14..PD15, PD0..PD1, PE7..PE15, PD8..PD10, PH8..PH15, PI0..PI3, PI6, PI7, PI9, PI10
- All above set to AF12 + Speed::VeryHigh in code


## Next Actions (Minimal)
1) Add CM7 exception handlers to trap faults immediately.
2) Keep validating each SDRAM command with breakpoints post-`SDCMR` and adjust BUSY polling if needed.
3) Mirror CubeIDE timings if hardware differences crop up; extend inter-command delay as required.
4) Consider restoring modest `opt-level` once bring-up remains stable.
5) Continue capturing `MPU_TRACE`/`MPU_DUMP`; they survive resets and confirm MPU state.

## SDRAM Bring‑Up (Unrolled PAC Init)

Context: Stepping the HAL helper was brittle under the debugger, so the `pac_sdram_init` feature now unrolls the init sequence with explicit PAC writes.

- Feature: `pac_sdram_init` (default in CM7 build).
- Sequence (FMC base 0x5200_4000):
  - Enable FMC clocks via `AHB3ENR.FMCEN` and `C1_AHB3ENR.FMCEN` (before HAL RCC).
  - `BCR1.FMCEN = 1` (controller enable).
  - Program `SDCR1` for IS42S32800G: NC=9, NR=12, MWID=32-bit, NB=4, CAS=3, SDCLK=/2, RBURST=1, RPIPE=0.
- Program `SDTR1` timings (@ ~75 MHz SDCLK): TMRD=2 cycles (write value 1), TXSR=7 cycles (value 6), TRAS=5 cycles (value 4), TRC=7 cycles (value 6), TWR=2 cycles (value 1), TRP=2 cycles (value 1), TRCD=2 cycles (value 1).
  - Issue commands via `SDCMR` with BUSY polling between each:
    1) Clock Enable (MODE=1, CTB1=1).
    2) Precharge All (MODE=2, CTB1=1).
    3) Auto-Refresh ×8 (MODE=3, NRFS=7 encoding 8 cycles).
    4) Load Mode Register (MODE=4, MRD=0x0230, CTB1=1).
- Program `SDRTR` for ~7.81 µs at 75 MHz: COUNT ≈ 566 (write `COUNT<<1`).

Observed (good): Post‑sequence `SDCMR.MODE=4` (LoadMode) and `SDSR.BUSY=0`. PAC path returns to caller reliably.

### MPU Region for SDRAM (M7)
Once HAL enables the MPU, external SDRAM must be covered by an MPU region or the first read can MemManage/BusFault. We install an SDRAM MPU region immediately after the PAC init:

- Region base: `0xC000_0000`; size: 32 MiB (SIZE field = 24)
- Attributes: Normal memory, non‑cacheable (TEX=1, C=0, B=0), Shareable=1, AP=Full Access
- MPU enabled with PRIVDEFENA, plus DSB/ISB barriers

Result: first SDRAM reads no longer fault; quick probes succeed.

## VS Code Debug Setup (Cortex‑Debug)

Key learnings to stabilize attach/run on H7 with OpenOCD:
- Avoid issuing `monitor reset halt` after attach — it forces re‑examine and lands at Reset (PC=0x08000298), wiping FMC state.
- Prefer a pure attach that immediately runs:
  - Add a configuration with `request: "attach"`, `servertype: "external"`, and `postAttachCommands: ["continue"]`.
- Load GDB macros safely:
  - `preLaunchCommands`: `set mem inaccessible-by-default off`, `add-auto-load-safe-path ${workspaceFolder}/.gdbinit`, `source ${workspaceFolder}/.gdbinit`
- Keep Peripheral Registers view closed while bringing up FMC/SDRAM; frequent polls can cause SWD “Busy” errors.
- Launch configs set `objdumpPath` + `showDisassembly: "always"`; Disassembly view will show instructions even when symbols are absent.

Minimal attach+run launch entry (excerpt):

```
{
  "name": "CM7 attach + run (no reset)",
  "type": "cortex-debug",
  "request": "attach",
  "servertype": "external",
  "gdbTarget": "localhost:3333",
  "preLaunchCommands": [
    "set mem inaccessible-by-default off",
    "add-auto-load-safe-path ${workspaceFolder}/.gdbinit",
    "source ${workspaceFolder}/.gdbinit"
  ],
  "postAttachCommands": ["continue"]
}
```

### OpenOCD invocation (recommended)
- Use slow SWD + connect‑under‑reset during bring‑up:
  - `transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init`

## GDB Macros and Tactics

- `.gdbinit` helpers (in repo root): `faultregs`, `sdramregs`, `wait_busy_clear`, `lrtrap_safe`.
- lrtrap vs inline BKPT:
  - `lrtrap` is effective at real call sites; inline `bkpt` has no LR you can tbreak — use `set $pc = $pc + 2` to skip.
- Watchpoints on `SDCMR @ 0x5200_4140` are useful, but can cause churn; prefer the PAC path + `sdramregs` dumps instead.

## Validation Steps (Quick)
- After the PAC init runs, verify SDRAM quickly via Debug Console:
  - `set {unsigned int}0xC0000000 = 0xDEADBEEF`
  - `x/wx 0xC0000000` → `0xDEADBEEF`
- FMC state checks:
  - `x/wx 0x52004140` (SDCMR) → MODE=4 after LoadMode
  - `x/wx 0x52004158` (SDSR)  → BUSY bit 5 = 0
  - `x/wx 0x52004080` (SDCR1), `0x52004104` (SDTR1) → non‑default values

## HAL vs PAC Paths

- `pac_sdram_init` is the default path and now runs before any HAL clock/power changes; it has been validated end-to-end.
- The older HAL helper (`hal_sdram`) remains available for experimentation but is no longer part of the standard CM7 build.

## Known Quirks (OpenOCD/H7)
- “read_memory … 0x5C001004 … examine‑end failed”: harmless attach noise; avoid frequent resets/examines from GDB.
- “target not halted … resume was requested”: occurs if Continue is pressed while already running; just Pause → Continue.
- If connection is flaky, restart OpenOCD with slow SWD and connect‑under‑reset, then attach with the no‑reset config above.
