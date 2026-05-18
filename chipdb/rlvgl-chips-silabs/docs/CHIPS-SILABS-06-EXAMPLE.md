<!--
CHIPS-SILABS-06-EXAMPLE.md - Example crate consuming the SiLabs BSP.
Status: Ratified 2026-05-14 (owner: Ira Abbott). See §15.
-->

# CHIPS-SILABS-06 — SLSTK3701A example crate

**Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.

The key words **MUST**, **MUST NOT**, **SHALL**, **SHOULD**, **SHOULD
NOT**, **MAY**, and **RECOMMENDED** are interpreted per RFC 2119 and
RFC 8174.

## 0. Authority policy

| Concern                                                   | Authority                                                                                                                                                                                              |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| EFM32GG11B register-level peripheral access               | **EFM32GG11B Family Reference Manual** (Silicon Labs) — CMU, GPIO, USART/LEUART, ROUTELOC/ROUTEPEN tables. Authoritative for any application-level peripheral fiddling beyond what `board::init()` does. |
| SLSTK3701A board pinout, on-board LEDs, push buttons, VCOM | **UG287: EFM32 Giant Gecko GG11 Starter Kit User's Guide** (Silicon Labs, Rev 1.30, 2024) — §6.1 (Push Buttons + RGB LEDs), §6.12 (Virtual COM Port). Mirrored into `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml`. |
| ARM Cortex-M4F boot, reset handler, vector table          | `cortex-m-rt = 0.7` and the **ARMv7-M Architecture Reference Manual** §B1.5. Reset entry, exception table layout, `_stack_start` symbol are owned by `cortex-m-rt`; this crate MUST NOT redefine them.  |
| Raw PAC surface                                           | `efm32gg11b-pac = 0.1.4` with the `efm32gg11b820` SKU feature flag enabled. The slate-6 `CHIPS-SILABS-02` SKU flatten amendment (commit `11075e6`) means the PAC's `Peripherals` type is reached via the `efm32gg11b820` sub-module that the generated `pac.rs` re-exports. |
| Linker scripts (`memory.x`, `efm32_gg11.x`)               | Emitted by the generator under `CHIPS-SILABS-05` (commit `37eef42`). Filenames and content frozen by `CHIPS-SILABS-05` §5; not redefined here.                                                          |

Authority precedence: when EFM32GG11B RM and `efm32gg11b-pac 0.1.4`
disagree on a register field name, **PAC wins** — same rule as
`CHIPS-SILABS-05` §0, because the consuming crate's `cargo check`
gate type-checks against the PAC, not the RM.

## 1. Purpose

`CHIPS-SILABS-06` ratifies a minimal example crate that consumes the
slate-9 SILABS BSP output (`{mod, pac, clocks, io_mux, peripherals,
board}.rs` + `memory.x` + `efm32_gg11.x`) for the SLSTK3701A Giant
Gecko Starter Kit, end-to-end proving the chipdb → generator → BSP →
example pipeline analogously to `examples/beetle-esp32c3/` (the ESP
precedent ratified at CHIPS-ESP-06-equivalent).

This chapter is the SILABS analogue of the "Unblocks" line in
`CHIPS-SILABS-05` §14: with linker emission ratified, an example
crate can now link against the chipdb-derived `memory.x` rather than
hand-authoring one. The v0 scaffold here is intentionally narrow —
it proves the pipeline, not application functionality. Real LED
blink (`-06a`), real console UART hello-world (`-06b`), and full
rlvgl integration (`-06c`) ride on follow-up sub-letter phases.

## 2. Problem statement

Slates 1–9 (`CHIPS-SILABS-01` through `-05`) closed the spec /
generator / compile-verify / linker triangle, but none of them
**link** a real binary that consumes the generator output. The
ESP precedent treats this gap as load-bearing: `compile-verify`
proves the generated BSP type-checks against the PAC in isolation,
and the example crate proves the BSP composes into a buildable
binary alongside `cortex-m-rt`, a panic handler, and a real entry
point — a strictly stronger gate, because `cargo check` does not
invoke the linker (see `CHIPS-SILABS-05` §10.2).

Without this chapter the SILABS family has no anchor proving the
linker scripts emitted by `CHIPS-SILABS-05a` actually compose with
`cortex-m-rt`'s bundled `link.x`. Hand-authoring a parallel example
crate without a ratified concepts doc would be a silent fork per
CLAUDE.md §"Definitions — reference vs. restatement".

## 3. Canonical glossary

| Term                         | Definition                                                                                                                                                                                                                  |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bsp_pac` feature            | Cargo feature on `rlvgl-example-slstk3701a` that selects the raw-PAC bring-up path. Mirrors `examples/beetle-esp32c3`'s `bsp_pac` feature. **As defined in `examples/beetle-esp32c3/Cargo.toml:67`; adapted: SILABS has no `esp_hal`-equivalent sibling feature, so `bsp_pac` is also the `default` feature.** |
| `board::init()`              | The entry point exported by the generated `bsp_generated/slstk3701_a/pac.rs::init`, which sequences `clocks::init()` → `io_mux::init()` → `peripherals::init()`. **As defined in `src/bin/creator/bsp/silabs/templates/pac.rs.jinja`; used without modification.** |
| `LED0_R_PORT` / `LED0_R_PIN` (and `LED0_G`, `LED0_B`, `LED1_R`, `LED1_G`, `LED1_B`) | Per-LED port-letter and pin-number constants emitted by the `board.rs` template from the SLSTK3701A board YAML. The SLSTK3701A carries **two RGB LEDs** wired as six discrete active-low GPIO outputs on PH10..PH15 (LED0 on PH10/PH11/PH12, LED1 on PH13/PH14/PH15). PWM-capable alt-functions on TIMER/WTIMER are routed but not enabled by `board::init()`. **As defined in `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml:84-101`; used without modification.** |
| `BTN0_PORT` / `BTN0_PIN` (and `BTN1`) | Push-button GPIO pin constants. PC8 (BTN0) and PC9 (BTN1, doubles as `GPIO_EM4WU2`). Active-low with on-board RC debounce. **As defined in `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml:71-74`; used without modification.** |
| VCOM (USART4 console)        | Virtual COM Port over USART4 at 115200-8N1, PH4 (TX) / PH5 (RX), gated by VCOM_ENABLE on PE1. Configured but not exercised by the v0 example. Real hello-world output is deferred to `-06b`. **As defined in `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml:48-63`; used without modification.** |
| `efm32gg11b820` PAC feature  | Cargo feature on the `efm32gg11b-pac` crate that gates the per-SKU `Peripherals` sub-module (per `CHIPS-SILABS-02` SKU flatten). MUST be enabled in this crate's `Cargo.toml`. **Owned by `efm32gg11b-pac 0.1.4`; does not exist in this repo.** |

## 4. Source-of-truth map

| Concept                                         | Owner / authoritative location                                                                                                                                                                |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Example crate manifest (`Cargo.toml`)            | `examples/slstk3701a/Cargo.toml` — owned by this chapter. Lists PAC dep + cortex-m-rt + panic handler + `bsp_pac` feature.                                                                    |
| Reset entry / `#[entry]` glue                    | `examples/slstk3701a/src/bsp_pac_main.rs` — owned by this chapter. Calls `bsp_generated::slstk3701_a::pac::init()` then enters a `wfi` loop.                                                  |
| BSP files (`board.rs`, `clocks.rs`, `io_mux.rs`, `pac.rs`, `peripherals.rs`) | `chipdb/rlvgl-chips-silabs/` via `rlvgl-creator bsp from-yaml --vendor silabs --board slstk3701a`. **NOT** owned by this chapter; regenerate on schema changes.                              |
| Linker scripts (`memory.x`, `efm32_gg11.x`)     | Same generator path. Frozen by `CHIPS-SILABS-05` §5. Pulled into `OUT_DIR` by the example crate's `build.rs`.                                                                                  |
| Host `bsp_generated/mod.rs` module index         | `examples/slstk3701a/src/bsp_generated/mod.rs` — hand-written. Re-exports `pub mod slstk3701_a;` (the snake-cased board stem the generator produces). Same pattern as `examples/beetle-esp32c3/src/bsp_generated/mod.rs`. |
| `_stack_start`, `.vector_table`, reset handler   | `cortex-m-rt 0.7` — not in repo. This chapter MUST NOT redefine these.                                                                                                                       |
| Target triple (`thumbv7em-none-eabihf`)          | `.cargo/config.toml` — owned by this chapter; matches the EFM32GG11B Cortex-M4F target per `chipdb/.../EFM32GG11.yaml` `arch:` field.                                                          |

## 10. Reconciliation with adjacent repo primitives

### 10.1 Complementary-gate framing with `bsp_silabs_slstk3701a_compile`

The compile-verify gate (`CHIPS-SILABS-04`, `tests/bsp_silabs_slstk3701a_compile.rs`)
materialises a throwaway cargo project that copies only the **5 .rs
files** from the generated BSP and runs `cargo check
--target thumbv7em-none-eabihf` against `efm32gg11b-pac 0.1.4`. It
type-checks the BSP in isolation but does **not** invoke the linker,
so the linker scripts emitted by `CHIPS-SILABS-05a` are not exercised
by compile-verify.

This crate is the **complementary gate**: it consumes the full 8-file
emission set (5 .rs + `mod.rs` + `memory.x` + `efm32_gg11.x`) plus
`cortex-m-rt 0.7`'s bundled `link.x` and runs `cargo check
--target thumbv7em-none-eabihf` on a real binary crate. Together the
two gates prove the BSP both type-checks *and* composes with the
canonical reset/link toolchain.

### 10.2 ESP precedent (`examples/beetle-esp32c3/`)

The shape of `examples/slstk3701a/` mirrors `examples/beetle-esp32c3/`
with two intentional differences:

1. The ESP crate has two parallel entry points (`esp_hal` and
   `bsp_pac`) selected by mutually exclusive features. The SILABS
   v0 crate has only the `bsp_pac` path — no SiLabs SDK / EMLIB
   feature, no `esp_hal`-equivalent. `bsp_pac` is therefore the
   `default` feature.
2. The ESP linker entrypoint is `riscv-rt` (with the `memory`
   feature, which `INCLUDE memory.x`s automatically). The SILABS
   linker entrypoint is `cortex-m-rt 0.7`, which uses the same
   `INCLUDE memory.x` pattern via its own `build.rs`. Both crates
   add `-T<chip>.x` to the linker args via their respective
   `build.rs` — the mechanism is identical, only the script name
   changes.

### 10.3 Workspace integration

The example crate is **NOT** added to the workspace `Cargo.toml`
by this chapter. The project-management role adds it at the next
v0.2.0 integration pass once the `cargo check` gate is confirmed
locally. This matches the workflow used when `examples/beetle-esp32c3/`
landed under `CHIPS-ESP-06`.

## 11. Non-goals (v0)

The v0 scaffold deliberately **does not** include:

- **LED blink in `main()`.** v0 `main()` does `bsp_generated::init()`
  then enters `loop { wfi() }`. Real blink (driving PH10/PH11/PH12
  for LED0 RGB) lands in `-06a` once the example crate boots and
  the `cargo check` gate is wired into pre-publish §Phase 4.7c.
- **USART4 console hello-world.** VCOM_ENABLE on PE1 is asserted by
  the generated `peripherals::init()` (via the board yaml's
  `initial: high`), but no characters are pushed onto USART4. The
  console-hello-world phase is `-06b`.
- **rlvgl render-stack integration.** The SLSTK3701A has no display
  panel on the kit itself, so any rlvgl integration would require
  an external display add-on (e.g. an SSD1306 over USART4/I2C2 in
  SPI mode). Deferred to `-06c` and gated on board-add-on selection.
- **Hardware bring-up.** This chapter's acceptance is `cargo check`
  only; flashing and JLink-CDC validation are scoped out per
  `CHIPS-SILABS-00` §12(g).
- **Touching the workspace `Cargo.toml`.** Project-management role
  adds the new crate at v0.2.0 integration; this chapter MUST NOT
  pre-empt that.

## 12. Acceptance checklist

This chapter is ratified when:

- [x] §0 authority table reviewed.
- [x] §3 glossary terms each carry a cite-vs-restate marker.
- [x] §4 source-of-truth map names one owner per concept.
- [x] §10 reconciles with compile-verify (`-04`), linker emission
      (`-05`), and the ESP precedent (`examples/beetle-esp32c3/`).
- [x] §11 enumerates every v0 non-goal so consumers can reason
      about what is missing.
- [x] §15 carries a dated ratification entry.

This chapter's behavioural acceptance (the implementation phase
`CHIPS-SILABS-06a`) is gated by:

- [ ] `cargo check -p rlvgl-example-slstk3701a --target thumbv7em-none-eabihf` —
      passes against `efm32gg11b-pac 0.1.4` + `cortex-m-rt 0.7`.
- [ ] The example binary links (no unresolved symbols against
      `cortex-m-rt`'s bundled `link.x` + the generator's
      `memory.x` + `efm32_gg11.x`).
- [ ] `board::init()` is callable from the `#[entry]` reset
      handler.

## 13. Files cited

- `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml` — board pin
  table including LED0/LED1 RGB assignments (PH10–PH15) and the
  VCOM USART4 console.
- `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` — chip
  memory map and linker hints consumed by `memory.x` / `efm32_gg11.x`.
- `chipdb/rlvgl-chips-silabs/docs/CHIPS-SILABS-00-CONCEPTS.md` —
  parent concepts chapter; §6.4 freezes the 6-file emission set
  this chapter consumes (extended to 8 by `-05`).
- `chipdb/rlvgl-chips-silabs/docs/CHIPS-SILABS-05-LINKER.md` —
  linker chapter this crate's `build.rs` consumes.
- `examples/beetle-esp32c3/Cargo.toml`,
  `examples/beetle-esp32c3/src/bsp_pac_main.rs`,
  `examples/beetle-esp32c3/src/bsp_generated/mod.rs`,
  `examples/beetle-esp32c3/build.rs` — ESP precedent for the
  example-crate shape.
- `src/bin/creator/bsp/silabs/templates/pac.rs.jinja` — declares
  `board::init()` and the SKU-flattened `pac` re-export.
- `src/bin/creator/bsp/silabs/templates/board.rs.jinja` — declares
  the `<LABEL>_PORT` / `<LABEL>_PIN` constant pairs the LED-blink
  follow-up phase (`-06a`) will consume.

## 14. Unblocks

- **CHIPS-SILABS-06a** — LED blink on LED0_R (PH10) using raw PAC
  GPIO writes after `board::init()`. First-pass duty cycle via a
  `nop`-busy-wait; SysTick-driven timing rides on a follow-up.
- **CHIPS-SILABS-06b** — USART4 hello-world over VCOM at 115200-8N1.
  Requires a console driver layered on the generated USART4 init.
- **CHIPS-SILABS-06c** — rlvgl integration on an external display
  add-on (board selection deferred).

## 15. Change log

| Date       | Status                       | Note                                                                                                                                                                                                                              |
| ---------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Initial ratification of the SLSTK3701A example crate.  Frozen: `bsp_pac` as the sole + default feature; `examples/slstk3701a/` as the crate path; v0 `main()` boots `board::init()` and enters `wfi` (no blink, no console).  v0 acceptance is `cargo check --target thumbv7em-none-eabihf`.  Workspace `Cargo.toml` registration intentionally deferred to PM-side v0.2.0 integration pass. |
