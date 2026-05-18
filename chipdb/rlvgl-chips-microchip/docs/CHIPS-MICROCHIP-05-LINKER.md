<!--
CHIPS-MICROCHIP-05-LINKER.md - Linker emission contract for the Microchip
SAM chipdb + BSP-generator initiative. Ratifies the already-shipping
`memory.x` template and adds the missing per-chip `<chip>.x` template so
the MICROCHIP emission set reaches parity with the CHIPS-TI tree (8
files per board).
-->

# CHIPS-MICROCHIP-05 — Microchip SAM BSP Linker Emission

> **Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.
> Closes the [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §11
> "linker emission deferred to a future chapter" item by ratifying the
> normative emission set `{memory.x, atsamd51j19a.x}` for the
> ATSAMD51J19A chip. Future linker-script behaviour changes route
> through this chapter's §15 amendment process; no behaviour PR rides
> on an unamended invariant.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared
in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline". RFC 2119 / RFC 8174 normative keywords (**MUST**,
**MUST NOT**, **SHALL**, **SHOULD**, **MAY**) carry their RFC meanings
when capitalised; lowercase use is narrative.

| Domain                                          | Authoritative source                                                                                                          |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| ATSAMD51J19A chip memory map                    | Microchip SAM D5x/E5x Family Data Sheet, DS60001507F (rev F, 2020-09), §10 "Physical Memory Map" Table 10-1; §11.2 Memory Map |
| ATSAMD51J19A errata                             | Microchip DS80000748 (SAM D5x/E5x errata sheet)                                                                               |
| Cortex-M4F architectural semantics              | ARM ARMv7-M Architecture Reference Manual (DDI 0403E.e)                                                                       |
| `atsamd51j19a 0.7.1` register-block addresses   | crates.io `atsamd51j19a` 0.7.1 (atsamd-rs/atsamd workspace; svd2rust output)                                                  |
| `cortex-m-rt` linker-script conventions         | crates.io `cortex-m-rt` `link.x` template; the `MEMORY { FLASH ... RAM ... }` plus `REGION_ALIAS` shape it consumes           |
| `atsamd51j19a` PAC build-script `device.x`      | The PAC crate's own `build.rs` emits `device.x` carrying the chip's interrupt-vector extensions; consumed via cortex-m-rt's `link.x` `INCLUDE device.x` line |
| Initiative ratification (parent)                | [`CHIPS-MICROCHIP-00-CONCEPTS.md`](CHIPS-MICROCHIP-00-CONCEPTS.md) §6 INV-MC6 template emission contract; §10 reconciliation; §11 #N linker-emission deferral |
| Sibling slate execution                         | CHIPS-MICROCHIP-01 (chip + board YAML), -02 (renderer + templates), -02 amendment (field-style PAC access), -01a (PB22/PB23 PMUX fix), -04 (compile-verify gate) |
| Cross-vendor precedent                          | [`CHIPS-TI-05-LINKER.md`](../../rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md) (TI SimpleLink Cortex-M4F linker emission, ratified 2026-05-14) |

If a phase needs to **modify** a cited authority (different PAC vintage;
amendment to the cortex-m-rt `link.x` contract; addition of a new D5x or
D21 chip whose linker shape differs from ATSAMD51J19A) the modification
ratifies in a §15 amendment **first**, in a separate PR, before any
behaviour PR rides on it.

## §1 Purpose

Slate 6 of the CHIPS-MICROCHIP initiative (`-01a` + `-02`) brought the
MICROCHIP BSP generator up to **7 files** per board (6 `.rs` files
listed in [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §6
INV-MC6 plus `memory.x`). The corresponding TI tree ships **8 files**
per board (the same 6 `.rs` files plus `memory.x` *and* a per-chip
`<chip>.x`). CHIPS-MICROCHIP-00 §11 deferred the second linker fragment
("`<chip>.x`") to a future `-05` chapter.

CHIPS-MICROCHIP-05 closes the gap. It:

1. **Ratifies the already-shipping `memory.x` template** as a frozen
   member of the emission set, on the same `MEMORY { ... }` +
   `REGION_ALIAS(...)` shape the TI and ESP trees use.
2. **Adds the missing per-chip `<chip>.x` template** (concretely
   `atsamd51j19a.x.jinja`) so the MICROCHIP emission set reaches parity
   with the TI tree at 8 files per board.
3. **Documents the layering interaction** with `atsamd51j19a 0.7.1`'s
   own `device.x` (emitted by the PAC's `build.rs`; consumed by
   cortex-m-rt's `link.x` via `INCLUDE device.x`). The generator's
   `<chip>.x` is **additive** to `device.x`, not a replacement.

The 8-file emission set unblocks `CHIPS-MICROCHIP-06` (example crate
under `examples/<microchip-board>/`) by giving the consuming crate's
`build.rs` a single, uniform pair of linker fragments to emit
`-T<name>.x` args against — same shape as the CHIPS-TI-06 surface.

## §2 Problem statement

Linker emission was deferred to a future `-05` chapter at
[`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §11 without a
normative specification. In the intervening slates (`-01`, `-01a`,
`-02`) the renderer grew a working `memory.x.jinja` template and the
render test asserts 7 files emit
([`tests/bsp_microchip_render.rs:33`](../../../tests/bsp_microchip_render.rs)
`assert_eq!(written.len(), 7)`), but no second linker fragment was
ratified. The compile-verify gate
[`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs)
type-checks the emitted BSP against `atsamd51j19a 0.7.1` on
`thumbv7em-none-eabihf` since slate 6 (CHIPS-MICROCHIP-04 of
2026-05-13).

The spec gap is twofold:

1. There is no concepts-doc citation declaring which addresses,
   lengths, and section layouts in `memory.x` are **frozen** vs.
   template-discretion. A future agent could rename `flash` → `code`
   or move `bkupram` to a different alias without any spec to push
   back against.
2. The emission set is intentionally incomplete: TI ships
   `<chip>.x` for SimpleLink-specific CCFG placement; the MICROCHIP
   tree has no equivalent slot established for chip-specific section
   directives (NVMCTRL fuse rows, future SmartEEPROM emulation, an
   alternate `BOOTPROT`-aware FLASH region split). When a future SAM
   D5x family member needs chip-specific linker directives, the slot
   does not exist.

This chapter closes both gaps.

## §3 Canonical glossary

Reserved CHIPS-MICROCHIP-05 vocabulary. Cite-vs-restate markers follow
the convention in [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md)
§3.

- **`memory.x`** — *As defined in the `cortex-m-rt` `link.x` template
  (crates.io `cortex-m-rt`); used without modification.* The linker
  fragment that declares the chip's address space via `MEMORY { ... }`
  and provides `REGION_ALIAS` lines that map chip-specific region
  names to the canonical `FLASH` / `RAM` symbols that `cortex-m-rt`'s
  `link.x` consumes. Emitted by
  [`memory.x.jinja`](../../../src/bin/creator/bsp/microchip/templates/memory.x.jinja)
  for every CHIPS-MICROCHIP board.
- **`atsamd51j19a.x`** — *Owned by this chapter; concrete file name is
  `<chip_link_stem>.x` where `<chip_link_stem>` is the lowercase-no-
  separator form of the chip name from CHIPS-MICROCHIP-00 §5 (e.g.
  `atsamd51j19a.x` for `ATSAMD51J19A`).* The linker fragment that
  supplies chip-specific section directives beyond the cortex-m-rt
  default `link.x` and beyond the PAC's auto-generated `device.x`. In
  v0 of this chapter the file is a **slot template** — header comment
  plus intentionally empty body — because the ATSAMD51J19A's `device.x`
  (auto-generated by `atsamd51j19a 0.7.1`'s `build.rs`) already covers
  the chip's interrupt-vector extensions. The slot exists so future
  D5x / D21 / L21 chips that DO need additional section directives can
  populate it without an emission-shape Standards Action.
- **FLASH region** — *As defined in
  [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §3 "Memory
  region"; used here as the `REGION_ALIAS("FLASH", ...)` target.* The
  512-KB internal Flash region at `0x00000000..0x00080000` on
  ATSAMD51J19A (`memory:` array `name: flash` row in
  [`chipdb/rlvgl-chips-microchip/db/chips/ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml)).
- **SRAM region** — *As defined in
  [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §3 "Memory
  region"; used here as the `REGION_ALIAS("RAM", ...)` target.* The
  192-KB on-chip SRAM region at `0x20000000..0x20030000` on
  ATSAMD51J19A. The D5x family's SRAM is a single contiguous block
  (unlike the SimpleLink Cortex-M4F's five-bank split).
- **BKUPRAM region** — *As defined in DS60001507F §10 Table 10-1;
  used here as a supplementary `MEMORY { ... }` entry, NOT aliased
  to a canonical name.* The 8-KB backup SRAM region at
  `0x47000000..0x47002000` retained across `STANDBY` and `BACKUP`
  sleep modes (DS60001507F §11.2 Table 11-1). Available to
  application code via `extern "C" { static mut BKUPRAM_START: ...; }`
  patterns; not consumed by cortex-m-rt's `link.x`.
- **REGION\_ALIAS** — *As defined in the GNU `ld` 2.41 manual §3.5.2;
  used without modification.* The linker directive that creates a
  second name for an existing memory region. Used to bridge the
  chip-specific region names (`flash`, `ram`, `bkupram`) to the
  canonical names that cortex-m-rt's `link.x` consumes (`FLASH`,
  `RAM`).
- **`linker:` block** — *As defined in
  [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §6 INV-MC6
  chip-IR optional field set; concretely populated at
  [`ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml) (`linker:` /
  `region_text: flash` / `region_data: ram`).* The optional chip-YAML
  block carrying linker-template data: `region_text` (alias source for
  `FLASH`), `region_data` (alias source for `RAM`). When absent, the
  generator emits neither `memory.x` nor `<chip>.x`.
- **PAC `device.x`** — *As defined by the `svd2rust` output convention;
  the `atsamd51j19a` crate's `build.rs` writes this file into
  `OUT_DIR` at compile time and the consuming crate's `link.x` line
  `INCLUDE device.x` pulls in the chip's interrupt-vector extensions
  (NVIC entries beyond the cortex-m-rt default).* Used without
  modification by this chapter; the generator's `<chip>.x` is
  **additive** to `device.x`, not a replacement.

## §4 Source-of-truth map

| Concept                                                       | Owner (canonical)                                                                | Mirrored / consumed by                                                                       |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| FLASH origin / length                                         | Chip IR YAML `memory:` array, `name: flash` row (DS60001507F §10 Table 10-1)     | `memory.x.jinja` `MEMORY { FLASH : ... }` line; `REGION_ALIAS("FLASH", ...)`                |
| SRAM origin / length                                          | Chip IR YAML `memory:` array, `name: ram` row (DS60001507F §10 Table 10-1)       | `memory.x.jinja` `MEMORY { RAM : ... }` line; `REGION_ALIAS("RAM", ...)`                    |
| BKUPRAM origin / length                                       | Chip IR YAML `memory:` array, `name: bkupram` row (DS60001507F §10 Table 10-1)   | `memory.x.jinja` `MEMORY { BKUPRAM : ... }` line; informational — not aliased               |
| `REGION_ALIAS("FLASH", ...)` target name                      | Chip IR YAML `linker.region_text` (`flash`)                                      | `memory.x.jinja` `REGION_ALIAS("FLASH", FLASH);` line                                       |
| `REGION_ALIAS("RAM", ...)` target name                        | Chip IR YAML `linker.region_data` (`ram`)                                        | `memory.x.jinja` `REGION_ALIAS("RAM", RAM);` line                                            |
| `<chip>.x` file name                                          | Chip lowercase-no-separator stem (`atsamd51j19a`)                                 | `render_microchip_pac` output path; consumed by `examples/` `build.rs` via `-T<chip>.x`     |
| Chip-specific section directives (currently empty)            | This chapter §5.3 (deferred slot)                                                 | `atsamd51j19a.x.jinja` body (currently header-only)                                          |
| PAC `device.x` (NVIC vector extensions)                       | `atsamd51j19a 0.7.1` `build.rs` (auto-emitted into `OUT_DIR`)                     | cortex-m-rt's `link.x` `INCLUDE device.x` line; consumed without modification                |
| cortex-m-rt `link.x` integration                              | Upstream `cortex-m-rt` crate                                                     | Consumer crate's `build.rs` emits `cargo:rustc-link-arg=-Tlink.x`                            |
| 8-file emission count                                         | This chapter §5.1                                                                | `tests/bsp_microchip_render.rs:33` `assert_eq!(written.len(), 8)` (after -05a lands)         |

## §5 Frozen decisions — emission shape

Each decision below names its registration policy per the
*Frozen enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Linker file set — Standards Action

The CHIPS-MICROCHIP BSP generator MUST emit exactly two linker
fragments per board when the chip-YAML's `linker:` block is populated,
with the file names:

```text
memory.x
<chip_link_stem>.x        (concretely atsamd51j19a.x for ATSAMD51J19A)
```

`<chip_link_stem>` is the **lowercase-no-separator** form of the chip
name from [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §5
(distinct from the per-board `snake_case` `board_stem` used for the
Rust file directory name — `adafruit_feather_m4_express`). For
`ATSAMD51J19A` the linker stem is `atsamd51j19a` (no underscores). The
distinction from the TI tree's `cc1352_r.x` (which uses snake_case) is
intentional: Microchip's PAC crate name on crates.io is itself
all-lowercase no-separator (`atsamd51j19a`), so the linker file name
matches the PAC crate name for grep-ability against documentation.

Renaming `<chip_link_stem>.x` to a generic name (e.g. `chip.x` or
`linker.x`), or splitting either fragment into multiple files, is
Standards Action and requires a §15 amendment here. Adding a third
linker fragment (e.g. a separate `boot.x` or `bootloader_protection.x`
for the SAM D5x `BOOTPROT` row) is also Standards Action.

When the chip-YAML's `linker:` block is **absent**, neither fragment
emits — the chip is presumed to be either non-bringable from
cortex-m-rt or under active YAML stubbing.

### 5.2 ATSAMD51J19A memory map — Standards Action

The following addresses are **frozen** for `ATSAMD51J19A` as the first
non-stub SAM D5x family member. Values are sourced from DS60001507F
§10 Table 10-1 (Physical Memory Map). Modifying any row requires a
§15 amendment here and an update to
[`ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml) in the same PR.

| Region        | Origin       | Length                  | Access | Source                                |
| ------------- | ------------ | ----------------------- | ------ | ------------------------------------- |
| `FLASH`       | `0x00000000` | `0x00080000` (512 KB)   | `rx`   | DS60001507F §10 Table 10-1            |
| `RAM`         | `0x20000000` | `0x00030000` (192 KB)   | `rwx`  | DS60001507F §10 Table 10-1            |
| `BKUPRAM`     | `0x47000000` | `0x00002000` (8 KB)     | `rwx`  | DS60001507F §10 Table 10-1; §11.2     |

Quick-reference snippet (matches the emitted `memory.x` and the
[`memory_x.snap`](../../../tests/snapshots/bsp_microchip_render__adafruit_feather_m4_express__memory_x.snap)
golden file verbatim):

```text
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 0x00080000    /* rx */
  RAM : ORIGIN = 0x20000000, LENGTH = 0x00030000    /* rwx */
  BKUPRAM : ORIGIN = 0x47000000, LENGTH = 0x00002000    /* rwx */
}

REGION_ALIAS("FLASH", FLASH);
REGION_ALIAS("RAM",   RAM);
```

Out-of-scope rows omitted from the frozen table (and from
`memory.x`) but reachable from application code via the PAC and the
chip-specific `<chip>.x` extension slot:

- ITCM at `0x00000000` aliased into TCM mode (4 KB; §10.2 Table 10-2).
  Reachable only when `CMCC.CTRL.CEN` selects TCM mode and the boot
  loader has set the `NVMCTRL.SEEBLK` carve-out. Out of v0 scope.
- DTCM at `0x20000000` aliased into TCM mode (4 KB; §10.2). Same
  scope-out rationale.
- The Cortex-M System Control Space (`0xE0000000..0xE00FFFFF`,
  ARMv7-M ARM). cortex-m-rt's `link.x` handles SCS placement
  internally; the generator MUST NOT emit a SCS row.

### 5.3 ATSAMD51J19A chip-specific section directives — Specification Required

ATSAMD51J19A in v0 of this chapter has **no** generator-emitted
chip-specific section directives. The emitted
[`atsamd51j19a.x`](../../../src/bin/creator/bsp/microchip/templates/atsamd51j19a.x.jinja)
is a slot file — header comment block plus intentionally empty body.
The chip-specific concerns that COULD live here are all delegated
elsewhere in v0:

| Concern                                  | v0 delegation                                                                  |
| ---------------------------------------- | ------------------------------------------------------------------------------ |
| Interrupt-vector extensions (NVIC)       | `atsamd51j19a 0.7.1` `build.rs` emits `device.x`; cortex-m-rt's `link.x` `INCLUDE`s it |
| NVMCTRL fuse rows / user row             | Out of v0 scope (§11). NVM USER ROW writes happen at run-time, not link-time   |
| SmartEEPROM emulation region             | Out of v0 scope (§11). SmartEEPROM is a NVMCTRL-managed virtual region          |
| QSPI XIP region                          | Out of v0 scope (§11). Application crates that boot from QSPI XIP manage their own region split |
| `BOOTPROT` carve-out                     | Out of v0 scope. The boot-protection region size is a NVM USER ROW fuse field; not link-time |
| Backup-SRAM linker section               | Optional consumer-crate concern; cortex-m-rt does not pre-allocate. The `MEMORY { BKUPRAM ... }` entry suffices |

The slot exists so that:

1. A future chip whose section needs are non-trivial (e.g. a D5x part
   with a `bootloader_protected_app_image.x`-style multi-image split,
   or an L21 part with `eflash.x` cache attributes) populates the slot
   in a CHIPS-MICROCHIP-NN walkthrough rather than racing the
   emission shape through a Standards Action amendment.
2. Consuming crates can write a stable `-T<chip>.x` line in their
   `build.rs` today, even though the file body is empty. When a
   future amendment populates the slot, the consumer build does not
   need a `build.rs` change to pick it up.

Adding non-empty content to `atsamd51j19a.x.jinja` (e.g. a SAM D5x
specific `SECTIONS { ... }` directive) is **Specification Required**:
the walkthrough that introduces the content updates this section,
updates `atsamd51j19a.x.jinja`, and re-blesses the
`atsamd51j19a_x.snap` snapshot in the same PR. **Removing** the slot
file entirely (folding any future content into `memory.x`) is
Standards Action.

### 5.4 `linker:` chip-YAML block — Specification Required

The optional `linker:` block in chip-IR YAML carries the fields
consumed by `memory.x.jinja` and `<chip>.x.jinja`. The ATSAMD51J19A
shape is minimal:

```yaml
linker:
  region_text: flash         # alias source for cortex-m-rt FLASH
  region_data: ram           # alias source for cortex-m-rt RAM
```

This is a strict subset of the TI tree's `linker:` block (which adds
`ccfg_origin` and `ccfg_length` for SimpleLink CCFG placement). The
Microchip `<chip>.x.jinja` template is unconditional in v0 — it always
emits a header-only body when the `linker:` block is present — so no
chip-YAML field gates the `SECTIONS` directive emission. Adding a
field to the `linker:` block (e.g. an `eflash_origin` for an L21
external-flash variant, or a `bootprot_length` for a future amendment)
is Specification Required.

### 5.5 cortex-m-rt linker integration — Specification Required

The emitted `memory.x` MUST resolve `FLASH` and `RAM` via the standard
`REGION_ALIAS` lines so that an unmodified `cortex-m-rt` `link.x`
template links successfully. The consuming crate's `build.rs` MUST
emit the linker-arg sequence that loads the two fragments **in this
order**:

```rust
// In examples/<microchip-board>/build.rs:
println!("cargo:rustc-link-arg=-Tmemory.x");
println!("cargo:rustc-link-arg=-Tatsamd51j19a.x");   // or -T<chip_link_stem>.x
println!("cargo:rustc-link-arg=-Tlink.x");           // from cortex-m-rt
```

Ordering rationale: `memory.x` declares the `MEMORY` block first;
`<chip>.x` references (when populated) `ORIGIN(FLASH)` /
`LENGTH(FLASH)` symbols whose resolution requires `memory.x` to have
been parsed first; `link.x` from cortex-m-rt then places
`.text`/`.rodata`/`.data`/`.bss` using the `REGION_ALIAS` names that
`memory.x` set up, and `INCLUDE`s `device.x` from `atsamd51j19a`'s
`OUT_DIR` for NVIC extensions.

This ordering is **NOT** enforced by the generator — the consuming
crate's `build.rs` is the source of truth. Misordering by the
consumer manifests as `undefined reference to ORIGIN(FLASH)` link
errors, not silent miscompilation, so the failure mode is loud.

## §6 Frozen decisions — file path emission

The two linker fragments MUST be written to the same per-board output
directory as the six Rust files
([`tests/bsp_microchip_render.rs:34`](../../../tests/bsp_microchip_render.rs)
`tmp.path().join("adafruit_feather_m4_express")`). Concrete paths for
the Adafruit Feather M4 Express board:

```text
<out>/adafruit_feather_m4_express/memory.x
<out>/adafruit_feather_m4_express/atsamd51j19a.x
```

Co-locating the linker scripts with the Rust BSP files means the
consuming crate's `build.rs` can compute one path prefix and
reference both fragment types from it. The render test
[`tests/bsp_microchip_render.rs:33`](../../../tests/bsp_microchip_render.rs)
asserts the total emission count of 8 after CHIPS-MICROCHIP-05a lands
— moving either fragment to a different directory would break that
assertion.

Modifying the per-board directory layout is Standards Action.

## §7 Verification gates

The verification surface for linker emission is the same three-layer
contract that [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md)
§12 names for Rust-file emission:

1. **Snapshot render test** —
   [`tests/bsp_microchip_render.rs`](../../../tests/bsp_microchip_render.rs)
   produces text-level snapshots for both `memory.x` and
   `atsamd51j19a.x`. Any template diff without an `insta` re-bless
   fails this gate.
2. **Compile-verify test** —
   [`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs)
   runs `cargo check --target thumbv7em-none-eabihf` against the
   emitted BSP and the real `atsamd51j19a 0.7.1` PAC crate. The
   throwaway cargo project the test materialises MAY consume the
   emitted linker scripts via its `build.rs` once the example crate
   surface lands; in v0 of this chapter the linker scripts are
   emitted but the compile-verify gate does not yet link a full
   binary (it runs `cargo check`, not `cargo build --release`). The
   slot file's empty body therefore cannot break the compile-verify
   gate.
3. **`examples/` consumer crate** — Deferred to a future
   CHIPS-MICROCHIP-06 chapter. No `examples/<microchip-board>/`
   crate exists today; when it lands its `build.rs` MUST follow the
   linker-arg sequence in §5.5.

Promoting the snapshot or compile-verify test to a stricter form
(e.g. asserting an `objdump`-extracted FLASH origin matches §5.2
verbatim) is Specification Required.

## §8 Round-trip / regression posture

A CHIPS-MICROCHIP-NN execution PR that modifies `memory.x.jinja` or
`atsamd51j19a.x.jinja` MUST update both snapshots in the same PR. A
PR that modifies the chip-YAML `memory:` array or `linker:` block
MUST update both the snapshot **and** re-run the compile-verify test.

This chapter introduces no new regression posture beyond
[`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §12; it
inherits that contract for the linker-fragment surface.

## §9 Non-goals

Explicit out-of-scope for v0 of this chapter:

- **No NVMCTRL fuse-row emission.** The SAM D5x NVM USER ROW
  (`0x00804000`) carries bootloader-protection size, EEPROM
  emulation size, BOD33 calibration, and several other fuse fields
  (DS60001507F §25.6.9 Table 25-9). The generator MUST NOT emit a
  `SECTIONS` directive that places content there; user-row writes
  happen at run-time via `NVMCTRL.CTRLA.CMD = EP / WAP / WQW`.
- **No SmartEEPROM emulation region.** SmartEEPROM (§25.6.10) is a
  NVMCTRL-managed virtual region carved out of the main Flash via
  the NVM USER ROW's `SEESBLK` / `SEEPSZ` fields. Linker-time
  carve-out is incorrect; the runtime is authoritative.
- **No QSPI XIP region.** The ATSAMD51J19A's QSPI controller
  (§34) supports execute-in-place at `0x04000000..0x05000000` but
  XIP requires `QSPI.CTRLA.ENABLE = 1` plus a memory-mapped read
  configuration written by the application at boot. Linker-time
  XIP region declaration is out of v0 scope; future D5x parts with
  larger external Flash may revisit.
- **No `BOOTPROT` carve-out.** The boot-protection region size is
  set by the NVM USER ROW `BOOTPROT` field (DS60001507F §25.6.9);
  it is a run-time read, not a link-time constant.
- **No multi-image OTA boot layout.** Bring-up is single-image; OTA
  scaffolding (e.g. `mcuboot`-style dual-bank flash split) is a
  separate initiative.
- **No emission tuned to a different PAC vintage.** The frozen
  values in §5.2 reflect `atsamd51j19a 0.7.1`. A future PAC bump
  that changes peripheral base addresses or moves the `device.x`
  emission convention is Standards Action here and in
  [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §6.
- **No D21 / L21 chip-specific linker shape.** Adding a second
  family member (D21 J18A; L21 J18B) is the CHIPS-MICROCHIP-05a /
  -05b lane and requires both a new chip YAML and an analogous §5.2
  / §5.3 frozen-decision table in a §15 amendment.

## §10 Reconciliation with adjacent repo primitives

### 10.1 cortex-m-rt `link.x` ownership

The emitted `memory.x` **does not replace** cortex-m-rt's `link.x` —
it supplies the `MEMORY` block + `REGION_ALIAS` lines that `link.x`
expects to find. The consuming crate continues to pass `-Tlink.x` to
the linker. The generator MUST NOT emit its own `link.x`; doing so
would compete with cortex-m-rt over `.text` / `.data` / `.bss`
placement and is explicitly forbidden under §9.

### 10.2 PAC `device.x` ownership

The `atsamd51j19a 0.7.1` PAC crate's `build.rs` emits its own
`device.x` into `OUT_DIR` at compile time. This file is consumed
through the standard cortex-m-rt mechanism: `link.x` contains an
`INCLUDE device.x` line and the linker resolves the path against
`-L $OUT_DIR`. The PAC's `device.x` is **authoritative** for the
chip's NVIC vector extensions (the `__INTERRUPTS` table that picks
up beyond cortex-m-rt's 16 standard exceptions).

The generator's `atsamd51j19a.x` is **additive** to `device.x`, not
a replacement. The two are layered: `device.x` covers NVIC vectors;
`atsamd51j19a.x` covers chip-specific `SECTIONS` directives that
neither cortex-m-rt nor the PAC build script handles. In v0 the
latter set is empty, so `atsamd51j19a.x`'s body is intentionally
empty — but the slot is reserved so future D5x parts that need
additional sections (e.g. a custom `.peripherals_init` section, a
boot-loader header `.app_header` block, etc.) populate it in a
walkthrough rather than racing the emission shape.

### 10.3 TI tree precedent

The CHIPS-TI tree's linker emission
([`src/bin/creator/bsp/ti/templates/memory.x.jinja`](../../../src/bin/creator/bsp/ti/templates/memory.x.jinja),
[`chip.x.jinja`](../../../src/bin/creator/bsp/ti/templates/chip.x.jinja))
shares **vocabulary** (`region_text`, `region_data` chip-YAML field
names; `MEMORY { ... }` + `REGION_ALIAS` shape) but not concrete
addresses or content. The TI `<chip>.x` is populated (CCFG section
directive); the MICROCHIP `<chip>.x` is a slot in v0. The two trees
diverge intentionally on file naming convention: TI uses snake_case
chip stems (`cc1352_r.x`); MICROCHIP uses lowercase-no-separator
stems (`atsamd51j19a.x`) to match the PAC crate name. Renaming a
shared chip-YAML field (`region_text` → `text_region`) is Standards
Action across **all** chipdb vendor trees, not just CHIPS-MICROCHIP;
coordination happens at the `docs/bsp/` initiative level.

### 10.4 Espressif tree precedent

The Espressif tree's linker emission similarly shares vocabulary
but emits per-chip riscv-target memory maps that have no overlap
with the SAM D5x address space. The two trees coexist without
either depending on the other's `linker:` block field semantics.

### 10.5 Hand-written STM32H747I-DISCO linker

Per [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §10 the
STM32H747I-DISCO `examples/stm32h747i-disco/memory.x` is OUT of
scope for the chipdb-driven generator. CHIPS-MICROCHIP-05 makes no
claim over that file; future DISCO PRs MAY modify it without
amending this chapter.

## §11 Non-goals

(See §9 — kept under that heading rather than re-titled here so the
chapter stays close to the standard §0–§15 shape; per CLAUDE.md
"per-chapter docs MAY omit sections that do not apply".)

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [x] §0 authority table reviewed; DS60001507F §10 Table 10-1
      confirmed as authority for §5.2 frozen values.
- [x] §3 glossary terms each carry a cite-vs-restate marker per
      CLAUDE.md §"Definitions — reference vs. restatement".
- [x] §4 source-of-truth map has exactly one owner per row.
- [x] §5.1 file-set frozen; concrete file-name substitution
      `<chip>.x` → `atsamd51j19a.x` is explicit.
- [x] §5.2 ATSAMD51J19A memory map values match
      [`ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml) `memory:`
      array verbatim and match the
      [`memory_x.snap`](../../../tests/snapshots/bsp_microchip_render__adafruit_feather_m4_express__memory_x.snap)
      golden file.
- [x] §5.3 v0 deferred slot rationale documented; future
      population is Specification Required.
- [x] §5.5 cortex-m-rt linker-arg sequence consistent with the
      TI tree precedent.
- [x] §10.1 cortex-m-rt boundary documented; §10.2 PAC
      `device.x` layering documented.
- [x] §15 dated ratification entry.

Behaviour PRs that ride on this chapter (CHIPS-MICROCHIP-05a and
beyond):

- [ ] §7 verification gate 1 (snapshot render test) extended to
      cover `atsamd51j19a.x` — render-test file count goes from 7 to
      8 (`tests/bsp_microchip_render.rs:33`). Snapshot file
      `bsp_microchip_render__adafruit_feather_m4_express__atsamd51j19a_x.snap`
      lands.
- [ ] §7 verification gate 2 (compile-verify) continues to pass;
      the slot file's empty body cannot break the gate. **MUST.**

## §13 Files cited

- [`CLAUDE.md`](../../../CLAUDE.md) — Spec-Before-Code Planning
  Discipline, RFC 2119 keywords, registration policy, initiative
  prefix.
- [`chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-00-CONCEPTS.md`](CHIPS-MICROCHIP-00-CONCEPTS.md)
  — parent concepts doc; §6 INV-MC6 template emission contract; §11
  linker-emission deferral; §15 slate change log.
- [`chipdb/rlvgl-chips-microchip/db/chips/ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml)
  — chip-IR YAML; authoritative source for §5.2 memory-map values.
- [`src/bin/creator/bsp/microchip/templates/memory.x.jinja`](../../../src/bin/creator/bsp/microchip/templates/memory.x.jinja)
  — `memory.x` template; consumes `ir.chip.memory` and
  `ir.chip.linker.region_*` fields.
- [`src/bin/creator/bsp/microchip/templates/atsamd51j19a.x.jinja`](../../../src/bin/creator/bsp/microchip/templates/atsamd51j19a.x.jinja)
  — `atsamd51j19a.x` template (slot file); v0 body intentionally
  empty.
- [`src/bin/creator/bsp/microchip/render.rs`](../../../src/bin/creator/bsp/microchip/render.rs)
  — render pipeline; emits `chip_link_stem` for the linker file
  name.
- [`tests/bsp_microchip_render.rs`](../../../tests/bsp_microchip_render.rs)
  — render test; asserts the 8-file emission set after -05a.
- [`tests/snapshots/bsp_microchip_render__adafruit_feather_m4_express__memory_x.snap`](../../../tests/snapshots/bsp_microchip_render__adafruit_feather_m4_express__memory_x.snap)
  — `memory.x` golden snapshot.
- [`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs)
  — compile-verify test; consumes the emitted BSP end-to-end against
  `atsamd51j19a 0.7.1` on `thumbv7em-none-eabihf`.
- [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md`](../../rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md)
  — sibling-tree precedent (TI SimpleLink linker emission, ratified
  2026-05-14). CHIPS-MICROCHIP-05 mirrors the §1–§7 shape; diverges
  on §5.1 file-naming convention (lowercase-no-separator vs.
  snake_case).
- Microchip DS60001507F — SAM D5x/E5x Family Data Sheet, rev F
  (2020-09); §10 Table 10-1 Physical Memory Map (authority for
  §5.2); §11.2 Memory Map (cross-check); §25.6 NVMCTRL (authority
  for §9 non-goals).
- Microchip DS80000748 — SAM D5x/E5x errata sheet; cited for
  authority closure even though no §5 frozen decision currently
  depends on an erratum.
- ARM ARMv7-M Architecture Reference Manual (DDI 0403E.e) — System
  Control Space (cited for §5.2 omitted-row rationale).
- crates.io `atsamd51j19a 0.7.1` — `device.x` auto-emission via
  `build.rs`; cited for §10.2 layering.
- crates.io `cortex-m-rt` — `link.x` template that consumes
  `memory.x` `REGION_ALIAS` lines and `INCLUDE`s `device.x`.

## §14 Unblocks

Ratifying and implementing this chapter unblocks:

- **CHIPS-MICROCHIP-06** — example crate
  (`examples/<microchip-board>/`) consuming the generated BSP with
  a `build.rs` that follows §5.5's linker-arg sequence. No example
  crate exists for Microchip today; CHIPS-MICROCHIP-06 is the
  natural follow-on to -02 + -05.
- **Future SAM D5x / E5x chip additions** (D51N, D51P, D51G,
  E51N/P/J/G, D52, D53, etc.). Each new chip-YAML re-uses the §5.4
  `linker:` block shape; per-chip frozen-decision tables analogous
  to §5.2 land in §15 amendments here when the chip joins.
- **Future SAM D21 / L21 chip additions**. The smaller D21 / L21
  parts have a different SRAM/flash mix (e.g. D21G17A = 256 KB
  flash / 32 KB SRAM) but the same `linker:` block shape; the slot
  template generalises across the SAM Cortex-M slice.
- **Future MCU-line additions that need non-trivial `<chip>.x`
  content** (e.g. an L21 part with a `BOR33` calibration section, or
  a D5x part with `mcuboot`-style multi-image layout). Each
  populates `atsamd51j19a.x.jinja`'s analogue via a Specification
  Required walkthrough (§5.3 lane).

## §15 Change log

| Date       | Status                       | Note                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Linker emission chapter ratified. Closes the [CHIPS-MICROCHIP-00 §11](CHIPS-MICROCHIP-00-CONCEPTS.md#11-non-goals) "linker emission deferred to a future chapter" item. `ATSAMD51J19A` frozen-decision tables in §5.2 codify the values that have been shipping in `memory.x` since slate 4 (the [`memory_x.snap`](../../../tests/snapshots/bsp_microchip_render__adafruit_feather_m4_express__memory_x.snap) golden file). The slot file `atsamd51j19a.x.jinja` (v0 body intentionally empty per §5.3) is added in the same slate (CHIPS-MICROCHIP-05a) so the MICROCHIP emission set reaches parity with the CHIPS-TI tree at 8 files per board. Document-only on ratification; behaviour PR lands as -05a in the same slate. |
