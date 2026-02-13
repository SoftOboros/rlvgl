<!--
docs/TODO-PLUGINS.md - rlvgl - Plugins Workstream TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl - Plugins Workstream TODO

> **Purpose** Track incremental porting of C-based LVGL add-ons to Rust crates for `rlvgl`. Tasks are ordered to respect technical dependencies so each layer builds on the previous one.

---

## 🛠️ Codex Pre-setup Instructions

Before tackling the plugin TODOs, Codex should set up the `rlvgl` workspace to support modular plugin development using Cargo features.

### 1. Update `Cargo.toml` with plugin features

Add the following to the `[features]` section:

```toml
[features]
default = []

# Level 1
png = ["dep:png"]
jpeg = ["dep:jpeg-decoder"]
gif = ["dep:gif"]
qrcode = ["dep:qrcode"]
fontdue = ["dep:fontdue"]

# Level 2
lottie = ["dep:rlottie"]
canvas = ["dep:embedded-canvas"]
pinyin = []
fatfs = ["dep:fatfs-embedded"]
nes = ["dep:yane"]
```

Also declare `[dependencies]` entries with `optional = true`, for example:

```toml
[dependencies.png]
version = "*"
optional = true
```

### 2. Crate structure

Ensure each plugin lives in its own `src/plugins/<name>.rs` file:

```rust
#[cfg(feature = "png")]
pub mod png;
```

Then in `lib.rs`:

```rust
#[cfg(feature = "png")]
pub use plugins::png;
```

### 3. Testing

Each plugin should have:

- `#[cfg(test)]` unit tests in its own file.
- Optional integration tests under `tests/plugins_png.rs`, etc.

Use feature flags in tests:

```rust
#[cfg(feature = "png")]
#[test]
fn test_png_decode() { /* … */ }
```

### 4. CI Matrix Stub

Support `cargo test --features gif,fontdue`, etc. Example CI job matrix:

```yaml
matrix:
  include:
    - features: "png jpeg gif"
    - features: "qrcode fontdue"
    - features: "lottie canvas"
```

---

## ⬛ Level 1 – Core Media & Text Pipeline

*Foundation components needed before
