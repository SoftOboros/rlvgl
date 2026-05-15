# CHIPS-ESP Retrospective — divergences, refactor points, forward constraints

**Status:** Drafted 2026-05-15. Initiative-completion retrospective
for the CHIPS-ESP initiative on rlvgl `v0.2.0`. Unlike its three
younger siblings (CHIPS-TI, CHIPS-SILABS, CHIPS-MICROCHIP), CHIPS-ESP
predates the §0–§15 Spec-Before-Code Planning Discipline that
[`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline" later established. There is no
`CHIPS-ESP-00-CONCEPTS.md`, no `CHIPS-ESP-05-LINKER.md`, no
`CHIPS-ESP-06-EXAMPLE.md`, and no slate-numbered §15 change log to
trace amendments through.

This retrospective is therefore a **structural catch-up**: it
captures, after the fact, the lessons-learned that CHIPS-TI /
CHIPS-SILABS / CHIPS-MICROCHIP enjoyed before their work began.
Behaviour PRs that touch `chipdb/rlvgl-chips-esp/`,
`src/bin/creator/bsp/espressif/`, or the `tests/bsp_esp32*_*` test
family at this point should treat the canonical concepts surface as
the CLAUDE.md §"Espressif BSP Generator" section plus the
template tree itself; this retrospective is the bridge between
*what shipped organically* and *what to do differently the next
time an Espressif chip is added, or the next time a new vendor lane
clones the ESP shape*.

Per CLAUDE.md "Spec-Before-Code Planning Discipline → Initiative
retrospective" the file is co-located with `chipdb/rlvgl-chips-esp/`
and follows the §1–§7 shape established by
[`docs/concepts/DCB-RETROSPECTIVE.md`](../../../docs/concepts/DCB-RETROSPECTIVE.md)
and re-used by the three sibling vendor retrospectives. Audience:
future Codex / Claude agents working on (a) a new Espressif chip
yaml addition, (b) a new vendor lane modelled on the ESP precedent,
or (c) a follow-up that retrofits the §0 concepts-doc shape onto
CHIPS-ESP itself.

## 1. Outcome snapshot

### Final architecture

`rlvgl-creator bsp from-yaml --vendor esp` emits an 8-file BSP set
per RISC-V board (6 `.rs` + `memory.x` + `<chip>.x`) and a 6-file
set per Xtensa board (no linker emission — see §2.1). Templates
live under `src/bin/creator/bsp/espressif/templates/`:

```text
mod.rs.jinja        (host crate-shaped module index)
pac.rs.jinja        (alias-style re-export — see §2.6 / §6.5)
clocks.rs.jinja     (SYSTEM / PCR / HP_SYS_CLKRST clock-gate ungate)
io_mux.rs.jinja     (IO_MUX + GPIO matrix Direct / Matrix / Plain branches)
peripherals.rs.jinja (UART0 console + I2C0 master + SPI / LEDC / TIMG stubs)
board.rs.jinja      (XTAL_HZ / APB_HZ + labelled-pin consts)
memory.x.jinja      (rv32-only; chip.memory regions + linker: aliases)
chip.x.jinja        (rv32-only; esp-riscv-rt symbol contract)
```

The `EspIr` adapter (`src/bin/creator/bsp/espressif/ir.rs`) parses
chip + board YAML through MiniJinja. The pipeline is gated by:

- **`cargo test -p rlvgl-chips-esp`** — chipdb adapter unit tests
  (YAML parse, chip / board lookup, `BoardInfo` compat shim).
- **`tests/bsp_esp_matrix_smoke.rs`** — cross-chip matrix smoke
  exercising every chip × board pair in the chipdb through the
  full load → merge → render path.
- **`tests/bsp_esp32{c3,c6,p4}_render.rs`** + **`bsp_esp32{c5,h2,c61}_render.rs`**
  — `insta`-snapshotted text-level regression on the rendered
  output. Full 8-file shape for C3 / C6 / P4; rv32-linker-only
  for C5 / H2 / C61 (the Rust file shape is already covered by
  the three full snapshots).
- **`tests/bsp_esp32{c3,c6,p4}_cli.rs`** — CLI surface for the
  `bsp from-yaml --vendor esp` subcommand against each chip.
- **`tests/bsp_esp32c3_compile.rs`** — opt-in `--features
  compile-verify` gate. Materialises a throwaway cargo project
  around the rendered BSP and runs `cargo check --target
  riscv32imc-unknown-none-elf` against the real `esp32c3 = 0.31`
  PAC. **This is the only ESP chip with a compile-verify gate;
  C6 / P4 / C5 / H2 / C61 are snapshot-only at initiative close.**

Chipdb inventory at close:

| Chip       | Arch        | Boards in `db/boards/`                                        | Linker block | Compile-verify |
| ---------- | ----------- | ------------------------------------------------------------- | ------------ | -------------- |
| ESP32-C3   | rv32imc     | `esp32c3_devkitm_1`, `beetle_esp32c3`                         | yes          | **yes**        |
| ESP32-C6   | rv32imac    | `beetle_esp32c6`, `firebeetle2_esp32c6`                       | yes          | no             |
| ESP32-P4   | rv32imafc   | `beetle_esp32p4`                                              | yes          | no             |
| ESP32-C5   | rv32imac    | `esp32c5_minimal`                                             | yes          | no             |
| ESP32-H2   | rv32imac    | `esp32h2_devkitm_1`                                           | yes          | no             |
| ESP32-C61  | rv32imc     | `esp32c61_minimal`                                            | yes          | no             |
| ESP32      | xtensa-lx6  | `esp32_devkits_r`, `firebeetle_esp32`                         | no           | no             |
| ESP32-S2   | xtensa-lx7  | `esp32s2_minimal`                                             | no           | no             |
| ESP32-S3   | xtensa-lx7  | `esp32s3_devkitc_1`                                           | no           | no             |

Total: 9 chips, 12 boards. The matrix smoke test ensures every
pair load → merge → renders without error; the snapshot tests
freeze the emitted text for the canonical board of each rv32 chip.

Example crates at close:

- **`examples/beetle-esp32c3/`** (workspace member) — dual-binary
  feature matrix:
  - `--features esp_hal` → `src/esp_hal_main.rs`. Known-working
    SSD1306 + rlvgl path through `esp-hal 1.0.0-beta.0`.
  - `--features bsp_pac` → `src/bsp_pac_main.rs`. Consumes the
    chipdb-generated BSP under `src/bsp_generated/` and blinks
    GPIO8 via raw PAC. Proves the chipdb → generator → BSP →
    hardware pipeline end-to-end. SSD1306 over raw PAC is
    deferred (see §5.1).
- **`examples/beetle-esp32p4/`** (workspace member) — bare-metal
  against `esp32p4` PAC + `esp-riscv-rt`. Consumes the
  P4 chipdb-generated BSP plus the slate-linker emission to drive
  a Riverdi 7" MIPI-DSI panel. Has a `blink_all` companion
  binary for bring-up.
- **`examples/esp32-devkits-r/`** (detached — empty `[workspace]`
  stanza) — Xtensa ESP32 + SSD1306 via `esp-hal 1.0` / `espup`
  toolchain. Excluded from the workspace because Xtensa is not
  on stable rustc.

### Deferred items (explicit)

1. **SSD1306-over-raw-PAC on `beetle-esp32c3 --features bsp_pac`.**
   Closed-with-deferral at slate `c486708`
   (`bsp-esp32c3: flesh out generator + feature-flag beetle
   consumer`). The SSD1306 BufferedGraphics flush sends ~1 KB in
   a single I2C write; the ESP32-C3 I2C TX FIFO is 32 bytes.
   Driving the panel requires command-list chunking via the `END`
   opcode, which is a transport-layer concern outside the
   chipdb-generator's scope. Reopen requires writing the chunked
   I2C transport as a separate module.
2. **Compile-verify gate beyond ESP32-C3.** Only `tests/bsp_esp32c3_compile.rs`
   materialises a throwaway cargo project against the real PAC.
   C6 / P4 / C5 / H2 / C61 are snapshot-only — any template
   regression that breaks `cargo check` for those PACs surfaces
   only when a downstream example crate fails to build. Reopen
   trigger: add `bsp_esp32{c6,p4}_compile.rs` for the two boards
   that have working example crates.
3. **No `§0 CHIPS-ESP-00-CONCEPTS.md` document.** CHIPS-ESP never
   passed through the ratification cycle the sibling vendors did.
   Reopen trigger: a new initiative `CHIPS-ESP-00b` (or similar)
   that retrofits the §0–§15 concepts-doc shape on top of what
   already shipped. Not a behaviour change — purely a discipline
   alignment.

### Known residual risks

- **PAC pin is range-based, not exact-equality.** The
  `beetle-esp32c3` example's `Cargo.toml` pins `esp32c3 = "0.31"`
  as a caret range. The CHIPS-TI retrospective §6.4 / forward
  constraint argues exact-equality (`= "0.31.0"`) when template
  amendments are calibrated against a specific PAC vintage. ESP
  predates that constraint; a re-publish of `esp32c3` at 0.32
  with renamed accessors would silently break the example on
  `cargo update`. The `compile-verify` gate would catch it on
  next CI run, but a developer running `cargo build` locally
  would not.
- **Xtensa chips have no compile-verify path.** The `esp32` /
  `esp32-s2` / `esp32-s3` entries exist in the chipdb but the
  workspace cannot build them on stable rustc (Xtensa requires
  `espup`). Their templates rendered text has never been
  `cargo check`'d against the corresponding xtensa-targeted PAC.
  Any template change validated only against C3 / C6 / P4 risks
  unnoticed Xtensa breakage.
- **`pac.rs` alias-style re-export is divergent from the three
  sibling vendors.** CHIPS-TI-07 / CHIPS-SILABS-07 /
  CHIPS-MICROCHIP-07 (2026-05-15) all converged on
  `pub use <pac_crate>::*;` (glob form) to flatten the consumer
  path. ESP still emits `pub use {{ pac_crate }} as pac;` (alias
  form), which forces double-segment consumer paths
  (`crate::bsp_generated::<board>::pac::pac::Peripherals`).
  Downstream consumers (`bsp_pac_main.rs`) work around the
  double-nest by importing the PAC crate directly at binary
  scope. The divergence is documented in §2.6 + §6.5 of this
  retrospective.
- **Six-file vs eight-file emission depends on `chip.arch`
  starting with `rv32`.** The slate-linker amendment (`41c9e16`,
  2026-04-30) added `memory.x` + `<chip>.x` emission conditional
  on the chip's arch string. Xtensa chips silently fall back to
  6-file emission; nothing in the chipdb yaml schema forces an
  author to acknowledge the difference. Adding a future Xtensa
  chip with a forgotten `linker:` block silently downgrades the
  output set.

## 2. Divergence log

Capturing where reality diverged from the initial trajectory. ESP
has no §0 concepts doc to diverge from, so each entry describes a
divergence from what the initial implementation assumed (the
genesis commit `c72a2b5`, 2026-04-11). Entries follow the same
shape used by the sibling retrospectives:
**Assumption** → **Symptom** → **Root cause** → **Detection gap**.

### 2.1 Linker emission was a slate-9 retrofit, not a v0 feature

- **Assumption.** The genesis commit shipped a six-file emission
  set (`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`,
  `board.rs`) on the theory that consumers would supply their own
  `memory.x` + linker glue, the way `esp-hal`-based crates do.
  The render-test file count was hard-coded to 6.
- **Symptom.** When the `beetle-esp32c3 --features bsp_pac` path
  shipped, the example crate had to hand-author `memory.x` and a
  `device.x`-shaped symbol contract for `esp-riscv-rt`. Every new
  board reopened the same hand-author overhead.
- **Root cause.** Espressif's runtime convention is split between
  `esp-hal` (which embeds the linker script in the HAL crate's
  `build.rs`) and `esp-riscv-rt` (which expects the consumer to
  provide both `memory.x` and a chip-specific `<chip>.x`). The
  v0 templates were modelled after `esp-hal`'s shape; the
  `bsp_pac` consumer pattern needs `esp-riscv-rt`'s.
- **Detection gap.** No snapshot test exercised end-to-end linking;
  the render snapshots only check emitted text. The gap surfaced
  during the `bsp_pac` consumer scaffolding, not during BSP
  generation. Fixed by `41c9e16` (2026-04-30) which added
  `memory.x.jinja` + `chip.x.jinja` and the `EspLinker` IR field,
  conditional on `chip.arch` starting with `rv32`. The render
  tests' expected file count flipped from 6 to 8 in the same
  commit.

### 2.2 svd2rust uppercase peripheral fields were inherited, not validated

- **Assumption.** The genesis commit's templates emitted
  lowercase peripheral fields (`p.uart0.*`, `p.io_mux.*`,
  `p.gpio.*`) on the theory that svd2rust output is field-style
  + lowercase. This was an unverified extrapolation.
- **Symptom.** Slate `c486708` (2026-04-11) "M0 — compile-verify
  the generator output" introduced the `bsp_esp32c3_compile.rs`
  gate and surfaced E0609 errors of the form `no field 'uart0'
  on type 'Peripherals'`. `esp32c3 = 0.31` (Dec 2023+ svd2rust)
  exposes peripherals as **uppercase fields** on `Peripherals`
  (`p.UART0`, `p.IO_MUX`, `p.GPIO`).
- **Root cause.** Modern svd2rust (2023+) emits uppercase
  peripheral identifiers; pre-2021 svd2rust emits lowercase. The
  ESP precedent set in this slate became the *cause* of the
  symmetric mistake in CHIPS-TI / CHIPS-SILABS / CHIPS-MICROCHIP
  later (they inherited the *fixed* ESP shape uppercase-by-default
  and had to *reverse it* to match their older PACs — see
  CHIPS-TI-RETROSPECTIVE §2.1 / CHIPS-SILABS-RETROSPECTIVE §2.2 /
  CHIPS-MICROCHIP-RETROSPECTIVE §2.2). The cross-vendor failure
  mode is symmetric: each vendor inherited the prior precedent
  without auditing PAC vintage.
- **Detection gap.** Snapshot tests are text-only and cannot catch
  PAC type-checking failures. Compile-verify (which slate
  `c486708` introduced) was the first gate that exercised actual
  type resolution. Fixed in the same slate by changing the
  `pac_path_filter` MiniJinja filter to uppercase peripheral
  instance names and rewriting `io_mux.rs.jinja` /
  `peripherals.rs.jinja` to use `p.UART0` / `p.IO_MUX` / `p.GPIO`
  directly.

### 2.3 `io_mux.gpio(N)` indexer was an unaudited assumption

- **Assumption.** The genesis `io_mux.rs.jinja` template emitted
  `p.io_mux().gpio{N}()` — a hard-coded per-pin method call.
- **Symptom.** Slate `c486708` "M0" compile-verify surfaced
  `error[E0599]: no method named 'gpio0' / 'gpio1' / ...` against
  `esp32c3 = 0.31`.
- **Root cause.** `esp32c3 = 0.31`'s `IO_MUX` struct exposes pin
  access as an **indexer** (`io_mux.gpio(N)`) rather than per-pin
  methods. The TRM describes the pin-mux array conceptually as
  GPIO0..GPIO21; the PAC encodes it as a single indexed accessor.
  This is the *opposite* direction of the CHIPS-TI §2.2 finding,
  where the older PAC had per-instance methods rather than an
  indexer. PAC vintage determines which shape applies.
- **Detection gap.** Same as §2.2 — snapshot only checks text;
  compile-verify is the first real gate. Fixed in slate `c486708`
  by rewriting `io_mux.rs.jinja` to use `p.IO_MUX.gpio({N})` with
  `{N}` parameterised by the IR rather than baked into the
  method name.

### 2.4 GPIO matrix routing collapsed multi-signal peripherals

- **Assumption.** The genesis `pick_route_for_signal` function in
  `src/bin/creator/bsp/espressif/render.rs` resolved a board
  pin's matrix routing by looking up the peripheral's name in
  the chip yaml's `peripherals` table and taking the first
  matching signal id.
- **Symptom.** The DFR0868 Beetle's `beetle_esp32c3.yaml` lists
  I2C0 SDA on GPIO1 and I2C0 SCL on GPIO2. Both pins resolved to
  the same signal id (the first I2C0 entry in the chip yaml),
  causing both `io_mux.rs` lines to write the same matrix
  routing. The board never made it past I2C0 bus init.
- **Root cause.** A peripheral instance like `I2C0` has multiple
  GPIO matrix signals (`I2CEXT0_SDA_IN`, `I2CEXT0_SDA_OUT`,
  `I2CEXT0_SCL_IN`, `I2CEXT0_SCL_OUT`); the v0 router didn't
  disambiguate between them. The board yaml has the disambiguator
  in the `signal:` column (e.g. `i2c0_sda`, `i2c0_scl`), but the
  router ignored it.
- **Detection gap.** The snapshot test would have caught this if
  the snapshot had been blessed *after* hardware bring-up
  confirmed the wiring worked. As-shipped, the bug was caught by
  attempting to bring up the I2C bus on actual hardware. Fixed
  in slate `c486708` "M1" by adding `pin_role_hint` to
  `render.rs`: extract a role (`sda` / `scl` / `tx` / etc.) from
  the board pin's `signal:` column and pass it to
  `pick_route_for_signal` for disambiguation.

### 2.5 Chip yaml grew organically without a §0 vocabulary lock

- **Assumption.** ESP32-C3 chip yaml (`db/chips/esp32c3.yaml`)
  was the first inventory file and set the schema by example.
  Subsequent chips (C6, P4, C5, H2, C61, ESP32 classic, S2, S3)
  cloned that shape.
- **Symptom.** Inconsistencies between chips emerged that aren't
  bugs per se but are vocabulary drift: ESP32-C3 lists its
  clock-gate register as `SYSTEM`, ESP32-C6 uses `PCR`, ESP32-P4
  uses `HP_SYS_CLKRST`. Each chip's `system_gates:` block has
  the same shape but the **register name** varies. The chip yaml
  contains the register name explicitly, which is correct, but
  there's no §0 doc declaring that "the clock-gate register
  identifier is a per-chip authority field" — it's implicit in
  the template-driven render path.
- **Root cause.** No §0 concepts doc enumerated the per-chip
  authority surface (which fields the chip yaml owns vs.
  inherits from a vendor-wide template). The sibling
  retrospectives all benefited from a §0 ratification that froze
  these enumerations before any chip was added. CHIPS-ESP
  skipped this step and grew the schema empirically.
- **Detection gap.** No automated gate. Drift surfaced during
  CHIPS-TI authoring when the TI author had to extract the
  per-chip-authority pattern from reading ESP code, rather than
  reading a CHIPS-ESP §0 doc. This is the primary motivation for
  §6.6 forward constraint (retrofit §0).

### 2.6 `pac.rs` alias-style re-export forces double-nested consumer paths

- **Assumption.** The genesis `pac.rs.jinja` template emitted
  `pub use {{ ir.chip.pac_crate }} as pac;` — alias-style
  re-export. Intent: give consumer code a stable `pac::` namespace
  regardless of which crate name the chip's PAC publishes under.
- **Symptom.** Downstream consumers in `examples/beetle-esp32c3/
  src/bsp_pac_main.rs` reach peripherals through
  `crate::bsp_generated::<board>::pac::pac::Peripherals` — a
  double `pac::pac::` segment that confuses readers and forces
  the example crate to import `esp32c3` directly at binary scope
  as a workaround.
- **Root cause.** Alias-style re-export creates a nested
  namespace: `bsp::pac` is the BSP's module, `bsp::pac::pac` is
  the re-aliased PAC crate, and `Peripherals` lives at the
  crate root inside the alias. A glob re-export
  (`pub use {{ pac_crate }}::*;`) would hoist `Peripherals` into
  `bsp::pac` directly, giving a single-segment path
  (`bsp::pac::Peripherals`).
- **Detection gap.** Compile-verify accepts the double-nest; the
  code type-checks. Only consumer ergonomics surface the
  divergence. The three sibling vendors caught this in their own
  slate-13 / -07 cleanup sweeps (CHIPS-TI-07 `59a2779`,
  CHIPS-SILABS-07 `2ae1930`, CHIPS-MICROCHIP-07 `3b94d13`, all
  2026-05-15). **ESP has not yet received the matching fix**;
  the divergence is logged here as a forward-constraint trigger
  (§6.5).

### 2.7 `tick_ref_always_on` / `st_utx_out` were nonexistent fields

- **Assumption.** Initial `peripherals.rs.jinja` (genesis
  commit) emitted UART0 init code that referenced
  `tick_ref_always_on` (clock-divider config) and `st_utx_out`
  (transmit-state polling) writer/reader methods on the
  `UART0` peripheral.
- **Symptom.** Slate `c486708` compile-verify against `esp32c3
  = 0.31` surfaced `error[E0599]: no method named ...` for both.
- **Root cause.** Both identifiers were authored from the
  ESP32-C3 TRM section headings but do not appear in the SVD /
  PAC. The TRM describes the underlying hardware state machine
  prose-style; the SVD only exposes the register-level fields
  that are actually documented in the TRM register summary
  tables. Prose chapter content is not register-bit content.
- **Detection gap.** No automated gate cross-checks template
  emission against the PAC's writer/reader surface. The
  compile-verify gate is the catch. Fixed in slate `c486708`
  by dropping both references from the UART0 init sequence;
  the UART0 clkdiv write was rewritten as a raw `w.bits(...)`
  call to sidestep field-width type errors. This is structurally
  the same class of bug as CHIPS-TI's §2.5 "latent typos"
  finding — TRM prose names are not authoritative against the
  PAC.

### 2.8 `func_in_sel_cfg.sig_in_sel` was renamed between PAC vintages

- **Assumption.** Initial `io_mux.rs.jinja` GPIO-matrix branch
  emitted a write to the `sig_in_sel` field of
  `func_in_sel_cfg`.
- **Symptom.** `esp32c3 = 0.31` compile-verify: `no field
  'sig_in_sel'`.
- **Root cause.** The field was renamed (or removed) in the
  svd2rust output between the PAC vintage the template was
  authored against and the 0.31 pin. The TRM still describes
  the field by its original name; the PAC reflects the SVD as
  republished by Espressif.
- **Detection gap.** Same as §2.7 — TRM-name vs. PAC-name
  divergence. Fixed in slate `c486708` by dropping the
  `sig_in_sel` write entirely; matrix routing works correctly
  without it on 0.31. Whether it's needed on later PAC vintages
  is unverified.

## 3. Refactor points

Decision inflection nodes where the CHIPS-ESP initiative changed
direction. Each as **Trigger → Alternatives → Selection rationale
→ Cost of switch**.

### 3.1 Slate `c486708` — compile-verify-or-snapshot-only

- **Trigger.** Slate `c72a2b5` (genesis) shipped templates that
  rendered green snapshots but had never been validated against a
  real PAC. The "M0" milestone of slate `c486708` had to choose
  whether to keep snapshot as the sole acceptance gate, or
  introduce a heavier compile-verify gate.
- **Alternatives.**
  - (a) Stay snapshot-only. Cheap, fast, but does not catch PAC
    type-mismatch bugs.
  - (b) Add a compile-verify gate that materialises a throwaway
    cargo project around the rendered BSP and runs `cargo check
    --target riscv32imc-unknown-none-elf` against `esp32c3 =
    0.31`. Slower (~30s end-to-end), needs `rustup target add
    riscv32imc-unknown-none-elf` and network access for the PAC
    crate, but catches every divergence in §2.2 / §2.3 / §2.7 /
    §2.8.
- **Selection rationale.** (b), but gated behind a
  `--features compile-verify` opt-in to keep the default `cargo
  test` cheap. The compile-verify gate became the pattern every
  subsequent vendor lane inherited (CHIPS-TI, CHIPS-SILABS,
  CHIPS-MICROCHIP all replicated it slate-by-slate).
- **Cost of switch.** ~200 lines of test scaffolding in
  `tests/bsp_esp32c3_compile.rs`. Target dir caching under
  `$TMPDIR/rlvgl-bsp-...-compile-verify-target` keeps reruns
  cheap. The investment paid for itself within the same slate by
  catching four bugs (§2.2, §2.3, §2.7, §2.8); the pattern paid
  for itself again three times over in the sibling vendor
  lanes.

### 3.2 Slate `41c9e16` — linker emission retrofit shape

- **Trigger.** §2.1: the genesis 6-file emission set was
  insufficient for `bsp_pac`-style consumers needing
  `esp-riscv-rt` glue. Either every example crate hand-authors
  `memory.x` + `<chip>.x`, or the generator emits them.
- **Alternatives.**
  - (a) Add two new templates (`memory.x.jinja`, `chip.x.jinja`)
    conditional on `chip.arch` starting with `rv32`. Bump
    every render test's file-count assertion from 6 to 8.
    Xtensa chips stay at 6.
  - (b) Add two new templates **unconditionally**. Xtensa chips
    get linker scripts that will never be used by the consumer
    (Xtensa uses different linker semantics under `esp-hal`).
  - (c) Per-arch template sets — `templates/rv32/` and
    `templates/xtensa/` subdirs.
- **Selection rationale.** (a). Xtensa BSP consumers go through
  `esp-hal`, which embeds linker logic in its own `build.rs`;
  generating linker scripts for them would be unused output.
  Per-arch template sets would have been overkill for two new
  files. The `chip.arch` gating is opaque (no schema warning if
  someone forgets to add `linker:` to a new RISC-V chip yaml)
  but bounded.
- **Cost of switch.** Two new templates, one new `EspLinker` IR
  field (`region_text` / `region_data` `Option<...>`), six new
  linker blocks added to existing chip yamls, twelve render-test
  edits to flip 6 → 8 for the affected chips, and a new
  `hex32` MiniJinja filter to format the address constants in
  the linker output. Net diff: ~200 lines across one commit.

### 3.3 ESP genesis → CHIPS-TI / CHIPS-SILABS / CHIPS-MICROCHIP fork

- **Trigger.** Once CHIPS-ESP demonstrated the chipdb +
  generator + compile-verify pattern, three sibling vendor
  lanes had to be authored. The choice was whether to **share**
  the ESP templates (parameterised on vendor) or **fork** them
  per-vendor.
- **Alternatives.**
  - (a) Single shared template tree at
    `src/bin/creator/bsp/templates/`, parameterised on
    vendor-specific values.
  - (b) Per-vendor template trees at
    `src/bin/creator/bsp/<vendor>/templates/`. Each vendor lane
    owns its own copy. Cross-vendor consistency is a code-review
    discipline, not a build-time guarantee.
- **Selection rationale.** (b). Each vendor's PAC has different
  accessor conventions (uppercase vs. lowercase fields, method
  vs. field accessors, indexer vs. per-instance methods, enum
  FieldWriter vs. BitWriter); shared templates would need
  branches everywhere. The per-vendor fork lets each lane
  optimise for its own PAC vintage without disturbing siblings.
  The cost is cross-vendor drift, surfaced in §2.6 (ESP's
  alias-style `pac.rs` re-export not yet receiving the slate-13
  glob-form fix the siblings landed).
- **Cost of switch.** No "switch" — this was the initial
  branching point. The cost is paid continuously: every
  cross-vendor amendment (like the slate-13 `pac.rs` flatten)
  has to be applied to all four trees. The ESP tree is currently
  one slate behind the siblings on that amendment.

## 4. Mitigation patterns

Abstract the fixes into reusable units. Each pattern is a
"When X + Y → apply Z" rule intended to short-circuit
re-discovery.

### 4.1 PAC vintage audit before template authoring

- **When.** A new vendor lane is being added, or a new chip
  within an existing vendor lane targets a different PAC version
  than the lane's primary chip.
- **Apply.** Before writing or amending templates, read the
  PAC's `docs.rs` page (or `src/lib.rs`) and check four
  shapes:
  1. **Peripheral field casing.** Uppercase (`p.UART0`, modern
     svd2rust 2023+) or lowercase (`p.uart0`, pre-2021)?
  2. **Register accessor shape.** Method (`.clkdiv()`, modern)
     or field (`.clkdiv`, pre-method-accessor)?
  3. **Per-pin/per-instance access.** Indexer (`.gpio(n)`,
     modern) or per-pin methods (`.gpio0()`, older)?
  4. **FieldWriter shape.** Single-bit `BitWriter` (call
     `.set_bit()`) or multi-bit enum FieldWriter (call
     `.variant_name()`)?
  Encode the answers in the vendor's §0 concepts doc (or in the
  chipdb crate's `README.md` for vendors without a §0).
- **Rationale.** §2.2 / §2.3 / §2.7 / §2.8 are all instances of
  the same class of bug: structural assumptions inherited from
  a more modern or older PAC than the target. The CHIPS-TI §4.3
  and CHIPS-SILABS §4.1 retrospectives codify the same pattern
  for their own PAC vintages. CHIPS-ESP **established** the
  pattern as a coping mechanism; future Espressif chip additions
  MUST apply it to themselves before assuming the C3 / 0.31
  shape carries over.

### 4.2 GPIO matrix role hint disambiguator

- **When.** A board's pin table assigns multiple pins to the
  same peripheral instance that exposes more than one GPIO matrix
  signal (e.g. I2C SDA + SCL, SPI MOSI + MISO + SCLK + CS, UART
  TX + RX).
- **Apply.** The board yaml `signal:` column MUST carry a
  role-disambiguating suffix (`i2c0_sda`, `i2c0_scl`,
  `spi2_mosi`, `uart0_tx`, etc.). The render-side
  `pick_route_for_signal` function MUST consult the role hint
  (via `pin_role_hint`) to disambiguate between signals of the
  same peripheral instance. The chip yaml's `gpio_matrix:`
  section MUST name signals at the same disambiguator granularity
  the board yaml uses.
- **Rationale.** §2.4. Without role-hint disambiguation, the
  matrix router silently collapses multi-signal peripherals to a
  single signal, producing hardware-visible bugs that snapshot
  tests do not catch. Future Espressif boards adding multi-signal
  peripherals (SPI, full-duplex UART, TWAI) MUST follow this
  pattern from the start; the v0 single-signal-per-peripheral
  shortcut does not scale.

### 4.3 Compile-verify is the only real gate

- **When.** A template or chipdb yaml change is about to land.
- **Apply.** Snapshot tests freeze emitted text. They catch
  *intended-output regressions*, not *type-correct output*. The
  only gate that exercises actual PAC type resolution, method
  resolution, field-access shape, and FieldWriter enum variants
  is the `compile-verify` test family. Block acceptance of new
  vendor lanes (and any template change to existing lanes) on
  the compile-verify gate passing end-to-end.
- **Rationale.** §2.2 / §2.3 / §2.7 / §2.8 all shipped through
  snapshot-passing emission and surfaced first at
  compile-verify. The pattern was pioneered here in slate
  `c486708`; CHIPS-TI §4.4, CHIPS-SILABS §forward-constraint-4,
  CHIPS-MICROCHIP §6.2 all enshrine the same rule. CHIPS-ESP's
  own coverage is incomplete (only C3 has a compile-verify gate
  at initiative close) — see §6.2 forward constraint.

### 4.4 Conditional template emission by chip-yaml introspection

- **When.** A vendor lane needs to support both modern (with
  linker glue) and legacy (without) runtimes within the same
  chipdb.
- **Apply.** Gate new template emissions on a chip-yaml field
  that the chip author opts into (`linker:` block presence,
  combined with `arch:` field-prefix check). The render path
  conditionally registers the new templates based on the IR
  field. Existing chips without the opt-in field continue to
  emit the legacy file set; new chips opt in by adding the
  field.
- **Rationale.** §2.1 + §3.2. The slate-9 retrofit
  (`41c9e16`) added `memory.x` + `chip.x` conditionally on
  `chip.arch` starting with `rv32` AND the chip yaml having a
  `linker:` block. Xtensa chips silently stay at 6-file
  emission. This pattern lets a vendor lane evolve without
  breaking legacy consumers, and is portable to any chipdb that
  needs to support multiple runtime shapes (esp-hal vs.
  esp-riscv-rt, cortex-m-rt with custom interrupt vectors, etc.).

### 4.5 Per-vendor template fork (not shared parameterised templates)

- **When.** A new vendor lane is being added to
  `rlvgl-creator`.
- **Apply.** Fork the ESP templates into
  `src/bin/creator/bsp/<vendor>/templates/`. Adapt to the
  vendor's PAC vintage in-place. Document cross-vendor patterns
  (the `pac.rs` re-export shape, the `pin_role_hint`
  disambiguator) in the per-vendor concepts doc rather than
  trying to share template code.
- **Rationale.** §3.3. Per-vendor PAC accessor shapes differ
  enough that shared templates would be branch-heavy. Per-vendor
  forks keep each lane readable but require cross-vendor
  amendments (like the slate-13 `pac.rs` flatten) to be
  applied four times. Acceptable cost; the alternative was a
  vendor-discriminating MiniJinja branch matrix that nobody
  would have maintained.

## 5. Deferred work reclassification

Classifying deferred items rather than leaving them as a flat
list. Per the framework: **Safe** (orthogonal, no impact on core
invariants), **Coupled** (named assumption), **Abandoned**
(resurrection-prevention).

### Safe (orthogonal, no impact on core invariants)

- **Multi-chip-per-board support.** The DFR1172 FireBeetle 2 P4
  board carries both an ESP32-P4 (main CPU) and an ESP32-C6
  (radio companion). The chipdb represents these as two
  separate boards (`beetle_esp32p4`, `beetle_esp32c6`) with
  independent BSPs that consumers must wire together at the
  example crate level. A multi-chip board representation would
  fold the pair into one yaml; the BSPs would emit two PAC
  consumer modules. **Orthogonal to v0** — single-chip BSPs
  work, and the DFR1172 example crate is the working reference
  for the multi-chip composition pattern.

### Coupled (affects assumptions; reopen requires named context)

- **SSD1306-over-raw-PAC on beetle-esp32c3 `bsp_pac`.** Coupled
  to **"ESP32-C3 I2C TX FIFO is 32 bytes, SSD1306 framebuffer
  flush is ~1 KB"**. Driving SSD1306 from raw PAC requires
  command-list chunking via the `END` opcode, which is a
  transport-layer concern outside the chipdb-generator's scope.
  Reopens when (a) a chunked-write I2C transport helper is
  written, OR (b) the example crate pivots to an alternate
  display (ST7789 over SPI, e-paper, etc.) whose flush
  semantics fit the TX FIFO. Until then `bsp_pac_main.rs` is
  scoped to LED blink.
- **Compile-verify gate beyond ESP32-C3.** Coupled to **"each
  ESP RISC-V chip has its own PAC crate version with potentially
  different svd2rust accessor shapes"**. Adding
  `bsp_esp32{c6,p4}_compile.rs` requires (a) ensuring the
  chosen PAC version pin is exact-equality, (b) verifying the
  PAC vintage matches what the templates emit (§4.1 audit), and
  (c) accepting the added CI cost (~30s per chip). Reopens
  when any of those gates becomes blocking (e.g. a downstream
  consumer ships a template-divergent build that escapes CI).
- **Pre-publish gate inclusion for ESP compile-verify.**
  Currently only Phase 4.6 of `CLAUDE.md`'s pre-publish list
  runs `bsp_esp32c3_compile`. C6 / P4 / C5 / H2 / C61
  compile-verify gates would need a Phase 4.6b addition once
  they exist (per §6.2 forward constraint).
- **Retrofit §0–§15 concepts-doc shape on CHIPS-ESP.** Coupled
  to **"future chipdb amendments need a §15 change log to be
  audit-traceable"**. The §0 doc doesn't exist; new chip yaml
  additions are landing through plain commit messages without
  the slate-NN cite structure the siblings use. Reopens when
  a non-trivial CHIPS-ESP amendment (e.g. swapping `esp32c3
  = 0.31` for a newer PAC) needs a §15-style decision trail.
  Until then the chipdb's `README.md` + this retrospective
  carry the documentation load.

### Abandoned (resurrection-prevention notes)

- **Xtensa chip compile-verify.** Targeting `xtensa-esp32-none-elf`
  / `xtensa-esp32s2-none-elf` / `xtensa-esp32s3-none-elf` from
  the workspace's stable rustc is impossible (Xtensa requires the
  `espup` toolchain fork). Compile-verify against Xtensa PACs is
  therefore out of scope for the CHIPS-ESP `tests/`-side
  pipeline. Resurrection requires either (a) `espup` stabilising
  into upstream rustc, OR (b) a parallel CI path that runs
  `espup`-based compile-verify out-of-workspace. Neither is on
  any near-term plan; the `examples/esp32-devkits-r/` crate
  exists as the hardware-verified equivalent.
- **Shared parameterised templates across vendors.** Killed by
  §3.3 decision. Resurrection requires either (a) all four
  vendor PACs converging on the same svd2rust accessor shape,
  OR (b) accepting a vendor-discriminating template branch
  matrix that the per-vendor fork was specifically avoiding.

## 6. Forward constraints

This is the only normative section in the retrospective. Future
chipdb-vendor initiatives and future CHIPS-ESP amendments treat
these as binding rules.

### 6.1 Future Espressif chip yaml additions MUST verify PAC vintage before assuming the C3 / 0.31 template shape carries over

Per §4.1. The Espressif PAC ecosystem at `esp32c3 = 0.31` is
modern svd2rust (uppercase fields, method accessors, indexer-style
pin access). Sibling Espressif PACs (`esp32c6`, `esp32p4`,
`esp32`, `esp32s2`, `esp32s3`) MAY be at different vintages on
crates.io at the time a new chip's templates are exercised.
Future agents adding a new chip yaml MUST read the target PAC's
`docs.rs` and verify all four shapes (§4.1 audit) before
expecting the templates to render correctly. The cost of skipping
this is N compile-verify amendments (where N = number of
accessor-shape divergences); the cost of running it is ~30
minutes of `docs.rs` reading.

### 6.2 Compile-verify gate MUST be added for every ESP chip that has a hardware example crate

CHIPS-ESP closed with `bsp_esp32c3_compile.rs` as the only ESP
compile-verify gate. The `beetle-esp32p4` example crate exists
and links against `esp32p4` PAC, but no compile-verify test
proves the chipdb-emitted BSP for the FireBeetle 2 P4 board
type-checks against that PAC. Future CHIPS-ESP slates that touch
ESP32-C6 / ESP32-P4 templates MUST add the corresponding
`bsp_esp32{c6,p4}_compile.rs` gate, gated behind the existing
`--features compile-verify` opt-in, before treating any
template amendment as stable. The pre-publish list (CLAUDE.md
§"Pre-Publish Validation" Phase 4.6) MUST grow to cover the new
gates.

### 6.3 Future Espressif chip yamls MUST cross-check register-field names against actual PAC field names

Per §2.7 / §2.8. TRM section headings and TRM prose chapter names
are NOT authoritative against the PAC's writer / reader surface;
the PAC follows the SVD's register-field names which come from
TRM register-summary tables. When chip yaml authoring touches a
register that the TRM names but the PAC implements, the PAC name
is canonical. The compile-verify gate (§6.2) is the only
mechanism that catches the divergence, and only for the chips
where the gate runs.

### 6.4 `chip.arch` schema MUST be authoritative for conditional template emission

Per §4.4. The `linker:` block in chip yaml is opt-in, gated on
`chip.arch` starting with `rv32`. Adding a new RISC-V chip
without populating the `linker:` block silently emits a 6-file
BSP (no linker glue), which downstream `bsp_pac` consumers will
fail to link. Future chip yaml additions MUST set both `arch:`
and `linker:` together for any chip that should consume
`esp-riscv-rt`-style runtime glue. Xtensa chips MAY omit
`linker:` because their consumer path (esp-hal) embeds linker
logic upstream.

### 6.5 Future ESP `pac.rs.jinja` MUST converge on the glob re-export shape

Per §2.6 + the three sibling slate-13 / -07 amendments (CHIPS-TI-07
`59a2779`, CHIPS-SILABS-07 `2ae1930`, CHIPS-MICROCHIP-07
`3b94d13`). The ESP `pac.rs.jinja` template currently emits
alias-style re-export (`pub use {{ pac_crate }} as pac;`), which
forces double-segment consumer paths. A future amendment slate
MUST flip ESP to glob form (`pub use {{ pac_crate }}::*;`),
matching the three siblings. Snapshot test re-bless is required;
no functional change at compile-verify (the alias form
type-checks). Downstream consumer crates
(`examples/beetle-esp32c3/src/bsp_pac_main.rs`,
`examples/beetle-esp32p4/src/bsp_pac_main.rs`) will need
parallel cleanup to drop the direct-PAC-import workaround.

### 6.6 A future CHIPS-ESP-00b initiative SHOULD retrofit the §0–§15 concepts-doc shape

Per §2.5 + §5 Coupled deferral. CHIPS-ESP grew organically
without a §0 ratification; CHIPS-TI / CHIPS-SILABS /
CHIPS-MICROCHIP each shipped one. A retrofit initiative SHOULD
produce:

- `CHIPS-ESP-00-CONCEPTS.md` co-located with this retrospective,
  declaring the per-chip-authority field surface (`pac_crate`,
  `arch`, `system_gates:` register name, `clock_tree:` shape,
  `gpio_matrix:` shape, `peripherals:` shape).
- `CHIPS-ESP-05-LINKER.md` documenting the `memory.x` +
  `<chip>.x` emission contract that slate `41c9e16` shipped.
- `CHIPS-ESP-06-EXAMPLE.md` documenting the `bsp_pac` example
  crate pattern (detached vs. workspace member, dual-binary
  feature matrix, PAC pin policy).
- `CHIPS-ESP-00 §15 change log` retroactively dated to each of
  the ESP commits in §7, so future amendments have a slate-NN
  anchor.

The retrofit is not a behaviour change; it's a documentation
alignment with the discipline the four-vendor family now expects.
Reopen trigger: any non-trivial CHIPS-ESP amendment (PAC version
bump, schema change, new vendor-wide pattern) that wants a
§15-style decision trail.

### 6.7 Future cross-vendor amendments MUST land in ESP at the same slate they land in TI / SILABS / MICROCHIP

Per §3.3 + §2.6. The per-vendor template fork pays for itself
in local clarity but requires every cross-vendor amendment to
be applied four times. The slate-13 `pac.rs` flatten (§2.6 +
§6.5) is the first amendment to land in three vendors but NOT
in ESP, and it's the inaugural violation of this constraint.
Future cross-vendor sweeps (consumer-path cleanups, MiniJinja
filter updates, IR-shape changes) MUST sweep all four vendors
in the same PR, OR explicitly note the deferred-vendor slate ID
in the §15 change log of each touched vendor.

## 7. Provenance hooks

Each divergence and refactor point linked to authoritative
artifacts (commit SHA, template path, external evidence).
Future agents traverse: **outcome → issue → fix → underlying
evidence** in one hop.

### 7.1 Divergence → fix → commit

| Divergence (§2) | Surfaced in | Fixed in | Anchor |
| --- | --- | --- | --- |
| 2.1 Linker emission as slate-9 retrofit | `c486708` (M1 consumer scaffolding) | `41c9e16` (slate-linker emission) | `src/bin/creator/bsp/espressif/templates/{memory,chip}.x.jinja` |
| 2.2 Uppercase peripheral fields | `c486708` (M0 compile-verify intro) | `c486708` (`pac_path_filter` uppercases) | `src/bin/creator/bsp/espressif/render.rs` `pac_path_filter` |
| 2.3 `io_mux.gpio(N)` indexer | `c486708` (M0 compile-verify) | `c486708` (template rewrite) | `src/bin/creator/bsp/espressif/templates/io_mux.rs.jinja` |
| 2.4 Multi-signal peripheral collapse | hardware bring-up against `beetle_esp32c3` | `c486708` (M1 `pin_role_hint`) | `src/bin/creator/bsp/espressif/render.rs` `pin_role_hint` |
| 2.5 Vocabulary drift across chips | CHIPS-TI authoring (cross-vendor) | unresolved; documented here as forward-constraint §6.6 | this retrospective |
| 2.6 `pac.rs` alias-style re-export | three sibling slate-13 sweeps | unresolved in ESP; forward-constraint §6.5 | `src/bin/creator/bsp/espressif/templates/pac.rs.jinja` |
| 2.7 `tick_ref_always_on` / `st_utx_out` nonexistent | `c486708` (M0 compile-verify) | `c486708` (UART0 init rewrite) | `src/bin/creator/bsp/espressif/templates/peripherals.rs.jinja` |
| 2.8 `func_in_sel_cfg.sig_in_sel` renamed | `c486708` (M0 compile-verify) | `c486708` (drop field write) | `src/bin/creator/bsp/espressif/templates/io_mux.rs.jinja` |

### 7.2 Refactor-point provenance

| Refactor (§3) | Slate range | Anchor commit |
| --- | --- | --- |
| 3.1 Compile-verify-or-snapshot-only | M0 of slate `c486708` | `c486708` (`tests/bsp_esp32c3_compile.rs` introduction) |
| 3.2 Linker emission retrofit shape | slate-9 linker | `41c9e16` (`memory.x.jinja` + `chip.x.jinja`) |
| 3.3 Per-vendor template fork | structural | `c72a2b5` (`src/bin/creator/bsp/espressif/` subdir established) |

### 7.3 Slate-level provenance (CHIPS-ESP organic history)

| Date | Commit | Description |
| --- | --- | --- |
| 2026-04-11 | `c72a2b5` | ESP32-C3 BSP generator: chipdb YAML, EspIr, render pipeline, CLI (genesis) |
| 2026-04-11 | `6de4653` | beetle: add Beetle BLE + FireBeetle 2 P4 example crates |
| 2026-04-11 | `c486708` | bsp-esp32c3: flesh out generator + feature-flag beetle consumer (M0–M4) |
| 2026-04-12 | `7154980` | chipdb: add ESP32-C6 and ESP32-P4 chip IR YAMLs |
| 2026-04-12 | `9a4a440` | P4/C6 BSP integration: board YAMLs, generated BSP, example restructure |
| 2026-04-12 | `eaab6ee` | tests: add ESP32-P4 and ESP32-C6 BSP snapshot, CLI, and IR round-trip tests |
| 2026-04-12 | `171a3da` | bsp: foundation fixes for full ESP32 product line expansion |
| 2026-04-12 | `33a40bc` | chipdb: complete ESP32-C6 peripheral addresses + DFR1075 FireBeetle 2 C6 board |
| 2026-04-12 | `3902326` | chipdb: complete ESP32-P4 peripheral addresses from TRM v1.3 Table 7.3-2 |
| 2026-04-12 | `1324019` | chipdb: add ESP32-H2 chip IR + DevKitM-1 board |
| 2026-04-12 | `5ea9761` | chipdb: add ESP32-C5 and ESP32-C61 chip IRs + minimal boards |
| 2026-04-12 | `30d8978` | chipdb: add ESP32-S3, ESP32-S2, and ESP32 classic chip IRs + boards |
| 2026-04-12 | `3368153` | bsp: add SPI master, LEDC, and TIMG peripheral template init |
| 2026-04-12 | `4adecb5` | bsp: matrix smoke test + list-chips/list-boards CLI subcommands |
| 2026-04-30 | `41c9e16` | creator: emit memory.x + `<chip>.x` linker scripts for ESP RISC-V bsp_pac |
| 2026-04-30 | `4024c13` | examples(beetle-esp32{c3,p4}): consume generated linker scripts |
| 2026-04-30 | `86956ba` | chipdb(esp32{c5,h2,c61}): linker blocks + render tests for bsp_pac |
| 2026-04-30 | `dfb2954` | test(esp): catch up bsp_esp_matrix_smoke + esp_ir_roundtrip with origin's fixture/emit drift |

This is not a numbered-slate sequence; it's a chronological
walk. Future amendments SHOULD adopt the `CHIPS-ESP-NN[a-z]:`
slate prefix and feed a §15 change log in `CHIPS-ESP-00-CONCEPTS.md`
once §6.6 is acted on.

### 7.4 External references

- **ESP32-C3 PAC.** `esp32c3 = 0.31` on crates.io. Modern
  svd2rust (uppercase fields, method accessors, indexer-style
  pin access). Drove §2.2 / §2.3 / §2.7 / §2.8 divergences.
- **ESP32-P4 PAC.** `esp32p4` on crates.io (version pinned by
  `examples/beetle-esp32p4/Cargo.toml`). Drove the P4
  peripheral-address population in `3902326`.
- **ESP32-C6 PAC.** `esp32c6` on crates.io. Drove the C6
  peripheral-address population in `33a40bc`.
- **ESP-RISCV-RT.** `esp-riscv-rt = 0.13`. Provides the
  reset / interrupt / linker-symbol contract that `memory.x` +
  `<chip>.x` satisfy. Drove §2.1 + §3.2.
- **ESP32-C3 TRM v1.4.** Source for the C3 chipdb inventory
  (Chapters 3, 5, 8, 16). Drove `db/chips/esp32c3.yaml`.
- **ESP32-P4 TRM v1.3 Table 7.3-2.** Source for the P4
  peripheral address map. Cited in `3902326`.
- **DFR0868 Beetle ESP32-C3 schematic** (memalpha notebook 15,
  `DFR0868_schematic_v2.pdf`). Source for the
  `beetle_esp32c3.yaml` pin assignments.
- **DFR1172 FireBeetle 2 P4 + C6 datasheet** (memalpha
  notebook 15, "Beetle BLE"). Source for `beetle_esp32p4.yaml`
  and `beetle_esp32c6.yaml`.
- **Sibling vendor retrospectives.** CHIPS-TI / CHIPS-SILABS /
  CHIPS-MICROCHIP retrospectives at
  `chipdb/rlvgl-chips-{ti,silabs,microchip}/docs/CHIPS-*-RETROSPECTIVE.md`
  enshrine the §0–§7 shape this document adopts, and codify the
  cross-vendor amendments (slate-13 `pac.rs` flatten, exact-
  equality PAC pin, compile-verify mandatory) that CHIPS-ESP
  is one slate behind on.

### 7.5 Memory-system traversal

Future Claude / Codex agents working on a CHIPS-ESP amendment
or a structurally similar new vendor lane should traverse:

- **CLAUDE.md §"Espressif BSP Generator"** for the
  contemporary how-to (regen a board's BSP, run compile-verify,
  beetle-esp32c3 feature matrix).
- **This retrospective** for the divergences-and-mitigations
  corpus.
- **`chipdb/rlvgl-chips-esp/README.md`** for the chipdb crate
  API surface.
- **`src/bin/creator/bsp/espressif/templates/*.jinja`** for the
  authoritative template emission rules. Read these as the
  current "spec" until §6.6 is acted on and a real
  `CHIPS-ESP-00-CONCEPTS.md` exists.
- **Sibling retrospectives** for cross-vendor patterns that
  apply transitively to ESP (slate-13 `pac.rs` flatten, PAC
  pin policy, compile-verify mandatory).

The traversal pattern for now is: **start at CLAUDE.md §Espressif
BSP Generator, drill into this retrospective for the failure-mode
analysis, drill into individual commits in §7.3 for the
per-commit option-space exploration, drill into the sibling
retrospectives for cross-vendor patterns**. Once §6.6 lands, the
traversal collapses to: **start at CHIPS-ESP-00, drill into §15
for canonical decisions, drill into this retrospective for the
historical context**.

## 8. Change log

- **2026-05-15 — Initial retrospective authored.** CHIPS-ESP
  was the elder of the four-vendor chipdb family; the §1–§7
  sections capture lessons-learned that predate the §0
  concepts-doc discipline established by CHIPS-TI / CHIPS-SILABS /
  CHIPS-MICROCHIP. This is a structural catch-up — CHIPS-ESP
  grew organically through commits between 2026-04-11
  (`c72a2b5`, genesis) and 2026-04-30 (`86956ba`,
  esp32{c5,h2,c61} linker blocks), with no §0–§15 ratification
  cycle along the way. Future CHIPS-ESP amendments SHOULD
  retrofit the §0 concepts-doc shape per §6.6 forward
  constraint. No follow-up retrospective expected unless a
  CHIPS-ESP-NN-A sub-letter work cycle resumes from a closed
  state.
