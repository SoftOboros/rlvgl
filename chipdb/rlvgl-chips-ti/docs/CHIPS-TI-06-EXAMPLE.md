<!--
CHIPS-TI-06-EXAMPLE.md - Example crate chapter for the Texas Instruments
chipdb + BSP-generator initiative. Ratifies the example-crate scaffold
that consumes the slate-9 generator output end-to-end (chipdb → generator
→ compile → linked binary).
-->

# CHIPS-TI-06 — Texas Instruments BSP Example Crate

> **Status:** Ratified 2026-05-14 (owner: Ira Abbott). See §15.
> Closes the §14 "Unblocks" entry in
> [`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md) that names the
> example-crate chapter as the natural follow-on to CHIPS-TI-01e + -05.
> Future example-crate behaviour changes route through this chapter's
> §15 amendment process; no behaviour PR rides on an unamended
> invariant.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared
in [`CLAUDE.md`](../../../CLAUDE.md) §"Spec-Before-Code Planning
Discipline". RFC 2119 / RFC 8174 normative keywords (**MUST**,
**MUST NOT**, **SHALL**, **SHOULD**, **MAY**) carry their RFC meanings
when capitalised; lowercase use is narrative.

| Domain                                                | Authoritative source                                                                                                                                  |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| CC13x2 / CC26x2 chip register / peripheral semantics  | TI SWCU185G (CC13x2/CC26x2 SimpleLink Wireless MCU TRM), all sections cited by §-number as needed                                                     |
| CC1352R1F3RGZ orderable-part datasheet                | TI SWRS196I (January 2018 / revised February 2021), §9.4 "Memory Organisation"                                                                        |
| LAUNCHXL-CC1352R1 board pinout / LEDs / buttons       | TI SWRU527 "CC1352R1 LaunchPad Development Kit Hardware User's Guide", §2.3 (XDS110 / backchannel UART), §2.4 (user LEDs), §2.5 (buttons), §2.6 (I2C) |
| Cortex-M4F architectural semantics                    | ARM ARMv7-M Architecture Reference Manual                                                                                                             |
| `cc13x2_26x2_pac 0.10.3`                              | crates.io `cc13x2_26x2_pac` 0.10.3 (BSD-3-Clause; SVD source `cc13x2_26x2.svd` from `seanmlyons22/ti-lprf-pacs`)                                      |
| `cortex-m-rt 0.7` linker / boot conventions           | crates.io `cortex-m-rt` 0.7 — `link.x` template + `#[entry]` macro contract                                                                           |
| Slate-9 BSP emission contract                         | [`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md) §5.5 cortex-m-rt linker-arg sequence; [`CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md) §5.3 Rust-file emission set |
| ESP precedent for `bsp_pac` feature shape             | `examples/beetle-esp32c3/` — `Cargo.toml` feature matrix, `src/bsp_pac_main.rs` entry, `src/bsp_generated/mod.rs` re-export shape                     |

If a phase needs to **modify** a cited authority (different PAC vintage;
amendment to the cortex-m-rt boot contract; new LaunchPad SKU whose
pinout differs from CHIPS-TI-00 §5.5 SimpleLink Cortex-M4F precedent)
the modification ratifies in a §15 amendment **first**, in a separate
PR, before any behaviour PR rides on it.

## §1 Purpose

CHIPS-TI-06 ratifies a **minimal example crate** that consumes the
slate-9 generator output for `launchxl_cc1352r1` and proves the
chipdb-generator pipeline end-to-end:

```text
chipdb YAML  →  rlvgl-creator (slate 1-9)  →  generated BSP  →
                                              cargo check / link  →
                                              bootable binary
```

The compile-verify gate ratified by CHIPS-TI-01d
([`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs))
already type-checks the *emitted* BSP against `cc13x2_26x2_pac 0.10.3`
inside a throwaway cargo project. CHIPS-TI-06 is the **complementary**
gate: the same BSP is also consumed by a real, committed,
linkable example crate whose source code lives under
`examples/launchxl-cc1352r1/`. The two gates exercise different
failure modes (see §10).

This is the same role that `examples/beetle-esp32c3/`'s `bsp_pac`
feature path plays for the Espressif tree (see §0 authority entry for
ESP precedent). Cross-vendor consistency in the example-crate shape is
a deliberate goal: future chipdb vendors (Microchip, Silicon Labs,
etc.) inherit the same `bsp_pac` feature pattern.

## §2 Problem statement

Slates 1-9 of the CHIPS-TI initiative grew a complete generator
pipeline:

- Chip + board YAML schemas (CHIPS-TI-00 §5).
- Render-time generator that emits 8 files per board (6 .rs +
  `memory.x` + `<chip_stem>.x`).
- Snapshot regression test
  ([`tests/bsp_ti_cc1352r_render.rs`](../../../tests/bsp_ti_cc1352r_render.rs)).
- Compile-verify regression test
  ([`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs))
  that confirms the emitted BSP type-checks against the real PAC on
  `thumbv7em-none-eabihf`.

The compile-verify gate proves the BSP is **internally** correct —
the generated files type-check and link as a standalone library.
It does **not** prove the BSP is correct **in use**: a consumer crate
that imports the BSP as a child module, calls its `init` entry point,
and links the linker fragments via its own `build.rs` may still fail
because of integration-side issues (linker-arg ordering,
`build.rs` mis-copying, module-path re-export mistakes, missing
`#[entry]` boilerplate).

CHIPS-TI-06 closes that gap. The example crate is the first consumer
that links the slate-9 BSP into a real binary on `thumbv7em-none-eabihf`.
A regression in any link-time or boot-time integration surface
surfaces here as a `cargo check --target thumbv7em-none-eabihf`
failure on the example crate.

## §3 Canonical glossary

Reserved CHIPS-TI-06 vocabulary. Cite-vs-restate markers follow the
convention in CHIPS-TI-00 §3.

- **`rlvgl-example-launchxl-cc1352r1`** — *Owned by this chapter; does
  not exist in repo before slate -06.* The crate name in
  `examples/launchxl-cc1352r1/Cargo.toml`. Crate is `publish = false`
  and is **not** added to the workspace `[workspace] members` array in
  the v0 slate (the PM adds it at integration time to avoid
  cherry-pick conflicts with sibling vendor workers — see §10.2).

- **`bsp_pac` feature** — *As defined in `examples/beetle-esp32c3/Cargo.toml`
  `[features]` table; adapted: applied to the CC1352R / cortex-m-rt
  boot model rather than ESP32-C3 / esp-riscv-rt.* The feature flag
  that selects the generated-BSP + raw-PAC bring-up path. Default
  feature for this crate (the only path; no `esp_hal`-style sibling
  exists for TI's SimpleLink family because TI driverlib is not
  exposed as a `*-hal`-shaped crate on crates.io).

- **`src/bsp_generated/`** — *Owned by this chapter; mirrors the
  `examples/beetle-esp32c3/src/bsp_generated/` shape.* The committed
  generator output directory. Contains a hand-written `mod.rs` at
  the directory root that re-exports the per-board generator output
  as a child module (`pub mod launchxl_cc1352_r1;`). Regenerate via:

  ```bash
  cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
      --vendor ti --board launchxl_cc1352r1 \
      --out examples/launchxl-cc1352r1/src/bsp_generated --emit-pac
  ```

- **`launchxl_cc1352_r1` module stem** — *Computed by the generator's
  snake_case helper from the board YAML `name:` field
  (`LAUNCHXL-CC1352R1` → `launchxl_cc1352_r1`).* The per-board
  subdirectory name under `src/bsp_generated/` and the corresponding
  Rust module name. The stem carries an underscore before the trailing
  `1` because the snake_case helper splits letter-to-digit boundaries.
  This matches the existing render test path:
  [`tests/bsp_ti_cc1352r_render.rs:42`](../../../tests/bsp_ti_cc1352r_render.rs)
  (`tmp.path().join("launchxl_cc1352_r1")`).

- **`board::init()`** — *As defined in `pac.rs` of the generator output
  (`src/bin/creator/bsp/ti/templates/pac.rs.jinja`); adapted: the v0
  example crate calls it via the top-level re-export
  `bsp_generated::launchxl_cc1352_r1::init()` rather than the
  `board::` module path.* The generator's `mod.rs` carries
  `pub use pac::init;`, so the top-level path is the public surface;
  `board.rs` is the constants module (`BOARD_NAME`, `CPU_HZ`, pin
  numbers — see this section's LED entries below). This chapter uses
  "`board::init()`" as the conceptual term for the entry point even
  though the concrete invocation site is `pac::init` (re-exported at
  the module root). Future -06a (LED blink) and -06b (UART
  hello-world) chapters MAY introduce additional `board::*` helpers
  beyond constants.

- **LED pin assignments** — *As defined in
  [`chipdb/rlvgl-chips-ti/db/boards/launchxl_cc1352r1.yaml`](../db/boards/launchxl_cc1352r1.yaml)
  pins array + features map; used without modification.* Both LEDs
  are routed through the SimpleLink IO Controller (IOC) on DIO pins:

  | Constant         | DIO   | Net name  | Board YAML row | TRM authority           |
  | ---------------- | ----- | --------- | -------------- | ----------------------- |
  | `board::LED_RED`   | DIO_6 | LED_RED   | label `led_red`   | SWRU527 §2.4 "User LEDs" |
  | `board::LED_GREEN` | DIO_7 | LED_GREEN | label `led_green` | SWRU527 §2.4 "User LEDs" |

  Both are high-drive DIOs (8 mA capable per SWRU527 §2.4) and are
  active-high (asserted by driving the DIO output to logic 1). LED
  *toggling* code (`out_w1ts` / `out_w1tc` semantics) is deferred to
  -06a; v0 only exposes the pin numbers via `board::LED_RED` /
  `board::LED_GREEN`.

- **`build.rs` linker-arg sequence** — *As defined in
  [`CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md) §5.5; used without
  modification.* The example crate's `build.rs` MUST emit
  `-Tmemory.x`, `-T<chip_stem>.x` (`-Tcc1352_r.x`), and rely on the
  cortex-m-rt-shipped `link.x` (referenced from `.cargo/config.toml`
  `rustflags = [ "-C", "link-arg=-Tlink.x" ]`). The fragments are
  copied from `src/bsp_generated/launchxl_cc1352_r1/` into `OUT_DIR`
  and the `cargo:rustc-link-search` directive points the linker at
  the copy location.

- **`thumbv7em-none-eabihf`** — *As defined in the Rust target spec
  catalog; used without modification.* The Rust target triple for
  Cortex-M4F with hardware single-precision floating point. CC1352R
  is Cortex-M4F per SWCU185G §3 "Cortex-M4F"; the target is set in
  the example crate's `.cargo/config.toml` `[build] target` key.

## §4 Source-of-truth map

| Concept                                          | Owner (canonical)                                                                          | Mirrored / consumed by                                                                                              |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| Example crate name + version                     | `examples/launchxl-cc1352r1/Cargo.toml` `[package]` block                                  | Workspace `[workspace] members` (PM-added at integration time; see §10.2)                                          |
| Binary name + entry path                         | `examples/launchxl-cc1352r1/Cargo.toml` `[[bin]]` block                                    | `examples/launchxl-cc1352r1/src/bsp_pac_main.rs` (the `#[entry] fn main()`)                                         |
| Feature flag matrix                              | `examples/launchxl-cc1352r1/Cargo.toml` `[features]` block                                 | `examples/launchxl-cc1352r1/src/bsp_pac_main.rs` `#[cfg(feature = "bsp_pac")]` guard                                |
| Direct dependencies (`cc13x2_26x2_pac`, `cortex-m-rt`, `cortex-m`, `panic-halt`) | `examples/launchxl-cc1352r1/Cargo.toml` `[dependencies]` block                                                                | n/a — leaf consumer                                                                                                  |
| `src/bsp_generated/launchxl_cc1352_r1/*.rs` (6 files) | `rlvgl-creator bsp from-yaml --vendor ti` (per CHIPS-TI-00 §5.3 contract; regeneratable but committed) | `examples/launchxl-cc1352r1/src/bsp_generated/mod.rs` (hand-written re-export wrapper)                              |
| `src/bsp_generated/launchxl_cc1352_r1/memory.x` + `cc1352_r.x` (2 linker fragments) | `rlvgl-creator bsp from-yaml --vendor ti` (per CHIPS-TI-05 §6 contract)                                                       | `examples/launchxl-cc1352r1/build.rs` (copies into OUT_DIR, emits `-T` link args)                                  |
| `src/bsp_generated/mod.rs` (hand-written index)  | `examples/launchxl-cc1352r1/src/bsp_generated/mod.rs` (this crate)                         | n/a — only consumed by the entry point in this crate                                                                |
| Target triple (`thumbv7em-none-eabihf`)          | `examples/launchxl-cc1352r1/.cargo/config.toml` `[build] target`                           | `examples/launchxl-cc1352r1/build.rs` (the riscv32-style target-arch guard pattern is not needed; the crate is single-target) |
| `link.x` (cortex-m-rt template)                  | Upstream `cortex-m-rt 0.7` `link.x`                                                        | `.cargo/config.toml` `rustflags = ["-C", "link-arg=-Tlink.x"]`                                                      |
| LED pin numbers (`LED_RED=DIO_6`, `LED_GREEN=DIO_7`) | `chipdb/rlvgl-chips-ti/db/boards/launchxl_cc1352r1.yaml` pins array (SWRU527 §2.4)         | Generated `board.rs` `pub const LED_RED: u8 = 6; pub const LED_GREEN: u8 = 7;` accessible via `bsp_generated::launchxl_cc1352_r1::board::*` |

There is exactly one canonical owner per concept. The generator output
under `src/bsp_generated/launchxl_cc1352_r1/` is committed for
deterministic builds but the canonical source remains the chipdb YAML
+ generator templates; a `cargo run --features creator ...` produces
the same files modulo a tracked `(c) generator-version` header line.

## §5 Frozen decisions — example crate shape (v0)

Each decision below names its registration policy per the
*Frozen enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Crate name + binary name — Standards Action

| Slot          | Value                                |
| ------------- | ------------------------------------ |
| Crate name    | `rlvgl-example-launchxl-cc1352r1`    |
| Binary name   | `rlvgl-launchxl-cc1352r1`            |
| Entry path    | `src/bsp_pac_main.rs`                |
| Default features | `["bsp_pac"]`                     |

Renaming either the crate or the binary, or splitting the binary into
multiple `[[bin]]` entries, is Standards Action and requires a §15
amendment here. The crate / binary names mirror the
`rlvgl-example-beetle-esp32c3` / `rlvgl-beetle-esp32c3-bsp-pac`
precedent — the leading `rlvgl-` prefix matches the workspace
convention; the `-bsp-pac` suffix on the ESP binary is omitted here
because the TI example has only one bring-up path (no sibling
`esp_hal`-style binary).

### 5.2 Feature flag matrix — Specification Required

| Feature   | Required-for-binary  | Pulls in                                                          |
| --------- | -------------------- | ----------------------------------------------------------------- |
| `bsp_pac` | yes (only feature)   | (no `dep:` activations — direct deps are non-optional in this v0) |

Adding a sibling feature path (e.g. a hypothetical `ti_hal` analogous
to `esp_hal`) is Specification Required — the per-chapter walkthrough
that introduces the new path updates this table and adds the
corresponding `[[bin]]` entry. Removing the `bsp_pac` feature would
defeat the purpose of the crate and is Standards Action.

### 5.3 Dependency pins — Specification Required

| Dependency        | Version          | Justification                                                                |
| ----------------- | ---------------- | ---------------------------------------------------------------------------- |
| `cc13x2_26x2_pac` | `0.10.3` (`features = ["rt"]`) | Matches the compile-verify gate (CHIPS-TI-01d / -01e). |
| `cortex-m-rt`     | `0.7`            | Standard cortex-m-rt for ARMv7E-M targets.                                   |
| `cortex-m`        | `0.7`            | `cortex-m::asm::wfi` for the v0 idle loop.                                  |
| `panic-halt`      | `0.2`            | Smallest panic handler; no console available in v0.                          |

Bumping any pin to a new major version is Specification Required and
ratifies in §15 alongside a re-bless of the compile-verify
snapshot if needed. Replacing `panic-halt` with a console-aware
panic handler is deferred to -06b (UART hello-world unblock).

### 5.4 `bsp_generated/` directory shape — Standards Action

```text
examples/launchxl-cc1352r1/src/bsp_generated/
├── mod.rs                      # hand-written index; declares `pub mod launchxl_cc1352_r1;`
└── launchxl_cc1352_r1/
    ├── mod.rs                  # generator output (re-exports pac/clocks/io_mux/peripherals/board)
    ├── pac.rs                  # generator output
    ├── clocks.rs               # generator output
    ├── io_mux.rs               # generator output
    ├── peripherals.rs          # generator output
    ├── board.rs                # generator output (constants)
    ├── memory.x                # generator output (linker fragment, CHIPS-TI-05 §5.2)
    └── cc1352_r.x              # generator output (linker fragment, CHIPS-TI-05 §5.3)
```

Renaming `bsp_generated`, moving the per-board subdirectory up to
`bsp_generated/` (collapsing one level), or splitting into separate
`bsp_generated_rust/` + `bsp_generated_linker/` trees is Standards
Action. The two-level shape matches `examples/beetle-esp32c3/`
where the per-board subdirectory is `dfr0868_beetle_esp32_c3/` (the
ESP precedent collapsed the level back to flat by copy; CHIPS-TI-06
keeps the level visible to mirror the generator's natural output and
to simplify regeneration).

### 5.5 v0 entry point shape — Standards Action

The v0 `src/bsp_pac_main.rs` is:

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
        bsp_generated::launchxl_cc1352_r1::init();
    }
    loop {
        cortex_m::asm::wfi();
    }
}
```

The entry MUST call `bsp_generated::launchxl_cc1352_r1::init()` as
the first action in `main`. Any drift from this shape (additional
calls before `init`; reorder; substitute a different module path) is
Standards Action because the v0 acceptance criterion in §12 is
"`init()` callable as the first action in `main`".

## §6 Verification gates

The verification surface inherits two gates from earlier slates and
introduces one new gate:

1. **Snapshot render test (CHIPS-TI-01c)** —
   [`tests/bsp_ti_cc1352r_render.rs`](../../../tests/bsp_ti_cc1352r_render.rs).
   Already shipping; not introduced by this chapter. A diff in
   `src/bsp_generated/launchxl_cc1352_r1/` is **either** the result of
   a planned re-bless (template edit) or a regeneration of a stale
   checkout. The render test catches the first; the manual
   regeneration step (§7) catches the second.

2. **Compile-verify regression (CHIPS-TI-01d)** —
   [`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs).
   Already shipping; not introduced by this chapter. Validates the
   generator output in a **throwaway** cargo project (no
   `examples/launchxl-cc1352r1/` involvement). Failure here means the
   generator output is internally broken.

3. **Example-crate compile gate (new in this chapter)** —
   `cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml --target thumbv7em-none-eabihf`.
   Validates the generator output **as integrated** into a real
   consumer (linker scripts wired via `build.rs`, module index
   re-export, `#[entry]` boilerplate, `panic-halt` linkage). Failure
   here without a corresponding failure in gate 2 indicates an
   integration-side regression (linker-arg ordering, mismatched
   module path, etc.). v0 acceptance per §12 is gate 3 passing.

Promoting gate 3 to a stricter form (e.g. `cargo build` rather than
`cargo check`, or running the binary on a real CC1352R LaunchPad via
probe-rs) is Specification Required — write the gate, update this
section, re-bless.

## §7 Regeneration procedure

To regenerate `src/bsp_generated/launchxl_cc1352_r1/*` after a
chipdb YAML or generator template edit, from the workspace root:

```bash
cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
    --vendor ti \
    --board launchxl_cc1352r1 \
    --out examples/launchxl-cc1352r1/src/bsp_generated \
    --emit-pac
```

The generator produces the per-board subdirectory under `--out`
automatically (`examples/launchxl-cc1352r1/src/bsp_generated/launchxl_cc1352_r1/`).
The hand-written `src/bsp_generated/mod.rs` (the wrapper that declares
`pub mod launchxl_cc1352_r1;`) MUST NOT be overwritten by the
regeneration; the generator's own top-level `mod.rs` (intended as a
crate root) lands inside the per-board subdirectory and does not
collide with the wrapper.

A regeneration PR MUST re-run the example-crate compile gate (§6 gate
3) and the snapshot render test (§6 gate 1). The compile-verify gate
(§6 gate 2) is unaffected by example-crate-internal edits but MUST be
re-run when the regenerated files change.

## §8 Round-trip / regression posture

This chapter inherits CHIPS-TI-00 §9 regression contract. Specifically:

- A PR modifying any generator template MUST re-bless the snapshot
  render test, re-run the compile-verify gate, AND re-run the
  example-crate compile gate. Drift in any of the three is a
  regression.
- A PR modifying chip / board YAML MUST do the same.
- A PR modifying **only** `examples/launchxl-cc1352r1/` files OUTSIDE
  `src/bsp_generated/` (e.g. `Cargo.toml`, `bsp_pac_main.rs`,
  `build.rs`, `.cargo/config.toml`) MUST re-run the example-crate
  compile gate but does not need the snapshot/compile-verify gates.
- A PR modifying `src/bsp_generated/` by hand (not via regeneration)
  is Specification Required — file the §15 entry naming the
  out-of-band edit and the reason regeneration was insufficient.

## §9 Non-goals — v0

Explicit out-of-scope for this chapter's v0 scaffold:

- **No LED blink demo.** v0 proves `init()` returns and the binary
  links; toggling `LED_RED` / `LED_GREEN` over time is deferred to
  **CHIPS-TI-06a**. The board YAML already names the LEDs so -06a is
  a `out_w1ts` / `out_w1tc` toggle inside a timed loop.
- **No console UART output.** v0 uses `panic-halt` (silent panic on
  failure). Wiring UART0 hello-world output over the XDS110
  backchannel is deferred to **CHIPS-TI-06b**. The board YAML already
  names DIO_2 / DIO_3 for UART0 RX / TX and the generated
  `peripherals.rs` carries `init_uart0_console()` per CHIPS-TI-00 §7
  template emission contract.
- **No rlvgl integration.** v0 does not depend on `rlvgl-core` /
  `rlvgl-platform` / `rlvgl-widgets`. Wiring an rlvgl widget tree
  over a CC1352R display target is deferred to **CHIPS-TI-06c**. The
  LAUNCHXL has no on-board display; -06c will require a BoosterPack
  display board (e.g. Sharp 96×96 LCD) and a touch / button input
  binding.
- **No `objcopy` to `.bin` / `.hex`.** v0 stops at `cargo check`.
  Producing flashable artifacts via `cargo-objcopy` or `cargo-binutils`
  is deferred; the existing `examples/beetle-esp32c3/` precedent
  does not currently `objcopy` either.
- **No probe-rs flash recipe.** Running the binary on a real
  LAUNCHXL-CC1352R1 over XDS110 / probe-rs is deferred to -06a (paired
  with the LED blink so the bring-up has a visual confirmation
  signal).
- **No workspace `[workspace] members` entry.** Adding the crate to
  the workspace is an integration-time action by the PM, not part of
  this chapter's scope. See §10.2.

## §10 Reconciliation with adjacent repo primitives

### 10.1 Example-crate gate vs. compile-verify gate

Both gates type-check the BSP against `cc13x2_26x2_pac 0.10.3` on
`thumbv7em-none-eabihf`, but they exercise different aspects:

| Aspect                               | Compile-verify gate (CHIPS-TI-01d)                                                                  | Example-crate gate (this chapter)                                                            |
| ------------------------------------ | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Project shape                        | Throwaway cargo project materialised in `$TMPDIR` by the test                                       | Committed crate at `examples/launchxl-cc1352r1/`                                            |
| BSP source                           | Generated fresh into `$TMPDIR` on every test run                                                    | Generator output committed under `src/bsp_generated/launchxl_cc1352_r1/`                    |
| `build.rs` shape                     | Test-internal `build.rs` template                                                                    | Hand-written `build.rs` mirroring `examples/beetle-esp32c3/build.rs`                        |
| Entry-point shape                    | Test-internal `lib.rs` (no `#[entry]`; library-only compile)                                        | Real `#[entry] fn main() -> !` in `src/bsp_pac_main.rs`                                     |
| Caught regression class              | Generator output is internally broken                                                                | Generator output is correct but integration-side wiring is broken                            |
| Run frequency                        | Opt-in (CHIPS-TI-01d `--features compile-verify`); pre-publish bullet uncommented at integration  | Standard `cargo check` (no opt-in feature gate); runs whenever the workspace is checked     |

Both gates are necessary. Removing either is Specification Required.

### 10.2 Workspace `[workspace] members` ownership

The example crate is **not** added to the workspace `[workspace] members`
in this chapter's slate. Rationale: this chapter ships in parallel
with sibling CHIPS-SILABS-06 and CHIPS-MICROCHIP-06 worker tasks that
similarly scaffold their respective example crates. Adding to the
workspace from three concurrent worker branches would produce a
guaranteed 3-way cherry-pick conflict on `Cargo.toml` `[workspace]
members`. The PM serialises the integration: at integration time
each example crate is added to `[workspace] members` as a single
atomic edit. Until then, the example crate compiles standalone via:

```bash
cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml --target thumbv7em-none-eabihf
```

This is the v0 acceptance command per §12.

### 10.3 ESP precedent inheritance

The example-crate shape closely mirrors `examples/beetle-esp32c3/`
with three deltas:

| Delta                              | ESP precedent                                                  | This chapter                                                              |
| ---------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Architecture                       | RISC-V (`riscv32imc-unknown-none-elf`)                         | ARM Cortex-M4F (`thumbv7em-none-eabihf`)                                  |
| Boot crate                         | `esp-riscv-rt 0.13` + `riscv-rt 0.16` (`memory` feature)       | `cortex-m-rt 0.7`                                                         |
| Sibling feature path               | `esp_hal` (full rlvgl stack) is the default                    | No sibling; `bsp_pac` is the only path                                    |
| Per-board subdirectory in tree     | Generator output **copied** into `src/bsp_generated/` flat     | Generator output **lives** under `src/bsp_generated/launchxl_cc1352_r1/` |
| Linker-arg invocation              | `build.rs` `cargo:rustc-link-arg=-Tlink.x` + `-Tesp32_c3.x`    | `.cargo/config.toml` for `-Tlink.x`; `build.rs` for `-Tmemory.x` + `-T<chip>.x` |

The two-level `src/bsp_generated/<board>/` shape is preserved on the
TI side because regeneration is cleaner (the generator emits the
subdirectory automatically; flattening would require a post-process
copy step in `Cargo.toml` regeneration instructions). Future
chipdb-vendor example crates SHOULD follow the TI two-level shape
unless they have a vendor-specific reason to flatten (the ESP tree's
flatten was a slate-1 decision predating slate-9's generator
maturity).

### 10.4 cortex-m-rt linker contract

Per CHIPS-TI-05 §5.5 the consuming crate's `build.rs` MUST emit the
linker fragment includes in the order `memory.x` → `<chip>.x` → `link.x`.
The example crate splits this responsibility:

- `.cargo/config.toml` carries `rustflags = ["-C", "link-arg=-Tlink.x"]`
  so `link.x` is always passed (cortex-m-rt's existing convention).
- `build.rs` carries `println!("cargo:rustc-link-arg=-Tmemory.x");`
  and `println!("cargo:rustc-link-arg=-Tcc1352_r.x");` so the chip
  fragments are passed only when this crate is the build root.

Both args are emitted **before** `link.x` because of how cargo /
rustc accumulates `-C link-arg=` values: args from
`.cargo/config.toml` `rustflags` come AFTER args emitted by
`build.rs` `cargo:rustc-link-arg`. This produces the
`-Tmemory.x -Tcc1352_r.x -Tlink.x` order that CHIPS-TI-05 §5.5
requires.

### 10.5 No CCFG static in v0

CHIPS-TI-05 §5.3 frozen the CCFG **section placement** (`SECTIONS {
.ccfg ... }` directive in `cc1352_r.x`) but explicitly named "no
emission of CCFG content" as a non-goal (CHIPS-TI-05 §9 bullet 1).
The v0 example crate does **not** define a `#[link_section = ".ccfg"]`
static. This is acceptable for `cargo check` (which does not link)
but means a `cargo build` would currently fail at link time with
"undefined reference to CCFG content" (or, more likely, the linker
would emit a zero-filled CCFG section which the boot ROM would reject
on a real chip). Defining the CCFG static is a -06a / -06b
prerequisite and tracked in §14.

## §11 Non-goals — broader scope

Beyond the v0 non-goals enumerated in §9, the following are
explicitly out of scope for the CHIPS-TI-06 series (any letter
suffix):

- **No BLE5-Stack / TI 15.4-Stack / Thread / Zigbee protocol-stack
  integration.** Per CHIPS-TI-05 §9 the generator emits a flat FLASH
  region; multi-image / protocol-stack layouts require additional
  linker scaffolding that is out of scope for chipdb-driven
  generation.
- **No FreeRTOS / TI-RTOS / NoRTOS-SDK port.** The example crate is
  bare-metal cortex-m-rt only. Inheriting TI-NoRTOS bring-up shape
  would couple to TI driverlib's `Power_init` / `Display_init`
  surface and is deferred.
- **No multi-LaunchPad coverage.** This chapter targets
  LAUNCHXL-CC1352R1 only. Adding LAUNCHXL-CC2652R1 / LAUNCHXL-CC1312R1
  / LAUNCHXL-CC26X2R1 sibling boards is each a separate `CHIPS-TI-06z`
  letter slate.
- **No XDS110 probe-rs auto-detect.** The example crate does not
  ship a `probe-rs.toml` or `.vscode/launch.json`; manual flashing
  (`cargo flash --chip CC1352R1F3RGZ`, or TI UniFlash) is deferred
  to -06a.

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) and the v0 slate complete
when:

- [x] §0 authority table reviewed; SWCU185G + SWRS196I + SWRU527 cited
      as canonical authorities for chip semantics, datasheet, and
      board pinout respectively.
- [x] §3 glossary terms each carry a cite-vs-restate marker per
      CLAUDE.md §"Definitions — reference vs. restatement"; LED pin
      assignments (`LED_RED=DIO_6`, `LED_GREEN=DIO_7`) cite both the
      board YAML and SWRU527 §2.4.
- [x] §4 source-of-truth map has exactly one owner per row.
- [x] §5.1 frozen crate / binary names; §5.2 frozen feature matrix;
      §5.3 frozen dep pins; §5.4 frozen `bsp_generated/` shape;
      §5.5 frozen v0 entry point.
- [x] §6 verification gates name three gates: snapshot render
      (existing), compile-verify (existing), example-crate compile
      (new in this chapter).
- [x] §10 reconciliation: §10.1 example-crate gate vs. compile-verify
      gate, §10.2 workspace `[workspace] members` deferral, §10.3 ESP
      precedent inheritance, §10.4 cortex-m-rt linker contract,
      §10.5 CCFG static deferral.
- [x] v0 scaffold landed at `examples/launchxl-cc1352r1/` with
      `Cargo.toml`, `build.rs`, `.cargo/config.toml`,
      `src/bsp_pac_main.rs`, `src/bsp_generated/mod.rs`, and the
      committed generator output under
      `src/bsp_generated/launchxl_cc1352_r1/`.
- [x] `cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml --target thumbv7em-none-eabihf` passes.
- [x] §15 dated ratification entry.

## §13 Files cited

- [`CLAUDE.md`](../../../CLAUDE.md) — Spec-Before-Code Planning
  Discipline, RFC 2119 keywords, registration policy, initiative
  prefix `CHIPS-TI-NN[a-z]:`.
- [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-00-CONCEPTS.md`](CHIPS-TI-00-CONCEPTS.md)
  — parent concepts; §5.3 Rust-file emission set; §7 template
  emission contract; §15 slate change log.
- [`chipdb/rlvgl-chips-ti/docs/CHIPS-TI-05-LINKER.md`](CHIPS-TI-05-LINKER.md)
  — slate-9 linker emission chapter; §5.5 cortex-m-rt linker-arg
  sequence; §6 file-path emission; §9 CCFG-content non-goal.
- [`chipdb/rlvgl-chips-ti/db/chips/CC1352R.yaml`](../db/chips/CC1352R.yaml)
  — chip inventory; `memory:` + `linker:` blocks consumed by the
  generator.
- [`chipdb/rlvgl-chips-ti/db/boards/launchxl_cc1352r1.yaml`](../db/boards/launchxl_cc1352r1.yaml)
  — board pinout; authoritative source for LED / button / UART /
  I2C pin assignments per SWRU527.
- [`tests/bsp_ti_cc1352r_render.rs`](../../../tests/bsp_ti_cc1352r_render.rs)
  — snapshot render test; CHIPS-TI-01c.
- [`tests/bsp_ti_cc1352r_compile.rs`](../../../tests/bsp_ti_cc1352r_compile.rs)
  — compile-verify regression test; CHIPS-TI-01d.
- `examples/beetle-esp32c3/Cargo.toml` — ESP feature-matrix precedent.
- `examples/beetle-esp32c3/src/bsp_pac_main.rs` — ESP `bsp_pac` entry-point precedent.
- `examples/beetle-esp32c3/src/bsp_generated/mod.rs` — ESP re-export-wrapper precedent.
- `examples/beetle-esp32c3/build.rs` — ESP linker-arg-emission precedent.
- TI SWCU185G — CC13x2/CC26x2 SimpleLink Wireless MCU TRM; authority
  for CC1352R register / peripheral semantics.
- TI SWRS196I — CC1352R datasheet; §9.4 Memory Organisation.
- TI SWRU527 — LAUNCHXL-CC1352R1 Hardware User's Guide; §2.4
  (LEDs, authority for §3 LED-pin entries).
- crates.io `cc13x2_26x2_pac` 0.10.3 — PAC pinned by §5.3.
- crates.io `cortex-m-rt` 0.7 — boot crate; `link.x` template
  consumed by §10.4.
- crates.io `cortex-m` 0.7 — `asm::wfi` for v0 idle loop.
- crates.io `panic-halt` 0.2 — silent panic handler for v0.

## §14 Unblocks

Ratifying this chapter unblocks:

- **CHIPS-TI-06a** — LED blink demo. Toggles `LED_RED` / `LED_GREEN`
  on a timed loop using `cc13x2_26x2_pac` `GPIO` `doutset31_0` /
  `doutclr31_0` registers. Requires a CCFG static (§10.5 deferral)
  so the binary actually boots on hardware. Adds a probe-rs flash
  recipe and visual confirmation step.
- **CHIPS-TI-06b** — UART hello-world. Routes `println!` over UART0 /
  XDS110 backchannel at 115200 Bd per the board YAML `console:`
  block. Replaces `panic-halt` with a UART-aware panic handler.
- **CHIPS-TI-06c** — rlvgl widget-tree integration. Requires a
  BoosterPack display (e.g. Sharp 96×96 LCD) and a touch / button
  input binding. Out of scope until a board with a display is
  added to the chipdb.
- **Sibling LaunchPad boards.** LAUNCHXL-CC2652R1 / LAUNCHXL-CC1312R1
  / LAUNCHXL-CC26X2R1 each inherit the §5 frozen shape; their per-
  board YAML produces a separate example crate with the same
  scaffolding pattern. Each new board is a letter slate
  (`CHIPS-TI-06d`, `CHIPS-TI-06e`, ...) with a §15 amendment here.

## §15 Change log

| Date       | Status                       | Note                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-05-14 | Ratified (owner: Ira Abbott) | Example crate chapter ratified for the v0 scaffold at `examples/launchxl-cc1352r1/`. Closes the [CHIPS-TI-05 §14](CHIPS-TI-05-LINKER.md#14-unblocks) "example crate" unblock. `cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml --target thumbv7em-none-eabihf` passes; `bsp_generated::launchxl_cc1352_r1::init()` callable as the first action in `main`. LED blink (-06a), UART hello-world (-06b), and rlvgl integration (-06c) deferred per §9. Workspace `[workspace] members` entry deferred to PM integration per §10.2. |
