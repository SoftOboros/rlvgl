# Test Vectors

This directory contains test vectors for the SctdPhilosophersLedTop state machine.

## Files

- `events.txt` - Input event sequence (one event name per line)
- `golden.trace.txt` - Expected output trace

## Trace Format

Each line in the trace is one of:
- `on_entry:<state>` - State entry
- `on_exit:<state>` - State exit
- `transition:<src>-><dst>` - Transition fired
- `no_transition:<state> on <event>` - No valid transition for event

## Running Verification

```bash
make verify
```

This will:
1. Run the Verilog simulation with events.txt as input
2. Generate output.trace.txt
3. Compare against golden.trace.txt
4. Print PASS or FAIL

## Custom Test Sequences

Create additional event files and run manually:
```bash
cp vectors/events.txt vectors/my_test.txt
# Edit my_test.txt
# Then manually copy to vectors/events.txt and run make verify
```