//! Historical SCTD compile-gate entry point.
//!
//! SCTD-04 superseded the controller-only gate with a flashable target. The
//! `rlvgl-stm32h747i-sctd` manifest target now uses `main.rs`, where the `sctd`
//! feature selects the tutorial payload after the board's established Rust
//! display, touch, Playit, DMA2D, LTDC and DSI initialization.
