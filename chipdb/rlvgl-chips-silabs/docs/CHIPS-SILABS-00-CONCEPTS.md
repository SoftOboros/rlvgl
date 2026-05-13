<!--
CHIPS-SILABS-00-CONCEPTS.md - Silicon Labs chipdb concepts & vocabulary.
Status: Ratified 2026-05-11 (owner: Ira Abbott). See §15 change log.
-->

# CHIPS-SILABS-00 — Silicon Labs chipdb Concepts & Vocabulary

**Status:** Ratified 2026-05-11 (owner: Ira Abbott). See §15.
`CHIPS-SILABS-NN[a-z]:` execution PRs MAY cite this chapter as
a frozen authority. Amendments require a new dated §15 entry
and the same review depth as the original ratification pass.

## 0. Authority policy

This chapter follows the spec-before-code planning discipline
declared in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code
Planning Discipline". RFC 2119 / RFC 8174 normative keywords
(**MUST**, **MUST NOT**, **SHOULD**, **MAY**, etc.) carry their
RFC meanings when capitalised; lowercase use is narrative.

This chapter is the **single normative source** for the
`rlvgl-chips-silabs` crate's chip + board YAML schema and for the
contract a future `rlvgl-creator bsp from-yaml --vendor silabs`
emission path will follow. It does not redocument Silicon Labs
reference manuals; it cites them. The authority split:

| Concern | Owner | CHIPS-SILABS-side relationship |
|---|---|---|
| Silicon Labs **Series 2** (EFR32xG2x) Cortex-M33 architectural semantics, SE / EM clock domains, CMU branch tree, DPLLOC digital peripheral allocator | EFR32 Series 2 Reference Manuals (per-family: `efr32mg21`, `efr32bg22`, `efr32fg23`, `efr32mg24`, `efr32bg27`, etc.) and the corresponding device data sheets | Cited; not redocumented. CHIPS-SILABS assumes architectural defaults (32-bit ARMv8-M Mainline, single-core, optional FPU + DSP, TrustZone-M present but unused at v0). |
| Silicon Labs **EFM32 Series 0/1** (Cortex-M3/M4) CMU, ROUTE/ROUTELOC GPIO routing, EM0–EM4 energy modes | EFM32 family Reference Manuals (Giant Gecko `EFM32GG11`, Pearl Gecko `EFM32PG12`, Tiny Gecko `EFM32TG11`, etc.) and corresponding data sheets | Cited; not redocumented. CHIPS-SILABS treats EFM32GG11 (already seeded in `db/chips/EFM32GG11.yaml`) as the canonical Series 1 reference; other Series 0/1 families enter via amendment. |
| Per-chip PAC crate (svd2rust-derived register access) | The upstream Rust PAC the chipdb names in each chip YAML's `pac_crate:` field | Consumed via published API. CHIPS-SILABS does not generate the PAC; it generates code that calls it. Pinned PAC versions are listed per chip in §4. |
| Vendor SDK headers (`em_cmu.h`, `em_gpio.h`, `em_usart.h`, etc.) | Silicon Labs Gecko SDK (a.k.a. EMLIB) | **Not consumed.** EMLIB is C; CHIPS-SILABS targets raw PAC access in Rust per the ESP vendor precedent (`chipdb/rlvgl-chips-esp`). EMLIB is cited only as cross-reference for verifying register field meanings. |
| Chip + board YAML grammar shape (top-level fields, peripheral instance shape, IO routing entries) | This chapter §5 | Owned here. The grammar mirrors `chipdb/rlvgl-chips-esp` and `chipdb/rlvgl-chips-nrf` for cross-vendor consistency; per-vendor extensions are §5 frozen-enum entries. |
| BSP code emission contract (6-file output: `mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`, `board.rs`) | `rlvgl-creator bsp` family (currently `src/bin/creator/bsp/espressif/templates/`); SiLabs sibling templates land in a future `silabs/templates/` directory | This chapter §6 freezes the file set; per-template register-level content is owned by the future `silabs/templates/*.rs.jinja` once `CHIPS-SILABS-02` lands. |
| Initiative prefix and registration policy for chipdb-vendor additions | `CLAUDE.md` §Spec-Before-Code §Frozen-enumerations / chipdb-vendor enum row | Cited; this chapter delegates. |

If a phase needs to **modify** a cited authority (e.g. to adopt a
different `pac_crate:` for a chip, to add a new IO MUX register
class for a future Series 3 part, or to extend the 6-file
emission set to seven files), the modification ratifies in a §15
amendment **first** before the consumer phase lands.

## 1. Purpose

Bring `rlvgl-chips-silabs` to parity with the other five
ratified vendor adapters (`chipdb/rlvgl-chips-esp`,
`chipdb/rlvgl-chips-nrf`, `chipdb/rlvgl-chips-nxp`,
`chipdb/rlvgl-chips-rp2040`, `chipdb/rlvgl-chips-renesas`) so
that:

1. A board author can drop a `db/chips/<chip>.yaml` and
   `db/boards/<board>.yaml` pair into the crate and have
   `rlvgl-creator bsp from-yaml --vendor silabs --board <board>`
   emit a buildable BSP skeleton.
2. The emitted BSP type-checks against a real Silicon Labs PAC
   crate on the chip's target triple (`thumbv8m.main-none-eabihf`
   for Series 2, `thumbv7em-none-eabihf` for EFM32 Cortex-M4F,
   `thumbv7m-none-eabi` for legacy Cortex-M3 parts).
3. Snapshot tests under `tests/bsp_silabs_*` exist and pass for
   each board YAML committed under `db/boards/`.

This phase produces **no executable artifacts and no Rust code**.
It establishes the contract that `CHIPS-SILABS-01` (chip + board
YAML inventory expansion), `CHIPS-SILABS-02` (template emission
plumbing), `CHIPS-SILABS-03` (snapshot tests), and
`CHIPS-SILABS-04` (optional `compile-verify` against a real PAC
crate) build on.

## 2. Problem statement

Evidence the parity gap is real, pinned to repo paths:

- **Stub status.** `chipdb/rlvgl-chips-silabs/` ships today with
  the boilerplate-only adapter (`src/lib.rs`, `build.rs`) plus a
  single placeholder pair: `db/chips/EFM32GG11.yaml` and
  `db/boards/EFM32GG11.yaml`. The placeholder chip YAML has only
  four scalar fields (`name`, `arch`, `package`, `pac_crate`)
  and no `memory:`, `clock_tree:`, `peripherals:`, `io_mux:`, or
  `gpio_matrix:` sections. The placeholder board YAML has
  `pins: []`. By contrast `chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml`
  is 320 lines and includes a full IO MUX + GPIO matrix table.
- **Generator routing.** `src/bin/creator/bsp/silabs.rs` is a
  17-line stub that calls `serde_yaml::from_str::<Ir>` directly.
  There is no per-vendor `src/bin/creator/bsp/silabs/` directory
  paralleling `src/bin/creator/bsp/espressif/` — no `load.rs`,
  no `templates/`, no peripheral-class router.
- **Pre-publish phase pressure.** `CLAUDE.md` Phase 4.6 / 4.7 /
  4.7b lists `bsp_esp32c3_compile`, `bsp_esp32p4_render`,
  `bsp_esp32c6_render`, `bsp_esp32c5_render`,
  `bsp_esp32h2_render`, `bsp_esp32c61_render` — eight Espressif
  chips' render tests. There is no equivalent
  `bsp_efm32gg11_render` (let alone `bsp_efr32mg24_render`)
  test family. Future maintainers wiring the SiLabs path in
  ad-hoc form will diverge from the ESP/Nordic shape unless this
  chapter freezes the contract first.
- **Series 2 divergence risk.** Silicon Labs Series 2 introduces
  a digital peripheral allocator (DPLLOC) that has **no analogue
  on ESP, Nordic, or NXP**. A naive port of the ESP `io_mux`
  template will not capture it; a naive port of the Nordic
  `route` template will miss the per-port routing latch. The
  divergence MUST be acknowledged in §10 before any template
  code is written, otherwise the SiLabs `io_mux.rs.jinja` will
  pick up either ESP's GPIO-matrix shape or Nordic's PSEL shape
  silently and lock that mistake in.

The cost of *not* freezing this contract is that the next
behaviour PR touching `rlvgl-chips-silabs` will either restate
existing chipdb vocabulary (silent drift) or invent SiLabs-only
vocabulary without crosswalk to ESP / Nordic (parallel forks
across the five vendors).

## 3. Canonical glossary

Reserved CHIPS-SILABS vocabulary. Capitalised use of these terms
in CHIPS-SILABS docs MUST refer to the defined meaning;
alternative phrasings introduce drift and are forbidden in
normative sections. Each term carries a cite-vs-restate marker
per the *Definitions — reference vs. restatement* rule in
`CLAUDE.md`.

| Term | Meaning | Owner |
|---|---|---|
| **Chip YAML** | A document under `chipdb/rlvgl-chips-silabs/db/chips/<chip>.yaml` describing one Silicon Labs SoC's memory map, clock tree, peripheral instances, and IO routing tables. The file stem is the canonical chip id. | *As defined in* the existing chipdb convention (`chipdb/rlvgl-chips-esp/db/chips/*.yaml`); *used without modification* for the file-naming + lookup contract. SiLabs-specific schema fields are §5 frozen-enum additions. |
| **Board YAML** | A document under `chipdb/rlvgl-chips-silabs/db/boards/<board>.yaml` naming a chip and a pin-by-pin assignment of the chip's peripheral signals to physical package pins. | *As defined in* the existing chipdb convention (`chipdb/rlvgl-chips-esp/db/boards/*.yaml`); *used without modification*. |
| **Chip id** | The `name:` field of a chip YAML. SHOULD match the silkscreen / orderable-part-number stem (e.g. `EFM32GG11`, `EFR32MG24`, `EFR32BG22`). MUST be unique within the crate. | Owned by this chapter; first reference in repo `db/chips/EFM32GG11.yaml`. |
| **Board id** | The `name:` field of a board YAML. SHOULD match the vendor's marketed board name with vendor-prefix stripped (e.g. `EFM32GG11_STK` for the SLSTK3701A Giant Gecko 11 starter kit; `BRD4002A` for the WPK mainboard). | Owned by this chapter. |
| **PAC crate** | The Rust `svd2rust`-derived peripheral access crate named by a chip YAML's `pac_crate:` field. CHIPS-SILABS does not generate or vendor PACs; it generates code that calls them. | *Owned by* each upstream PAC's maintainer; *cited* by chip YAML. |
| **6-file emission set** | The fixed output produced by `rlvgl-creator bsp from-yaml --vendor silabs`: `mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`, `board.rs`. See §6. *Linker scripts (`memory.x`, `chip.x`)* are NOT in this set at v0 — they are §11 deferred work. | Owned by this chapter; mirrors the ESP set defined in `src/bin/creator/bsp/espressif/templates/`. |
| **CMU** | Clock Management Unit. The Silicon Labs peripheral that owns clock source selection, branch enables, and per-peripheral clock gating across Series 0/1/2. CMU register layout differs between Series 0/1 (single-register CMU_HFPERCLKEN0) and Series 2 (`CMU.CLKEN0/CLKEN1` + per-peripheral `CMU.<periph>CLKCTRL`). | *As defined in* per-family Silicon Labs RMs (EFM32GG11 RM §11, EFR32MG24 RM §8); *cited*. |
| **HFCLK / HFXO / HFRCO** | High-frequency clock tree roots. HFXO is the external crystal; HFRCO is the internal RC oscillator; HFCLK is the post-mux high-frequency root that feeds the CPU and most peripherals. Series 2 renames these to SYSCLK / HFXO / HFRCODPLL. | *As defined in* per-family RMs; *cited*. |
| **EM mode** | Energy Mode. The Silicon Labs naming for low-power states: EM0 (run), EM1 (sleep), EM2 (deep sleep, retains state), EM3 (stop), EM4 (off/hibernate). CHIPS-SILABS template output MUST leave the part in EM0 at the end of `init()`. | *As defined in* per-family RMs; *cited*. |
| **GPIO ROUTE / ROUTELOC** | Series 0/1 peripheral pin routing mechanism: each peripheral has a `<PERIPH>_ROUTEPEN` enable register and one or more `<PERIPH>_ROUTELOC0/1` location-select registers that pick which physical pin a peripheral signal exits on. Distinct from Series 2 DPLLOC. | *As defined in* per-family RMs (EFM32GG11 RM §32.5); *cited*. |
| **DPLLOC (digital peripheral allocator / pin allocator)** | Series 2 replacement for ROUTELOC: each peripheral signal (e.g. `USART0_TX`) is allocated to a specific GPIO via `GPIO.<PERIPH>ROUTE.<signal>ROUTE` registers, with `GPIO.<PERIPH>ROUTE.ROUTEEN.<signal>PEN` gating the connection. The §10 reconciliation notes that DPLLOC is NOT a "matrix" in the ESP sense — there is no opaque signal-id table; instead each peripheral has a *named* per-signal route register. | *As defined in* Series 2 family RMs (EFR32MG24 RM §24.5 "GPIO pin routing"); *cited*. **TBD: needs Silabs EFR32xG2x RM cite** for the exact register-block naming (some Series 2 parts use `GPIO_USART0_ROUTEEN`, others use `GPIO_USART0ROUTE_ROUTEEN`). |
| **System gate** | A row in a chip YAML's `clock_tree.system_gates:` map: `{ clk_en_reg, clk_en_field, rst_reg, rst_field, [clk_sel_reg, clk_sel_field] }`. Drives the `clocks.rs.jinja` template's per-peripheral clock-enable + reset-pulse emission. Mirrors the ESP / Nordic shape; SiLabs-specific differences are noted in §10. | Owned by this chapter; mirrors `chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml` lines 39–108. |
| **Peripheral instance** | A row in a chip YAML's `peripherals:` map: `{ class, instance, base, irq, signals: [...] }`. The `class` is a frozen enum (§5); `instance` is the PAC peripheral name; `signals` lists the pin-routable signals each peripheral exposes. | Owned by this chapter; mirrors ESP/Nordic shape. |
| **Signal** | One entry in a peripheral's `signals:` list: `{ role, direction, [route_reg, route_field, pen_field] }`. The `role` is peripheral-specific (`tx`, `rx`, `sck`, `mosi`, etc.). The route fields name DPLLOC route registers on Series 2 or ROUTELOC slots on Series 0/1. The route-field naming convention is one of the §10 reconciliation decisions because it differs between Series 0/1 and Series 2. | Owned by this chapter. |
| **Pin route** | An entry in a board YAML's `pins:` list: `{ port, pin, signal, peripheral, direction, [pull], [drive_strength], [label] }`. SiLabs GPIO addressing is `(port, pin)` (e.g. `(PortA, 5)`), distinct from ESP's flat `gpio:` integer and Nordic's `(port, pin)` packed `psel` u32. The schema MUST keep `port` and `pin` as separate scalar fields. | Owned by this chapter. |
| **Console** | The peripheral instance that the generated `peripherals.rs` brings up as a UART debug console, named by a board YAML's `console:` block. SiLabs Series 0/1 uses `USARTn`, `LEUARTn`, or `UART0`; Series 2 uses `USARTn` or `EUSARTn`. The `console.peripheral:` value MUST be one of the chip's frozen peripheral instances. | Owned by this chapter. |

## 4. Source-of-truth map

The crates and APIs CHIPS-SILABS depends on. **The
`rlvgl-chips-silabs` crate MUST NOT reach inside these surfaces
outside the cited entry points.** When a cited entry point needs
to grow, the vendor's spec lineage ratifies the change first.

| Concept | Owner (canonical) | Mirrored / consumed by |
|---|---|---|
| Chip inventory file format | This chapter §5 / §6 | `db/chips/*.yaml`; loaded by `build.rs` and by `rlvgl-creator` (future `silabs/load.rs`). |
| Board inventory file format | This chapter §5 / §6 | `db/boards/*.yaml`; loaded by `build.rs` and by `rlvgl-creator`. |
| Vendor key | `CLAUDE.md` chipdb-vendor enum row (currently `{esp, stm, ti, nxp, nrf, renesas, silabs, rp2040, microchip}`) | `silabs` is the stable key; returned by `rlvgl_chips_silabs::vendor()`. |
| PAC crate per chip | Each chip YAML's `pac_crate:` field | Templates emit `use <pac_crate> as pac;`. The PAC version is pinned by the consuming example's `Cargo.toml`, NOT by `rlvgl-chips-silabs`. |
| CMU register-block names | Per-family RM (cited per chip YAML's `name:`) | `clock_tree.system_gates:` rows reference exact PAC field paths. |
| GPIO routing register-block names | Per-family RM | `peripherals[].signals[].route_*` fields reference exact PAC field paths. Naming differs between Series 0/1 (ROUTELOC) and Series 2 (DPLLOC) — see §10. |
| 6-file emission contract | This chapter §6 | Future `src/bin/creator/bsp/silabs/templates/*.rs.jinja`. |
| BSP loader (`yaml_to_ir`) | `src/bin/creator/bsp/silabs.rs` (currently a 17-line stub) | Becomes per-vendor module `src/bin/creator/bsp/silabs/{mod.rs,load.rs}` once `CHIPS-SILABS-02` lands; the entry-point function name `yaml_to_ir` is preserved for caller-side stability. |
| Snapshot test family | This chapter §12 | Future `tests/bsp_silabs_<chip>_render.rs` paralleling `tests/bsp_esp32c3_render.rs`. |
| `compile-verify` against a real PAC crate | This chapter §12 (optional gate) | Future `tests/bsp_silabs_<chip>_compile.rs` paralleling `tests/bsp_esp32c3_compile.rs`. Target triple is **chip-arch-dependent**: `thumbv8m.main-none-eabihf` for EFR32xG2x, `thumbv7em-none-eabihf` for EFM32GG/PG/MG1, `thumbv7m-none-eabi` for legacy Cortex-M3 parts. |

Path-internal references that are NOT authoritative (this
chapter owns their replacement or expansion):

- `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` (4-field
  placeholder) — superseded by `CHIPS-SILABS-01a` expansion to
  the full §5 schema.
- `chipdb/rlvgl-chips-silabs/db/boards/EFM32GG11.yaml`
  (`pins: []` placeholder) — superseded by `CHIPS-SILABS-01b`
  expansion once at least one real board (e.g. EFM32GG11_STK) is
  in scope.
- `src/bin/creator/bsp/silabs.rs` (17-line `yaml_to_ir` stub) —
  superseded by `CHIPS-SILABS-02`'s per-vendor module split.

Authoritative external documents:

- Silicon Labs **EFR32 Series 2 Reference Manuals** —
  per-family (`efr32mg21`, `efr32bg22`, `efr32fg23`,
  `efr32mg24`, `efr32bg27`). Sections cited: CMU, GPIO pin
  routing (DPLLOC), USART, EUSART, I2C, TIMER. **TBD: per-family
  RM revision pins for each chip YAML committed.**
- Silicon Labs **EFM32 Series 0/1 Reference Manuals** —
  per-family (`efm32gg11`, `efm32pg12`, `efm32tg11`, etc.).
  Sections cited: CMU, GPIO ROUTE/ROUTELOC, USART, LEUART, I2C,
  TIMER. **TBD: per-family RM revision pins.**
- Silicon Labs **per-chip data sheets** — pinout, package
  options, supply voltages, electrical characteristics. Used
  for board YAML pin assignment validation only; not for
  register-bit positions.

## 5. Frozen decisions — enums & registration policy

Each frozen enum names its registration policy per the *Frozen
enumerations — registration policy* rule in CLAUDE.md.

### 5.1 SiLabs family set — **Standards Action**

Families in scope for `CHIPS-SILABS-NN` initial coverage:

```text
{ efm32gg11, efr32mg24 }
```

`efm32gg11` is the **Series 1 reference family** (already seeded
in `db/chips/EFM32GG11.yaml`); `efr32mg24` is the **Series 2
reference family** representing the modern Cortex-M33 + radio
line. Both MUST be modelled before §12 ratification — the two
together exercise both GPIO routing mechanisms (ROUTELOC and
DPLLOC), the two CMU shapes, and the two target triples
(`thumbv7em-none-eabihf` and `thumbv8m.main-none-eabihf`).

Adding a family requires a §15 amendment and an explicit go-ahead
from the initiative owner. Candidate later additions, in
priority order:

- `efr32mg21`, `efr32bg22`, `efr32fg23` (Series 2 siblings of
  MG24; same DPLLOC and CMU shapes).
- `efm32pg12`, `efm32tg11` (Series 1 siblings of GG11; same
  ROUTELOC shape).
- `efr32bg27` (Series 2 with TZ-M extensions; may require §11
  TrustZone-M handling to come out of non-goals first).

### 5.2 Out-of-scope family set — **Standards Action**

Explicitly out of scope at v0; entry requires a §15 amendment
**and** the corresponding §11 non-goal entry to be reversed:

```text
{ c8051f,            # 8-bit C8051F MCU line (no Cortex-M)
  ezr32,             # Series 0 wireless (subsumed by Series 2)
  bgm,               # Bluetooth Gecko modules (board-level only)
  mgm,               # Multi-Protocol Gecko modules (board-level)
  zgm,               # Z-Wave Gecko modules (board-level)
  series_3 }         # Pre-release / sampling SiLabs Series 3 parts
```

Adding any of these requires both (a) a §15 amendment naming the
first-user chip + board, and (b) clearing the corresponding §11
non-goal. See §11 for the rationale on each entry.

### 5.3 Peripheral class set — **Specification Required**

```text
{ usart,    # Series 0/1 USART; Series 2 USART (legacy)
  eusart,   # Series 2 enhanced USART (replaces USART for new designs)
  leuart,   # Series 0/1 low-energy UART
  uart,     # Series 0 plain UART (e.g. EFM32GG11 UART0)
  i2c,      # I2C master/slave (all series)
  spi_master, spi_slave,
  timer,    # TIMER (16-bit) — all series
  letimer,  # Low-energy TIMER
  wdog,     # Watchdog
  rtc, rtcc, # Real-time clock / RTC compare
  adc,      # IADC on Series 2; ADC on Series 0/1
  dac,
  gpio,     # The GPIO block itself (not pin entries)
  ldma,     # Linked DMA controller (Series 0/1/2)
  cryotimer,# Series 1 sleep timer
  cmu,      # Clock Management Unit (rarely used as a peripheral instance — exposed for completeness)
  emu,      # Energy Management Unit
  msc,      # Memory System Controller (flash)
  rmu }     # Reset Management Unit
```

Adding a class requires a per-chapter walkthrough update (a
sub-letter `CHIPS-SILABS-NNx`) plus a template-side
`peripherals.rs.jinja` `{% elif periph.class == "<new>" %}` arm.
Class names MUST be lowercase ASCII identifiers with no vendor
prefix.

### 5.4 Energy Mode targeted by `init()` — **Standards Action**

```text
{ em0 }
```

The generated `init()` function MUST leave the part in EM0 (run)
at completion. Future support for EM1-init (e.g. for ULP demo
boards that boot directly into sleep with peripheral wake-up)
requires §15 amendment.

### 5.5 Target triple set — **Specification Required**

```text
{ thumbv7m-none-eabi,         # Cortex-M3 (legacy Series 0)
  thumbv7em-none-eabi,        # Cortex-M4 no FPU
  thumbv7em-none-eabihf,      # Cortex-M4F (EFM32GG11, EFM32PG12, MG1)
  thumbv8m.main-none-eabi,    # Cortex-M33 no FPU
  thumbv8m.main-none-eabihf } # Cortex-M33 + FPU (Series 2)
```

The chip YAML's `arch:` field MUST resolve to exactly one of
these triples via a §6.3 lookup table. Adding a triple requires a
per-chapter walkthrough; no §15 amendment for additions in this
enum, but the lookup table MUST be updated in the same PR.

### 5.6 Initiative prefix — **Standards Action**

```text
CHIPS-SILABS-NN[a-z]:
```

Matches the `CHIPS-<VENDOR>-NN[a-z]:` convention in
`CLAUDE.md` §"Spec-Before-Code Planning Discipline" / Execution
discipline. Per-vendor chipdb crates use this prefix for any PR
scoped to a ratified phase of this initiative.

### 5.7 Registration policy summary

| Enum | Policy | Reason |
|---|---|---|
| §5.1 Family set | Standards Action | Cross-phase contract; adding a family affects template emission + snapshot test families. |
| §5.2 Out-of-scope family set | Standards Action | Symmetric with §5.1; a §11 non-goal reversal is the harder gate. |
| §5.3 Peripheral class set | Specification Required | Adding a class is local to one template arm; no concepts-doc amendment needed. |
| §5.4 EM target | Standards Action | Adding an EM target changes the BSP's runtime contract. |
| §5.5 Target triple set | Specification Required | Adding a triple is a lookup-table edit. |
| §5.6 Initiative prefix | Standards Action | Cross-phase contract (commit subject convention). |

## 6. Frozen decisions — schema shape

### 6.1 Chip YAML required fields

A chip YAML at `db/chips/<chip>.yaml` MUST contain at minimum:

```yaml
# Chip identity
name:        <Chip id, §3>          # MUST match the file stem
arch:        <arch tag>             # MUST resolve via §6.3 to a §5.5 triple
package:     <package code>         # Free-form string (e.g. "QFN48", "BGA112")
pac_crate:   <crate name>           # MUST be a published Rust crate name

# Memory map (consumed by future memory.x emission, see §11)
memory:
  - { name: flash,  base: 0x0,        size: <bytes>, access: rx }
  - { name: sram,   base: 0x20000000, size: <bytes>, access: rwx }
  # ... per-family extra regions (RAM_RET, USERDATA, LOCKBITS, etc.)

# Clock tree
clock_tree:
  hfxo_hz:       <Hz>                # External crystal frequency; null if no HFXO
  hfrco_hz:      <Hz>                # HFRCO factory-default frequency
  sysclk_hz:     <Hz>                # Series 2: SYSCLK after CMU bring-up
  hfclk_hz:      <Hz>                # Series 0/1: HFCLK after CMU bring-up
  cpu_hz:        <Hz>                # CPU clock after init()
  system_gates:
    <periph_name>:
      clk_en_reg:   <PAC path>       # e.g. "cmu.clken0" (Series 2) or "cmu.hfperclken0" (Series 1)
      clk_en_field: <field name>     # e.g. "usart0"
      rst_reg:      <PAC path or null>   # Series 2 has CMU peripheral reset; Series 0/1 uses RMU per-peripheral or none
      rst_field:    <field name or null>

# Peripheral instances
peripherals:
  <periph_name>:
    class:    <§5.3 class tag>
    instance: <PAC instance name>    # Uppercased in templates (e.g. "USART0")
    base:     <0xADDRESS>            # Informational; templates use `p.<INSTANCE>` access
    irq:      <number or null>
    signals:
      - { role: <signal role>, direction: <in|out|inout>,
          route_reg: <route reg PAC path>,    # Series 2 DPLLOC or Series 1 ROUTELOC
          route_field: <route field name>,
          pen_reg: <route-enable reg PAC path>,
          pen_field: <pen field name> }

# GPIO + routing
gpio:
  ports: [A, B, C, D, E, F]          # MUST list every port the package exposes
  pins_per_port:
    A: 16  # etc., per package
# (No top-level `io_mux:` or `gpio_matrix:` — those are ESP-specific. See §10.)

# Linker hints
linker:
  region_text:   flash
  region_data:   sram
```

Fields above are normative; additional vendor-specific fields
are permitted but MUST be documented in a §15 amendment when
first introduced.

### 6.2 Board YAML required fields

A board YAML at `db/boards/<board>.yaml` MUST contain at minimum:

```yaml
name:        <Board id, §3>
chip:        <Chip id, must match a db/chips/*.yaml `name:`>
flash_mb:    <integer>

console:
  peripheral: <peripheral name from chip's `peripherals:` map>
  baud:       <integer>

pins:
  - { port: A, pin: 5, signal: USART0_TX, peripheral: usart0,
      direction: out, label: console_tx }
  - { port: A, pin: 6, signal: USART0_RX, peripheral: usart0,
      direction: in,  label: console_rx }
  # ... full pin assignment table for every routed signal

# Optional per-peripheral runtime configs
i2c_configs:
  i2c0:
    scl_hz: 400000

spi_configs:
  usart1:                            # USART-in-SPI-mode on Series 1
    clk_hz: 1000000
    mode:   0

features:                            # Free-form key/value for board-named conveniences
  led:        port_a_pin_4
  i2c_bus:    i2c0
  display:    ssd1306_128x64_i2c
```

### 6.3 `arch:` → target triple lookup

The chip YAML's `arch:` scalar resolves to a §5.5 target triple
via this table. The lookup is owned by the generator (future
`src/bin/creator/bsp/silabs/load.rs`); adding a row is a §5.5
"Specification Required" change.

| `arch:` value | Target triple | First-user family |
|---|---|---|
| `cortex-m3`   | `thumbv7m-none-eabi`       | Series 0 (deferred — see §11). |
| `cortex-m4`   | `thumbv7em-none-eabi`      | None at v0; reserved. |
| `cortex-m4f`  | `thumbv7em-none-eabihf`    | EFM32GG11 (placeholder seed). |
| `cortex-m33`  | `thumbv8m.main-none-eabi`  | None at v0; reserved for FPU-less Series 2 SKUs. |
| `cortex-m33f` | `thumbv8m.main-none-eabihf`| EFR32MG24 (target Series 2 reference). |

### 6.4 6-file emission set

`rlvgl-creator bsp from-yaml --vendor silabs --board <board>`
MUST emit exactly these six files into the output directory,
mirroring the ESP precedent:

| File | Owner | Content |
|---|---|---|
| `mod.rs` | future `silabs/templates/mod.rs.jinja` | Module index. Re-exports `pac::init`. Lists the five sibling modules. |
| `pac.rs` | future `silabs/templates/pac.rs.jinja` | Top-level `init()` calling `super::clocks::init()`, `super::io_mux::init()`, `super::peripherals::init()` in order. |
| `clocks.rs` | future `silabs/templates/clocks.rs.jinja` | CMU bring-up: enable HFXO, switch SYSCLK / HFCLK to HFXO, then per-peripheral clock-gate enables driven by `chip.clock_tree.system_gates`. |
| `io_mux.rs` | future `silabs/templates/io_mux.rs.jinja` | Per-pin route emission. **Series 1**: writes `<PERIPH>_ROUTEPEN` + `<PERIPH>_ROUTELOC0/1`. **Series 2**: writes `GPIO.<PERIPH>ROUTE.<signal>ROUTE` + `GPIO.<PERIPH>ROUTE.ROUTEEN`. The template MUST branch on the chip's `arch:` (or an explicit `routing_kind:` field — see §10.2). |
| `peripherals.rs` | future `silabs/templates/peripherals.rs.jinja` | Per-peripheral init by class (§5.3). Real init for `usart` / `eusart` / `uart` (console), `i2c`, `spi_master`, `timer` (watchdog disable). Stubs for everything else, mirroring the ESP shape. |
| `board.rs` | future `silabs/templates/board.rs.jinja` | `pub const` exports for `BOARD_NAME`, `CHIP`, `PACKAGE`, `FLASH_MB`, `CPU_HZ`, `HFXO_HZ`, and per-pin `pub const <LABEL>_PORT: char = '<P>'; pub const <LABEL>_PIN: u8 = <N>;`. The two-constant-per-pin emission is the SiLabs divergence from ESP's one-constant emission (see §10.3). |

Linker scripts (`memory.x`, `chip.x`) are **NOT** in this set at
v0. They are §11 deferred work; the consuming example crate
provides its own `memory.x` until `CHIPS-SILABS-05` adds the
emission path.

### 6.5 Peripheral instance access style

Generated SiLabs templates MUST use **uppercase field access**
matching the svd2rust convention for SiLabs PACs (e.g.
`p.USART0.frame().write(|w| ...)`, not `p.usart0()` or
`p.usart0.frame.write(|w| ...)`). Sibling-module references MUST
use `super::` so the output works both as a crate root and as a
child module of a host crate. Both rules mirror the ESP
convention from `chipdb/rlvgl-chips-esp` and are repeated here
because they're load-bearing for the snapshot tests.

## 7. Frozen decisions — register / bit positions

No register-bit positions are frozen by this concepts doc. Each
chip YAML committed under `db/chips/*.yaml` carries its own
PAC-path references, and the PAC crate cited by `pac_crate:` is
the authoritative source of bit positions. Frozen-bit-position
documents (if needed for a future flash-write path, CMU lock
sequences, or TrustZone-M setup) are owned by per-chip
sub-letter docs (`CHIPS-SILABS-01a-EFM32GG11-CMU.md` etc.) and
ratified independently of this concepts doc.

If a future template arm hard-codes a bit position (e.g. a CMU
HFXO ready-flag wait loop), that hard-coded value MUST be
accompanied by a `// SAFETY:`-style comment naming the family RM
section and register/field, and the value MUST appear in a §15
amendment to this doc once written.

## 8. Frozen decisions — snapshot & compile-verify gates

### 8.1 Snapshot tests

Every board YAML committed under `db/boards/*.yaml` MUST be
accompanied by a snapshot test at
`tests/bsp_silabs_<board_stem>_render.rs` paralleling
`tests/bsp_esp32c3_render.rs`. The test invokes the generator on
the board YAML and compares the six emitted files against
pinned reference output committed under
`tests/snapshots/bsp_silabs_<board_stem>/`.

Snapshot drift detection is the primary regression gate. Pinning
*content* (whitespace + register-write sequence) is the safety
net that catches template-emission regressions before they reach
a real board.

### 8.2 `compile-verify` against a real PAC crate

A board MAY additionally provide a `compile-verify` test at
`tests/bsp_silabs_<board_stem>_compile.rs` paralleling
`tests/bsp_esp32c3_compile.rs`. The test materializes a
throwaway cargo project around the generated files for the
board, pulls in the chip's `pac_crate:` from crates.io, and
runs `cargo check` on the chip's §5.5 target triple.

`compile-verify` is `SHOULD`, not `MUST`, at v0 — the snapshot
test is the load-bearing gate; `compile-verify` is the
sufficiency gate. For an EFR32MG24 board, `compile-verify`
requires `rustup target add thumbv8m.main-none-eabihf` and
network access to fetch the SiLabs PAC crate.

### 8.3 Hardware bring-up

Hardware bring-up on a generated SiLabs BSP MAY follow at
initiative close, paralleling the `beetle-esp32c3` `bsp_pac`
feature path. It is NOT a §12 acceptance gate; the snapshot +
`compile-verify` discipline is what `CHIPS-SILABS-NN` ratifies.

## 9. Discipline invariants

The chipdb-vendor crates participate in the workspace-wide
`rlvgl-platform` Register-Mashing Discipline only by *referring
to* PAC crates in generated code. The discipline rules
(typed framebuffer ownership, InFlight tokens, MmioAddr<T> /
PhysAddr / DmaAddr, ISR channels) are platform-side concerns and
do NOT apply to the chipdb's emitted register-write code.

However, generated SiLabs templates MUST follow these local
invariants (mirroring `chipdb/rlvgl-chips-esp` Phase 4.6
expectations):

- **INV-SL1.** Every emitted `unsafe { ... }` block carries a
  `// SAFETY:` comment naming the PAC field width and the
  rationale for the `bits()` write (most often "field width
  varies across PAC revisions; raw-bits write sidesteps
  field-width errors in svd2rust 0.31"). Mirrors the
  `chipdb/rlvgl-chips-esp/.../peripherals.rs.jinja` lines 42–43
  comment shape.
- **INV-SL2.** Generated `init()` functions are top-down: clocks
  first, then IO routing, then per-peripheral init. The order is
  fixed in `pac.rs.jinja` and MUST NOT be reorderable from chip
  YAML.
- **INV-SL3.** Sibling-module references use `super::` not
  `crate::`. Lets the emitted BSP work both as a crate root and
  as a child module under a host crate.
- **INV-SL4.** No emitted code calls `cortex_m::interrupt::free`
  or `critical_section::with` at v0. Interrupt-safety scoping is
  a consumer-side concern; the BSP's `init()` runs single-threaded
  before any ISR vectors are installed.
- **INV-SL5.** Watchdog disable MUST happen during
  `peripherals.rs.jinja`'s `class == "wdog"` arm if the board has
  a watchdog instance. SiLabs WDOG_CTRL on Series 0/1 has a
  3-cycle wait after disable; the template MUST emit the wait.
  **TBD: needs Silabs EFM32GG11 RM §22 cite** for the exact wait
  cycle count.

Discipline-invariant additions are §5.7 Standards Action once
ratified.

## 10. Reconciliation with adjacent repo primitives

### 10.1 ESP / Nordic / NXP shape vs. SiLabs

The five ratified vendor adapters share a common YAML grammar
shape (top-level `name`, `arch`, `pac_crate`, `memory:`,
`clock_tree:`, `peripherals:`, plus per-vendor pin-routing
tables). SiLabs MUST adopt the same grammar where possible;
where SiLabs diverges, the divergence MUST be named here.

**Shared shape** (mirrors `chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml`):

- `name:`, `arch:`, `package:`, `pac_crate:` — identical.
- `memory:` — identical.
- `clock_tree.system_gates:` — identical row shape.
- `peripherals.<name>.{class,instance,base,irq,signals}` —
  identical, with SiLabs-specific `class` values per §5.3.

**SiLabs divergences** (each must appear in §10.2–§10.5):

### 10.2 Pin routing: ROUTELOC (Series 0/1) vs. DPLLOC (Series 2)

The ESP shape uses two structures: an `io_mux:` table (per-pin
function slot 0–3) and a `gpio_matrix:` table (signal-id-indexed
many-to-one routing). Nordic uses a flat `psel:` u32 per
peripheral signal (packed `port:pin`).

SiLabs has **two distinct mechanisms across families**:

- **Series 0/1 (EFM32GG11 et al.):** Each peripheral has its
  own `<PERIPH>_ROUTEPEN` register and `<PERIPH>_ROUTELOC0/1`
  registers; routing is *per-peripheral*, with a "location" enum
  selecting which fixed pin-set the peripheral's signals exit on.
  Pins are NOT independently routable; you pick a *location*
  per peripheral.
- **Series 2 (EFR32MG24 et al.):** Each peripheral signal is
  independently routable via `GPIO.<PERIPH>ROUTE.<signal>ROUTE`
  (which port + pin) and `GPIO.<PERIPH>ROUTE.ROUTEEN.<signal>PEN`
  (gate). This is the **DPLLOC** mechanism. It is closer to ESP's
  GPIO matrix in flexibility but NOT in encoding — there is no
  integer "signal id" lookup table; each peripheral has named
  per-signal route registers.

**Resolution:** The chip YAML carries an explicit
`routing_kind:` enum at top level. Frozen set:

```text
{ routeloc,    # Series 0/1 ROUTELOC mechanism
  dplloc }     # Series 2 DPLLOC mechanism
```

`routing_kind:` is **Standards Action** (because it gates
template branching). The `io_mux.rs.jinja` template MUST branch
on this field. Future families with a third mechanism (Series 3?)
require a §15 amendment.

### 10.3 Pin identity: (port, pin) tuple vs. flat integer

ESP boards use flat `gpio: <integer>` (0–48). Nordic boards use
flat `psel: <u32>` (port:pin packed). SiLabs MUST use the
**(port, pin) tuple** form because:

- SiLabs GPIO addresses are natively `(port, pin)` in the PAC
  (`p.GPIO.port_a().dout()`, `p.GPIO.port_b().dout_set()`).
- A flat-integer encoding would have to invent a chip-specific
  port-stride convention (`port * 16 + pin`?) and that convention
  would diverge between packages — EFM32GG11 BGA112 has Port A–F
  with up to 16 pins per port, EFR32MG24 has Port A–D with up to
  10 pins per port.

**Resolution:** Board YAML `pins:` entries MUST keep `port:` and
`pin:` as separate scalar fields. The `board.rs.jinja` template
emits two `pub const` entries per labelled pin:

```rust
pub const CONSOLE_TX_PORT: char = 'A';
pub const CONSOLE_TX_PIN:  u8   = 5;
```

This is a deliberate divergence from the ESP
`pub const CONSOLE_TX: u8 = 21;` shape and is one of the
load-bearing decisions snapshot tests will lock in.

### 10.4 Console peripheral class: USART / EUSART / LEUART / UART

The ESP shape has one console-class candidate (`uart`). Nordic
has `uarte`. SiLabs has **four**:

- `usart`   — Series 0/1 USART; Series 2 USART (legacy).
- `eusart`  — Series 2 enhanced USART (the modern choice).
- `leuart`  — Series 0/1 low-energy UART (for EM2 wakeup designs).
- `uart`    — Series 0 plain UART (EFM32GG11 has a `UART0`
  instance distinct from `USART0`).

The `peripherals.rs.jinja` template's console-init arm MUST
branch on `periph.class` and emit class-specific init code.
Treating all four as "uart" is a defect — USART bring-up writes
to `<INSTANCE>.frame()`, `.clkdiv()`, `.cmd()`, `.routepen()`;
EUSART bring-up writes to `<INSTANCE>.cfg0()`, `.cfg1()`,
`.cfg2()`, `.frame_cfg()`. The register paths are not aliased.

### 10.5 Clock-source bring-up: HFXO start sequence

ESP's `clocks.rs.jinja` assumes the bootrom has already brought
up the PLL and only handles per-peripheral clock-gating. SiLabs
templates **cannot** assume the same — bootrom on EFM32GG11
leaves the part running on the 19 MHz HFRCO until application
code starts HFXO and switches `HFCLK` to it.

**Resolution:** `clocks.rs.jinja`'s `init()` MUST emit the
following sequence when the chip YAML specifies an
`hfxo_hz: <non-null>`:

1. Enable HFXO (`p.CMU.oscencmd().write(|w| w.hfxoen().set_bit())`
   on Series 1; `p.CMU.cmd().write(|w| w.hfxoen().set_bit())` on
   Series 2 — **TBD: needs Silabs RM cite for exact Series 2
   register path**).
2. Wait for HFXO ready (`p.CMU.status().read().hfxordy().bit()`
   on Series 1).
3. Switch SYSCLK / HFCLK source to HFXO.
4. Then enter the per-peripheral `system_gates` loop (mirrors
   ESP's `clocks.rs.jinja` lines 22–37).

The HFXO start sequence is template-emitted, not chip-yaml-driven
— it's small enough to live in the template directly with branching
on `chip.arch`.

### 10.6 Initiative naming vs. existing chipdb conventions

The `CHIPS-SILABS-NN` prefix matches the CLAUDE.md convention.
But there is currently NO ratified concepts doc for any other
chipdb-vendor crate (`chipdb/rlvgl-chips-esp/docs/` does not
exist; nor do `nrf`, `nxp`, `renesas`, `rp2040` `docs/`
directories). This chapter is therefore the **first** chipdb-vendor
concepts doc and may set a precedent that retroactively applies to
the other five vendor crates.

**Resolution:** Treat this chapter as the *reference shape* for
future ESP / Nordic / NXP / Renesas / RP2040 concepts docs if
those families ever produce a ≥3-phase initiative. No
retroactive obligation — the other five vendors shipped without
concepts docs because their initiatives were single-phase prototypes
absorbed directly into `CLAUDE.md`'s "Espressif BSP Generator"
section. SiLabs warrants a concepts doc because it (a) has not
shipped yet and (b) must reconcile two distinct GPIO routing
mechanisms (ROUTELOC vs. DPLLOC) across two distinct CMU shapes
across two distinct target triples — a structurally harder
problem than ESP's "one architecture, one PAC family" shape.

### 10.7 Asset / theme / state machine schemas (out of scope here)

The application-level schema concerns (`docs/app-schema/`) are
explicitly out of scope. CHIPS-SILABS owns chip + board IR only;
the `rlvgl` application schema cites the chipdb vendor key
(`silabs`) but does NOT reach into per-chip YAML.

## 11. Non-goals

Explicit out-of-scope for `CHIPS-SILABS-NN` at v0. Each entry is
a **Standards Action** reversal target — reversing requires a
§15 amendment AND clearing the corresponding §5.2 frozen-set
entry.

- **C8051F 8-bit MCU line.** The C8051F family is 8051-core,
  not Cortex-M. The chipdb-vendor abstraction targets Cortex-M
  PAC crates; supporting 8051 would require an entirely
  different toolchain assumption (Keil / SDCC) and a different
  IR shape. Standards-Action enum entry: `c8051f` (§5.2).
- **EZR32 Series 0 wireless line.** Subsumed by Series 2 EFR32
  for new designs; no first-user pressure. Standards-Action
  enum entry: `ezr32` (§5.2).
- **BGM / MGM / ZGM Gecko modules.** These are *board-level*
  parts (a Series 2 die plus matching network + antenna in a
  shielded can). They are addressed at the board YAML layer,
  not the chip YAML layer — a BGM221 board YAML cites
  `chip: EFR32BG21` as its underlying chip. Listing them
  separately would double-count. Standards-Action enum
  entries: `bgm`, `mgm`, `zgm` (§5.2).
- **SiLabs Series 3.** Series 3 parts (e.g. SiWG917) are at
  early-engineering-sample stage as of 2026-05; their RMs are
  not generally available. Adding Series 3 requires both an
  available RM and a published PAC crate. Standards-Action
  enum entry: `series_3` (§5.2).
- **TrustZone-M / Secure Engine bring-up.** Series 2 parts
  expose ARM TrustZone-M (Secure / Non-Secure split) and a
  dedicated Secure Engine subsystem for crypto. Generated
  BSPs run in Non-Secure-only mode at v0. TZ-M secure-side
  bring-up is a separate initiative; reversing this non-goal
  requires §15 amendment naming a first-user board with a
  secure-side use case.
- **Bluetooth LE / Mesh / Z-Wave / Zigbee stacks.** Network
  stack code is application-layer, not BSP. The generated
  BSPs bring up the radio peripheral's *clock and pin
  routing* if the board YAML asks for it, but do not
  configure radio mode, PHY, or link-layer state.
- **EMLIB consumption.** Silicon Labs' Gecko SDK ships
  `em_cmu.c` / `em_gpio.c` / `em_usart.c` (the EMLIB layer)
  as the vendor-recommended C abstraction. The generated
  BSP targets raw PAC access in Rust, NOT EMLIB. EMLIB is
  cited for register-bit verification only.
- **Linker-script emission (`memory.x`, `chip.x`).** Phase
  4.7b of the ESP pre-publish discipline emits linker
  scripts; SiLabs defers this to a future `CHIPS-SILABS-05`
  sub-letter. v0 boards provide their own `memory.x`.
- **Multi-chip-on-board (heterogeneous packages).** Some
  Silicon Labs boards combine a Series 2 wireless die with
  a companion EFM8 sensor MCU (e.g. radio + sensor
  co-design). Board YAML at v0 cites exactly one chip;
  multi-chip boards are deferred.
- **Runtime energy-mode transitions (EM1/EM2/EM3/EM4).**
  §5.4 freezes the init-time target as EM0. Runtime sleep
  transitions are application-side concerns.
- **`compile-verify` as a hard `MUST`.** Per §8.2,
  `compile-verify` is `SHOULD`. Hard-`MUST` would require
  every CI runner to install both `thumbv7em-none-eabihf`
  and `thumbv8m.main-none-eabihf` toolchains; the
  snapshot-test gate is sufficient for behaviour-PR
  regression catch.

Reversal of any of the above MUST file a §15 amendment naming
the first-user chip + board AND a sub-letter doc
(`CHIPS-SILABS-NNx`) covering the reversal-specific
reconciliation work.

## 12. Acceptance checklist

This concepts chapter is ratified (§15 entry dated) when:

- [ ] §0 authority table reviewed; no silent restatement of
      existing repo definitions.
- [ ] §3 glossary terms each carry a cite-vs-restate marker.
- [ ] §4 source-of-truth map has exactly one owner per row;
      no row claims authority over a concept owned by
      `chipdb/rlvgl-chips-esp` or `chipdb/rlvgl-chips-nrf`.
- [ ] §5.1 family set reviewed against repo state; both
      EFM32GG11 and EFR32MG24 confirmed as first-user
      targets.
- [ ] §5.2 out-of-scope family set reviewed; each entry has a
      §11 non-goal counterpart.
- [ ] §5.3 peripheral class set reviewed against the existing
      `db/chips/EFM32GG11.yaml` placeholder; the placeholder's
      eventual expansion in `CHIPS-SILABS-01a` MUST use only
      classes from this set.
- [ ] §6.4 6-file emission set matches the ESP precedent
      exactly (six files, same names, same ordering).
- [ ] §10.2 (ROUTELOC vs DPLLOC) decision ratified.
- [ ] §10.3 (port-pin tuple vs flat integer) decision
      ratified; snapshot-test consequences (`<LABEL>_PORT`,
      `<LABEL>_PIN` two-constant emission) understood.
- [ ] §10.4 (four console classes) consequences for
      `peripherals.rs.jinja` understood.
- [ ] §11 non-goals reviewed; nothing currently in scope is
      silently excluded.
- [ ] §15 has a dated ratification entry signed off by the
      initiative owner.

A conforming `CHIPS-SILABS-NN` initiative satisfies the
following execution gates (each is the §12 (a) / (b) / (c)
shape from `DCB-00-CONCEPTS.md` §12):

- **(a)** `CHIPS-SILABS-01`: at least two real chip YAMLs
  committed under `db/chips/` (one Series 1 family — EFM32GG11
  expanded from placeholder; one Series 2 family — EFR32MG24
  new). Each chip YAML satisfies §6.1 minimum field set.
- **(b)** `CHIPS-SILABS-01b`: at least one real board YAML
  committed under `db/boards/` for each of the two §5.1
  families. Each board YAML satisfies §6.2 minimum field set
  and lists every pin needed to bring up the console + at
  least one peripheral.
- **(c)** `CHIPS-SILABS-02`: per-vendor module split lands at
  `src/bin/creator/bsp/silabs/{mod.rs,load.rs}` and the
  six template files at
  `src/bin/creator/bsp/silabs/templates/*.rs.jinja`. The
  templates branch on `routing_kind:` (§10.2) and `arch:`
  (§6.3) as required.
- **(d)** `CHIPS-SILABS-03`: snapshot tests per §8.1 land for
  every board committed in (b). The tests are wired into the
  pre-publish discipline (a new Phase 4.7c bullet citing
  `bsp_silabs_*_render`).

A conforming `CHIPS-SILABS-NN` deployment MAY additionally
satisfy:

- **(e)** `CHIPS-SILABS-04`: `compile-verify` tests per §8.2
  for at least one board per family. Requires
  `rustup target add thumbv7em-none-eabihf` AND
  `rustup target add thumbv8m.main-none-eabihf`. Independently
  conformant.
- **(f)** `CHIPS-SILABS-05`: linker-script emission per §11
  (deferred). Independently conformant.
- **(g)** Hardware bring-up of the generated BSP on at least
  one physical board (e.g. SLSTK3701A Giant Gecko 11 starter
  kit or BRD2601B EFR32MG24 dev kit). NOT a gate; informative.

## 13. Files cited

Existing repo files this chapter references:

- [`/CLAUDE.md`](../../../CLAUDE.md) — spec-before-code
  planning discipline, enum registration policy,
  initiative-prefix convention, Phase 4.6 / 4.7 / 4.7b
  shape this chapter mirrors.
- [`chipdb/rlvgl-chips-silabs/Cargo.toml`](../Cargo.toml) —
  crate metadata; `silabs` vendor key.
- [`chipdb/rlvgl-chips-silabs/src/lib.rs`](../src/lib.rs) —
  current adapter API (`vendor`, `boards`, `find`,
  `chip_yaml`, `board_yaml`); preserved by this chapter.
- [`chipdb/rlvgl-chips-silabs/build.rs`](../build.rs) —
  build-time YAML loader; preserved.
- [`chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml`](../db/chips/EFM32GG11.yaml)
  — placeholder chip YAML; superseded by `CHIPS-SILABS-01a`.
- [`chipdb/rlvgl-chips-silabs/db/boards/EFM32GG11.yaml`](../db/boards/EFM32GG11.yaml)
  — placeholder board YAML; superseded by
  `CHIPS-SILABS-01b`.
- [`src/bin/creator/bsp/silabs.rs`](../../../src/bin/creator/bsp/silabs.rs)
  — 17-line `yaml_to_ir` stub; superseded by
  `CHIPS-SILABS-02`.
- [`chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml`](../../rlvgl-chips-esp/db/chips/esp32c3.yaml)
  — reference shape for chip YAML.
- [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml`](../../rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml)
  — reference shape for board YAML.
- [`src/bin/creator/bsp/espressif/templates/mod.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/mod.rs.jinja),
  [`pac.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/pac.rs.jinja),
  [`clocks.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/clocks.rs.jinja),
  [`io_mux.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/io_mux.rs.jinja),
  [`peripherals.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/peripherals.rs.jinja),
  [`board.rs.jinja`](../../../src/bin/creator/bsp/espressif/templates/board.rs.jinja)
  — six-file emission contract that this chapter freezes for
  SiLabs.
- [`docs/concepts/DCB-00-CONCEPTS.md`](../../../docs/concepts/DCB-00-CONCEPTS.md)
  — phase-document shape this chapter models (§0–§15 layout,
  Standards Action / Specification Required policy, §12
  acceptance-checklist (a)/(b)/(c) shape).
- [`docs/app-schema/00-concepts.md`](../../../docs/app-schema/00-concepts.md)
  — application-schema concepts doc that cites the chipdb
  vendor set; §10.7 reconciliation point.

Cross-vendor references (read-only):

- `chipdb/rlvgl-chips-nrf/` — Nordic adapter; the
  flat-`psel:` u32 routing shape is contrasted in §10.3.
- `chipdb/rlvgl-chips-nxp/` — NXP adapter.
- `chipdb/rlvgl-chips-rp2040/` — RP2040 adapter.
- `chipdb/rlvgl-chips-renesas/` — Renesas adapter.

Authoritative external documents (none vendored in repo):

- **Silicon Labs EFR32 Series 2 Reference Manuals** —
  per-family (`efr32mg21`, `efr32bg22`, `efr32fg23`,
  `efr32mg24`, `efr32bg27`). **TBD: per-chip RM revision pins
  for each chip YAML committed under `CHIPS-SILABS-01a`.**
- **Silicon Labs EFM32 Series 0/1 Reference Manuals** —
  per-family (`efm32gg11`, `efm32pg12`, `efm32tg11`). **TBD:
  per-chip RM revision pins.**
- **Silicon Labs per-chip data sheets** — pinout, package
  options. **TBD: data-sheet revision pins per chip.**

## 14. Unblocks

Ratifying this chapter unblocks:

- `CHIPS-SILABS-01a` — full `db/chips/EFM32GG11.yaml`
  expansion to satisfy §6.1.
- `CHIPS-SILABS-01b` — first `db/boards/<board>.yaml` real
  pin map (likely SLSTK3701A or a similar GG11-based dev kit).
- `CHIPS-SILABS-01c` — new `db/chips/EFR32MG24.yaml` (Series 2
  reference chip).
- `CHIPS-SILABS-01d` — first `db/boards/<board>.yaml` for
  EFR32MG24 (likely BRD2601B or BRD4186C dev kit).
- `CHIPS-SILABS-02` — per-vendor module split at
  `src/bin/creator/bsp/silabs/{mod.rs,load.rs}` + six
  template files under
  `src/bin/creator/bsp/silabs/templates/*.rs.jinja`.
- `CHIPS-SILABS-03` — snapshot test families per §8.1.
- `CHIPS-SILABS-04` (optional) — `compile-verify` tests per
  §8.2.
- `CHIPS-SILABS-05` (deferred per §11) — linker-script
  emission.
- A new pre-publish Phase 4.7c bullet citing
  `bsp_silabs_*_render` (and optionally `bsp_silabs_*_compile`).
- An eventual `CHIPS-SILABS-RETROSPECTIVE.md` once the §12
  (a)/(b)/(c)/(d) gates close — per `CLAUDE.md` §Initiative
  retrospective.

## 15. Change log

| Date       | Status | Note                                                                                  |
| ---------- | ------ | ------------------------------------------------------------------------------------- |
| 2026-05-11 | Ratified (owner: Ira Abbott) | Doc *shape* ratified. `CHIPS-SILABS-NN[a-z]:` PRs MAY now cite §-numbers as frozen authority. Open TBDs (§3 DPLLOC register-block naming, §4 per-family RM revision pins, §9 INV-SL5 wait-cycle count, §10.5 Series 2 HFXO register path) remain open and gate `CHIPS-SILABS-01` chip-YAML population rather than this doc. |
| 2026-05-11 | DRAFT — awaiting ratification | Initial skeleton. Argument target — review §10.2 (ROUTELOC vs DPLLOC) first, then §10.3 (port-pin tuple), then §5.1 (family set), then §11 (non-goals). |

### 2026-05-13 — SILABS-02 SKU-flatten amendment for efm32gg11b-pac 0.1.4

- **Divergence**: `efm32gg11b-pac` 0.1.4 (svd2rust 0.28.0) gates the
  per-SKU `Peripherals` type behind a SKU sub-module
  (`efm32gg11b_pac::efm32gg11b820::Peripherals`) rather than re-exporting
  it at the crate root. The original SILABS-02 templates emitted
  `use efm32gg11b_pac as pac;` and called `pac::Peripherals::steal()`,
  which produced 5×E0433 ("could not find `Peripherals` in `pac`")
  errors and 61×E0282 cascading type-inference failures during the
  CHIPS-SILABS-04 compile-verify gate (one E0433 per `use ... as pac;`
  site across `pac.rs`, `clocks.rs`, `io_mux.rs`, and `peripherals.rs`).
- **Schema extension**: `SilabsChip` gained an optional
  `pac_sku_module: Option<String>` field (defaulted via `#[serde(default)]`
  so existing chip YAMLs deserialise unchanged). `EFM32GG11.yaml` sets
  `pac_sku_module: efm32gg11b820`. No other chip YAMLs in the chipdb
  are affected.
- **Template change**: all four templates that import the PAC crate
  (`pac.rs.jinja`, `clocks.rs.jinja`, `io_mux.rs.jinja`,
  `peripherals.rs.jinja`) now emit
  `use efm32gg11b_pac::efm32gg11b820 as pac;` when `pac_sku_module` is
  set, falling back to the original `use efm32gg11b_pac as pac;` when
  the field is absent. This binds the `pac` alias to the SKU
  sub-module so `pac::Peripherals::steal()` resolves without
  per-call-site changes. Render snapshots re-blessed.
- **Residual divergence (NOT addressed by this amendment)**:
  post-SKU-flatten the compile-verify gate still fails with a separate
  class of errors — `efm32gg11b-pac 0.1.4` exposes register blocks as
  **fields** on the peripheral instance (`p.CMU.hfperclken0`, no
  parentheses) rather than as **methods** (`p.CMU.hfperclken0()`) as
  the current SILABS templates emit. Resulting errors:
  - 41×E0599 ("no method named `<register>` found for struct
    `efm32gg11b_pac::efm32gg11b820::<PERIPH>`") covering CMU
    (`.hfperclken0`), GPIO (`.ph_dout`, `.ph_model`, `.ph_modeh`,
    `.pb_model`, `.pb_dout`, `.pe_model`, `.pe_dout`, `.pc_modeh`,
    `.pc_dout`, `.pi_model`), USART4 (`.routeloc0`, `.routepen`,
    `.cmd`, `.ctrl`, `.frame`, `.clkdiv`), and I2C2 (`.routeloc0`,
    `.routepen`).
  - 61×E0282 cascade — same as before, because `w` in the
    `.modify(|_, w| ...)` closures still cannot infer its type when
    the receiver method is missing.
  This is structurally identical to the CHIPS-MICROCHIP-04 divergence
  documented in `CLAUDE.md` (atsamd51j19a 0.7 also uses field-style
  accessors). Resolution deferred to a follow-up amendment that
  switches the SILABS templates from `p.CMU.hfperclken0().modify(...)`
  to `p.CMU.hfperclken0.modify(...)` form, gated on the same chip
  yaml schema. NOT in scope for this SILABS-02 amendment; the
  CHIPS-SILABS-04 gate stays commented out in `CLAUDE.md` until both
  divergences are resolved.
- **Validation**:
  - `cargo test -p rlvgl-chips-silabs` — pass (4 tests).
  - `cargo test -p rlvgl --test bsp_silabs_slstk3701a_render
    --features creator,regression` — pass (10 tests, snapshots
    re-blessed).
  - `cargo test -p rlvgl --test bsp_silabs_slstk3701a_compile
    --features compile-verify -- --test-threads=1` — **still FAILS**
    on the residual divergence above. E0433 count went from 5 to 0;
    E0282 count unchanged at 61; new E0599 count is 41.
  - The older `bsp_silabs.rs` round-trip test (against the generic
    `Ir` adapter at `src/bin/creator/bsp/silabs.rs`) was already
    failing at v0.2.0 baseline (committed `.snap.new` artifacts);
    unaffected by this amendment.
