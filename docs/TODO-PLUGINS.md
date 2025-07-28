# rlvgl – Plugins Workstream TODO

> **Purpose**  Track incremental porting of C‑based LVGL add‑ons to Rust crates for `rlvgl`.  Tasks are ordered to respect technical dependencies so each layer builds on the previous one.

---

## ⬛ Level 1 – Core Media & Text Pipeline  
*Foundation components needed before higher‑level widgets or rich content can work.*

| ✔︎ | Component | Adopted Rust crate(s) | Task(s) | Depends on |
|----|-----------|-----------------------|---------|------------|
| [ ] | **PNG decoder** | `png` crate citeturn241136297508662 | • Write `rlvgl_png::decode()` wrapper that converts to `embedded‑graphics::ImageRaw`.<br>• Add compile‑time feature flag `png`.| – |
| [ ] | **JPEG decoder / SJPG** | `jpeg-decoder` crate citeturn655888278065328 | • Add basic JPEG wrapper.<br>• Investigate tiled‐stream (“SJPG”) support → may require small fork or port of tinyjpeg C core (partial refactor). | PNG |
| [ ] | **GIF animation** | `gif` crate citeturn764961070150154 | • Streaming frame decoder into `ImageRaw`.<br>• Expose `Image::play()` widget util.<br>• Needs timer tick integration. | PNG |
| [ ] | **QR‑code generator** | `qrcode` crate citeturn811324940056358 | • Wrap `QrCode::new()` → bitmap.<br>• Provide `QrWidget` using embedded‑graphics draw‑target. | PNG |
| [ ] | **Dynamic font rasteriser** | `fontdue` (no_std) or `rusttype` citeturn451122131593768 | • Select crate (pref `fontdue`).<br>• Create `FontProvider` trait.<br>• Replace stub bitmap fonts in Label/Text. | – |

---

## ◻️ Level 2 – Extended & UX Enhancements  
*Can start once all Level 1 items compile on target.*

| ✔︎ | Component | Rust crate / source | Task(s) | Depends on |
|----|-----------|--------------------|---------|------------|
| [ ] | **Lottie / dotLottie animations** | `dotlottie-rs` (player) citeturn236649155616415 | • Evaluate WASM/thorvg backend footprint.<br>• Expose `LottiePlayer` widget.<br>• Might need feature gate `lottie` (std‑only). | GIF, Font |
| [ ] | **Sketchpad / Canvas widget** | `embedded‑canvas` citeturn184290798726883 | • Add `CanvasWidget` integrating pan/zoom.<br>• Provide to‑PNG export using PNG feature. | PNG |
| [ ] | **IME – Pinyin support** | `pinyin` crate citeturn137135872219639 | • Build `PinyinInputMethod` service.<br>• Hook into TextField once implemented. | Font |
| [ ] | **File‑explorer (SD/FAT)** | `fatfs-embedded` citeturn791986641516626 | • Implement `BlockDevice` for target flash/SD.<br>• Add `FilePicker` widget demo. | Canvas |
| [ ] | **Example cartridge (NES)** | `yane` crate citeturn794589435371464 | • Optional showcase app; embed emulator surface via `CanvasWidget`.<br>• Demonstrates real‑time framebuffer streaming. | Canvas |

---

### Sequencing summary
1. **PNG** → unlocks base image drawing pipeline.
2. **JPEG** and **GIF** build on image infra.
3. **QR‑code** uses PNG draw‑target but independent of animations.
4. **Font rasteriser** can progress in parallel; required by IME & Lottie text.
5. Once Level 1 green, tackle **Lottie**, then **Canvas** (sketchpad) which many advanced widgets share.
6. **IME**, **File‑explorer**, and optional **NES** demo depend on Canvas and/or Font work.

---

## Definition of Done checklist
- [ ] Every plugin behind a `cfg(feature = "…")` gate.
- [ ] Unit tests decode/render sample asset under `tests/assets/…`.
- [ ] `no_std` build passes for Level 1 crates (PNG, JPEG, GIF, QR, Font).
- [ ] CI job `plugins‑examples` runs on desktop simulator and saves PNG diff images.

*Last updated {{DATE}}*

