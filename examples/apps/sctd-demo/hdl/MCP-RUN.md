<!--
MCP-RUN.md - iState MCP HDL generation run log for the SCTD HDL probe.
-->

# MCP iState HDL Run Log

## Document

- Slug: `rlvgl-v0-2-5-sctd-philosophers-led-top`
- Source: `philosophers_led_top.scxml`
- Target languages: `verilog`, `vhdl`

## MCP Jobs

| Job | Date | Result |
|---|---:|---|
| `a17141ac73944fe5b213ee35d2215e91` | 2026-07-05 | Failed: `'dict object' has no attribute 'all'` |
| `71813e2b6a0b42b8b6834536bd0464ad` | 2026-07-05 | Failed: `'dict object' has no attribute 'all'` |
| `4287c90acc5148c9ab50eb2023c0c4da` | 2026-07-05 | Failed: `'dict object' has no attribute 'all'` |

The current deployed MCP codegen worker is still failing before artifact
creation. No MCP-authoritative HDL bundle is available yet.

## Local Diagnostic Render

A local diagnostic render was produced at:

`_generated/istate-local-preview/`

This directory is not MCP-authoritative. It exists so the Quartus handoff can
start parser and synthesis work on the Windows machine while the MCP worker is
being fixed.

Local generator fixes used for the diagnostic render:

- Codegen Jinja autoescape disabled so HDL operators render as `<`, `>`, `>=`,
  and `&&`, not HTML entities.
- Verilog boolean guard templates use dict-safe `node.get(...)`.
- VHDL boolean guard templates use dict-safe `node.get(...)`.
- Verilog state/event dispatch groups duplicate `(source, event)` transitions
  into one ordered priority chain, preserving SCXML document order.
- VHDL guard signals render as boolean expressions instead of inline
  conditional `std_logic` expressions.

Local structural checks:

- XML parse: pass.
- Local render: 10 states, 37 transitions, 19 grouped transition cases,
  18 datamodel fields.
- Generated Verilog check: no `&lt;`, `&gt;`, or duplicate state/event `case`
  labels found.

Local tools not available on this machine:

- `iverilog`
- `verilator`
- `ghdl`
- `yosys`
- `quartus_sh`

The first real HDL parser/synthesis check is therefore expected to happen on
the Windows Quartus machine.

## Open Generator Issues

1. Deploy or reload the MCP worker with the dict-safe HDL guard template fixes.
2. Carry the autoescape fix into the MCP worker.
3. Carry the Verilog grouped-transition fix into the MCP worker.
4. Add generator-level physical port binding so the chart can emit
   `button_raw` and one-bit RGB LED ports directly instead of using the generic
   `event_valid` / `event_code` / `dm_*` scaffold.
5. Add integer or bit-width metadata for cycle counters; the current generated
   HDL uses 16.16 fixed-point datamodel registers.
