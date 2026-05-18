# CHIPS-SILABS-05 — Linker-script emission for SiLabs BSPs

**Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.

The key words **MUST**, **MUST NOT**, **SHALL**, **SHOULD**, **SHOULD
NOT**, **MAY**, and **RECOMMENDED** are interpreted per RFC 2119 and
RFC 8174.

## 0. Authority policy

| Concern                                 | Authority                                                                                                                                                            |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| EFM32GG11B memory map (flash / SRAM)    | **Silicon Labs EFM32GG11B Family Reference Manual**, "System Overview / Memory Map" — also EFM32GG11B820 data sheet Table 2.1 ("Memory Map") for the SKU sizes.       |
| Info / lock-bits / DEVINFO regions      | EFM32GG11B RM "Memory System Controller (MSC)" §9.3 (User Data / Lock Bits / DEVINFO addresses).                                                                      |
| PAC SKU bounds (which SKU's `Peripherals` is exported) | `efm32gg11b-pac 0.1.4` — the `efm32gg11b820` Cargo feature gates the SKU sub-module flattened by CHIPS-SILABS-02 (commit `11075e6`).                          |
| Linker script conventions               | `cortex-m-rt` `link.x` / `link.x.in` (`cortex-m-rt` 0.7.x) — the consuming crate's `build.rs` rewrites `memory.x` into the search path and `cortex-m-rt` then `INCLUDE`s it from `link.x`. |
| Cortex-M4F vector table requirement     | ARM ARMv7-M Architecture Reference Manual §B1.5 — vector table at the start of FLASH region, 4-byte aligned; `cortex-m-rt` places this for us via `.vector_table`.    |

Authority precedence when EFM32GG11B RM and `efm32gg11b-pac 0.1.4`
disagree on a memory-map field, follow the precedent set by
CHIPS-SILABS-01c (commit `b003c42`): **PAC wins**, because the
compile-verify gate (`bsp_silabs_slstk3701a_compile.rs`) type-checks
against the actual PAC.  This rule is structural — the same divergence
class that bit GPIO clock-gate routing could bite memory-region naming
in a future SKU.

## 1. Purpose

CHIPS-SILABS-05 closes the file-emission gap between the SiLabs BSP
generator and the TI / Microchip BSP generators.  The SILABS pipeline
prior to this chapter emitted **6 files**
(`{mod, pac, clocks, io_mux, peripherals, board}.rs`) and explicitly
deferred linker emission per CHIPS-SILABS-00 §11.  After this chapter
the pipeline emits **8 files**:

- the six existing `.rs` modules, unchanged;
- `memory.x` — the cortex-m-rt-compatible `MEMORY` block plus the
  canonical `REGION_ALIAS("FLASH" / "RAM", …)` declarations;
- `efm32gg11b.x` — a chip-named linker supplement reserved for
  EFM32GG11B-specific section directives (currently a documentation
  header; no `SECTIONS` block at v0 because EFM32GG11 has no SiLabs
  equivalent of the SimpleLink CCFG block).

Slate 8 (CHIPS-SILABS-01c + -02c, commits `b003c42` and `7cb8d76`,
2026-05-14) brought SiLabs to compile-verify parity with TI and
Microchip.  This chapter brings the SILABS *file emission set* to
parity, so the v0.2.0 chipdb-family-wide acceptance gate
"snapshot tests pass; compile-verify SHOULD pass; linker scripts emitted
when `chip.linker` populated" is satisfied for every vendor.

The first user of these linker scripts is the eventual SILABS example
crate (CHIPS-SILABS-06, not yet ratified).  Per CHIPS-SILABS-00
acceptance §12(g) the example crate is informative; this chapter's
acceptance gates (§12) do **not** depend on it landing.

## 2. Problem statement

CHIPS-SILABS-00 §11 deferred linker-script emission with the rationale
"v0 boards provide their own `memory.x`".  Slate 8 ratification of the
compile-verify gate (CHIPS-SILABS-04, commit `52f0533`) tested only
`cargo check`, which does **not** invoke the linker, so the deferral
was harmless to the gate.  However:

- TI (CHIPS-TI-02, `src/bin/creator/bsp/ti/render.rs`) emits 8 files
  including `memory.x` + `<chip>.x`.
- Microchip (CHIPS-MICROCHIP-02,
  `src/bin/creator/bsp/microchip/render.rs`) emits 7 files including
  `memory.x` (no chip-supplement script).
- SILABS emits 6 files.

A future example crate consuming the SILABS BSP would have to
hand-author its own `memory.x` against EFM32GG11B Reference Manual data
that the chipdb already carries (`chipdb/rlvgl-chips-silabs/db/chips/
EFM32GG11.yaml` `memory:` block, lines 50–61).  Hand-authoring is a
silent-fork hazard per CLAUDE.md §"Definitions — reference vs.
restatement"; the chipdb is the canonical source of truth, so
`memory.x` MUST be derived from it.

The deferral exists because at slate-1 (`CHIPS-SILABS-01`) the
chipdb's `linker:` block was a *placeholder* with no consumer.  Slate 8
turned `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` into a
type-checked source of truth (compile-verify proves every field
landed); now we close the §11 deferral and ratify emission.

## 3. Canonical glossary

| Term                  | Definition                                                                                                                                                                                                                                   |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `memory.x`            | The cortex-m-rt linker fragment carrying the chip's `MEMORY { ... }` block and the `REGION_ALIAS` directives that map chip-specific region names onto the canonical names `FLASH` and `RAM` that `cortex-m-rt`'s bundled `link.x` consumes.   |
| `<chip>.x`            | Per-chip linker supplement included after `memory.x` and before `link.x`. For EFM32GG11B820 the file is named `efm32gg11b.x` (the package-family name flattened by §5 below). Carries chip-specific section directives; v0 carries header only. |
| FLASH origin / length | `0x00000000` / `0x00200000` (2048 KB) for EFM32GG11B820F2048GL192 (the SKU pinned by the chipdb yaml). Owned by EFM32GG11B Family RM "Memory Map" subsection and EFM32GG11B820 data sheet Table 2.1.                                            |
| SRAM origin / length  | `0x20000000` / `0x00080000` (512 KB) for EFM32GG11B820F2048GL192. Same authority pair as FLASH.                                                                                                                                                |
| AAP region            | EFM32 "Authentication Access Port" — a one-shot debug-recovery region; intentionally out of scope (see §11).                                                                                                                                  |
| DCI region            | "Device Configuration Information" — debug-recovery / SE communication mailbox region on Series 2 parts. EFM32GG11 (Series 1) has no DCI; reserved for a future Series 2 chapter.                                                              |
| User data page        | Flash-mapped page at `0x0FE00000` (2 KB) consumed by application-level lockbit / user-data writes.  Surfaced by `chipdb/.../EFM32GG11.yaml` `memory:` as region `userdata`; emitted into the `MEMORY` block verbatim so application code can `INSERT` into it. |
| Vector table          | Cortex-M4F exception/IRQ vector table, placed by `cortex-m-rt` at FLASH origin via its built-in `.vector_table` section. **Owned by cortex-m-rt; not by this chapter.** This chapter MUST NOT redefine `.vector_table` placement.              |

Cite-vs-restate markers:

- `memory.x` / `<chip>.x` — as defined by `cortex-m-rt` 0.7.x
  documentation; **used without modification**.
- FLASH / SRAM origin & length — as defined in
  `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` (lines 50–61);
  **used without modification**. The repo's existing values are
  canonical for this chapter.
- AAP / DCI — owned by future Series 2 chapter; **do not exist in repo
  yet**.

## 4. Source-of-truth map

| Concept                            | Owner / authoritative location                                                                                                                                                          |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FLASH origin / length              | `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` `memory:` block, lines 50–61. Authority: EFM32GG11B Family RM "Memory Map"; EFM32GG11B820 data sheet Table 2.1 ("Memory Map").       |
| SRAM origin / length               | Same as FLASH — `EFM32GG11.yaml`. Authority: EFM32GG11B RM "Memory Map".                                                                                                                |
| User data / lock-bits / DEVINFO    | `EFM32GG11.yaml` `memory:` block (entries `userdata`, `lockbits`, `devinfo`). Authority: EFM32GG11B RM "Memory System Controller (MSC)" §9.3.                                            |
| REGION_ALIAS target ("FLASH"/"RAM")| `EFM32GG11.yaml` `linker:` block (`region_text`, `region_data`). The render code translates these into `REGION_ALIAS` directives in `memory.x`.                                          |
| Vector table layout / size         | **`cortex-m-rt`** — not in repo. EFM32GG11 has 56 maskable interrupts beyond the standard 16 Cortex-M exceptions; `cortex-m-rt`'s `interrupt!` / `cortex-m-rt-macros` plus the PAC vector table own this. This chapter MUST NOT emit `.vector_table` directives. |
| `<chip>.x` filename                | Render code (`src/bin/creator/bsp/silabs/render.rs`) — derived from the snake-cased chip family name via the existing `snake_case` helper. For chip `EFM32GG11` the file name is `efm32_gg11.x`.  See §5 frozen decision below for the chosen rule. |
| File-emission set count            | Render code — `assert_eq!(written.len(), 8)` after this chapter lands (was 6 before). Snapshot test (`tests/bsp_silabs_slstk3701a_render.rs`) is the regression boundary.                  |

## 5. Frozen decisions

### 5.1 File-emission set — Standards Action

The SILABS BSP file-emission set is frozen as:

```
{ mod.rs, pac.rs, clocks.rs, io_mux.rs, peripherals.rs, board.rs,
  memory.x, efm32gg11b.x }
```

Per-board the chip-named supplement filename is derived by snake-casing
the chip's family name from the chipdb yaml (`EFM32GG11` →
`efm32_gg11.x`).  The literal filename `efm32gg11b.x` referenced in
this doc's title refers to the *chip family*; the rendered filename on
disk for the SLSTK3701A board is `efm32_gg11.x` because of how
`snake_case("EFM32GG11")` lowers the identifier — same convention as
the TI render where `CC1352R` becomes `cc1352_r.x`.  Snapshots are
canonical for the on-disk name.

Adding a file to the emission set requires a §15 amendment AND
matching `assert_eq!` count update in
`tests/bsp_silabs_slstk3701a_render.rs` (Standards Action — touching a
frozen enum cardinality).

### 5.2 EFM32GG11B820 memory map — Standards Action

The SKU pinned by the chipdb (EFM32GG11B820F2048GL192, the chip on the
SLSTK3701A starter kit) MUST have:

| Region   | Origin       | Length        | Access | Authority                                                              |
| -------- | ------------ | ------------- | ------ | ---------------------------------------------------------------------- |
| FLASH    | `0x00000000` | `0x00200000` (2048 KB) | `rx`   | EFM32GG11B RM "Memory Map"; EFM32GG11B820 data sheet Table 2.1.       |
| SRAM     | `0x20000000` | `0x00080000` (512 KB)  | `rwx`  | Same as FLASH.                                                         |
| userdata | `0x0FE00000` | `0x00000800` (2 KB)    | `rw`   | EFM32GG11B RM "MSC" §9.3 (User Data page).                            |
| lockbits | `0x0FE04000` | `0x00000800` (2 KB)    | `rw`   | EFM32GG11B RM "MSC" §9.3 (Lock Bits page).                            |
| devinfo  | `0x0FE08000` | `0x00000800` (2 KB)    | `r`    | EFM32GG11B RM "MSC" §9.3 (Device Information page, read-only).        |

These values are already pinned by
`chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` lines 50–61; this
chapter freezes them as the canonical input to `memory.x` emission.
Any change to those values is a Standards Action amendment per
CHIPS-SILABS-00 §5 and requires a `CHIPS-SILABS-NNx` follow-up.

### 5.3 REGION_ALIAS shape — Specification Required

`memory.x` MUST emit:

- `REGION_ALIAS("FLASH", FLASH);`
- `REGION_ALIAS("RAM",   SRAM);`

Plus the same six aux aliases the TI / Microchip generators emit
(`REGION_TEXT`, `REGION_RODATA`, `REGION_DATA`, `REGION_BSS`,
`REGION_HEAP`, `REGION_STACK`) so the file is interchangeable with
older or newer `cortex-m-rt` `link.x` versions.

Adding or removing aux aliases is Specification Required (snapshot
must re-bless; no §15 amendment).

### 5.4 `<chip>.x` supplement shape at v0 — Specification Required

For EFM32GG11B at v0 the supplement file (`efm32_gg11.x` on disk)
carries only a documentation header — no `SECTIONS` block.  EFM32GG11
has no fixed-address chip-specific boot block analogous to the
SimpleLink CCFG region; the only reason this file exists at all is to
keep the emission set symmetrical across vendors and to provide a
hook for future amendments (e.g. relocating the user-data page into
its own `KEEP(*(.userdata))` section).

Adding a `SECTIONS` block here is Specification Required and rides on
a CHIPS-SILABS-NNx amendment that names the first-user board.

## 10. Reconciliation with adjacent repo primitives

### 10.1 `cortex-m-rt` `link.x` interaction

`cortex-m-rt` 0.7.x ships a `build.rs` that copies the consumer-side
`memory.x` into the linker search path, then its bundled `link.x`
parses that file with `INCLUDE memory.x;`.  The consuming crate's own
`build.rs` writes `memory.x` to the target output dir from
`OUT_DIR/../memory.x` so it sits where cortex-m-rt expects it.  The
SILABS-generated `memory.x` is **drop-in compatible** with this flow
because:

1. It defines the canonical `FLASH` and `RAM` aliases that
   `cortex-m-rt`'s `link.x` references.
2. It does **not** redeclare `_stack_start`, `_stext`, or `__pre_init`
   — those are owned by `cortex-m-rt`.
3. It does **not** emit `ENTRY()` or `SECTIONS { .vector_table … }` —
   those are also owned by `cortex-m-rt`.

The `<chip>.x` supplement is included by the consuming crate's
`build.rs` via the same `println!("cargo:rustc-link-arg=-Tefm32_gg11.x");`
mechanism the TI side already uses for `cc1352_r.x`.  Sequence is:
`-Tmemory.x` → `-T<chip>.x` → `-Tlink.x`.

### 10.2 Compile-verify (`bsp_silabs_slstk3701a_compile.rs`)

The compile-verify test currently:

1. Renders the BSP into a tempdir.
2. Copies the **5 .rs files** (excluding `mod.rs` which becomes
   `lib.rs`) into a throwaway cargo project's `src/`.
3. Runs `cargo check --target thumbv7em-none-eabihf`.

`cargo check` is type-check-only and does not invoke the linker, so
the new linker scripts emitted into the BSP source dir are **not**
consumed by compile-verify.  After this chapter lands the assertion
`assert_eq!(written.len(), 6)` inside the compile-verify test MUST be
updated to `assert_eq!(written.len(), 8)`, but the materialised
cargo project still copies only the 5 .rs files — no linker emission
is fed to `cargo check`.

This is the same shape used by `bsp_microchip_compile.rs` and
`bsp_ti_cc1352r_compile.rs`: the render-test surface widens to 7 / 8
files, the compile-verify materialisation surface stays at the 5 .rs
files.

### 10.3 SKU sub-module flatten (CHIPS-SILABS-02) interaction

CHIPS-SILABS-02 introduced `pac_sku_module: efm32gg11b820` to flatten
the SKU sub-module.  That field affects the `pac.rs` template only —
it has no bearing on the linker emission, which keys off
`chip.memory` and `chip.linker` (already present in the IR).  The
SKU flatten is structurally orthogonal to linker emission and stays
that way.

### 10.4 Cross-vendor naming — `<chip>.x` vs. generic `chip.x.jinja`

The TI render uses a generic template named `chip.x.jinja` and
renames it on write to `<chip_stem>.x` (e.g. `cc1352_r.x`).  This
chapter follows the same convention: the SILABS template is named
`chip.x.jinja` and the renderer writes it to `<chip_stem>.x` on disk
(`efm32_gg11.x` for EFM32GG11).  Naming the on-disk file after the
chip family rather than the package SKU keeps the file name stable
across SKU variants — a future EFM32GG11B840 board would consume the
same `efm32_gg11.x`.

## 11. Non-goals

- **AAP / DCI / UD region content.**  EFM32 chips carry an
  Authentication Access Port region used for debug-recovery and (on
  Series 2) a Device Configuration Information mailbox.  Application
  code may need to read or write entries in those regions, but the
  layout of their *contents* is not chipdb data — it is
  consumer-defined or vendor-defined.  Out of scope.
- **XIP from external flash.**  EFM32GG11 has an EBI (External Bus
  Interface) capable of mapping external NOR / PSRAM into the address
  space; QSPI-style XIP is not in scope for the `memory.x` emitted by
  this chapter.  A future SILABS board YAML that wires EBI MAY carry
  an additional `memory:` region for the external bank, in which case
  it appears in `memory.x` automatically because the renderer loops
  over `ir.chip.memory`.
- **DMA buffer placement sections.**  No `SECTIONS { .dma_buffers
  ... } > SRAM` directive at v0.  DMA buffer placement is an
  application-level concern (see how `examples/stm32h747i-disco/`
  handles SDRAM-mapped frame buffers — that's all consumer code,
  emitted at link time via `#[link_section]`).
- **Vector table customisation.**  Vector table placement and IRQ
  table layout are owned by `cortex-m-rt`; this chapter MUST NOT
  emit `.vector_table` directives or override the default IRQ table.
- **Series 2 EFR32xG21+ memory map.**  EFR32MG24 and other Series 2
  parts have different region layouts (smaller flash, more SRAM
  banks, secure-engine mailboxes).  CHIPS-SILABS-00 §11 already
  defers Series 2 chip-yaml population to a future chapter; linker
  emission for Series 2 ships with whichever chapter ratifies a
  Series 2 chip yaml.

## 12. Acceptance checklist

This chapter is ratified when:

- [x] §0 authority table reviewed.
- [x] §3 glossary terms each carry a cite-vs-restate marker.
- [x] §4 source-of-truth map names one owner per concept.
- [x] §5 freezes the FLASH / SRAM origin/length and the
      file-emission-set cardinality (8 files) explicitly.
- [x] §10 reconciles cleanly with `cortex-m-rt`'s `link.x`,
      compile-verify, and CHIPS-SILABS-02's SKU flatten.
- [x] §11 names every deferred region (AAP, DCI, XIP, DMA, vectors,
      Series 2) so consumers can reason about what is missing.
- [x] §15 carries a dated ratification entry.

This chapter's behavioural acceptance (the implementation phase
`CHIPS-SILABS-05a`) is gated by:

- [ ] `cargo test -p rlvgl-chips-silabs` — passes.
- [ ] `cargo test -p rlvgl --test bsp_silabs_slstk3701a_render
      --features creator,regression` — passes; render snapshot file
      count is 8; `memory.x` and `efm32_gg11.x` snapshots exist and
      assert correct ORIGIN / LENGTH values.
- [ ] `cargo test -p rlvgl --test bsp_silabs_slstk3701a_compile
      --features compile-verify -- --test-threads=1` — still
      passes.  Compile-verify only consumes the .rs files (see
      §10.2); linker emission is exercised by the render snapshot
      gate, not by compile-verify.

## 13. Files cited

- `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` — memory map +
  linker hint source of truth (lines 50–61, 723–729).
- `chipdb/rlvgl-chips-silabs/docs/CHIPS-SILABS-00-CONCEPTS.md` —
  parent concepts chapter; §11 deferral being closed by this chapter.
- `src/bin/creator/bsp/silabs/render.rs` — renderer extended by
  `-05a` to emit the two new files.
- `src/bin/creator/bsp/silabs/templates/` — new templates
  `memory.x.jinja`, `chip.x.jinja` land here.
- `src/bin/creator/bsp/ti/render.rs`,
  `src/bin/creator/bsp/ti/templates/memory.x.jinja`,
  `src/bin/creator/bsp/ti/templates/chip.x.jinja` — reference
  implementation; SILABS shape mirrors TI minus the CCFG section.
- `src/bin/creator/bsp/microchip/render.rs`,
  `src/bin/creator/bsp/microchip/templates/memory.x.jinja` — second
  reference; demonstrates the simpler "memory.x only, no chip.x"
  shape.
- `tests/bsp_silabs_slstk3701a_render.rs` — render snapshot gate,
  extended by `-05a` from 6 files to 8.
- `tests/bsp_silabs_slstk3701a_compile.rs` — compile-verify gate;
  `written.len()` assertion adjusted from 6 to 8 but the
  materialised cargo project still copies only the 5 .rs files.

## 14. Unblocks

- **CHIPS-SILABS-06** (not yet ratified) — example crate consuming the
  generated BSP.  Once this chapter lands, the example crate can link
  against the chipdb-derived `memory.x` instead of carrying a
  hand-authored copy.  No example crate exists today; this chapter
  closes the file-emission gap that would otherwise block one.

## 15. Change log

| Date       | Status                       | Note                                                                                                                                                                                                                              |
| ---------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Initial ratification of linker-script emission.  Frozen: file emission set {6 .rs + memory.x + efm32_gg11.x}; EFM32GG11B820 FLASH `0x00000000 / 0x00200000`, SRAM `0x20000000 / 0x00080000`.  Closes the §11 deferral from CHIPS-SILABS-00.  Implementation phase: `CHIPS-SILABS-05a` (separate commit). |
