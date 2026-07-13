<!--
README.md - Quartus handoff for the SCTD philosophers LED HDL proof.
-->

# Quartus Handoff

This directory prepares the SCTD philosophers LED proof for a Windows Quartus
machine.

## Inputs

The Quartus project uses:

- `../_generated/istate-local-preview/verilog/runtime.v`
- `sctd_philosophers_led_quartus_top.v`
- `sctd_philosophers_led.sdc`
- optional local pin script: `pins.local.tcl`

The generated Verilog is a local diagnostic render because the deployed MCP
iState worker currently fails before artifact creation. See `../MCP-RUN.md`.

## Windows Steps

1. Open a Quartus command prompt.
2. Copy `pins.local.tcl.template` to `pins.local.tcl`.
3. Replace every `PIN_*` placeholder in `pins.local.tcl` with board-specific
   pin names.
4. Run:

```bat
run_quartus.bat <QUARTUS_DEVICE>
```

Example only:

```bat
run_quartus.bat 10M50DAF484C7G
```

You can also call Quartus directly:

```bat
quartus_sh -t create_project.tcl -device <QUARTUS_DEVICE>
```

## Top-Level Ports

| Port | Direction | Meaning |
|---|---:|---|
| `clk_12mhz` | in | 12 MHz clock |
| `rst` | in | active-high reset |
| `button_raw` | in | raw button input |
| `p1_r`..`p5_b` | out | five RGB LED triples |

The shim has two Verilog parameters:

- `BUTTON_ACTIVE_LOW`, default `0`
- `LED_ACTIVE_LOW`, default `0`

If the board button or LEDs are active-low, set these parameters in Quartus or
edit the shim for the bench run.

## What The Shim Does

The current iState HDL runtime exposes:

- `event_valid`
- `event_code`
- `dm_*` 32-bit signed datamodel outputs

It does not yet emit physical button or LED ports. The shim therefore:

- synchronizes `button_raw`;
- converts button edges into `button_high` / `button_low` events;
- sends `tick_12mhz` on all other clock cycles;
- maps nonzero generated datamodel LED fields to one-bit RGB outputs.

The behavioral state machine remains generated from `../philosophers_led_top.scxml`.

## Expected First Quartus Findings

The first Quartus run should answer:

1. Does Quartus parse the generated Verilog runtime?
2. Does the temporary shim elaborate against the generated runtime ports?
3. Are the 12 MHz timer counters accepted as 32-bit signed fixed-point
   registers?
4. Does synthesis preserve the LED output cone?
5. What device resources and timing margin does the design report?

If parsing fails, keep the Quartus error output: it is the next HDL generator
test case.
