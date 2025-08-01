# rlvgl – Plugins Workstream TODO

> **Purpose**  Track incremental porting of C-based LVGL add-ons to Rust crates for `rlvgl`.  Tasks are ordered to respect technical dependencies so each layer builds on the previous one.

---

## 🛠️ Codex Pre‑setup Instructions

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
lottie = ["dep:dotlottie"]
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

*Foundation components needed before higher-level widgets or rich content can work.*

| ✔︎  | Component                   | Adopted Rust crate(s)                                        | Task(s)                                                                                                                                      | Depends on |
| --- | --------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| [x] | **PNG decoder**             | `png` crate citeturn241136297508662                       | • Write `rlvgl_png::decode()` wrapper that converts to `embedded-graphics::ImageRaw`.• Add compile-time feature flag `png`.                  | –          |
| [x] | **JPEG decoder / SJPG**     | `jpeg-decoder` crate citeturn655888278065328              | • Add basic JPEG wrapper.• Investigate tiled‐stream (“SJPG”) support → may require small fork or port of tinyjpeg C core (partial refactor). | PNG        |
| [x] | **GIF animation**           | `gif` crate citeturn764961070150154                       | • Streaming frame decoder into `ImageRaw`.• Expose `Image::play()` widget util.• Needs timer tick integration.                               | PNG        |
| [x] | **QR-code generator**       | `qrcode` crate citeturn811324940056358                    | • Wrap `QrCode::new()` → bitmap.• Provide `QrWidget` using embedded-graphics draw-target.                                                    | PNG        |
| [x] | **Dynamic font rasteriser** | `fontdue` (no\_std) or `rusttype` citeturn451122131593768 | • Select crate (pref `fontdue`).• Create `FontProvider` trait.• Replace stub bitmap fonts in Label/Text. |  FONTDUE                        | 
| [ ] | **APNG Decoder** | `apng` crate | • Create `apng` trait / devoder feature. | APNG

--

## ◻️ Level 2 – Extended & UX Enhancements

*Can start once all Level 1 items compile on target.*

| ✔︎  | Component                         | Rust crate / source                                | Task(s)                                                                                                                | Depends on |
| --- | --------------------------------- | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------- |
| [x] | **Lottie / dotLottie animations** | `dotlottie-rs` (player) citeturn236649155616415 | • Evaluate WASM/thorvg backend footprint.• Expose `LottiePlayer` widget.• **For embedded targets, pre-render keyframes in the platform instead of decoding with `dotlottie-rs`.** | GIF, Font  |
| [x] | **Sketchpad / Canvas widget**     | `embedded-canvas` citeturn184290798726883       | • Add `CanvasWidget` integrating pan/zoom.• Provide to-PNG export using PNG feature.                                   | PNG        |
| [x] | **IME – Pinyin support**          | `pinyin` crate citeturn137135872219639          | • Build `PinyinInputMethod` service.• Hook into TextField once implemented.                                            | Font       |
| [x] | **File-explorer (SD/FAT)**        | `fatfs-embedded` citeturn791986641516626        | • Implement `BlockDevice` for target flash/SD.• Add `FilePicker` widget demo.                                          | Canvas     |
| [x] | **Example cartridge (NES)**       | `yane` crate citeturn794589435371464            | • Optional showcase app; embed emulator surface via `CanvasWidget`.• Demonstrates real-time framebuffer streaming.     | Canvas     |
| [x] | **Dash Lottie player**            | stand-alone                   | • standalone Dash Lottie player (rendered Lottie key files)                                           | Lottie     |
| [x] | **Dash Lottie renderer**          | `dotlottie-rs`                | • dotlottie-rs-based renderer to create its keyframes (rendered files)                              | Lottie     |

---

### Sequencing summary

1. **PNG** → unlocks base image drawing pipeline.
2. **JPEG** and **GIF** build on image infra.
3. **QR-code** uses PNG draw-target but independent of animations.
4. **Font rasteriser** can progress in parallel; required by IME & Lottie text.
5. Once Level 1 green, tackle **Lottie**, then **Canvas** (sketchpad) which many advanced widgets share.
6. **IME**, **File-explorer**, and optional **NES** demo depend on Canvas and/or Font work.

---

## Definition of Done checklist

-

*Last updated 2025-07-30*

