# rlvgl-core

This crate contains the runtime abstractions that underpin every widget and
backend used in **rlvgl**.

Currently implemented pieces:

- `Widget` trait defining drawing and event callbacks
- `WidgetNode` tree for hierarchical composition
- `Event` enum for basic input
- `Renderer` trait for target-agnostic drawing
- `Style` struct with builder for widget appearance

These APIs are early and will evolve as more widgets and backends come online.
