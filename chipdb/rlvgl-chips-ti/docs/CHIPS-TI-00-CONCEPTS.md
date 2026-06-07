<!--
CHIPS-TI-00-CONCEPTS.md - Concepts & vocabulary for the Texas Instruments
chipdb + BSP-generator initiative.
-->

# CHIPS-TI-00 — Texas Instruments chipdb + BSP Generator Concepts

> **Status:** Ratified 2026-05-11 (owner: Ira Abbott). See §15.
> `CHIPS-TI-NN[a-z]:` execution PRs MAY cite this chapter as a
> frozen authority. Amendments require a new dated §15 entry and
> the same review depth as the original ratification pass.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared
in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline". RFC 2119 / RFC 8174 normative keywords (**MUST**,
**MUST NOT**, **SHALL**, **SHOULD**, **MAY**) carry their RFC meanings
when capitalised; lowercase use is narrative.

For every concept this chapter names, the authority is one of the
following. Where the authority is a TI document, the spec lineage is
cited by SPRU\* / SLA\* / SWRU\* part number rather than re-derived.

| Domain                                          | Authoritative source                                                                                                        |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Cortex-M4F architectural semantics              | ARM ARMv7-M Architecture Reference Manual                                                                                   |
| CC13x0 / CC26x0 chip TRM                        | TI SWCU117 (CC13x0/CC26x0 SimpleLink Wireless MCU Technical Reference Manual)                                               |
| CC13x2 / CC26x2 chip TRM                        | TI SWCU185 (CC13x2/CC26x2 SimpleLink Wireless MCU Technical Reference Manual)                                               |
| CC32xx chip TRM                                 | TI SWRU367 (CC3220 SimpleLink Wi-Fi Technical Reference Manual)                                                             |
| AM335x Sitara TRM                               | TI SPRUH73Q (AM335x and AMIC110 Sitara Processors TRM). **Cited but out of scope** for chipdb-driven generation — see §10.1 |
| MSPM0 family TRM                                | TI SLAU893 (MSPM0 G-series TRM) / SLAU847 (MSPM0 L-series TRM). **Out of scope** for CHIPS-TI-01 — see §11                  |
| svd2rust-shaped TI PAC crates                   | `cc26x0`, `cc26x2`, `cc3200` (or successor) crates on crates.io; vendored where required                                    |
| chipdb crate API (`chip_yaml` / `board_yaml`)   | `chipdb/rlvgl-chips-ti/src/lib.rs`                                                                                          |
| BSP generator IR / templates                    | `src/bin/creator/bsp/ti.rs` (current YAML→IR pass) and the per-vendor template tree pattern in `src/bin/creator/bsp/espressif/templates/` |
| Chip / board YAML shape across the chipdb       | `chipdb/rlvgl-chips-esp/db/{chips,boards}/*.yaml` (the reference shape)                                                     |
| Existing hand-written TI bring-up               | `examples/beaglebone-black/src/bsp/` (Linux / bare-metal AM335x prong)                                                      |

If a phase needs to **modify** a cited authority (e.g. assume a
different PAC crate version, or add a TI family whose TRM is not in
the table above), the modification ratifies in a §15 amendment
**first**, in a separate PR, before any behaviour PR rides on it.

## §1 Purpose

`chipdb/rlvgl-chips-ti` ships today as a stub: it has the
`chip_yaml` / `board_yaml` accessors and two placeholder YAML files
(`AM335x.yaml`, `MSP432P401R.yaml`), but no per-chip inventory data,
no per-board pin tables, and no BSP-generator templates. The current
TI adapter in `src/bin/creator/bsp/ti.rs` is a single `serde_yaml`
pass into the generic `Ir`, with no rendering pipeline behind it.

The CHIPS-TI initiative brings the TI tree to **parity** with the
five vendor pipelines that already render: Espressif, Nordic, NXP,
RP2040, Renesas. Parity here means:

1. Per-chip IR YAML covering memory map, clock tree, peripheral
   clock-gate table, IO MUX function table, and pin-routing subset
   sufficient to reproduce a `cargo check`-clean BSP against a
   real svd2rust-shaped PAC crate on the chip's target triple.
2. Per-board IR YAML covering pin assignments, console config, and
   optional I2C / SPI / LEDC-style overlays.
3. A 6-file template emission set (`mod.rs`, `pac.rs`, `clocks.rs`,
   `io_mux.rs`, `peripherals.rs`, `board.rs`) plus the two linker-
   script templates (`memory.x`, `chip.x`) that the Espressif tree
   already produces for the RISC-V chips.
4. Snapshot render tests (`bsp_ti_*_render`) and an opt-in
   `compile-verify` test family that materialises a throwaway
   cargo project around the emitted BSP and runs `cargo check`
   against the real PAC crate on the chip's target triple. This
   is the same pattern that `tests/bsp_esp32c3_compile.rs`
   already uses.
5. At least one example crate under `examples/` that consumes the
   generated BSP and demonstrates an LED blink (the
   `--features bsp_pac` end of the beetle-esp32c3 feature matrix is
   the precedent).

### 1.1 Initial scope — SimpleLink Cortex-M4F (CC13xx / CC26xx / CC32xx)

CHIPS-TI-01 SHALL bring up the **SimpleLink Cortex-M4F** family as
the first generation-capable target. The rationale:

- **PAC ecosystem fit.** The existing chipdb templates assume a
  svd2rust-shaped PAC crate with uppercase peripheral instance
  access (`p.UART0.clkdiv()`-style). The SimpleLink family has
  `cc26x0`, `cc26x2`, `cc3200` (and successor) PAC crates that
  match this shape. AM335x (Cortex-A8, no svd2rust-shaped PAC) and
  MSPM0 (newer, thinner crates.io coverage at draft time) do not.
- **Cortex-M4F at the bottom of the toolchain.** Target triple is
  `thumbv7em-none-eabihf`, identical to the STM32H747I-DISCO build
  profile already in `CLAUDE.md`. No new rustup target install is
  required for `compile-verify`.
- **Existing in-house mental model.** SimpleLink's PRCM / IO MUX /
  GPIO matrix shape is structurally close to Espressif's
  SYSTEM/PCR + IO_MUX + GPIO matrix model — close enough that the
  six-template plan port cleanly without a redesign, but distinct
  enough that the work product is not a copy-paste (see §10.2).
- **Discrete authority boundary.** Sitara (AM3 / AM5 / AM6) and TDA
  SoCs would each require their own bring-up vocabulary; bringing
  in the SimpleLink line first keeps CHIPS-TI-01 within a single
  TRM family (SWCU117 / SWCU185 / SWRU367).

MSPM0 was considered as an alternative. It is rejected for CHIPS-
TI-01 on the grounds that the PAC story is less mature at draft
time and that the M0+ register surface differs from the M4F shape
that the Espressif templates were tuned against. MSPM0 is a
**§11 non-goal for v0** of this initiative; revisiting it requires
a §15 amendment.

This chapter does **NOT** ratify the choice of *which* SimpleLink
chip lands first. CHIPS-TI-01 will name the first chip (likely
`CC1352R1F3RGZ` or `CC2652R1FRGZ`, both R-suffix variants with the
M4F / 352 KB flash configuration) in its own §15 entry, and SHALL
constrain its choice to a chip whose svd2rust-shaped PAC crate is
publicly available on crates.io.

## §2 Problem statement

Evidence that the TI chipdb is *not* at parity with the other
vendor trees, pinned to code paths:

- **Stub adapter.** `src/bin/creator/bsp/ti.rs` is a 17-line
  pass-through that calls `serde_yaml::from_str` against the
  generic `Ir` and returns it directly. There is no TI-specific
  IR (compare `EspIr`, `NrfIr`, `NxpIr`, `RpIr`, `RenesasIr`).
- **Stub chip YAML.** `chipdb/rlvgl-chips-ti/db/chips/AM335x.yaml`
  is 10 lines (name, arch, package, linux_kernel_dts). It carries
  no memory map, clock tree, clock-gate table, IO MUX, or GPIO
  matrix. `MSP432P401R.yaml` is the same shape — placeholder only.
- **Stub board YAML.** `db/boards/beaglebone_black_nhd_cape.yaml`
  lists name, chip, sdram\_mb, console; `pins: []` is empty. The
  real BBB bring-up data lives in
  `examples/beaglebone-black/src/bsp/` (hand-written) and
  `docs/beaglebone-black/`, **not** in the chipdb.
- **No template tree.** `src/bin/creator/bsp/` has subdirectories
  `espressif/`, `nordic/`, `nxp/`, `rp/`, `renesas/`, each with
  their own `templates/` tree of `.rs.jinja` files. There is no
  `ti/` subdirectory.
- **No snapshot test family.** `tests/bsp_esp32c3_render.rs`,
  `tests/bsp_esp32p4_render.rs`, etc. each cover a chipdb-emitted
  BSP. No `tests/bsp_ti_*` family exists today.
- **No `compile-verify` family.** Likewise, no
  `tests/bsp_ti_*_compile.rs` materialises a throwaway cargo
  project around a TI-generated BSP.

The cost of this is that any consumer who wants a code-generated
TI BSP today has to fork the Espressif template tree by hand, and
that the BeagleBone Black work is structurally orphaned from the
chipdb pipeline (see §10.1).

## §3 Canonical glossary

Reserved CHIPS-TI vocabulary. Capitalised use of these terms in
`CHIPS-TI-NN` docs MUST refer to the defined meaning; alternative
phrasings introduce drift and are forbidden in normative sections.

Definitions carry the *cite-vs-restate* marker required by
CLAUDE.md §"Definitions — reference vs. restatement".

- **TI chipdb crate** — *As defined in `chipdb/rlvgl-chips-ti/src/lib.rs`;
  used without modification.* The crate that embeds chip + board
  YAML at build time and exposes `chip_yaml`/`board_yaml`/`boards`/
  `find`. Vendor key is `"ti"`.
- **Chip IR YAML** — *Pattern owned by `chipdb/rlvgl-chips-esp/db/chips/*.yaml`;
  adapted: TI-specific fields per §5.* The per-chip inventory file
  under `chipdb/rlvgl-chips-ti/db/chips/<chip>.yaml`.
- **Board IR YAML** — *Pattern owned by `chipdb/rlvgl-chips-esp/db/boards/*.yaml`;
  adapted: TI-specific fields per §5.* The per-board pin / console /
  optional-config file under `chipdb/rlvgl-chips-ti/db/boards/<board>.yaml`.
- **`TiIr`** — *Owned by this chapter; does not exist in repo yet.*
  The TI-specific IR struct that replaces the
  pass-through `serde_yaml::from_str(text)` in
  `src/bin/creator/bsp/ti.rs`. Holds parsed chip + board YAML plus
  a derived peripheral-use set (per the `peripherals_used` pattern
  in `templates/peripherals.rs.jinja`).
- **SimpleLink Cortex-M4F family** — *Owned by this chapter;
  references TI SWCU117 / SWCU185 / SWRU367.* The set of TI parts
  whose CPU is Cortex-M4F and whose PRCM / IO MUX architecture
  matches the SimpleLink convention. Initial member set frozen in
  §5.5; expanding it is Standards Action.
- **PRCM (Power, Reset and Clock Module)** — *As defined in
  TI SWCU117 §4.* The block that owns peripheral clock gating,
  reset assertion, and standby/idle wake-up routing on SimpleLink
  Cortex-M4F. This is the TI analogue of Espressif's
  SYSTEM / PCR / HP\_SYS\_CLKRST block.
- **IO Controller (IOC)** — *As defined in TI SWCU117 §11.* The
  block that owns per-pin function selection on SimpleLink. The TI
  analogue of Espressif's IO MUX. **MUST NOT** be confused with
  CubeMX's `.ioc` file format — the two share a three-letter name
  but are unrelated authority surfaces. CHIPS-TI docs MUST always
  qualify "IOC" with "(IO Controller)" or "(CubeMX file)" on first
  use in any section.
- **Peripheral instance access style** — *As defined in the existing
  Espressif templates (`peripherals.rs.jinja`); used without
  modification.* The convention `p.PERIPHERAL_NAME.register_name()`
  where `PERIPHERAL_NAME` is uppercase and `register_name()` is the
  svd2rust accessor. CHIPS-TI templates MUST use this style;
  deviating requires a §15 amendment.
- **6-file template emission contract** — *As defined in the
  Espressif tree (`src/bin/creator/bsp/espressif/templates/`);
  adapted: TI templates emit the same six files plus the same two
  linker-script files.* The set `{mod.rs, pac.rs, clocks.rs,
  io_mux.rs, peripherals.rs, board.rs}` plus `{memory.x, chip.x}`.
  Frozen per §7.
- **`compile-verify` family** — *As defined in
  `tests/bsp_esp32c3_compile.rs`; adapted to the TI target triple.*
  An opt-in test family gated by a Cargo feature that materialises
  a throwaway cargo project around the generated BSP and runs
  `cargo check` against the real PAC crate.
- **Hand-written prong** — *As defined in
  [`docs/beaglebone-black/README.md`](../../../docs/beaglebone-black/README.md);
  used without modification.* A TI bring-up that lives outside the
  chipdb-driven generator and is co-located with an `examples/`
  consumer crate. AM335x / BBB is the canonical case (§10.1).
- **Target triple (TI)** — *Owned by this chapter.* The rustc
  target each CHIPS-TI chip compiles against. For the
  SimpleLink Cortex-M4F family this is `thumbv7em-none-eabihf`.

## §4 Source-of-truth map

One row per concept; one owner per concept. If two trees claim
authority over the same row, the schema has a defect — file an
amendment in §15 before writing code that depends on the conflict.

| Concept                                  | Owner (canonical)                                                | Mirrored / consumed by                                                          |
| ---------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| TI chipdb vendor key                     | `chipdb/rlvgl-chips-ti/src/lib.rs::vendor()` returning `"ti"`    | `rlvgl-creator` vendor enumeration                                              |
| Chip inventory schema                    | This chapter §5 (frozen field set)                               | `db/chips/*.yaml`, `TiIr` parser                                                |
| Board inventory schema                   | This chapter §5 (frozen field set)                               | `db/boards/*.yaml`, `TiIr` parser                                               |
| SimpleLink Cortex-M4F initial member set | This chapter §5.5                                                | CHIPS-TI-01 first-chip selection                                                |
| Target triple per chip                   | This chapter §6 (frozen table)                                   | `compile-verify` test family, `examples/` consumer crate                        |
| PRCM clock-gate table                    | Chip IR YAML `prcm:` block                                       | `clocks.rs` template                                                            |
| IOC (IO Controller) function table       | Chip IR YAML `ioc:` block                                        | `io_mux.rs` template                                                            |
| Per-board pin assignment                 | Board IR YAML `pins:` array                                      | `io_mux.rs` template + `board.rs` template                                      |
| Console peripheral selection             | Board IR YAML `console:` block                                   | `peripherals.rs` template (real init, per the Espressif `uart0` precedent)     |
| 6-file template emission set             | This chapter §7                                                  | `src/bin/creator/bsp/ti/templates/` (does not exist yet)                       |
| Linker-script emission                   | This chapter §7                                                  | `src/bin/creator/bsp/ti/templates/memory.x.jinja` + `chip.x.jinja`             |
| Peripheral instance access style         | Espressif templates (cited)                                      | All CHIPS-TI templates                                                          |
| PAC crate pin                            | This chapter §6 (per-chip crate name + version range)            | `Cargo.toml` of any `examples/` crate consuming the generated TI BSP            |
| Snapshot render tests                    | This chapter §12                                                 | `tests/bsp_ti_*_render.rs` (does not exist yet)                                 |
| `compile-verify` tests                   | This chapter §12                                                 | `tests/bsp_ti_*_compile.rs` (does not exist yet)                                |
| AM335x bring-up                          | `examples/beaglebone-black/src/bsp/` (hand-written prong)        | **NOT** consumed by CHIPS-TI generator (§10.1)                                  |
| MSPM0 bring-up                           | **None** — §11 non-goal at v0                                    | n/a                                                                             |
| C2000 / MSP430 / TDA SoC bring-up        | **None** — §11 non-goal at v0                                    | n/a                                                                             |
| Initiative prefix                        | This chapter §5.6                                                | execution PR commit subjects (`CHIPS-TI-NN[a-z]:`)                              |

## §5 Frozen decisions — enums & registration policy

Each frozen enum names its registration policy per the
*Frozen enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Chip IR YAML required fields — Standards Action

Every chipdb-generated TI chip YAML under
`chipdb/rlvgl-chips-ti/db/chips/<chip>.yaml` MUST contain at least
the following top-level keys. Additional vendor-specific keys MAY
be added under `vendor_ext:` (Specification Required, per §5.4).

```text
name           : human-friendly chip identifier (matches file stem)
arch           : one of { cortex-m4f, cortex-m0plus }  — restricted to §5.5
package        : human-friendly package code (e.g. RGZ, RKP)
pac_crate      : svd2rust-shaped PAC crate name (must publish on crates.io)
pac_crate_version : semver range pin (e.g. "^0.10")
target_triple  : rustc target triple (e.g. "thumbv7em-none-eabihf")
memory         : array of { name, base, size, access }  — same shape as ESP
clock_tree     : block with xtal_hz, sysclk_hz, and family-specific subfields
prcm           : peripheral clock-gate / reset table (TI analogue of ESP system_gates)
ioc            : per-pin function-select table (TI IO Controller; not CubeMX .ioc)
peripherals    : map of { name → { class, base, irq } }
```

Adding, renaming, or removing a top-level required key is
Standards Action and requires a §15 amendment to this chapter
**first**, in a separate PR.

### 5.2 Board IR YAML required fields — Standards Action

Every chipdb-generated TI board YAML under
`chipdb/rlvgl-chips-ti/db/boards/<board>.yaml` MUST contain at
least:

```text
name           : human-friendly board identifier (matches file stem)
chip           : exact chip name from §5.1 (lookup key into db/chips/)
flash_mb       : on-board flash in MB (or 0 for unused/external)
console        : { peripheral, baud }
pins           : array of { gpio, signal, peripheral?, direction, pull?, label }
```

Optional top-level blocks (Specification Required, per §5.4):

```text
i2c_configs    : { <peripheral_name> : { scl_hz } }       — same shape as ESP
spi_configs    : { <peripheral_name> : { clk_hz, mode } } — same shape as ESP
features       : free-form map keyed by board feature name  — same shape as ESP
```

Adding a top-level required key is Standards Action; adding an
optional block is Specification Required.

### 5.3 6-file template emission set — Standards Action

The set of files emitted per board MUST be exactly:

```text
mod.rs           (host crate-shaped module index)
pac.rs           (PAC re-export + init entry point)
clocks.rs        (PRCM ungate + reset-release sequence)
io_mux.rs        (IOC + GPIO matrix per-pin writes)
peripherals.rs   (per-peripheral init: console + I2C + SPI + …)
board.rs         (top-level board constants: XTAL_HZ, APB_HZ, etc.)
```

Plus the two linker-script templates:

```text
memory.x         (rust-embedded linker memory map)
chip.x           (chip-specific section additions, if any)
```

Adding, renaming, removing, or splitting any of these files is
Standards Action. The set matches the Espressif tree exactly so
that the generator's per-vendor `templates/` directory layout
stays uniform. Mid-stream divergence is a §15 amendment.

### 5.4 `vendor_ext:` registration — Specification Required

Per-chip and per-board YAML MAY carry a `vendor_ext:` map for
TI-specific data that does not fit the cross-vendor schema (e.g.
SimpleLink RF Core firmware patch table references). Adding,
renaming, or removing a `vendor_ext:` key requires an update to
the per-chapter walkthrough that *owns* that key, but does **NOT**
require a §0 / §5 amendment. Keys live as long as their walkthrough
chapter does.

### 5.5 SimpleLink Cortex-M4F initial member set — Standards Action

The frozen initial member set for CHIPS-TI-01 is:

```text
{ CC13x0, CC13x2, CC26x0, CC26x2, CC32xx }
```

Members are families, not individual part numbers. The
*first* part number to ship a chipdb YAML + generated BSP is
chosen by CHIPS-TI-01 and named in its §15 entry; this chapter
does not ratify that choice (see §1.1).

Adding a SimpleLink Cortex-M4F family member (e.g. a future
CC4x line) is Standards Action; adding a non-SimpleLink TI
family (e.g. MSPM0, AM62, C2000) is **NOT** an extension to
this set — those are separate initiatives that would each ratify
their own §5.5-shaped frozen member set.

### 5.6 Initiative prefix — Standards Action

`CHIPS-TI-NN[a-z]:` for execution PRs scoped to this initiative.
Matches the `DISCO-`, `BBB-`, `CREATOR-`, `CHIPS-ESP-`,
`CHIPS-STM-` convention named in CLAUDE.md.

### 5.7 Discipline scope — Specification Required

The STM32H747I-DISCO "Register-Mashing Discipline" in CLAUDE.md
applies to `platform/` and `examples/stm32h747i-disco/`. CHIPS-TI
generator output is **NOT** in scope of that discipline — generated
BSP code lives behind a typed svd2rust PAC and does not trigger the
discipline scanner's BASELINE. This matches the existing carve-out
for `examples/beetle-esp32c3/src/bsp_generated/`. Restating the
scope in any normative section of a CHIPS-TI execution PR is
defective; cite this clause instead.

## §6 Frozen decisions — target triples and PAC crate pins

The following table is **frozen** for CHIPS-TI-01. Adding a row is
Standards Action (§15 amendment). Modifying an existing row's
`target_triple` or upper bound on `pac_crate_version` is also
Standards Action — these values bind the `compile-verify` test
runtime environment and the `examples/` consumer's `Cargo.toml`.

| Chip family | arch          | target\_triple             | pac\_crate (prospective)        | pac\_crate\_version |
| ----------- | ------------- | -------------------------- | ------------------------------- | ------------------- |
| CC13x0      | cortex-m4f    | `thumbv7em-none-eabihf`    | `cc26x0` *(shared family PAC)*  | TBD: pin during CHIPS-TI-01a |
| CC13x2      | cortex-m4f    | `thumbv7em-none-eabihf`    | `cc26x2`                        | TBD: pin during CHIPS-TI-01a |
| CC26x0      | cortex-m4f    | `thumbv7em-none-eabihf`    | `cc26x0`                        | TBD: pin during CHIPS-TI-01a |
| CC26x2      | cortex-m4f    | `thumbv7em-none-eabihf`    | `cc26x2`                        | TBD: pin during CHIPS-TI-01a |
| CC32xx      | cortex-m4f    | `thumbv7em-none-eabihf`    | `cc3200` *(or successor)*       | TBD: pin during CHIPS-TI-01a |

**TBD: needs TI vendor PAC ecosystem audit during CHIPS-TI-01a.**
The crate names above reflect the historical TI svd2rust-shaped
crate naming. If the upstream maintainer has renamed or
consolidated these by the time CHIPS-TI-01a starts, the §15
amendment names the actual crate; this row is the working
hypothesis, not a contract.

## §7 Frozen decisions — template emission contract

The TI generator MUST emit the eight files of §5.3 from per-board
YAML, using the **same MiniJinja template tree shape** as the
Espressif tree:

```text
src/bin/creator/bsp/ti/
  ir.rs              -- TiIr struct (replaces the pass-through serde_yaml)
  templates/
    mod.rs.jinja
    pac.rs.jinja
    clocks.rs.jinja
    io_mux.rs.jinja
    peripherals.rs.jinja
    board.rs.jinja
    memory.x.jinja
    chip.x.jinja
```

Sibling-module references in generated code MUST use `super::`
so the emitted BSP works both as a standalone crate root and as
a child module of a host crate (this is the existing Espressif
convention; cite without restatement).

Per-template normative content for CHIPS-TI-01:

- **`clocks.rs`** MUST emit a `pub fn init()` that, for every
  peripheral named in the board's `pins:` and `console:` blocks,
  writes `MODULEMODE_ENABLE` (or the SimpleLink-PRCM equivalent)
  on that peripheral's CLKCTRL register and bounded-polls for the
  IDLEST=FUNC bit. **TBD: needs TI SWCU117 §4 PRCM register-bit
  cite during CHIPS-TI-01.** The pattern is established by the
  hand-written BBB `prcm::enable_*` functions
  (`examples/beaglebone-black/src/bsp/prcm.rs`); CHIPS-TI-01 ports
  that pattern to the SimpleLink PRCM register set.
- **`io_mux.rs`** MUST emit a `pub fn init()` that walks the
  board's `pins:` array and writes the IOC (IO Controller)
  PORT\_CFG entry per pin to select that pin's `signal` / `peripheral`
  / `direction` / `pull`. **TBD: needs TI SWCU117 §11 IOC register-
  bit cite during CHIPS-TI-01.**
- **`peripherals.rs`** MUST emit real init for the board's
  `console:` peripheral (UART, with baud derived from
  `clock_tree.apb_hz` and the board's console baud), and for any
  I2C / SPI peripherals listed under §5.2 optional blocks. All
  other peripherals receive a TODO stub. This matches the
  Espressif template's `peripherals_used` walk.
- **`board.rs`** MUST emit `pub const XTAL_HZ`, `pub const APB_HZ`,
  and any other clock constants derived from the chip's
  `clock_tree:` block.
- **`memory.x`** MUST emit linker-section directives for the chip
  IR YAML's `memory:` array. The shape matches the Espressif
  `memory.x.jinja` precedent.
- **`chip.x`** MUST emit any chip-specific section directives
  beyond the cortex-m-rt default. For the SimpleLink M4F family
  CHIPS-TI-01 will determine whether this is empty or carries
  e.g. `.ccfg` (chip configuration) section placement.
  **TBD: needs TI SWCU117 §6 cite during CHIPS-TI-01.**

The peripheral instance access style is fixed by §3 (cite without
restatement).

## §8 Frozen decisions — verification gates

The verification surface is **three layers** matching the existing
chipdb-vendor pattern:

1. **Snapshot render tests.** `tests/bsp_ti_<chip>_render.rs` runs
   the generator against each chipdb board YAML and snapshots the
   eight emitted files. Failure means a template change diverged
   without an `insta` snapshot update. Text-level only; does not
   prove the output type-checks.
2. **`compile-verify` tests.** `tests/bsp_ti_<chip>_compile.rs`
   materialises a throwaway cargo project around the generated
   BSP and runs `cargo check` against the real PAC crate on the
   chip's target triple. Gated by the existing `compile-verify`
   Cargo feature so it is opt-in (network access + rustup target
   install required for the target triple).
3. **`examples/` consumer crate.** At least one `examples/<board>/`
   crate consumes the generated BSP under `src/bsp_generated/`
   and demonstrates an LED blink (the
   `examples/beetle-esp32c3/src/bsp_pac_main.rs` precedent).

These three layers are **Standards Action**; demoting any of them
to "optional" or adding a fourth layer is a §15 amendment.

Hardware bring-up on the generated BSP is **MAY** at initiative
close — the chipdb family's conformance level is "render tests
MUST pass; `compile-verify` SHOULD pass; hardware MAY pass",
matching the CLAUDE.md "Conformance targets" example for
rlvgl-creator. CHIPS-TI-01 inherits this layering verbatim.

## §9 Frozen decisions — round-trip / regression posture

CHIPS-TI-NN execution PRs that modify a `.rs.jinja` template MUST
update the corresponding snapshot under `tests/bsp_ti_*_render.rs`
in the same PR. PRs that modify a chipdb YAML field MUST update
both the snapshot and (if the field affects emitted code) the
`compile-verify` expectation in the same PR.

This is the existing chipdb regression posture (cite without
restatement); restated here because CHIPS-TI-01 is the first
phase of a new vendor and its initial snapshots define the
baseline against which all subsequent CHIPS-TI-NN diff.

## §10 Reconciliation with adjacent repo primitives

The non-trivial coupling questions. Each item names the conflict
and the proposed resolution; resolution becomes binding only when
listed in §15 with a ratification date.

### 10.1 BSP generator vs. hand-written AM335x / BeagleBone Black prong

The single most load-bearing reconciliation in this chapter.

**Conflict.** The BeagleBone Black + NHD-7.0CTP-CAPE-P port
already does TI silicon bring-up (AM3358, Cortex-A8) in
`examples/beaglebone-black/src/bsp/` and
`docs/beaglebone-black/`. The chipdb crate
`chipdb/rlvgl-chips-ti/db/{chips,boards}/` has placeholder
`AM335x.yaml` + `beaglebone_black_nhd_cape.yaml` files referring
to that work. If CHIPS-TI-01 generated against AM335x, the
generator output would compete with the hand-written prong over
the same vocabulary (`reg_write` / `reg_read` / `enable_lcdc` /
the `DevMem` translate-mut abstraction).

**Resolution — proposed, ratifies in §15 once accepted.**

1. **AM335x is OUT of scope for CHIPS-TI-01.** The chipdb-driven
   generator SHALL NOT emit code targeting AM335x in v0 of this
   initiative. The placeholder YAML files at
   `db/chips/AM335x.yaml` and `db/boards/beaglebone_black_nhd_cape.yaml`
   remain as stubs and continue to satisfy the `am335x_chip_is_present`
   / `beaglebone_black_board_is_present` smoke tests in
   `chipdb/rlvgl-chips-ti/src/lib.rs`. They MUST NOT grow the
   `prcm:` / `ioc:` / `peripherals:` blocks required by §5.1
   until a separate initiative ratifies a chipdb-driven AM335x
   pipeline.
2. **`examples/beaglebone-black/src/bsp/` remains the authority**
   for AM335x bring-up. CHIPS-TI vocabulary in §3 (PRCM, IOC,
   peripheral instance access style) MUST NOT be retro-applied
   to the hand-written prong; the hand-written prong uses TI
   AM335x terminology directly (`CM_PER_*_CLKCTRL`,
   `MODULEMODE_ENABLE`, `reg_write(pa, val)`) as captured in
   `examples/beaglebone-black/src/bsp/prcm.rs`. Future BBB-NN PRs
   continue to operate inside `docs/beaglebone-black/`'s vocabulary
   without coordinating with CHIPS-TI-NN.
3. **The vocabulary boundary is the chip-family axis, not the
   register-naming axis.** A `CHIPS-TI-NN` PR touches
   SimpleLink Cortex-M4F (PRCM register block at SimpleLink
   addresses, M4F target triple, svd2rust-shaped PAC). A
   `BBB-NN` PR touches AM335x Cortex-A8 (different PRCM register
   block at AM335x addresses, A8 target / userspace `/dev/mem`
   bare-metal split, no upstream svd2rust-shaped PAC). The two
   never collide on a peripheral register because they never
   reference the same silicon.
4. **No future PR may unify the two paths without a separate
   ratified initiative.** Proposing a chipdb-driven AM335x BSP,
   or proposing that the BBB hand-written code consume generator
   output, is **explicitly out of scope for both CHIPS-TI-NN and
   BBB-NN execution PRs**. Such a unification would constitute a
   new cross-cutting initiative under
   CLAUDE.md §"Spec-Before-Code Planning Discipline" and require
   its own §0 / §5 / §10 / §15 cycle.

This carve-out is structurally identical to the
`docs/app-schema/00-concepts.md` §10.1 carve-out that protects
the hand-written `platform/src/stm32h747i_disco.rs` from being
silently overwritten by chipdb generation. The same shape
protects the hand-written BBB prong here.

### 10.2 TI PRCM vs. Espressif SYSTEM / PCR / HP\_SYS\_CLKRST

The Espressif generator emits `clocks.rs` against one of three
system register blocks depending on chip (SYSTEM for C3, PCR for
C6, HP\_SYS\_CLKRST for P4). The SimpleLink Cortex-M4F family
emits against **one** register block — PRCM — with no
chip-internal variation in block name across the §5.5 member set.

**Proposed.** The `clocks.rs.jinja` template MAY assume the block
name `PRCM` is the single source of clock-gating across all
§5.5 member-set chips. If a future chip joins §5.5 with a different
block name, that chip's §15 entry MUST name the new block and the
template MAY add a per-chip switch (analogous to the Espressif
SYSTEM/PCR/CLKRST switch). Until that day, the template is
single-block.

### 10.3 TI IOC (IO Controller) vs. Espressif IO MUX + GPIO matrix

Espressif chips have a **two-stage** pin routing model: IO MUX
selects per-pin "function 0..N" (peripheral instance access), and
the GPIO matrix optionally re-routes peripheral signals to
arbitrary GPIOs. SimpleLink Cortex-M4F is **single-stage** — the
IOC writes a per-pin PORT\_CFG entry that names the peripheral
signal directly; there is no separate matrix.

**Proposed.** The `io_mux.rs.jinja` template MUST NOT assume a
GPIO-matrix second stage. The TI flavour of the template emits
only the IOC stage (per-pin PORT\_CFG write). The template
filename remains `io_mux.rs.jinja` for cross-vendor uniformity
even though the TI emitted module's *content* is IOC-specific.
Renaming the file requires a §15 amendment that updates §5.3
and §7 in lockstep across all vendor trees.

### 10.4 `tools/afdb` STM32 XML pipeline

The `tools/afdb` package (and its `chipdb/rlvgl-chips-stm/`
consumer) ingests STM32 vendor XML into canonical JSON. There is
**no TI analogue** of `STM32_open_pin_data` — TI does not
distribute a comparable vendor XML corpus on a permissive
licence. CHIPS-TI YAML is therefore **hand-authored from the TRM**,
not scraped from vendor XML.

**Proposed.** No CHIPS-TI-NN PR MAY introduce a TI-equivalent
`tools/afdb`-style scraper in v0 of this initiative. If a future
phase wants automated chipdb generation from TI SDK files (e.g.
TI Driverlib `Board.c` or SysConfig `.syscfg` exports), it
constitutes a separate initiative under the
Spec-Before-Code planning discipline.

### 10.5 Discipline scope vs. CHIPS-TI

Per §5.7, generated CHIPS-TI BSP code is **OUT** of scope of the
STM32H747I-DISCO "Register-Mashing Discipline" scanner. This is
not a per-PR carve-out — it is a chipdb-wide carve-out that the
Espressif tree already enjoys. CHIPS-TI-NN PRs MUST NOT modify
`platform/tests/discipline.rs` or its BASELINE.

### 10.6 Application Schema (`docs/app-schema/`) consumption

The Application Schema chapter
([`docs/app-schema/00-concepts.md`](../../../docs/app-schema/00-concepts.md))
§5.2 cites the chipdb vendor set
`{esp, stm, ti, nxp, nrf, renesas, silabs, rp2040, microchip}` by
reference. Once CHIPS-TI-01 ratifies, app-schema execution PRs
MAY consume the TI vendor key without further coordination.
This chapter does NOT redefine the vendor set; it inhabits it.

## §11 Non-goals

Explicit out-of-scope for v0 of this initiative. Each item carries
a resurrection-prevention note so a future agent does not
re-derive a path that was already rejected.

- **MSP430 16-bit MCUs.** Different ISA (MSP430 ISA, not Cortex-M);
  different toolchain (msp430-elf); different PAC story (no
  established svd2rust-shaped crate). Adding MSP430 would require
  a parallel template tree from scratch.
  *Rationale for resurrection:* would require ratifying a second
  `target_triple` set, a second PAC ecosystem audit, and a
  second `compile-verify` toolchain.
- **C2000 DSP family (F2806x, F2837x, F2838x, F28004x).** Different
  ISA (C28x), no rustc support, and `cargo check` against a real
  PAC is not currently achievable. Even within the rustc-supported
  M4F coprocessor on dual-core C2000 parts, the architecture is
  closer to a Cortex-M4 + DSP heterogeneous SoC than to SimpleLink
  and would not benefit from the SimpleLink-shaped templates.
  *Rationale for resurrection:* rustc support is the blocker.
- **Sitara AM3 / AM5 / AM6 (including AM335x / AM3358).** Covered
  by the hand-written prong under
  `examples/beaglebone-black/`. See §10.1 for the boundary contract.
  *Rationale for resurrection:* would require a separate
  initiative that ratifies the chipdb ↔ hand-written prong
  unification.
- **TDA SoCs (TDA2 / TDA4VM family).** Heterogeneous SoCs with
  DSP + Cortex-A + Cortex-R + accelerators; no svd2rust-shaped
  PAC; not a single-target chipdb shape.
  *Rationale for resurrection:* not a single-target shape.
- **MSPM0 Cortex-M0+ line.** Newer family with thinner crates.io
  PAC coverage at draft time. Considered as alternative initial
  scope in §1.1; rejected for CHIPS-TI-01 on PAC-maturity grounds.
  *Rationale for resurrection:* re-audit PAC ecosystem before
  proposing a CHIPS-TI MSPM0 phase.
- **SimpleLink Cortex-R5F variants (TMS570 / RM48 family).**
  Different ISA in practice (FPv5-SP, dual-core lockstep), and
  not customarily considered "SimpleLink Cortex-M4F".
- **Vendor-XML scraping for TI parts.** See §10.4.
- **Hardware bring-up on the generated TI BSP.** May follow at
  initiative close per §8; not required for ratification of
  any individual CHIPS-TI-NN phase.
- **Unifying the chipdb-generated path with the BBB hand-written
  prong.** Explicitly forbidden in §10.1.

## §12 Acceptance checklist

This concepts chapter is ratified (§15 entry dated) when:

- [ ] §0 authority table reviewed; every cited TI TRM part number
      verified against an accessible copy (memalpha notebook or
      vendor portal).
- [ ] §3 glossary terms each carry a cite-vs-restate marker.
- [ ] §4 source-of-truth map has exactly one owner per row; the
      AM335x and MSPM0 rows resolve to **None** per §11.
- [ ] §5.1 chip-IR required field set reviewed against the
      Espressif precedent.
- [ ] §5.2 board-IR required field set reviewed against the
      Espressif precedent.
- [ ] §5.3 template emission set confirmed identical to the
      Espressif tree (cross-vendor uniformity).
- [ ] §5.5 SimpleLink Cortex-M4F member set ratified; first chip
      under that umbrella is deferred to CHIPS-TI-01a.
- [ ] §6 target-triple table confirmed; PAC crate version pins
      remain `TBD: pin during CHIPS-TI-01a` and are not
      load-bearing for this chapter's ratification.
- [ ] §10.1 AM335x reconciliation reviewed by the BBB initiative
      owner; the carve-out is mutually accepted (no future CHIPS-
      TI-NN PR modifies `examples/beaglebone-black/` and no
      future BBB-NN PR modifies the TI chipdb under `db/chips/`
      or `db/boards/`).
- [ ] §11 non-goals each carry a resurrection-prevention note.
- [ ] §15 has a dated ratification entry signed off by the
      initiative owner.

CHIPS-TI-01 (the first execution phase) is unblocked by
ratification of §3, §5, §6, §7, §8, §10.1.

The acceptance gates that CHIPS-TI-01 itself must clear are
named in §8 (snapshot render tests + `compile-verify` +
`examples/` consumer crate). CHIPS-TI-01 does NOT inherit
acceptance gates from this chapter beyond ratification.

## §13 Files cited

- [`CLAUDE.md`](../../../CLAUDE.md) — Spec-Before-Code Planning
  Discipline, RFC 2119 normative keywords, registration policy,
  initiative-prefix convention; Register-Mashing Discipline (§5.7).
- [`docs/concepts/DCB-00-CONCEPTS.md`](../../../docs/concepts/DCB-00-CONCEPTS.md)
  — reference shape for §0 authority policy table and §10
  reconciliation block.
- [`docs/app-schema/00-concepts.md`](../../../docs/app-schema/00-concepts.md)
  — reference shape for §3 cite-vs-restate glossary markers and
  §10.1 carve-out pattern; cited by §10.6.
- [`docs/beaglebone-black/README.md`](../../../docs/beaglebone-black/README.md)
  — AM335x hand-written prong owner; cited by §10.1 and §11.
- [`docs/bsp/CHIP-SUPPORT.md`](../../../docs/bsp/CHIP-SUPPORT.md)
  — vendor IR status table; cited by §2.
- [`chipdb/rlvgl-chips-ti/src/lib.rs`](../src/lib.rs) — current TI
  chipdb crate API.
- [`chipdb/rlvgl-chips-ti/db/chips/AM335x.yaml`](../db/chips/AM335x.yaml)
  — placeholder; cited by §2 and §10.1.
- [`chipdb/rlvgl-chips-ti/db/chips/MSP432P401R.yaml`](../db/chips/MSP432P401R.yaml)
  — placeholder; cited by §2.
- [`chipdb/rlvgl-chips-ti/db/boards/beaglebone_black_nhd_cape.yaml`](../db/boards/beaglebone_black_nhd_cape.yaml)
  — placeholder; cited by §2 and §10.1.
- [`chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml`](../../rlvgl-chips-esp/db/chips/esp32c3.yaml)
  — reference shape for §5.1 chip-IR required field set.
- [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml`](../../rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml)
  — reference shape for §5.2 board-IR required field set.
- [`src/bin/creator/bsp/ti.rs`](../../../src/bin/creator/bsp/ti.rs)
  — current TI adapter; cited by §2.
- [`src/bin/creator/bsp/espressif/templates/`](../../../src/bin/creator/bsp/espressif/templates/)
  — reference template tree; cited by §3 and §7.
- [`examples/beaglebone-black/src/bsp/`](../../../examples/beaglebone-black/src/bsp/)
  — hand-written AM335x bring-up; cited by §3, §10.1, and §11.
- [`examples/beaglebone-black/src/bsp/prcm.rs`](../../../examples/beaglebone-black/src/bsp/prcm.rs)
  — `enable_*` pattern; cited by §7.
- TI SWCU117 — CC13x0/CC26x0 TRM; authority for SimpleLink PRCM
  (§3, §7) and IOC (§3, §7) register blocks.
- TI SWCU185 — CC13x2/CC26x2 TRM; cited by §0 and §5.5.
- TI SWRU367 — CC3220 TRM; cited by §0 and §5.5.
- TI SPRUH73Q — AM335x TRM; cited by §0 and §10.1 (out of scope).
- TI SLAU893 / SLAU847 — MSPM0 TRMs; cited by §0 and §11 (out of scope).

## §14 Unblocks

Ratifying this chapter unblocks:

- `CHIPS-TI-01` — first SimpleLink Cortex-M4F chip + board YAML
  + `TiIr` adapter + 8-file template tree. Sub-letters:
  - `CHIPS-TI-01a` — pin PAC crate names + versions in §6;
    populate one chip's YAML against TI SWCU117 / SWCU185 /
    SWRU367 (whichever family the chosen chip belongs to).
  - `CHIPS-TI-01b` — port the 6 + 2 templates from the Espressif
    tree to `src/bin/creator/bsp/ti/templates/`, with the
    SimpleLink PRCM / IOC adaptations named in §10.2 and §10.3.
  - `CHIPS-TI-01c` — snapshot render tests
    `tests/bsp_ti_<chip>_render.rs`.
  - `CHIPS-TI-01d` — `compile-verify` test
    `tests/bsp_ti_<chip>_compile.rs`.
  - `CHIPS-TI-01e` — `examples/<board>/` consumer crate
    demonstrating LED blink against the generated BSP.
- `CHIPS-TI-02` and later — additional chips within the §5.5 set;
  shape ratified by CHIPS-TI-01.

No `BBB-NN` PR is blocked or unblocked by ratification of this
chapter. The BBB initiative continues independently per §10.1.

## §15 Change log

| Date       | Status | Note                                                                                                                                                            |
| ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-11 | Ratified (owner: Ira Abbott) | Doc *shape* ratified. `CHIPS-TI-NN[a-z]:` PRs MAY now cite §-numbers as frozen authority. Open TBDs (§6 PAC crate version pins, §7 TI SWCU117 register-bit cites) remain open and gate `CHIPS-TI-01a` chip-YAML population rather than this doc. AM335x carve-out per §10.1 stands: this initiative does NOT subsume the BBB hand-written prong. |
| 2026-05-11 | DRAFT — awaiting ratification | Initial skeleton. Initial scope: SimpleLink Cortex-M4F per §1.1; AM335x carve-out resolved in §10.1; MSPM0 / Sitara / C2000 / MSP430 / TDA non-goals in §11. |

### 2026-05-13 — TI-01b template amendment for cc13x2_26x2_pac 0.10

- Templates `clocks.rs.jinja`, `io_mux.rs.jinja`, `peripherals.rs.jinja`
  now emit **lowercase** peripheral field accessors (`p.prcm`, `p.ioc`,
  `p.gpio`, `p.uart0`) instead of the originally-frozen uppercase form
  (`p.PRCM`, `p.IOC`, `p.GPIO`, `p.UART0`). This matches
  `cc13x2_26x2_pac 0.10`'s pre-uppercase svd2rust-era output. The
  `pac::Peripherals` *type* stays capitalized — only field-access on
  the `p` instance changes. This row supersedes the "Peripheral
  instance access style" rule in §3 and §5 for the SimpleLink
  Cortex-M4F family; the original style was inherited from the
  Espressif precedent and assumed a newer svd2rust version than
  `cc13x2_26x2_pac 0.10` was generated against.
- The `bsp_ti_cc1352r_render` snapshot test was re-blessed; the
  diffs are limited to the three files above. `chip_x`, `memory_x`,
  `mod.rs`, `pac.rs`, `board.rs` snapshots were unaffected.
- The `bsp_ti_cc1352r_compile` test gate (CHIPS-TI-01d) advanced
  from ~40+ casing errors to 18 remaining errors. **The casing
  amendment alone is not sufficient to make the gate pass.** Three
  additional `cc13x2_26x2_pac 0.10` divergences remain, all
  structural rather than cosmetic:
    1. `pac::ioc::Ioc` exposes per-DIO methods (`iocfg0()`,
       `iocfg1()`, ... `iocfg31()`) rather than an indexed
       `iocfg(n)` accessor. `io_mux.rs.jinja` currently emits
       `p.ioc.iocfg({{ pin.dio }})` which has no analogue in
       this PAC and needs a per-pin codegen branch.
    2. `pac::prcm::uartclkgr::CLK_EN` is a 2-bit enum field
       (`ClkEn::Uart0`/`Uart1`) not a single bit; the template's
       `.clk_en().set_bit()` does not type-check. Real bring-up
       needs `.clk_en().uart0()` / `.uart1()` driven by chipdb
       data.
    3. `pac::prcm::resetuart::W` field is `uart0` (per TI SWCU185
       UART instance numbering), not the generic `uart` named in
       the chip YAML `prcm.resetuart.rst_field` entry. Same
       pattern for `reseti2c.i2c` -> `.i2c0`. This may be a
       chip-YAML data fix rather than a template fix.
  These belong to a follow-up worker (CHIPS-TI-01e or similar) with
  scope to re-shape the IOC/PRCM template structure and audit the
  chipdb YAML field-name conventions against the actual PAC.
  `bsp_ti_cc1352r_compile` therefore remains expected-FAIL and the
  pre-publish bullet for it stays commented out.
- No other initiative was touched. CLAUDE.md was not modified.
  Sister initiatives `CHIPS-SILABS-01b` and `CHIPS-MICROCHIP-01b`
  remain on independent parallel paths.

### 2026-05-13 — TI-01e three structural amendments for cc13x2_26x2_pac 0.10

- **iocfg per-DIO methods**: `io_mux.rs.jinja` now emits
  `p.ioc.iocfg{{ pin.dio }}()` (per-DIO method form) instead of
  `p.ioc.iocfg({{ pin.dio }})` (indexer call). `cc13x2_26x2_pac::ioc::Ioc`
  exposes `iocfg0()`..`iocfg31()` accessors directly, with no
  `iocfg(n)` indexer — this matches the pre-indexer-API svd2rust
  output era the PAC was generated under.
- **`uartclkgr.clk_en` enum FieldWriter**: `clocks.rs.jinja` now
  branches on a new optional chip-YAML field `clk_en_variant`. When
  present, the template emits `w.{{ clk_en_field }}().{{ clk_en_variant }}()`
  (e.g. `w.clk_en().uart0()`); when absent, it falls back to
  `.set_bit()` for single-bit BitWriter fields. `TiPrcmGate` in
  `src/bin/creator/bsp/ti/ir.rs` gained the corresponding
  `Option<String>` field. The matrix of which `*CLKGR` registers
  carry an enum FieldWriter in `cc13x2_26x2_pac 0.10` is:
    * Enum (variants encode the instance):
      `uartclkgr.clk_en` (`Uart0`/`Uart1`),
      `ssiclkgr.clk_en` (`Ssi0`/`Ssi1`),
      `gptclkgr.clk_en` (`Gpt0`/`Gpt1`/`Gpt2`/`Gpt3`).
    * BitWriter (`set_bit`/`clear_bit`):
      `i2cclkgr.clk_en`, `i2sclkgr.clk_en`, `gpioclkgr.clk_en`, and
      the per-class `secdmaclkgr.{crypto,trng,pka,dma}_clk_en`.
  This is encoded data-side in `CC1352R.yaml` rather than template
  conditionals, so future SimpleLink chips can carry their own
  PAC-vintage matrix in the same shape.
- **PRCM reset-register field naming (chip-YAML fix)**: Fixed the
  `prcm:` block in `chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml` so
  `rst_field` matches the PAC's actual writer-method names. The
  fixes split into two classes:
    * Per-instance bits in the reset register: `resetuart.uart0` /
      `resetuart.uart1` — `uart0` / `uart1` rst_field (was the
      generic `uart`).
    * Single shared reset bit per peripheral class:
      `resetssi.ssi`, `reseti2c.i2c`, `reseti2s.i2s`, `resetgpio.gpio`,
      `resetgpt.gpt`, `resetsecdma.{crypto,trng,pka,dma}` — chip
      YAML now names the correct register (`reseti2s`, not the
      bogus `resetaudio`; `resetsecdma`, not the bogus `resetsec`)
      and the actual single-field name from the PAC. None of the
      currently-used peripherals on `launchxl_cc1352r1` exercise
      these (only `uart0` and `i2c0` are touched at compile-verify
      time), but the corrected YAML is now accurate for sibling
      board YAMLs that may use SSI / I2S / GPT.
  Chip-YAML was preferred over template-side stringification because
  the reset-register topology is genuinely *data* — it varies per
  PAC vintage and per chip member (CC2640R may differ from CC1352R).
  Template-side instance-suffix derivation would hard-code one PAC
  family's convention.
- `bsp_ti_cc1352r_compile` test gate (CHIPS-TI-01d) now **PASSES**
  on `thumbv7em-none-eabihf` against `cc13x2_26x2_pac 0.10.3`.
  Snapshots re-blessed; render test (CHIPS-TI-01c) green; all 13
  cases pass. `chip_x`, `memory_x`, `mod.rs`, `pac.rs`, `board.rs`,
  `peripherals.rs` snapshots were unaffected — only `clocks.rs`
  and `io_mux.rs` diff against -01b.
- No other initiative touched. CLAUDE.md was not modified — the PM
  will uncomment the Phase 4.7d `bsp_ti_cc1352r_compile` bullet in
  the next slate. Sister initiatives `CHIPS-SILABS-02b` and the
  Microchip post-promotion path remain on independent parallel
  workers.

### 2026-05-14 — CHIPS-TI-05 ratified

Linker emission chapter ratified at
[`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md). Closes the §11
deferral. The 8-file emission set (6 .rs + memory.x + cc1352_r.x)
that has shipped since slates 4-5 is now backed by a normative
spec; future linker-script behaviour changes route through CHIPS-TI-05's
§15 amendment process.

### 2026-05-14 — CHIPS-TI-06 ratified and v0 scaffold landed

Example crate chapter ratified at
`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-06-EXAMPLE.md` and v0 scaffold
shipped at `examples/launchxl-cc1352r1/`. The `bsp_pac` feature path
consumes the slate-9 BSP output (8-file emission set) and links
against the slate-9 linker scripts. `cargo check --target
thumbv7em-none-eabihf` passes; LED blink + UART hello-world deferred
to -06a / -06b.

### 2026-05-14 — CHIPS-TI-06a LED blink shipped

`examples/launchxl-cc1352r1/src/bsp_pac_main.rs` now drives DIO_6
(LED_RED) in a busy-wait toggle loop using cortex_m::asm::delay for
timing. Validates the slate-10 BSP integration end-to-end: clocks +
io_mux + peripherals init + a real GPIO write succeeds against the
slate-9 linker scripts. UART hello-world deferred to -06b; rlvgl
widget tree deferred to -06c.

The slate-9 `io_mux::init()` already routes DIO_6 to plain GPIO
(IOC.IOCFG6 port_id=0x00) and sets DOE31_0 bit 6, so main only flips
DOUTSET31_0 / DOUTCLR31_0 via the per-DIO bit-field writers
(`p.gpio.doutset31_0().write(|w| w.dio6().set_bit())`). The PAC
import path is `use cc13x2_26x2_pac as pac;` at the binary scope;
re-using the generator's `pac.rs` module would have required a
double-segment `pac::pac::Peripherals` path because pac.rs itself
performs `pub use cc13x2_26x2_pac as pac;`. Direct PAC import is
simpler and matches the chipdb yaml's `cc13x2_26x2_pac` declaration.

`cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml
--target thumbv7em-none-eabihf` passes.

### 2026-05-14 — CHIPS-TI-06b UART0 hello-world shipped

`examples/launchxl-cc1352r1/src/bsp_pac_main.rs` now sends "hello\r\n"
over UART0 (LAUNCHXL VCOM via XDS110 bridge) before entering the
slate-11 LED toggle loop. Validates the slate-7 -01e `clk_en` enum
FieldWriter amendment end-to-end: the generated peripherals::init()
turns on the UART0 clock-gate using the `ClkEn::Uart0` variant, and
the consumer code can then poll FR.TXFF + write DR successfully.
rlvgl widget tree deferred to -06c.

### 2026-05-15 — CHIPS-TI-07 generator pac.rs re-export flatten

`pac.rs.jinja` now emits `pub use cc13x2_26x2_pac::*;` instead of
`pub use cc13x2_26x2_pac as pac;`. Consumer-side paths reach
`Peripherals` via `bsp_generated::<board>::pac::Peripherals`
(single-level) instead of the slate-11 workaround of importing the
PAC crate directly to avoid `bsp_generated::<board>::pac::pac::Peripherals`
(double-nest). `examples/launchxl-cc1352r1/src/bsp_pac_main.rs`
updated to use the clean path.

### 2026-05-15 — CHIPS-TI-05a CCFG byte-count comment correction

`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml` linker section comment
corrected. The emitted `ccfg_length: 0x58` (88 bytes per SWCU185G
§11.1 Table 11-1) is the CCFG **structure** size; it resides within
the last Flash sector but does not span it. The previous comment
conflated CCFG structure size with the Flash erase-sector size and
asserted "4 KB" which was misleading. No emitted value changes.
