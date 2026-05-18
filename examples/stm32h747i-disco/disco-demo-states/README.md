This bundle contains a generated Rust crate providing a minimal executor and harness.

Build
- cargo build --release

Harness (vectors)
- Input: vectors/events.txt (newline-delimited event names)
- Golden: vectors/golden.trace.txt (expected trace)
- Run harness: cargo run --bin harness vectors/events.txt
- Verify against golden: cargo run --bin harness vectors/events.txt vectors/golden.trace.txt (prints PASS on success)
Environment flags
- ISTATE_INTERNAL_EVENTS: when set (1/true/yes), raises/sends enqueue internal events which are drained after each top-level transition.
- ISTATE_LOG_TO_STDERR: enable/disable stderr logging from actions (default: enabled).
- Makefile is provided: make verify