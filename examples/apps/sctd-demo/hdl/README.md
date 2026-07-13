<!--
README.md - HDL proof target for the SCTD Dining Philosophers LED projection.
-->

# SCTD Dining Philosophers HDL Probe

This directory contains an iState-over-MCP proof target for generating VHDL
and Verilog from a state chart, then trying the result on an FPGA toolchain.
The behavioral source is
`philosophers_led_top.scxml`.

The current Quartus handoff is under `quartus/`. MCP generation is still
blocked on the deployed worker; see `MCP-RUN.md` for the run log and
`_generated/istate-local-preview/` for the local diagnostic render.

## Intent

- Clock source: 12 MHz.
- Physical input: one raw button.
- Debounce: expressed in the upper state chart, not in hand-written HDL.
- Timer cadence: expressed in the upper state chart, not in hand-written HDL.
- Outputs: five philosopher RGB triples, fifteen logical LED signals total.
- Generation authority: Softoboros MCP iState tools:
  `istate_upload_xml`, `istate_codegen_create`, `istate_codegen_status`, and
  `istate_codegen_download` with `target_langs=["verilog","vhdl"]`.

## HDL Contract Under Test

The desired generated top-level HDL should expose:

| Signal | Direction | Width | Meaning |
|---|---:|---:|---|
| `clk` | in | 1 | 12 MHz clock |
| `rst` | in | 1 | active-high reset |
| `button_raw` | in | 1 | asynchronous or externally synchronized button input |
| `p1_r`..`p5_b` | out | 15 x 1 | RGB LED drive, one triple per philosopher |

The current generic iState HDL templates expose an event-coded interface
instead:

| Signal | Direction | Meaning |
|---|---:|---|
| `event_valid` | in | one-cycle event strobe |
| `event_code` | in | encoded SCXML event |
| `state_out` | out | encoded active root state |
| `dm_*` | out | 32-bit signed datamodel fields |

That mismatch is intentional for this probe: the target should reveal whether
the HDL backend can grow from an event-simulation scaffold into a synthesizable
chart-defined top with physical inputs and LED outputs.

## LED Phase Encoding

The chart maps each philosopher phase to one RGB triple:

| Phase | RGB | Meaning |
|---|---:|---|
| `empty` | `000` | no seated philosopher |
| `thinking` | `001` | blue |
| `hungry` | `100` | red |
| `waiting` | `110` | yellow |
| `eating` | `010` | green |

The proof chart projects the SCTD Dining Philosophers demo into a flat
HDL-friendly observable cycle. It does not replace the faithful or interactive
Rust SCTD machines; those remain under `../machines/`.

## Quartus Handoff

The Windows Quartus machine should start at `quartus/README.md`.

The handoff includes a small temporary top-level shim because the current
iState HDL backend does not yet emit physical input/output ports directly.
That shim only adapts the generic generated interface to:

- `clk_12mhz`
- `rst`
- `button_raw`
- `p1_r`..`p5_b`

All debounce, timer, and LED phase behavior remains in the SCXML chart and the
generated `SctdPhilosophersLedTop_fsm` runtime.
