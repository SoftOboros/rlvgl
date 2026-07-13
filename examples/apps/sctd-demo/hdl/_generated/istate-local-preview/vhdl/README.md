# SctdPhilosophersLedTop - VHDL State Machine

Generated iState VHDL implementation.

## Files

- `runtime.vhd` - Synthesizable state machine entity
- `harness_tb.vhd` - Simulation testbench
- `Makefile` - Build automation (GHDL)

## Requirements

- [GHDL](https://ghdl.github.io/ghdl/) - Open-source VHDL simulator
- Or any VHDL-2008 compatible simulator (ModelSim, Vivado, etc.)

## Build & Simulate (GHDL)

```bash
make build    # Compile VHDL files
make run      # Run simulation
make verify   # Run and compare against golden trace
make clean    # Remove build artifacts
```

## Interface

### Entity: `SctdPhilosophersLedTop_fsm`

**Ports:**
| Port | Direction | Type | Description |
|------|-----------|------|-------------|
| `clk` | in | std_logic | Clock input |
| `rst` | in | std_logic | Synchronous reset (active high) |
| `event_valid` | in | std_logic | Event strobe (pulse high for 1 cycle) |
| `event_code` | in | std_logic_vector | Encoded event |
| `state_out` | out | std_logic_vector | Current state encoding |
| `transition_taken` | out | std_logic | High when transition fires |
| `dm_timer_count` | out | signed(31:0) | Datamodel: timer_count (16.16 fixed-point) |
| `dm_phase_ms` | out | signed(31:0) | Datamodel: phase_ms (16.16 fixed-point) |
| `dm_debounce_ms` | out | signed(31:0) | Datamodel: debounce_ms (16.16 fixed-point) |
| `dm_p1_r` | out | signed(31:0) | Datamodel: p1_r (16.16 fixed-point) |
| `dm_p1_g` | out | signed(31:0) | Datamodel: p1_g (16.16 fixed-point) |
| `dm_p1_b` | out | signed(31:0) | Datamodel: p1_b (16.16 fixed-point) |
| `dm_p2_r` | out | signed(31:0) | Datamodel: p2_r (16.16 fixed-point) |
| `dm_p2_g` | out | signed(31:0) | Datamodel: p2_g (16.16 fixed-point) |
| `dm_p2_b` | out | signed(31:0) | Datamodel: p2_b (16.16 fixed-point) |
| `dm_p3_r` | out | signed(31:0) | Datamodel: p3_r (16.16 fixed-point) |
| `dm_p3_g` | out | signed(31:0) | Datamodel: p3_g (16.16 fixed-point) |
| `dm_p3_b` | out | signed(31:0) | Datamodel: p3_b (16.16 fixed-point) |
| `dm_p4_r` | out | signed(31:0) | Datamodel: p4_r (16.16 fixed-point) |
| `dm_p4_g` | out | signed(31:0) | Datamodel: p4_g (16.16 fixed-point) |
| `dm_p4_b` | out | signed(31:0) | Datamodel: p4_b (16.16 fixed-point) |
| `dm_p5_r` | out | signed(31:0) | Datamodel: p5_r (16.16 fixed-point) |
| `dm_p5_g` | out | signed(31:0) | Datamodel: p5_g (16.16 fixed-point) |
| `dm_p5_b` | out | signed(31:0) | Datamodel: p5_b (16.16 fixed-point) |

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

The `runtime.vhd` file is fully synthesizable for FPGA or ASIC targets:
- All logic is synchronous to rising edge of `clk`
- Reset is synchronous (change to async if needed for your target)
- Datamodel uses 32-bit signed (16.16 fixed-point for fractional values)
- No vendor-specific primitives used
