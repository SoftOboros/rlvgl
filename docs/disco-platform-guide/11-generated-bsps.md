<!--
11-generated-bsps.md - Volume II Chapter 11: the auto-generated BSP path epilogue.
-->

**[← Prev](10-star-crawl-part-2.md) · [Index](README.md) · Next →**

# Chapter 11 — Generated BSPs (Epilogue)

## Volume I reference

Vol I
[Chapter 2](../disco-tutorial/02-splash-and-assets.md)
introduced `rlvgl-creator` as the asset-conversion CLI/UI.
That same binary has a completely separate subcommand —
`bsp from-ioc` — that consumes a CubeMX `.ioc` and spits
out the kind of bring-up code Volume II walked through by
hand. This chapter closes the loop.

## What this chapter covers

1. What `rlvgl-creator platform import` + `platform gen`
   produce when pointed at a CubeMX `.ioc`.
2. Where the output lives (`chips/stm/bsps/`).
3. What the generator **automates well** — and what it
   cannot, which is exactly the
   [Chapter 1 gap gallery](01-why-bare-metal.md#the-gap-gallery).
4. When to use the generator vs. hand-writing.

## The HAL / PAC gap

The generator targets the **PAC + TRM** posture this guide
has been defending. It does not generate `stm32h7xx-hal`
calls — it generates direct register writes against the
svd2rust PAC, templated from YAML IR. In that sense the
generator is Volume II, automated.

What the generator still cannot do is detect and patch the
PAC itself. If the PAC has the FMC SDTR offset wrong
(Chapter 3), the generator will emit wrong writes unless
the template author puts in a raw-address workaround. The
gap list from Chapter 1 is a **template backlog**, not a
silicon property.

## Walkthrough

### 1. Two stages — import and generate

Per
[`docs/CREATOR-CLI.md §bsp from-ioc`](../CREATOR-CLI.md#bsp-from-ioc)
and
[`docs/STM_BSP_GENERATION.md`](../STM_BSP_GENERATION.md):

```bash
rlvgl-creator platform import \
    --vendor st --input board.ioc --out board.yaml

rlvgl-creator platform gen \
    --spec board.yaml \
    --templates templates/stm32h7 \
    --out src/generated.rs
```

Stage 1 (`import`) reads the CubeMX XML, mines it for clocks,
pins, DMA routing, and peripheral configuration, and emits a
**vendor-neutral YAML IR**. Stage 2 (`gen`) renders MiniJinja
templates against the IR.

The IR schema (documented in the repo README under "BSP
Generator") looks like:

```yaml
mcu: STM32H747XIHx
package: LQFP176
power:   { supply: smps, vos: scale1 }
clocks:
  sources:  { hse_hz: 25000000 }
  pll:      { pll1: { m: 5, n: 400, p: 2, q: 4, r: 2 } }
  kernels:  { usart1: pclk2 }
pinctrl:
  - group: usart1-default
    signals:
      - { pin: PA9,  func: USART1_TX, af: 7, pull: none, speed: veryhigh }
      - { pin: PA10, func: USART1_RX, af: 7, pull: up,   speed: veryhigh }
peripherals:
  usart1:
    class: serial
    params: { baud: 115200, parity: none, stop_bits: 1 }
    pinctrl: [ usart1-default ]
reserved_pins: [ PA13, PA14 ]
```

This YAML is the same information Volume II's Chapter 2
(clocks), Chapter 4 (pin mux), and Chapter 8 (USART1) wrote
by hand, just reified.

### 2. Where the output lives

[`chips/stm/bsps/`](../../chips/stm/bsps/) is the Rust crate
that hosts generated STM32 BSP modules. Its structure:

- `src/` — generated modules, one per board.
- `csrc/` — optional C glue when CubeMX's `_it.c`/`_hal_msp.c`
  is needed.
- `build.rs` — orchestrates `rlvgl-creator` invocation at
  compile time when `RLVGL_CHIP_SRC` is set.
- `README.md` and `OPTIONS.md` — feature surface.

The disco firmware can consume generated BSP output through
the `c_hal` / `c_hal_cm4` features in
[`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml)
L36–37 — that's the toggle that switches the firmware from
"hand-written PAC path" (every Volume II chapter) to
"generator-produced bring-up."

### 3. What the generator does well

The generator is strong at anything that's a **table lookup**
from the IR:

- **Clock setup** — computing PLL dividers, setting AHB/APB
  prescalers, enabling peripheral clocks. Volume II Chapter 2
  boilerplate.
- **Pin mux** — every pin's MODER / OSPEEDR / AFRL / AFRH
  writes. Volume II Chapter 4's AF12 flood is exactly what
  this automates.
- **DMA routing** — matching peripheral DMA requests to
  DMAMUX / DMA streams. Not covered in Volume II but
  uniformly mechanical.
- **Alternate-function numbers** — from the embedded vendor
  AF database, no external JSON required.

### 4. What the generator cannot do

The Chapter 1 gap gallery is exactly the set of things a
template author still has to write:

| Gotcha (Ch 1 ref) | Automatable? | Why |
|-------------------|--------------|-----|
| PLL3ON not set ([§1](01-why-bare-metal.md#1-pll3_r_ck-silently-leaves-pll3-off)) | Yes, if template knows | Template must emit the raw RCC_CR poke explicitly. |
| FMC SDTR offset ([§2](01-why-bare-metal.md#2-fmc-sdbank1sdtr-offset-is-wrong-in-the-pac)) | Yes, if template uses raw addresses | The PAC has the bug; the template has to route around it. |
| I2C blocking for 120 Hz touch ([§3](01-why-bare-metal.md#3-embedded-hal-i2c-blocks-per-byte)) | No | This is a runtime design choice, not init. The generator outputs init only. |
| QSPI errata 2.8.5 ([§4](01-why-bare-metal.md#4-qspi-errata-285--wrong-default-kernel-clock)) | Yes, if template encodes errata | Templates can embed errata fixes. |
| D-cache transparency ([§5](01-why-bare-metal.md#5-d-cache-transparency--the-write-back-trap)) | No | Runtime concern; not in the IR. |

The pattern: **init-time** gotchas can be baked into
templates. **Runtime** gotchas (ISR orchestration, admission
control, cache maintenance) have to live in hand-written
code — that's Volume II Chapters 5, 7, and 10.

### 5. When to use which

A decision heuristic:

- New board, same chip family, same peripheral mix → **use
  the generator**. You get pin mux, clock tree, DMA routing
  for free.
- New chip with a PAC you don't trust yet → **hand-write
  first**, then fold what you learn back into templates.
- New feature on an existing board → **extend the templates**
  if the feature is mechanical, or write inline if it's an
  orchestration pattern.
- Reproducing the disco demo on a different H7 board →
  generator-derived clocks + pins, plus the hand-written
  LTDC/DSI/touch/DMA2D orchestration this guide documents.

## Register diagram

None — this chapter is about tooling, not silicon.

## Verify

- `rlvgl-creator --help` lists the `platform import` and
  `platform gen` subcommands.
- `cargo run --bin rlvgl-creator --features creator -- \
   platform import --vendor st --input path/to/board.ioc \
   --out /tmp/board.yaml` produces a YAML IR you can read.
- `cargo run --bin rlvgl-creator --features creator -- \
   platform gen --spec /tmp/board.yaml \
   --templates templates/stm32h7 --out /tmp/generated.rs`
  produces compilable Rust.
- Diff `/tmp/generated.rs` against the equivalent region of
  [`examples/stm32h747i-disco/src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) —
  the Chapter 2 clock tree and Chapter 4 pin mux should
  match in spirit.

## Going deeper

- [`docs/STM_BSP_GENERATION.md`](../STM_BSP_GENERATION.md)
  — end-to-end walkthrough of the import → gen pipeline.
- [`docs/CREATOR-CLI.md`](../CREATOR-CLI.md) — full
  `rlvgl-creator` reference, including every `bsp` flag.
- [`docs/CREATOR-TEMPLATES.md`](../CREATOR-TEMPLATES.md) —
  how to write your own template pack when the ones shipped
  don't cover your peripheral.
- [`chips/stm/bsps/README.md`](../../chips/stm/bsps/README.md)
  — the generated-BSP crate layout and consumption pattern.
- [`src/bin/creator/README.md`](../../src/bin/creator/README.md)
  — the binary that hosts the generator.

## End of Volume II

You now have:

- A working disco demo (Volume I).
- A platform crate you can read line by line (Chapters 1–8).
- A finished star-crawl implementation you understand in
  full (Chapters 9–10).
- A generator path to bootstrap the next board without
  re-writing the bring-up boilerplate (this chapter).

From here:

- Port the bring-up to another H7 board using the generator.
- Extend the crawl with new effects, layered over the same
  DMA2D pipeline.
- Enable `sd_storage` and fill in the Files wing Vol I
  [Chapter 5](../disco-tutorial/05-menu-stubs.md) left as a
  stub.
- Add register-level telemetry via the `cpu_stats` feature
  in
  [`src/cpu_stats.rs`](../../examples/stm32h747i-disco/src/cpu_stats.rs).

---

**[← Prev](10-star-crawl-part-2.md) · [Index](README.md) · Next →**
