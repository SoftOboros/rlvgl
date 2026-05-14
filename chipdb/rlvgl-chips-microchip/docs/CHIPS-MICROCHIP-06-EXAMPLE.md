<!--
CHIPS-MICROCHIP-06-EXAMPLE.md - Example-crate scaffold for the Microchip
SAM chipdb + BSP-generator initiative. Mirrors the CHIPS-TI-06 /
CHIPS-SILABS-06 lane: the first consumer crate that builds against the
slate-9 generator output (8-file emission set + `memory.x` +
`<chip>.x`) and proves the chipdb → generator → compile pipeline lands
at a linkable binary for a real Microchip board.
-->

# CHIPS-MICROCHIP-06 — Adafruit Feather M4 Express Example Crate

> **Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.
> Closes the [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §14
> "CHIPS-MICROCHIP-06 example crate" unblock by ratifying the v0
> scaffold shape and naming the per-feature follow-on lanes (-06a /
> -06b / -06c). Future example-crate surface changes route through
> this chapter's §15 amendment process; no behaviour PR rides on an
> unamended invariant.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared
in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline". RFC 2119 / RFC 8174 normative keywords (**MUST**,
**MUST NOT**, **SHALL**, **SHOULD**, **MAY**) carry their RFC meanings
when capitalised; lowercase use is narrative.

| Domain                                          | Authoritative source                                                                                                          |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| ATSAMD51J19A register / memory layout           | Microchip SAM D5x/E5x Family Data Sheet, DS60001507F (rev F, 2020-09), §10 "Physical Memory Map" Table 10-1; §11.2 Memory Map |
| Adafruit Feather M4 Express board pinout        | Adafruit Feather M4 Express schematic (rev B) and learn-guide pinout page; same authority cited in `db/boards/adafruit_feather_m4_express.yaml`'s `source:` block (accessed 2026-05-11) |
| Onboard "L" LED location (PA23)                 | Adafruit Feather M4 Express schematic (rev B); cross-reference Adafruit `ArduinoCore-samd` `variants/feather_m4/variant.cpp` `PIN_LED_13 = 13` → PA23 |
| Cortex-M4F architectural semantics              | ARM ARMv7-M Architecture Reference Manual (DDI 0403E.e); `cortex-m` crate (~0.6, matched to PAC vintage)                       |
| Boot / reset handler                            | `cortex-m-rt` ~0.6.12 (matched to the `atsamd51j19a 0.7.1` PAC's `rt` feature dependency)                                       |
| `atsamd51j19a 0.7.1` PAC                        | crates.io `atsamd51j19a` 0.7.1 (atsamd-rs/atsamd workspace; svd2rust output); the PAC's `build.rs` emits `device.x` into `OUT_DIR` when the `rt` feature is enabled |
| Generator output contract                       | [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §6 INV-MC6 (template emission), [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md) §5.1 (linker file set), §5.5 (cortex-m-rt linker integration) |
| Cross-vendor precedent (esp_hal/bsp_pac matrix) | [`examples/beetle-esp32c3/Cargo.toml`](../../../examples/beetle-esp32c3/Cargo.toml); [`examples/beetle-esp32c3/src/bsp_pac_main.rs`](../../../examples/beetle-esp32c3/src/bsp_pac_main.rs); [`examples/beetle-esp32c3/src/bsp_generated/mod.rs`](../../../examples/beetle-esp32c3/src/bsp_generated/mod.rs) |
| Initiative ratification (parent)                | [`CHIPS-MICROCHIP-00-CONCEPTS.md`](CHIPS-MICROCHIP-00-CONCEPTS.md) §14 (unblocks list, named CHIPS-MICROCHIP-06 explicitly)    |
| Sibling slate execution                         | CHIPS-MICROCHIP-01 (chip + board YAML), -02 (renderer + templates + field-style PAC amendment), -01a (PB22/PB23 PMUX fix), -04 (compile-verify gate), -05 (linker emission ratification) |

If a phase needs to **modify** a cited authority (different PAC vintage;
amendment to the `cortex-m-rt` `link.x` contract; addition of a second
Microchip board whose schematic places the user LED on a different
pad) the modification ratifies in a §15 amendment **first**, in a
separate PR, before any behaviour PR rides on it.

## §1 Purpose

Land the first **consumer crate** for the CHIPS-MICROCHIP generator
output. Slates 1–9 closed every gate the
[`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md) §12 acceptance
checklist named **except** §12(f) "an `examples/<microchip-board>/`
crate exists and consumes the generated BSP". This chapter closes
that gap.

CHIPS-MICROCHIP-06 establishes:

1. The **canonical example-crate shape** for the Microchip lane — an
   `examples/feather-m4-express/` directory with a single `bsp_pac`
   feature flag, a `bsp_pac_main.rs` binary entry-point, a
   `build.rs` that copies the slate-9 linker fragments into
   `OUT_DIR`, and a hand-written `src/bsp_generated/mod.rs` module
   index that re-exports the generator-emitted child modules.
2. The **v0 acceptance bar** — `cargo check --target
   thumbv7em-none-eabihf` MUST pass and the resulting binary MUST
   link. The crate is **not** required to drive a peripheral in v0;
   peripheral exercises (LED blink, UART hello-world, rlvgl
   integration) defer to -06a / -06b / -06c.
3. The **complementary-gate framing** with the compile-verify test
   (slate 6 / CHIPS-MICROCHIP-04). The compile-verify gate proves the
   generator output type-checks in **isolation**; the example crate
   proves the same output type-checks in **use** alongside
   `cortex-m-rt` and a real `#[entry]` consumer. The two gates
   together close the "type-checks under isolation but breaks under
   composition" hole that surfaces when a PAC vintage's `link.x`
   contract diverges from what the generator's `<chip>.x` template
   assumes (cf. the §10.2 layering decision in
   [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md)).

This chapter produces a **minimal binary scaffold**, not a full
demo. Per §11 below, peripheral bring-up is explicitly out of v0
scope; landing it under this slate would couple chip-bring-up
debugging to the chipdb-shape ratification and re-introduce the
spec-vs-code drift CLAUDE.md "Spec-Before-Code Planning Discipline"
exists to prevent.

## §2 Problem statement

Slate 9 (`CHIPS-MICROCHIP-08`) brought the Microchip emission set to
8 files per board with the addition of `atsamd51j19a.x` alongside the
already-shipping `memory.x`. The compile-verify test
([`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs))
materialises a throwaway cargo project around the generated files
and runs `cargo check` against `atsamd51j19a 0.7.1` on
`thumbv7em-none-eabihf` — but the throwaway project is a `[lib]`
crate. It never links a binary; it never consumes the linker
scripts; it never instantiates `cortex-m-rt`'s `#[entry]` macro.

The gap is structural:

1. **`cargo check --lib`** is not the same gate as **`cargo check
   --bin`**. The PAC's `device.x` emission (gated by the PAC's `rt`
   feature, which depends on `cortex-m-rt`'s `device` feature) only
   runs when `cortex-m-rt` itself is part of the dependency
   closure. The compile-verify test does not pull `cortex-m-rt`,
   so the `device.x` integration path is untested.
2. **`memory.x` + `atsamd51j19a.x`** are emitted by the generator
   but never **consumed** by anything in the test surface. A
   template edit that breaks `memory.x` syntax (a stray
   `REGION_ALIAS` typo; a `MEMORY` block missing a comma) would
   pass the compile-verify gate (which doesn't link) and pass the
   snapshot test (which only diffs text). The failure mode is
   "first downstream consumer can't link" — exactly the kind of
   silent fork CLAUDE.md "Definitions — reference vs. restatement"
   warns against.
3. **The hand-written `src/bsp_generated/mod.rs` module index** has
   no precedent in the Microchip tree. The Espressif tree
   ([`examples/beetle-esp32c3/src/bsp_generated/mod.rs`](../../../examples/beetle-esp32c3/src/bsp_generated/mod.rs))
   demonstrates the pattern but it carries vendor-specific quirks
   (the generator emits `#![no_std]` + `#![deny(missing_docs)]` at
   crate-root scope; the host crate needs to wrap with a more
   permissive `mod.rs` so the generator's output works both as a
   crate root and as a child module). Without a Microchip-specific
   chapter ratifying the wrapping shape, future authors are free to
   re-derive incompatible variants.

This chapter closes all three gaps.

## §3 Canonical glossary

Reserved CHIPS-MICROCHIP-06 vocabulary. Cite-vs-restate markers follow
the convention in [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md)
§3.

- **`bsp_pac` feature** — *As defined in
  [`examples/beetle-esp32c3/Cargo.toml`](../../../examples/beetle-esp32c3/Cargo.toml)
  (the `bsp_pac = [...]` feature row); adapted: the Microchip variant
  enables only the PAC + `cortex-m-rt` + `panic-halt` set, with no
  `esp-hal` companion. There is no `mchip_hal` partner feature in v0
  because no high-level HAL crate currently spans the Microchip SAM
  D5x slice in the way `esp-hal` spans Espressif; if a future
  `atsamd-hal`-shaped sibling lands the §10 reconciliation here
  amends.* The Cargo feature that selects the raw-PAC path. In v0 of
  this chapter `bsp_pac` is the **only** feature; it is also the
  `default` feature so a bare `cargo check` exercises the bring-up
  path.
- **`init()` entry** — *As emitted by
  [`src/bin/creator/bsp/microchip/templates/pac.rs.jinja`](../../../src/bin/creator/bsp/microchip/templates/pac.rs.jinja)
  via `pub use pac::init` in the generator's `mod.rs.jinja`; used
  without modification.* The single-call bring-up entry. Internally
  it calls `super::clocks::init()` → `super::io_mux::init()` →
  `super::peripherals::init()` (per the generator's `pac.rs.jinja`).
  v0 of this chapter MUST invoke
  `bsp_generated::adafruit_feather_m4_express::init()` before the
  `loop { wfi() }` idle. The `init` function is re-exported at the
  generator's `adafruit_feather_m4_express/mod.rs` root (via
  `pub use pac::init`); the host `bsp_generated/mod.rs` does NOT
  shadow this re-export.
- **LED pin** — *As defined in
  [`chipdb/rlvgl-chips-microchip/db/boards/adafruit_feather_m4_express.yaml`](../db/boards/adafruit_feather_m4_express.yaml)
  (`pins:` array, `{ pad: PA23, signal: LED, direction: out,
  label: led }` row); used without modification.* The Adafruit
  Feather M4 Express carries the standard Arduino-style "L" LED on
  **PA23** (Arduino pin D13). The board also carries a single
  WS2812 NeoPixel on PB03 (`pins:` row `label: neopixel`); **that
  is not the LED for this chapter's purposes** — driving a NeoPixel
  requires a 800-kHz bit-banged or PWM-aliased timing pattern that
  is materially distinct from a discrete-LED `dirset` + `outset`
  GPIO toggle. PA23 is the canonical "blinky" pad and is the only
  pad named `LED` in the chip YAML's `io_mux.fn_*` columns. Future
  -06a / -06b / -06c lanes that touch the NeoPixel MUST cite the
  PB03 row separately from PA23; conflating them is a §15
  amendment.
- **`build.rs` linker-script copy** — *As defined in
  [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md) §5.5
  (cortex-m-rt linker integration); adapted: this chapter wraps the
  three-line `println!("cargo:rustc-link-arg=-T...")` sequence in a
  `build.rs` that first copies `memory.x` and `atsamd51j19a.x` from
  `src/bsp_generated/adafruit_feather_m4_express/` into `OUT_DIR` so
  the linker's `-L $OUT_DIR` search path finds them.* The build
  script that bridges the generator output (under `src/`) and the
  linker's expectations (OUT_DIR + `-L` search path). Mirrors
  [`examples/beetle-esp32c3/build.rs`](../../../examples/beetle-esp32c3/build.rs)
  in shape; diverges on linker-arg sequence (cortex-m-rt's
  `link.x` ordering vs. `esp-riscv-rt`'s).
- **`bsp_generated` module index** — *Owned by this chapter; the
  hand-written `examples/feather-m4-express/src/bsp_generated/mod.rs`
  file.* A thin Rust module that `pub mod`-re-exports the
  generator-emitted child directory
  (`adafruit_feather_m4_express/`). Necessary because the
  generator's own `mod.rs.jinja` emits crate-root inner attributes
  (`#![no_std]` + `#![deny(missing_docs)]`) and `pub use pac::init`,
  which conflict with the host crate's own `#![no_std]` inner
  attribute when included as a child module. Same wrapping pattern
  as
  [`examples/beetle-esp32c3/src/bsp_generated/mod.rs`](../../../examples/beetle-esp32c3/src/bsp_generated/mod.rs).
- **Board directory stem** — *As emitted by
  [`src/bin/creator/bsp/microchip/render.rs`](../../../src/bin/creator/bsp/microchip/render.rs)
  (the `out_dir.join(&board_stem)` line); used without modification.*
  The snake_case form of the board name from
  [`adafruit_feather_m4_express.yaml`](../db/boards/adafruit_feather_m4_express.yaml).
  Distinct from the `<chip_link_stem>` (`atsamd51j19a`, no separators)
  named in
  [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md) §5.1. Renaming
  is Standards Action per §10 of CHIPS-MICROCHIP-05.

## §4 Source-of-truth map

| Concept                                                              | Owner (canonical)                                                                                            | Mirrored / consumed by                                                                                              |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| Example crate `Cargo.toml`                                           | `examples/feather-m4-express/Cargo.toml` (this chapter)                                                      | `cargo check --manifest-path examples/feather-m4-express/Cargo.toml --target thumbv7em-none-eabihf`                  |
| Binary entry point (`#[entry] fn main()`)                            | `examples/feather-m4-express/src/bsp_pac_main.rs` (this chapter)                                             | `cortex-m-rt`'s `#[entry]` macro; binary name `rlvgl-feather-m4-express`                                            |
| `bsp_generated/` module index                                        | `examples/feather-m4-express/src/bsp_generated/mod.rs` (this chapter; hand-written)                          | `bsp_pac_main.rs`'s `mod bsp_generated` declaration                                                                 |
| Generator-emitted BSP child directory                                | `rlvgl-creator bsp from-yaml --vendor microchip --board adafruit_feather_m4_express` (slates 1–9)            | Committed at `examples/feather-m4-express/src/bsp_generated/adafruit_feather_m4_express/` (8 files: 6 `.rs` + 2 `.x`) |
| `memory.x` content                                                   | Slate 6 (`src/bin/creator/bsp/microchip/templates/memory.x.jinja`)                                            | Copied by `build.rs` into `OUT_DIR`; consumed by `cortex-m-rt`'s `link.x` `INCLUDE memory.x`                         |
| `atsamd51j19a.x` content                                             | Slate 9 (`src/bin/creator/bsp/microchip/templates/atsamd51j19a.x.jinja`; v0 body intentionally empty)         | Copied by `build.rs` into `OUT_DIR`; consumed by linker via explicit `-Tatsamd51j19a.x`                              |
| `device.x` (NVIC vector extensions)                                  | `atsamd51j19a 0.7.1`'s `build.rs` (emits when `rt` feature enabled; consumed via `cortex-m-rt`'s `INCLUDE device.x`) | The PAC dependency declaration in `examples/feather-m4-express/Cargo.toml` (with `features = ["rt"]`)                |
| Linker-arg sequence                                                  | [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md) §5.5                                                    | `examples/feather-m4-express/build.rs` `println!("cargo:rustc-link-arg=-T...")` lines                                |
| LED pad → group/pin tuple                                            | [`adafruit_feather_m4_express.yaml`](../db/boards/adafruit_feather_m4_express.yaml) `pins:` `label: led` row | Generator emits `pub const LED: (u8, u8) = (0, 23);` in `board.rs`                                                  |
| Cortex-M ABI / `#[entry]`                                            | `cortex-m-rt ~0.6.12` (matched to PAC `rt` dependency)                                                        | `bsp_pac_main.rs` `use cortex_m_rt::entry`                                                                          |
| Panic strategy                                                       | This chapter §5.6                                                                                            | `panic-halt 0.2` (default-features); `use panic_halt as _` in `bsp_pac_main.rs`                                      |

## §5 Frozen decisions — example-crate shape

Each decision below names its registration policy per the
*Frozen enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Example-crate directory location — Standards Action

The example crate MUST live at `examples/feather-m4-express/`. The
directory name uses the **kebab-case** form of the board name (matching
the `examples/beetle-esp32c3/` and `examples/beetle-esp32p4/`
precedents). The chipdb-side board YAML and the generated BSP child
directory use **snake_case** (`adafruit_feather_m4_express`); the
example-crate directory deliberately strips the vendor prefix
(`feather-m4-express`, not `adafruit-feather-m4-express`) to match
the `beetle-*` / `esp32-*` precedents that drop the upstream vendor
qualifier when it carries no disambiguation.

Renaming the directory or moving it outside `examples/` is Standards
Action.

### 5.2 Cargo package name — Standards Action

The crate MUST declare `name = "rlvgl-example-feather-m4-express"`
and the binary MUST declare `name = "rlvgl-feather-m4-express"`.
Both follow the `rlvgl-example-<board>` / `rlvgl-<board>` naming
convention shared with
[`examples/beetle-esp32c3/Cargo.toml`](../../../examples/beetle-esp32c3/Cargo.toml).

### 5.3 Feature matrix — Standards Action

In v0 of this chapter the example crate carries exactly **one**
feature: `bsp_pac`. The feature is **default**.

```toml
[features]
default = ["bsp_pac"]
bsp_pac = []
```

There is no `mchip_hal` companion feature because no high-level HAL
crate currently spans the SAM D5x slice in the way `esp-hal` spans
Espressif. `atsamd-hal` exists but is a workspace-shaped dependency
whose feature-flag matrix is itself in flux upstream; adding a second
feature path under this chapter's scope would couple the Microchip
example-crate ratification to upstream `atsamd-hal` API stability and
delay -06's acceptance gate.

Adding a second feature (e.g. `mchip_hal`, `rlvgl`, `usb`) is
**Specification Required** — a -06a / -06b / -06c walkthrough updates
this section, adds the feature, and re-blesses the §12 acceptance
checklist. The §10 reconciliation row about HAL companions also
amends.

### 5.4 Binary entry-point shape — Specification Required

The single binary `rlvgl-feather-m4-express` MUST live at
`src/bsp_pac_main.rs` and MUST follow this shape:

```rust
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[entry]
fn main() -> ! {
    #[cfg(feature = "bsp_pac")]
    {
        bsp_generated::adafruit_feather_m4_express::init();
    }
    loop {
        cortex_m::asm::wfi();
    }
}
```

Notes:

1. `bsp_generated::adafruit_feather_m4_express::init()` is the
   through-the-host-module-index access path. The host `mod.rs`
   (§5.5) re-exports the generator's child directory under its
   snake_case stem; the generator's
   `adafruit_feather_m4_express/mod.rs` carries `pub use pac::init`
   which re-exports the `pub fn init()` from `pac.rs`. That `init`
   internally chains `clocks::init -> io_mux::init ->
   peripherals::init`.
2. The `#[cfg(feature = "bsp_pac")]` gates exist to preserve the
   future-proofing for an additional feature without forcing a
   re-write of `main.rs` when that feature lands. When the only
   feature is `bsp_pac` (= default) the gates are no-ops.
3. The final `loop { wfi() }` is the v0 idle. Promoting this to
   a peripheral exercise (LED blink, UART hello-world) is
   -06a-and-beyond work; v0 acceptance does NOT require any
   pin to toggle.

Adding a peripheral exercise to `bsp_pac_main.rs` itself is
**Specification Required** — the walkthrough updates this section
and the §11 non-goals list. **Removing** `#[no_std]` /
`#[no_main]` / `#[entry]` is Standards Action (it would break the
cortex-m-rt boot contract).

### 5.5 `bsp_generated/mod.rs` host index shape — Specification Required

The hand-written host `src/bsp_generated/mod.rs` MUST:

1. Declare an inner `#![allow(dead_code)]` attribute (the generator
   emits more constants than any one consumer will use).
2. Re-export the generator-emitted child directory via
   `pub mod adafruit_feather_m4_express;` (matching the snake_case
   board stem from §3 "Board directory stem").
3. NOT carry `#![no_std]` or `#![deny(missing_docs)]` — those
   crate-root attributes belong to the parent
   `bsp_pac_main.rs` (`#![no_std]`) and are conflict-prone when
   the generator's `mod.rs` is consumed as a child module
   directly. The generator's own `mod.rs.jinja` is intentionally
   not pulled in by this index for the same reason; the
   generator-emitted child directory's `mod.rs` is included via
   the `pub mod adafruit_feather_m4_express;` declaration as a
   nested child.

The wrapping shape mirrors
[`examples/beetle-esp32c3/src/bsp_generated/mod.rs`](../../../examples/beetle-esp32c3/src/bsp_generated/mod.rs)
in intent (host module index re-exports generator output without
inheriting the generator's crate-root inner attributes) but the
Microchip variant nests the board-stem directory one level deeper
because the Microchip generator emits to
`<out>/<board_stem>/{mod.rs,...}` whereas the Espressif generator
emits to `<out>/<board_stem>/{board.rs,...}` (the Espressif
example crate flattens by copying the child files up one level;
the Microchip example crate leaves them nested for one-shot
regeneration).

The flatten-vs-nest decision is **Specification Required** to
amend — switching to a flatten layout requires updating both this
section and the §5.4 access path
(`bsp_generated::adafruit_feather_m4_express::init()` →
`bsp_generated::init()`).

### 5.6 Panic strategy — Standards Action

The example crate MUST use `panic-halt 0.2` (default features) and
import it as `use panic_halt as _;`. Switching to a different
panic strategy (`panic-semihosting`, `panic-probe`, etc.) is
Standards Action — the choice affects binary size, debug surface,
and the run-time behaviour the §12 acceptance gate's "binary
links" check tolerates.

`panic-halt` is the same choice as
[`examples/beetle-esp32c3/`](../../../examples/beetle-esp32c3/Cargo.toml)
and was selected for that crate over `esp-backtrace` precisely
because its `bsp_pac` feature path runs in a context where the
HAL-level backtrace tooling isn't pulled in. The Microchip lane
has no HAL-level backtrace crate to consider; `panic-halt`
remains the minimal choice.

### 5.7 Cargo.toml `[profile.release]` shape — Specification Required

The release profile MUST set `opt-level = "z"`, `lto = true`,
`codegen-units = 1`, and `debug = false`. These match the
profile shape that landed for `examples/beetle-esp32c3/` after
slate 5 of that initiative and are oriented at minimal-binary
flash footprint for a Cortex-M4F. Changing any field is
Specification Required.

## §6 Frozen decisions — `build.rs` and linker integration

### 6.1 `build.rs` shape — Specification Required

The `build.rs` MUST:

1. Read `CARGO_MANIFEST_DIR` and locate
   `src/bsp_generated/adafruit_feather_m4_express/`.
2. Copy `memory.x` and `atsamd51j19a.x` from that directory into
   `OUT_DIR` (skipping silently if either file is absent — handles
   the first-commit transient state before the BSP is regenerated).
3. Emit `cargo:rustc-link-search=<OUT_DIR>` so the linker's `-L`
   search path picks up the copied fragments.
4. Emit `cargo:rerun-if-changed=<bsp_dir>` so a regenerate-and-build
   cycle picks up the new fragments.
5. NOT emit `cargo:rustc-link-arg=-T...` lines for the linker
   fragments — those live in `.cargo/config.toml` instead (per
   §6.2 below). This diverges from
   [`examples/beetle-esp32c3/build.rs`](../../../examples/beetle-esp32c3/build.rs)
   which emits the `-T` lines from the build script. The Microchip
   variant prefers `.cargo/config.toml` because the linker-arg
   sequence is short (only `-Tlink.x` is needed; `link.x` from
   `cortex-m-rt` does `INCLUDE memory.x` and `INCLUDE device.x`
   itself) and putting it in config makes the per-target
   relationship more explicit.

### 6.2 `.cargo/config.toml` shape — Specification Required

The crate MUST carry a `.cargo/config.toml` with:

```toml
[build]
target = "thumbv7em-none-eabihf"

[target.thumbv7em-none-eabihf]
rustflags = [
  "-C", "link-arg=-Tlink.x",
]
```

The `target = "thumbv7em-none-eabihf"` line is what makes `cargo
check` from inside `examples/feather-m4-express/` use the right
ABI without an explicit `--target` flag.

The single `-Tlink.x` link-arg is sufficient because `cortex-m-rt
~0.6.12`'s `link.x` template:

- `INCLUDE`s `memory.x` (which the `build.rs` copied into `OUT_DIR`
  per §6.1);
- `INCLUDE`s `device.x` (which the `atsamd51j19a 0.7.1` PAC's
  own `build.rs` emits into `OUT_DIR` when the `rt` feature is
  enabled);
- Does **not** auto-include the chip-specific `atsamd51j19a.x`
  slot file. In v0 the slot file body is empty (per
  [`CHIPS-MICROCHIP-05`](CHIPS-MICROCHIP-05-LINKER.md) §5.3), so
  the binary links without it. When a future amendment populates
  `atsamd51j19a.x` with `SECTIONS` directives, the consuming
  crate MUST add `-Tatsamd51j19a.x` to the rustflags array; that
  amendment lands as a -06d (or analogous) walkthrough under this
  chapter's §15.

Changing the linker-arg ordering or adding a second `-T...` line
is Specification Required.

## §7 Verification gates

The verification surface for this chapter is the same three-layer
contract that [`CHIPS-MICROCHIP-00`](CHIPS-MICROCHIP-00-CONCEPTS.md)
§12 names, plus a new fourth gate specific to this chapter:

1. **Snapshot render test** — unchanged. Still gated by
   [`tests/bsp_microchip_render.rs`](../../../tests/bsp_microchip_render.rs)
   on the 8-file emission set.
2. **Compile-verify test** — unchanged. Still gated by
   [`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs)
   on `cargo check --lib` against `atsamd51j19a 0.7.1`.
3. **Example-crate binary check** — **NEW with this chapter.**
   `cargo check --manifest-path examples/feather-m4-express/Cargo.toml
   --target thumbv7em-none-eabihf` MUST pass. This gate consumes
   `memory.x`, `atsamd51j19a.x` (when populated), the PAC's
   `device.x`, and `cortex-m-rt`'s `link.x` together; it closes the
   "compile-verify type-checks the lib but never links a binary"
   gap from §2.
4. **Hardware bring-up** — Deferred to -06a / -06b / -06c. A
   physical Adafruit Feather M4 Express plus a SWD probe is
   required; the gate is **MAY** (per the CLAUDE.md "Conformance
   targets" tier system, hardware-on-real-silicon is the third
   tier above the cargo-check tier).

Promoting gate 3 from `cargo check` to `cargo build --release` is
Specification Required; the build gate would catch additional
classes of linker error (e.g. an `atsamd51j19a.x` body that
references a symbol the PAC's `device.x` doesn't provide) but
would also lengthen CI wall time.

## §8 Round-trip / regression posture

A CHIPS-MICROCHIP-NN execution PR that modifies any generator
template (`mod.rs.jinja`, `board.rs.jinja`, etc.) MUST re-generate
the committed `src/bsp_generated/adafruit_feather_m4_express/`
directory in the same PR. The §7 gate 3 example-crate check then
runs against the freshly-regenerated output.

A PR that modifies `examples/feather-m4-express/Cargo.toml`,
`build.rs`, `.cargo/config.toml`, or `bsp_pac_main.rs` runs only
§7 gate 3 (the snapshot test and compile-verify test do not
depend on the example crate).

A PR that bumps the `atsamd51j19a` PAC version, the
`cortex-m`/`cortex-m-rt` versions, or `panic-halt` version
constitutes a dependency-vintage change and MUST land as a §15
amendment first (per CLAUDE.md "Execution discipline").

This chapter introduces no new regression posture beyond §7.

## §9 Files cited

- [`CLAUDE.md`](../../../CLAUDE.md) — Spec-Before-Code Planning
  Discipline, RFC 2119 keywords, registration policy, initiative
  prefix.
- [`chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-00-CONCEPTS.md`](CHIPS-MICROCHIP-00-CONCEPTS.md)
  — parent concepts doc; §14 unblocks list named CHIPS-MICROCHIP-06
  explicitly.
- [`chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-05-LINKER.md`](CHIPS-MICROCHIP-05-LINKER.md)
  — linker emission chapter; §5.1 file-set rule (`memory.x` +
  `atsamd51j19a.x`); §5.5 cortex-m-rt linker-arg sequence; §10.2
  PAC `device.x` layering.
- [`chipdb/rlvgl-chips-microchip/db/boards/adafruit_feather_m4_express.yaml`](../db/boards/adafruit_feather_m4_express.yaml)
  — board IR YAML; authoritative source for the LED pin (PA23) and
  the SERCOM5 default-console choice future -06b will consume.
- [`chipdb/rlvgl-chips-microchip/db/chips/ATSAMD51J19A.yaml`](../db/chips/ATSAMD51J19A.yaml)
  — chip IR YAML; pinned PAC vintage = `atsamd51j19a 0.7.1`.
- [`src/bin/creator/bsp/microchip/templates/`](../../../src/bin/creator/bsp/microchip/templates/)
  — generator template directory; this chapter does not modify any
  template, only consumes the existing emission.
- [`tests/bsp_microchip_render.rs`](../../../tests/bsp_microchip_render.rs)
  — render test; unchanged by this chapter.
- [`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs)
  — compile-verify test (`cargo check --lib`); complement gate to
  §7 gate 3.
- [`examples/beetle-esp32c3/Cargo.toml`](../../../examples/beetle-esp32c3/Cargo.toml)
  — Espressif precedent; `bsp_pac` feature shape; profile.release
  settings.
- [`examples/beetle-esp32c3/src/bsp_pac_main.rs`](../../../examples/beetle-esp32c3/src/bsp_pac_main.rs)
  — Espressif precedent; `#[no_std]` + `#[no_main]` + `#[entry]`
  shape adapted for Cortex-M.
- [`examples/beetle-esp32c3/src/bsp_generated/mod.rs`](../../../examples/beetle-esp32c3/src/bsp_generated/mod.rs)
  — Espressif precedent; host module index that wraps generator
  output without inheriting its crate-root inner attributes.
- [`examples/beetle-esp32c3/build.rs`](../../../examples/beetle-esp32c3/build.rs)
  — Espressif precedent; `build.rs` shape (diverges per §6.1).
- Microchip DS60001507F — SAM D5x/E5x Family Data Sheet, rev F
  (2020-09); §10 Table 10-1 Physical Memory Map.
- Adafruit Feather M4 Express schematic (rev B) — authoritative
  source for the PA23 LED pin and the PB03 NeoPixel pad.
- crates.io `atsamd51j19a 0.7.1` — pinned PAC; `rt` feature
  pulls in `cortex-m-rt ~0.6.12`.
- crates.io `cortex-m-rt ~0.6.12` — `link.x` template; `INCLUDE
  memory.x` + `INCLUDE device.x` integration.

## §10 Reconciliation with adjacent repo primitives

### 10.1 Compile-verify complement

The slate-6 compile-verify gate
([`tests/bsp_microchip_compile.rs`](../../../tests/bsp_microchip_compile.rs))
and this chapter's example crate are **complementary**, not
redundant:

| Gate                         | What it proves                                                                                                      | What it does NOT prove                                                                                              |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Compile-verify (`cargo check --lib`) | Generated `.rs` files type-check against `atsamd51j19a 0.7.1` in **isolation** (no `cortex-m-rt`, no linker scripts) | Linker fragments are syntactically valid; `cortex-m-rt` integration works; `device.x` symbols resolve              |
| Example-crate check (this chapter) | Generated files compile **and link** alongside `cortex-m-rt`, `panic-halt`, `cortex-m`, and the consumer-side `build.rs` linker-script plumbing | Hardware boots; peripherals function; ISRs deliver; clock tree actually configures (gate 4 / -06a-and-beyond)         |

Removing or weakening either gate is Standards Action — the
compile-verify gate catches PAC-shape regressions in the absence
of a HAL; the example-crate check catches integration regressions
in the presence of a HAL. Together they close the "spec-vs-code
drift" hole CLAUDE.md "Spec-Before-Code Planning Discipline"
exists to prevent.

### 10.2 PAC `device.x` layering

The `atsamd51j19a 0.7.1` PAC's `build.rs` emits `device.x` into
`OUT_DIR` **only** when the PAC's `rt` feature is enabled
(`features = ["rt"]` in the Cargo.toml dependency declaration).
This chapter's example crate MUST enable the `rt` feature so the
NVIC vector extensions resolve at link time. Disabling `rt`
would surface as a stream of `undefined reference to ...`
errors at link time; the failure mode is loud, not silent.

The PAC's `rt` feature pulls in `cortex-m-rt ~0.6.12`'s `device`
feature, which makes its `link.x` do `INCLUDE device.x`. The two
crates' linker-script contracts are **layered** here; this
chapter does not invent the layering, it consumes it.

### 10.3 esp_hal / bsp_pac feature matrix divergence

[`examples/beetle-esp32c3/`](../../../examples/beetle-esp32c3/Cargo.toml)
ships **two** mutually-exclusive features (`esp_hal` and
`bsp_pac`). This chapter ships **one** (`bsp_pac` only). The
divergence is intentional and reflects:

- No high-level HAL crate currently spans the SAM D5x slice in
  the way `esp-hal` spans Espressif. `atsamd-hal` exists but
  carries an unstable feature surface; coupling -06's acceptance
  to upstream `atsamd-hal` stability would block ratification.
- The dual-feature pattern in the Espressif tree exists
  specifically to validate that the BSP generator output and
  the HAL crate can **coexist** in one crate. There is no analog
  coexistence claim to validate on the Microchip side until a
  HAL companion lands.

When a HAL companion lands (a future -06d-or-later walkthrough),
the §10.3 row amends and the §5.3 feature-matrix row gains a
second feature.

### 10.4 NeoPixel pad disambiguation

The Adafruit Feather M4 Express schematic labels both PA23 (the
discrete LED) and PB03 (the WS2812 NeoPixel) on the user-visible
silkscreen. CHIPS-MICROCHIP-06 explicitly chooses **PA23** as
the v0 LED concept because:

1. The Arduino-style D13 pin maps to PA23 by convention; the
   Adafruit `ArduinoCore-samd` variant file
   (cited in the §0 authority table) and the
   [`adafruit_feather_m4_express.yaml`](../db/boards/adafruit_feather_m4_express.yaml)
   `pins:` `label: led` row both anchor on PA23.
2. PA23 is driven by a `dirset` + `outset`/`outclr` GPIO pair; a
   single PORT register write toggles it. PB03 requires a
   bit-banged 800-kHz WS2812 timing pattern that materially
   exceeds the v0 scope of "binary links."

Driving PB03 (NeoPixel) in v0 is **out of scope**. A future
-06c-or-later walkthrough that drives the NeoPixel MUST cite
PB03 separately from PA23 and add a §11-style "NeoPixel"
non-goal-becomes-goal row to that walkthrough's §15.

### 10.5 Hand-written `examples/stm32h747i-disco/` precedent

The hand-written `examples/stm32h747i-disco/` crate predates the
chipdb initiative and uses a different shape entirely
(hand-written `memory.x`; no generator; `cortex-m-rt 0.7`
instead of the PAC-matched 0.6 line). CHIPS-MICROCHIP-06
makes no claim over `examples/stm32h747i-disco/`; the two
example crates coexist without sharing structure.

## §11 Non-goals (v0)

Explicit out-of-scope for v0 of this chapter:

- **No LED blink.** The v0 binary calls `init()` and parks in
  `wfi()`. Driving the PA23 LED (a `PORT.group0.dirset` write
  followed by a periodic `outset`/`outclr` loop) is **-06a** work.
  Why deferred: a blink loop introduces a busy-wait / timer
  decision that ratifies under a separate frozen-decision
  section; landing it under this slate would couple the binary
  scaffold ratification to a timing-source choice.
- **No console UART hello-world.** The chipdb board YAML's
  `console:` block names SERCOM5 USART; emitting a "Hello,
  Feather M4 Express!" string over PB16/PB17 (`u_tx`/`u_rx`) is
  **-06b** work. Why deferred: SERCOM USART bring-up requires
  the GCLK PCHCTRL channel for SERCOM5 to be enabled and the
  PMUX C function on PB16/PB17 to be selected; both happen in
  the generator's `peripherals.rs`/`io_mux.rs` output, so
  the binary side is "use that init plus a polled
  `intflag.dre` write loop." That work amends §5.3 to add a
  `console` feature and §5.4 to extend `bsp_pac_main.rs`.
- **No rlvgl integration.** The example crate does NOT depend on
  `rlvgl-core` / `rlvgl-platform` / `rlvgl-widgets` in v0.
  Pulling those crates in is **-06c** work. Why deferred:
  rlvgl pulls in an allocator (the rendering pipeline assumes
  `alloc`), which forces a `global_allocator` choice; the
  Feather M4 Express has 192 KB of SRAM, so the allocator
  choice is non-trivial. The choice ratifies under -06c.
- **No I2C / SPI / ADC driver exercise.** Touching SERCOM2 (I2C),
  SERCOM1 (SPI), or ADC0 is **out of v0 scope**. Each is a
  separate -06d-or-later walkthrough.
- **No USB CDC.** The native USB peripheral (PA24/PA25 USB_DM/DP)
  is the board's primary debug channel but bringing it up
  requires a USB device stack (e.g. `usb-device` + a board
  variant of `usbd-serial`) that is itself out of v0 scope.
- **No QSPI / external flash.** The on-board GD25Q16C QSPI flash
  is documented in the board YAML's `features:` block but the
  v0 binary does not touch it. Bringing it up is a separate
  walkthrough.
- **No hardware bring-up gate.** v0 acceptance does not require
  the binary to be flashed to a real Feather M4 Express. The
  hardware bring-up gate is per §7 gate 4 (deferred to
  -06a-and-beyond).
- **No probe-rs / GDB integration.** This chapter does not name
  a debug-probe path. The CLAUDE.md "Flashing And Debug"
  section is STM32H747I-DISCO-specific; the Microchip lane
  uses standard `cargo embed` / `cargo flash` / `probe-rs run`
  tooling but the v0 scaffold does not commit a probe-rs
  config — a future -06a walkthrough that adds an LED blink
  also commits the `Embed.toml` or equivalent.
- **No CI integration.** This chapter does not add a CI matrix
  entry for `examples/feather-m4-express/`. The
  `cargo check --target thumbv7em-none-eabihf` gate is
  developer-side; promoting it to CI is a separate workflow PR.

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [x] §0 authority table reviewed; Adafruit Feather M4 Express
      schematic (rev B) confirmed as authority for PA23 LED pin.
- [x] §3 glossary terms each carry a cite-vs-restate marker per
      CLAUDE.md §"Definitions — reference vs. restatement".
- [x] §4 source-of-truth map has exactly one owner per row.
- [x] §5.1 directory location frozen at `examples/feather-m4-express/`.
- [x] §5.2 package + binary names frozen.
- [x] §5.3 feature matrix frozen at single `bsp_pac` feature
      (default).
- [x] §5.4 binary entry-point shape frozen; access path
      `bsp_generated::adafruit_feather_m4_express::init()`
      explicit.
- [x] §5.5 host `bsp_generated/mod.rs` index shape frozen.
- [x] §5.6 panic strategy frozen at `panic-halt 0.2`.
- [x] §5.7 release profile shape frozen.
- [x] §6.1 `build.rs` shape frozen; copy-to-OUT_DIR pattern
      consistent with §6.1.
- [x] §6.2 `.cargo/config.toml` shape frozen; single `-Tlink.x`
      argument sufficient for v0.
- [x] §10.1 complementary-gate framing with compile-verify
      documented; §10.2 PAC `device.x` layering documented.
- [x] §11 non-goals enumerated explicitly; -06a / -06b / -06c
      lanes named.
- [x] §15 dated ratification entry.

Behaviour PRs that ride on this chapter (CHIPS-MICROCHIP-06a and
beyond):

- [ ] §7 gate 3 (example-crate binary check)
      `cargo check --manifest-path
      examples/feather-m4-express/Cargo.toml --target
      thumbv7em-none-eabihf` passes. **MUST** before -06's v0
      scaffold lands.
- [ ] §7 gate 1 (snapshot render) and gate 2 (compile-verify)
      continue to pass. **MUST.**

## §13 Files cited

(Subsumed under §9 above; this chapter does not split the cited-files
list from the source-of-truth map narrative. The standard CLAUDE.md
§"Phase document shape" allows §13 to fold into §9 when the chapter
is short and the two lists overlap.)

## §14 Unblocks

Ratifying and implementing this chapter unblocks:

- **CHIPS-MICROCHIP-06a** — LED blink on PA23. Picks a timing source
  (busy-loop vs. SysTick vs. TC0), drives `PORT.group0.dirset =
  1<<23` once at init, then alternates `PORT.group0.outset` /
  `PORT.group0.outclr` on a ~500 ms duty cycle. Amends §5.4 binary
  shape, §5.3 feature matrix (adds `led_blink` cargo feature or
  removes the `loop { wfi() }` outright), and §11 non-goals.
- **CHIPS-MICROCHIP-06b** — Console UART hello-world over SERCOM5
  USART on PB16/PB17. Consumes the generator's `peripherals::init`
  SERCOM5 setup; adds a polled write loop on `usart_int.intflag.dre`.
  Amends §5.4 and §11.
- **CHIPS-MICROCHIP-06c** — rlvgl integration. Pulls in
  `rlvgl-core` / `rlvgl-platform` / `rlvgl-widgets` and a
  `global_allocator` (likely `embedded-alloc` or a fixed-region
  bump allocator). Amends §10.3 once the HAL or alloc companion
  choice is named.
- **Future Microchip example crates.** Adding a second Microchip
  board (e.g. SAM D51 Xplained Pro, SparkFun SAMD51 Thing Plus) is
  a new walkthrough that mirrors -06's §0–§14 shape with the board
  name substituted; the §5.3 feature matrix and §5.5 host module
  index shapes apply unchanged. Adding the **third** Microchip
  example crate is a §15 amendment indicating the multi-crate
  layout has crossed the threshold where a shared library
  (`examples/microchip-common/`?) becomes warranted.

## §15 Change log

| Date       | Status                       | Note                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Example-crate chapter ratified. Closes the [CHIPS-MICROCHIP-00 §14](CHIPS-MICROCHIP-00-CONCEPTS.md#14-unblocks) "CHIPS-MICROCHIP-06 example crate" unblock by ratifying the v0 scaffold shape (single `bsp_pac` feature; `examples/feather-m4-express/` directory; `cortex-m-rt ~0.6.12` + `atsamd51j19a 0.7.1` `features=["rt"]`; `panic-halt 0.2`). Names the -06a (LED blink), -06b (UART hello-world), and -06c (rlvgl integration) follow-on lanes. Frames the example-crate check (§7 gate 3) as a complementary gate to the compile-verify test (slate 6 / CHIPS-MICROCHIP-04). Document-only on ratification; v0 scaffold lands as -06 (no sub-letter; same slate) in the immediately following commit. |
