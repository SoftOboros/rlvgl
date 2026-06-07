Vectors
- Input: vectors/events.txt (newline-delimited event names)
- Golden: vectors/golden.trace.txt (expected trace)

Usage
- cargo run --bin harness vectors/events.txt
- cargo run --bin harness vectors/events.txt vectors/golden.trace.txt  # compare
- make verify

Trace format
- on_entry:STATE
- on_exit:STATE
- transition:SRC->DST
- no_transition:STATE on EVENT
