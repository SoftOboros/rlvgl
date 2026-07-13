# SctdPhilosophersLedTop - Verilog State Machine

Generated iState Verilog implementation.

## Files

- `runtime.v` - Synthesizable state machine module
- `harness_tb.v` - Simulation testbench
- `Makefile` - Build automation

## Requirements

Any Verilog simulator supporting SystemVerilog (IEEE 1800):
- [Icarus Verilog](http://iverilog.icarus.com/) (default, open-source)
- Synopsys VCS
- Xilinx Vivado Simulator (xsim)
- ModelSim/QuestaSim

## Build & Simulate

```bash
make build    # Compile Verilog files
make run      # Run simulation
make verify   # Run and compare against golden trace
make clean    # Remove build artifacts

# Use different simulator:
make VSIM=vcs run
make VSIM=xsim run
```

## Interface

### Module: `SctdPhilosophersLedTop_fsm`

**Ports:**
| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `clk` | input | 1 | Clock input |
| `rst` | input | 1 | Synchronous reset (active high) |
| `event_valid` | input | 1 | Event strobe (pulse high for 1 cycle) |
| `event_code` | input | 2 | Encoded event |
| `state_out` | output | 4 | Current state encoding |
| `transition_taken` | output | 1 | High when transition fires |
| `dm_timer_count` | output | 32 (signed) | Datamodel: timer_count (16.16 fixed-point) |
| `dm_phase_ms` | output | 32 (signed) | Datamodel: phase_ms (16.16 fixed-point) |
| `dm_debounce_ms` | output | 32 (signed) | Datamodel: debounce_ms (16.16 fixed-point) |
| `dm_p1_r` | output | 32 (signed) | Datamodel: p1_r (16.16 fixed-point) |
| `dm_p1_g` | output | 32 (signed) | Datamodel: p1_g (16.16 fixed-point) |
| `dm_p1_b` | output | 32 (signed) | Datamodel: p1_b (16.16 fixed-point) |
| `dm_p2_r` | output | 32 (signed) | Datamodel: p2_r (16.16 fixed-point) |
| `dm_p2_g` | output | 32 (signed) | Datamodel: p2_g (16.16 fixed-point) |
| `dm_p2_b` | output | 32 (signed) | Datamodel: p2_b (16.16 fixed-point) |
| `dm_p3_r` | output | 32 (signed) | Datamodel: p3_r (16.16 fixed-point) |
| `dm_p3_g` | output | 32 (signed) | Datamodel: p3_g (16.16 fixed-point) |
| `dm_p3_b` | output | 32 (signed) | Datamodel: p3_b (16.16 fixed-point) |
| `dm_p4_r` | output | 32 (signed) | Datamodel: p4_r (16.16 fixed-point) |
| `dm_p4_g` | output | 32 (signed) | Datamodel: p4_g (16.16 fixed-point) |
| `dm_p4_b` | output | 32 (signed) | Datamodel: p4_b (16.16 fixed-point) |
| `dm_p5_r` | output | 32 (signed) | Datamodel: p5_r (16.16 fixed-point) |
| `dm_p5_g` | output | 32 (signed) | Datamodel: p5_g (16.16 fixed-point) |
| `dm_p5_b` | output | 32 (signed) | Datamodel: p5_b (16.16 fixed-point) |

### Event Encoding
- `button_high` → 0
- `button_low` → 1
- `tick_12mhz` → 2

### State Encoding
- `IdleReleased` → 0
- `DebouncePress` → 1
- `DebounceRelease` → 2
- `ThinkAll` → 3
- `HungryOdd` → 4
- `EatOdd` → 5
- `HungryEven` → 6
- `EatEven` → 7
- `HungryFive` → 8
- `EatFive` → 9

## Test Vectors

- Input: `vectors/events.txt` (one event name per line)
- Golden: `vectors/golden.trace.txt` (expected trace output)
- Output: `output.trace.txt` (generated during simulation)

## Synthesis Notes

The `runtime.v` file is fully synthesizable:
- All logic is synchronous to rising edge of `clk`
- Reset is synchronous (modify for async reset if needed)
- Datamodel uses 32-bit signed fixed-point (16.16 format)
- No vendor-specific primitives
- Compatible with Vivado, Quartus, Synplify, etc.

## Waveform Debugging

To generate VCD waveforms (Icarus Verilog):
```bash
# Add to testbench or run:
iverilog -g2012 -o tb.vvp -DDUMP_VCD runtime.v harness_tb.v
vvp tb.vvp
gtkwave dump.vcd
```
