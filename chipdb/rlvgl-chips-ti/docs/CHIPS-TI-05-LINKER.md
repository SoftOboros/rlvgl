<!--
CHIPS-TI-05-LINKER.md - Linker emission contract for the Texas Instruments
chipdb + BSP-generator initiative. Ratifies the already-shipping
`memory.x` + `<chip>.x` emission set introduced informally during slates
4-5 of CHIPS-TI-01.
-->

# CHIPS-TI-05 — Texas Instruments BSP Linker Emission

> **Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.
> Closes the §11 "linker emission deferred to -05" item in
> [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md). Future linker
> -script behaviour changes route through this chapter's §15 amendment
> process; no behaviour PR rides on an unamended invariant.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared
in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline". RFC 2119 / RFC 8174 normative keywords (**MUST**,
**MUST NOT**, **SHALL**, **SHOULD**, **MAY**) carry their RFC meanings
when capitalised; lowercase use is narrative.

| Domain                                          | Authoritative source                                                                                                        |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| CC13x2 / CC26x2 chip memory map                 | TI SWCU185G (CC13x2/CC26x2 SimpleLink Wireless MCU TRM), §1.5 "Memory Map" Table 1-5; §11.1 "Customer Configuration Area (CCFG)" Table 11-1 |
| CC1352R1F3RGZ orderable-part datasheet          | TI SWRS196I (January 2018 / revised February 2021), §9.4 "Memory Organisation"                                              |
| Cortex-M4F architectural semantics              | ARM ARMv7-M Architecture Reference Manual                                                                                   |
| `cc13x2_26x2_pac 0.10.3` register-block addresses | crates.io `cc13x2_26x2_pac` 0.10.3 (BSD-3-Clause; SVD source `cc13x2_26x2.svd` from `seanmlyons22/ti-lprf-pacs`)         |
| `cortex-m-rt` linker-script conventions         | crates.io `cortex-m-rt` `link.x` template; the `MEMORY { FLASH ... RAM ... }` plus `REGION_ALIAS` shape it consumes         |
| Initiative ratification (parent)                | [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md) §7 "template emission contract", §10 "reconciliation"                  |
| Sibling slate execution                         | CHIPS-TI-01b (templates), -01c (render snapshots), -01d (compile-verify gate), -01e (PAC 0.10 amendments)                   |

If a phase needs to **modify** a cited authority (different PAC vintage;
amendment to the cortex-m-rt `link.x` contract; new chip family whose
linker shape differs from the SimpleLink Cortex-M4F precedent) the
modification ratifies in a §15 amendment **first**, in a separate PR,
before any behaviour PR rides on it.

## §1 Purpose

The TI BSP generator emits **eight files** per board, not six. Beyond
the six Rust files frozen in [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
§5.3, every generated board ships with two linker fragments:

```text
memory.x      (rust-embedded `MEMORY { ... }` + `REGION_ALIAS(...)` block)
<chip>.x      (per-chip section directives; CCFG placement for SimpleLink)
```

These fragments cover two needs:

1. **`memory.x`** declares the chip's address space so that
   `cortex-m-rt`'s `link.x` can resolve `ORIGIN(FLASH)` / `LENGTH(FLASH)`
   / `ORIGIN(RAM)` / `LENGTH(RAM)` without the consuming crate having
   to author its own memory map. This is the standard pattern used by
   every chipdb-driven BSP in the workspace (the Espressif tree, the
   Silicon Labs / Microchip trees, etc.).
2. **`<chip>.x`** declares chip-specific sections that the cortex-m-rt
   default `link.x` does not know about. For the SimpleLink Cortex-M4F
   family this is the **Customer Configuration Area (CCFG)** — a
   fixed-address 88-byte structure in the last Flash sector that the
   chip's boot ROM reads on every reset (SWCU185G §11.1 Table 11-1).
   Without CCFG present the chip will not run user code; without
   `<chip>.x` the consuming crate cannot place a `#[link_section =
   ".ccfg"]` static at the correct offset.

The 8-file emission set has been shipping since CHIPS-TI-01 slate 5;
this chapter retroactively ratifies it.

## §2 Problem statement

Linker emission was deferred to a future -05 chapter at
`CHIPS-TI-00-CONCEPTS.md` §11 ("Linker emission") without a normative
specification. In the intervening slates the generator grew working
`memory.x.jinja` + `cc1352_r.x.jinja` templates, the render test
asserts the 8-file emission set
([`tests/bsp_ti_cc1352r_render.rs:41`](../../../tests/bsp_ti_cc1352r_render.rs)
`assert_eq!(written.len(), 8)`), and the compile-verify gate
([`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs))
type-checks the emitted scripts against `cc13x2_26x2_pac 0.10.3` on
`thumbv7em-none-eabihf`. The behaviour is correct; the spec gap is
that no concepts doc names which addresses, lengths, and section
layouts are **frozen** vs. which are template-discretion.

A future agent inspecting this corner of the chipdb could plausibly
move CCFG to a different offset, rename `<chip>.x` to a generic name
shared across vendors, or fold the CCFG section into `memory.x`
itself, with no concepts-doc citation to push back against. This
chapter closes that gap.

## §3 Canonical glossary

Reserved CHIPS-TI-05 vocabulary. Cite-vs-restate markers follow the
convention in CHIPS-TI-00 §3.

- **`memory.x`** — *As defined in the `cortex-m-rt` `link.x` template
  (crates.io `cortex-m-rt`); used without modification.* The linker
  fragment that declares the chip's address space via `MEMORY { ... }`
  and provides `REGION_ALIAS` lines that map chip-specific region
  names to the canonical `FLASH` / `RAM` symbols that `link.x`
  consumes. Emitted by `memory.x.jinja` for every CHIPS-TI board.
- **`<chip>.x`** — *Owned by this chapter; concrete file name is
  `<chip_stem>.x` where `<chip_stem>` is the snake-cased chip name
  from `CHIPS-TI-00` §5.1 (e.g. `cc1352_r.x` for `CC1352R`).* The
  linker fragment that supplies chip-specific section directives
  beyond the cortex-m-rt default. For the SimpleLink Cortex-M4F
  family this is the CCFG `SECTIONS { .ccfg ... } INSERT BEFORE
  .text;` block.
- **CCFG (Customer Configuration Area)** — *As defined in TI SWCU185G
  §11.1 "Customer Configuration Area (CCFG)" Table 11-1.* An 88-byte
  fixed-layout structure that lives in the **last** 88 bytes of
  internal Flash (`0x00057FA8..0x00057FFF` on CC1352R1F3) and is
  consumed by the SimpleLink boot ROM at chip reset. Fields include
  bootloader configuration, debug-port lock, TI-RTOS-mode select,
  SRAM repurpose flags, and the application-image vector-table
  pointer. Absent or malformed CCFG → chip does not run user code.
- **FLASH region** — *As defined in CHIPS-TI-00 §3 "Memory region";
  used here as the `REGION_ALIAS("FLASH", ...)` target.* The 352-KB
  internal Flash region at `0x00000000..0x00058000` on CC1352R1F3
  (`memory:` array `name: flash` row in
  [`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml`](../db/chips/CC1352R.yaml)).
- **SRAM region** — *As defined in CHIPS-TI-00 §3 "Memory region";
  used here as the `REGION_ALIAS("RAM", ...)` target.* The 80-KB
  System SRAM region at `0x20000000..0x20014000` on CC1352R1F3
  (five 16-KB blocks per SWCU185G §8.2). Maps to canonical `RAM`.
- **REGION\_ALIAS** — *As defined in the GNU `ld` 2.41 manual
  §3.5.2; used without modification.* The linker directive that
  creates a second name for an existing memory region. Used to
  bridge the chip-specific region names (`flash`, `sram`, `gpram`)
  to the canonical names that `cortex-m-rt`'s `link.x` consumes
  (`FLASH`, `RAM`, `REGION_TEXT`, `REGION_DATA`, etc.).
- **`linker:` block** — *As defined in CHIPS-TI-00 §5.1 chip-IR
  required field set extension; concretely populated in
  [`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml:517`](../db/chips/CC1352R.yaml).*
  The optional chip-YAML block carrying linker-template data:
  `region_text` (alias source for `FLASH`), `region_data` (alias
  source for `RAM`), `ccfg_origin`, `ccfg_length`. When the block is
  absent or missing the CCFG fields, the CCFG `SECTIONS` directive
  is **NOT** emitted (`{% if ir.chip.linker.ccfg_origin is defined
  ... %}` branch in `chip.x.jinja`).

## §4 Source-of-truth map

| Concept                                                | Owner (canonical)                                                       | Mirrored / consumed by                                                                       |
| ------------------------------------------------------ | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| FLASH origin / length                                  | Chip IR YAML `memory:` array, `name: flash` row (SWCU185G §1.5 Table 1-5) | `memory.x.jinja` `MEMORY { FLASH : ... }` line; `REGION_ALIAS("FLASH", ...)`                |
| SRAM origin / length                                   | Chip IR YAML `memory:` array, `name: sram` row (SWCU185G §1.5 Table 1-5)  | `memory.x.jinja` `MEMORY { SRAM : ... }` line; `REGION_ALIAS("RAM", ...)`                  |
| Auxiliary regions (ROM, GPRAM, AUX_RAM, PERIPHERALS, FCFG1, CPU_SCS) | Chip IR YAML `memory:` array, per-region row                          | `memory.x.jinja` `MEMORY { ... }` block; **NOT** aliased to canonical names — informational |
| `REGION_ALIAS("FLASH", ...)` target name               | Chip IR YAML `linker.region_text`                                       | `memory.x.jinja` line 23 (`REGION_ALIAS("FLASH", {{ ... }});`)                              |
| `REGION_ALIAS("RAM", ...)` target name                 | Chip IR YAML `linker.region_data`                                       | `memory.x.jinja` line 24                                                                     |
| CCFG origin (per-chip fixed address)                   | Chip IR YAML `linker.ccfg_origin` (SWCU185G §11.1)                      | `chip.x.jinja` `.ccfg ORIGIN(FLASH) + LENGTH(FLASH) - ccfg_length` arithmetic               |
| CCFG length                                            | Chip IR YAML `linker.ccfg_length` (SWCU185G §11.1 Table 11-1: 88 bytes) | `chip.x.jinja` `SECTIONS` directive size                                                     |
| `<chip>.x` file name                                   | Chip stem (snake_case chip name)                                        | `render_ti_pac` output path; consumed by `examples/` `build.rs` via `-T<chip>.x`            |
| cortex-m-rt `link.x` integration                       | Upstream `cortex-m-rt` crate (the `link.x` it ships)                    | Consumer crate's `build.rs` `println!("cargo:rustc-link-arg=-Tlink.x");` line               |
| 8-file emission count                                  | This chapter §5                                                         | `tests/bsp_ti_cc1352r_render.rs:41` `assert_eq!(written.len(), 8)`                          |

## §5 Frozen decisions — emission shape

Each decision below names its registration policy per the
*Frozen enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Linker file set — Standards Action

The CHIPS-TI BSP generator MUST emit exactly two linker fragments per
board, with the file names:

```text
memory.x
<chip_stem>.x      (concretely cc1352_r.x for CC1352R)
```

`<chip_stem>` is the snake_case form of the chip name from CHIPS-TI-00
§5.1. Renaming `<chip_stem>.x` to a generic name (e.g. `chip.x` or
`linker.x`), or splitting either fragment into multiple files, is
Standards Action and requires a §15 amendment here. Adding a third
linker fragment (e.g. a separate `boot.x` or `vectors.x`) is also
Standards Action.

This is consistent with CHIPS-TI-00 §5.3 which lists `memory.x` +
`chip.x` as the two linker-script templates. The concrete file name
substitution (`chip.x` → `<chip_stem>.x`) is ratified **here** rather
than in -00; the -00 listing used the generic placeholder name for
cross-vendor uniformity in that doc.

### 5.2 CC1352R memory map — Standards Action

The following addresses are **frozen** for `CC1352R` as a SimpleLink
Cortex-M4F member chip. Values are sourced from SWCU185G §1.5 Table
1-5 (Memory Map) and cross-checked against SWRS196I §9.4 (CC1352R1F3
Memory Organisation). Modifying any row requires a §15 amendment
here and an update to
[`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml`](../db/chips/CC1352R.yaml)
in the same PR.

| Region        | Origin       | Length       | Access | Source                       |
| ------------- | ------------ | ------------ | ------ | ---------------------------- |
| `FLASH`       | `0x00000000` | `0x00058000` (352 KB) | `rx`   | SWCU185G §1.5 / SWRS196I §9.4 |
| `ROM`         | `0x10000000` | `0x00040000` (256 KB) | `rx`   | SWCU185G §1.5 (TI-RTOS + driverlib + bootloader) |
| `GPRAM`       | `0x11000000` | `0x00002000` (8 KB)   | `rwx`  | SWCU185G §8.2 (cache repurposed when `CCFG.SRAM_CFG.SRAM_REPL=0`) |
| `SRAM`        | `0x20000000` | `0x00014000` (80 KB)  | `rwx`  | SWCU185G §1.5 (five 16-KB blocks) |
| `AUX_RAM`     | `0x400E0000` | `0x00001000` (4 KB)   | `rwx`  | SWCU185G §1.5 (Sensor Controller scratch) |
| `PERIPHERALS` | `0x40000000` | `0x00100000` (1 MB)   | `rw`   | SWCU185G §1.5 (MCU + AON + AUX peripheral region) |
| `FCFG1`       | `0x50001000` | `0x00001000` (4 KB)   | `r`    | SWCU185G §1.5 (Factory Configuration) |
| `CPU_SCS`     | `0xE0000000` | `0x00100000` (1 MB)   | `rw`   | ARMv7-M ARM (System Control Space) |

Quick-reference snippet (matches the emitted `memory.x` and the
[`memory_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap)
golden file verbatim):

```text
MEMORY
{
  FLASH       : ORIGIN = 0x00000000, LENGTH = 0x00058000    /* rx  */
  ROM         : ORIGIN = 0x10000000, LENGTH = 0x00040000    /* rx  */
  GPRAM       : ORIGIN = 0x11000000, LENGTH = 0x00002000    /* rwx */
  SRAM        : ORIGIN = 0x20000000, LENGTH = 0x00014000    /* rwx */
  AUX_RAM     : ORIGIN = 0x400E0000, LENGTH = 0x00001000    /* rwx */
  PERIPHERALS : ORIGIN = 0x40000000, LENGTH = 0x00100000    /* rw  */
  FCFG1       : ORIGIN = 0x50001000, LENGTH = 0x00001000    /* r   */
  CPU_SCS     : ORIGIN = 0xE0000000, LENGTH = 0x00100000    /* rw  */
}

REGION_ALIAS("FLASH", FLASH);
REGION_ALIAS("RAM",   SRAM);

/* Optional aliases for newer link.x scripts. */
REGION_ALIAS("REGION_TEXT",   FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA",   SRAM);
REGION_ALIAS("REGION_BSS",    SRAM);
REGION_ALIAS("REGION_HEAP",   SRAM);
REGION_ALIAS("REGION_STACK",  SRAM);
```

### 5.3 CC1352R CCFG placement — Standards Action

The CCFG section MUST be placed at the **end** of the FLASH region,
sized at 88 bytes (`0x58`). The arithmetic is
`ORIGIN(FLASH) + LENGTH(FLASH) - 0x58 = 0x00057FA8` for CC1352R1F3.
Authority: SWCU185G §11.1 "Customer Configuration Area (CCFG)" Table
11-1 (CCFG layout, 88 bytes total).

Quick-reference snippet (matches the emitted `cc1352_r.x` and the
[`cc1352_r_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap)
golden file verbatim):

```text
SECTIONS
{
  .ccfg ORIGIN(FLASH) + LENGTH(FLASH) - 0x00000058 :
  {
    KEEP(*(.ccfg))
    . = ALIGN(4);
  } > FLASH
} INSERT BEFORE .text;
```

The `KEEP(*(.ccfg))` directive prevents `--gc-sections` from
discarding the CCFG static if the consumer crate is the only
emitter. The `INSERT BEFORE .text` clause ensures the CCFG layout
is resolved before the application's reset vector is placed,
matching the precedent set by TI's reference linker scripts (e.g.
`ti-cgt-armllvm` CCFG samples). The consuming crate MUST define a
`#[link_section = ".ccfg"]` static covering the full 88 bytes for
the SimpleLink boot ROM to find on reset.

### 5.4 `linker:` chip-YAML block — Specification Required

The optional `linker:` block in chip-IR YAML carries the fields
consumed by `memory.x.jinja` and `<chip>.x.jinja`:

```yaml
linker:
  region_text:  flash         # alias source for cortex-m-rt FLASH
  region_data:  sram          # alias source for cortex-m-rt RAM
  ccfg_origin:  0x00057FA8    # CCFG base (last 88 bytes of FLASH)
  ccfg_length:  0x00000058    # 88 bytes (SWCU185G §11.1 Table 11-1)
```

Adding a field to the `linker:` block (e.g. for future TI families
that need a vector-relocation directive, or a non-CCFG fixed-address
section) is Specification Required — the per-chapter walkthrough that
introduces the field updates `<chip>.x.jinja` and re-blesses the
`<chip>_x.snap` snapshot in the same PR. Removing a field (e.g. if
SimpleLink boot ROM evolves to not require CCFG) is Standards Action.

Chips that do **NOT** need a chip-specific section directive (`<chip>.x`
content beyond the file header) MAY omit `ccfg_origin` / `ccfg_length`;
the template's `{% if ir.chip.linker.ccfg_origin is defined %}` branch
emits an explanatory no-CCFG comment instead of a `SECTIONS` block.
This branch is the extension point for non-SimpleLink chips that join
CHIPS-TI-00 §5.5 in the future.

### 5.5 cortex-m-rt linker integration — Specification Required

The emitted `memory.x` MUST resolve `FLASH` and `RAM` via the standard
`REGION_ALIAS` lines so that an unmodified `cortex-m-rt` `link.x`
template links successfully. The consuming crate's `build.rs` MUST
emit the linker-arg sequence that loads the two fragments in the
documented order:

```rust
// In examples/<board>/build.rs:
println!("cargo:rustc-link-arg=-Tmemory.x");
println!("cargo:rustc-link-arg=-Tcc1352_r.x");   // or -T<chip_stem>.x
println!("cargo:rustc-link-arg=-Tlink.x");       // from cortex-m-rt
```

Ordering rationale: `memory.x` declares the `MEMORY` block first;
`<chip>.x` references `ORIGIN(FLASH)` / `LENGTH(FLASH)` in its CCFG
`SECTIONS` directive (so `memory.x` MUST be parsed first); `link.x`
from cortex-m-rt then places `.text`/`.rodata`/`.data`/`.bss` using
the `REGION_ALIAS` names that `memory.x` set up.

This ordering is **NOT** enforced by the generator — the consuming
crate's `build.rs` is the source of truth. Misordering by the consumer
manifests as `undefined reference to ORIGIN(FLASH)` link errors, not
silent miscompilation, so the failure mode is loud.

## §6 Frozen decisions — file path emission

The two linker fragments MUST be written to the same per-board output
directory as the six Rust files
([`tests/bsp_ti_cc1352r_render.rs:42`](../../../tests/bsp_ti_cc1352r_render.rs)
`tmp.path().join("launchxl_cc1352_r1")`). Concrete paths for the
LAUNCHXL-CC1352R1 board:

```text
<out>/launchxl_cc1352_r1/memory.x
<out>/launchxl_cc1352_r1/cc1352_r.x
```

Co-locating the linker scripts with the Rust BSP files means the
consuming crate's `build.rs` can compute one path prefix and reference
both fragment types from it. The render test
[`tests/bsp_ti_cc1352r_render.rs:41`](../../../tests/bsp_ti_cc1352r_render.rs)
asserts the total emission count of 8 — moving either fragment to a
different directory would break that assertion.

Modifying the per-board directory layout is Standards Action.

## §7 Verification gates

The verification surface for linker emission is the same three-layer
contract that CHIPS-TI-00 §8 names for Rust-file emission:

1. **Snapshot render test** — `tests/bsp_ti_cc1352r_render.rs`
   produces text-level snapshots for both `memory.x` and `cc1352_r.x`
   ([`bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap),
   [`bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap)).
   Any template diff without an `insta` re-bless fails this gate.
2. **Compile-verify test** — `tests/bsp_ti_cc1352r_compile.rs` runs
   `cargo check --target thumbv7em-none-eabihf` against the emitted
   BSP and the real `cc13x2_26x2_pac 0.10.3` PAC crate. The
   throwaway cargo project the test materialises consumes the
   emitted linker scripts via its `build.rs`; a broken `memory.x`
   surfaces as a link error here. Currently green (CHIPS-TI-01e §15
   2026-05-13).
3. **`examples/` consumer crate** — Deferred to a future CHIPS-TI-06
   chapter. No `examples/launchxl-cc1352r1/` crate exists today;
   when it lands its `build.rs` MUST follow the linker-arg sequence
   in §5.5.

Promoting the snapshot or compile-verify test to a stricter form
(e.g. asserting `objdump`-extracted CCFG section offset against the
TRM rather than text-comparing the linker script) is Specification
Required — write the assertion, update this section, re-bless.

## §8 Round-trip / regression posture

A CHIPS-TI-NN execution PR that modifies `memory.x.jinja` or
`<chip>.x.jinja` MUST update both snapshots in the same PR
([`memory_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap),
[`cc1352_r_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap)).
A PR that modifies the chip-YAML `memory:` array or `linker:` block
MUST update both the snapshot **and** re-run the compile-verify test
(CHIPS-TI-00 §9, cited without restatement).

This chapter introduces no new regression posture beyond CHIPS-TI-00
§9; it inherits that contract for the linker-fragment surface.

## §9 Non-goals

Explicit out-of-scope for v0 of this chapter:

- **No emission of CCFG content.** The generator emits only the
  `SECTIONS { .ccfg ... }` placement directive; the consuming crate
  authors its own `#[link_section = ".ccfg"]` static per SWCU185G
  §11.1 Table 11-1. Generating the CCFG static body (boot-mode
  bytes, debug-port lock, TI-RTOS-mode select, etc.) from chipdb
  YAML would couple the BSP generator to TI driverlib's
  `CCFG_FIELDS` macro layout and is deferred.
- **No emission of stack-size or heap-size directives.** Consuming
  crates set `_stack_start` / `_stack_end` via cortex-m-rt's existing
  `_stack_size` mechanism; the generator does not pre-allocate.
- **No BLE / Zigbee / Thread / TI 15.4 protocol-stack flash regions.**
  Application crates that link against TI BLE5-Stack or
  TI 15.4-Stack manage their own region split; the generator
  emits a flat FLASH region.
- **No XIP from QSPI external Flash.** SimpleLink Cortex-M4F parts
  in the §5.5 family do not have a QSPI XIP controller. If a future
  member (e.g. a hypothetical CC13x4 with external Flash) joins,
  this is a §15 amendment.
- **No multi-image boot layout** (TI MCUBoot OTA, dual-bank flash).
  Bring-up is single-image; OTA scaffolding is a separate
  initiative.
- **No emission tuned to a different PAC vintage.** The frozen
  values in §5.2 / §5.3 reflect `cc13x2_26x2_pac 0.10.3`. A future
  PAC bump that changes peripheral base addresses (e.g. consolidated
  `cc26xx` crate replacing the per-family PAC) is Standards Action
  here and in CHIPS-TI-00 §6.

## §10 Reconciliation with adjacent repo primitives

### 10.1 cortex-m-rt `link.x` ownership

The `memory.x` fragment **does not replace** `cortex-m-rt`'s
`link.x` — it supplies the `MEMORY` block + `REGION_ALIAS` lines that
`link.x` expects to find. The consuming crate continues to pass
`-Tlink.x` to the linker (as the standard cortex-m-rt
`build.rs`-less / `build.rs`-driven workflow already does). The
generator MUST NOT emit its own `link.x`; doing so would compete
with cortex-m-rt over `.text` / `.data` / `.bss` placement and is
explicitly forbidden under §9.

### 10.2 Espressif tree precedent

The Espressif tree's linker emission (e.g.
`src/bin/creator/bsp/espressif/templates/memory.x.jinja`) follows
the same `MEMORY { ... }` + `REGION_ALIAS` shape but emits against
the ESP RISC-V memory model (`riscv32imc-unknown-none-elf` and
friends). The two trees share **vocabulary** (`region_text`,
`region_data` chip-YAML field names) but not concrete addresses —
addresses are per-chip and live in their respective chip-YAML
`memory:` arrays. Renaming a shared chip-YAML field (e.g.
`region_text` → `text_region`) is Standards Action across **all**
chipdb vendor trees, not just CHIPS-TI; coordination happens at
the `docs/bsp/` initiative level, not here.

### 10.3 CCFG section placement vs. cortex-m-rt vector table

cortex-m-rt's `link.x` places the vector table at `ORIGIN(FLASH)`
(i.e. `0x00000000` on CC1352R1F3). The CCFG section lands at the
**end** of FLASH (`0x00057FA8`). The two never alias; the `INSERT
BEFORE .text` clause in the emitted `cc1352_r.x` ensures the CCFG
layout is resolved before `.text` is placed but does not move
`.text` away from `ORIGIN(FLASH)`. SimpleLink boot ROM jumps to the
application's reset vector at `ORIGIN(FLASH)+4` only after reading
CCFG, so the dual-end layout is correct by construction.

### 10.4 BeagleBone Black hand-written prong

Per CHIPS-TI-00 §10.1 the AM335x / BeagleBone Black prong is OUT of
scope for the chipdb-driven generator. The hand-written BBB linker
script lives at `examples/beaglebone-black/src/bare/link.ld` and
does **not** consume `memory.x` / `<chip>.x`. CHIPS-TI-05 makes no
claim over that file; future BBB-NN PRs MAY modify it without
amending this chapter.

## §11 Non-goals

(See §9 — kept under that heading rather than re-titled here so the
chapter stays close to the standard §0–§15 shape; per CLAUDE.md
"per-chapter docs MAY omit sections that do not apply".)

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [x] §0 authority table reviewed; SWCU185G §1.5 (Memory Map) and
      §11.1 (CCFG) confirmed as authority for §5.2 / §5.3 frozen
      values.
- [x] §3 glossary terms each carry a cite-vs-restate marker per
      CLAUDE.md §"Definitions — reference vs. restatement".
- [x] §4 source-of-truth map has exactly one owner per row.
- [x] §5.1 file-set frozen; concrete file-name substitution
      `<chip>.x` → `<chip_stem>.x` is explicit.
- [x] §5.2 CC1352R memory map values match
      [`CC1352R.yaml:49-57`](../db/chips/CC1352R.yaml) verbatim and
      match the [`memory_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap)
      golden file.
- [x] §5.3 CCFG placement values match
      [`CC1352R.yaml:520-521`](../db/chips/CC1352R.yaml) verbatim and
      match the [`cc1352_r_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap)
      golden file.
- [x] §5.5 cortex-m-rt linker-arg sequence consistent with the
      `examples/beetle-esp32c3/` precedent.
- [x] §7 verification gates already implemented and green
      (CHIPS-TI-01c render test; CHIPS-TI-01d compile-verify test).
- [x] §10.1 cortex-m-rt boundary documented; §10.4 BBB carve-out
      preserved.
- [x] §15 dated ratification entry.

## §13 Files cited

- [`CLAUDE.md`](../../../CLAUDE.md) — Spec-Before-Code Planning
  Discipline, RFC 2119 keywords, registration policy, initiative
  prefix.
- [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
  — parent concepts doc; §7 template emission contract, §11 linker-
  emission deferral, §15 slate change log.
- [`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml`](../db/chips/CC1352R.yaml)
  — chip-IR YAML; authoritative source for §5.2 memory-map values
  and §5.3 CCFG values.
- [`src/bin/creator/bsp/ti/templates/memory.x.jinja`](../../../src/bin/creator/bsp/ti/templates/memory.x.jinja)
  — `memory.x` template; consumes `ir.chip.memory` and
  `ir.chip.linker.region_*` fields.
- [`src/bin/creator/bsp/ti/templates/chip.x.jinja`](../../../src/bin/creator/bsp/ti/templates/chip.x.jinja)
  — `<chip>.x` template; consumes `ir.chip.linker.ccfg_*` fields.
- [`tests/bsp_ti_cc1352r_render.rs`](../../../tests/bsp_ti_cc1352r_render.rs)
  — render test; asserts the 8-file emission set (line 41) and
  snapshots both linker fragments (lines 113-125).
- [`tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__memory_x.snap)
  — `memory.x` golden snapshot.
- [`tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap`](../../../tests/snapshots/bsp_ti_cc1352r_render__launchxl_cc1352r1__cc1352_r_x.snap)
  — `cc1352_r.x` golden snapshot.
- [`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs)
  — `compile-verify` test; consumes the emitted linker fragments
  end-to-end against `cc13x2_26x2_pac 0.10.3` on
  `thumbv7em-none-eabihf`.
- TI SWCU185G — CC13x2/CC26x2 SimpleLink Wireless MCU TRM; §1.5
  Memory Map (authority for §5.2), §11.1 CCFG (authority for §5.3).
- TI SWRS196I — CC1352R datasheet; §9.4 Memory Organisation
  (cross-check for §5.2).
- ARM ARMv7-M Architecture Reference Manual — System Control Space
  (`CPU_SCS` row in §5.2).
- crates.io `cortex-m-rt` `link.x` template — consumed by the
  emitted `memory.x` via `REGION_ALIAS("FLASH", ...)` /
  `REGION_ALIAS("RAM", ...)`.
- crates.io `cc13x2_26x2_pac 0.10.3` — peripheral base addresses
  cross-checked against SWCU185G §1.5; consumed by `compile-verify`.

## §14 Unblocks

Ratifying this chapter unblocks:

- **CHIPS-TI-06** — example crate (`examples/launchxl-cc1352r1/`
  or similar) consuming the generated BSP with a `build.rs` that
  follows §5.5's linker-arg sequence. No example crate exists for
  TI today; CHIPS-TI-06 is the natural follow-on to -01e + -05.
- **Future SimpleLink chip additions** (CC2652R / CC1312R /
  CC2652RB / CC1352P / CC2652P per CHIPS-TI-00 §5.5 sibling note).
  Each new chip-YAML re-uses the §5.4 `linker:` block shape;
  per-chip frozen-decision tables analogous to §5.2 / §5.3 land
  in §15 amendments here when the chip joins.
- **Future non-SimpleLink chip additions** that pick up CHIPS-TI-00
  §5.5 membership (e.g. hypothetical CC4x line). Those chips use
  the §5.4 `linker:` block extension point — if their chip-specific
  section is **not** CCFG (e.g. a different boot-ROM header
  structure), the `<chip>.x.jinja` template's existing
  `{% if linker.ccfg_origin is defined %}` branch absorbs the
  delta; alternative section types are §15 amendments.

## §15 Change log

| Date       | Status                       | Note                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Linker emission chapter ratified for the already-shipping 8-file emission set (6 .rs + `memory.x` + `cc1352_r.x`). Closes the [CHIPS-TI-00 §11](CHIPS-TI-00-CONCEPTS.md#11-non-goals) "linker emission deferred to -05" item. `CC1352R` frozen-decision tables in §5.2 / §5.3 codify the values that have been shipping since slate 5 and that the `bsp_ti_cc1352r_compile` gate type-checks since -01e. Document-only; no template, YAML, or test changes. |
