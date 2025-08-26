<!--
docs/TODO-PLUGGABLE-BLITTER.md - Epic: Pluggable Rendering/Display Backends (CPU, DMA2D, winit/wgpu).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Epic: Pluggable Rendering/Display Backends (CPU, DMA2D, winit/wgpu)

**Description**: Introduce a `Blitter` strategy trait and multiple implementations (CPU fallback, STM32H7 DMA2D, desktop wgpu). Wire these under `platform/` so the same widget/render code targets embedded and desktop. Adds LTDC/DSI + OTM8009A (DISCO) and FT5336 touch. Updates simulator to use `winit + wgpu` (window + GPU) for speed.  
**Outcome**: Hardware‑accelerated flush paths on H7; high‑FPS simulator; unified testing.

---

## A) Blitter Abstraction (platform)

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Define `Blitter` trait: `caps()`, `fill()`, `blit()`, `blend()`, PFC support | `bitflags` (caps) | Rect + surface types live in `platform::blit`. |
| [x] | Add `Surface` (buf/stride/fmt/w,h) + `PixelFmt` enum | none | Include ARGB8888, RGB565, L8/A8/A4. |
| [x] | Add `BlitPlanner` to batch dirty rects per frame | none | Optional: coalesce touching rects. |
| [x] | Thread through renderer → blitter (no API leak to widgets) | platform renderer | Renderer owns a `&mut dyn Blitter`. |

---

## B) CPU Fallback Blitter

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Implement `CpuBlitter` (scalar loops) | none | Correctness baseline, used in tests. |
| [x] | Fast paths for common formats (ARGB8888→RGB565, fills) | none | Consider `bytemuck` for casts. |
| [x] | Unit tests (golden buffers) | `proptest` optional | Reuse same tests across all backends. |

---

## C) STM32H7 DMA2D (“GPU”) Blitter

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Create `Dma2dBlitter` with PAC register access | `stm32h7` PAC, `cortex-m` | HAL lacks full DMA2D; use PAC. |
| [x] | Init: clock, fore/back layer config, line offset | PAC | Keep safe wrapper; no `unsafe` in API. |
| [x] | Implement R2M (fill) | PAC | Blocking first; add IRQ later. |
| [x] | Implement M2M/PFC (copy + convert) | PAC | Common ARGB8888→RGB565 path. |
| [x] | Implement M2M blend (FG over BG, const/per‑pixel alpha) | PAC | Straight‑alpha assumption; doc it. |
| [x] | Optional: non‑blocking w/ interrupt/completion | EXTI/IRQ | Queue ops; fence before VSYNC. |
| [ ] | Reuse CPU tests to assert identical pixels | `std` test via host build | Use small test images, crops. |

---

## D) STM32H747I‑DISCO Display (LTDC/DSI + OTM8009A)

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Bring‑up clocks for LTDC/DSI (RCC config) | `stm32h7xx-hal` (RCC) | Match panel timing. |
| [x] | SDRAM (FMC) if FB in external RAM | HAL FMC or PAC | AXI SRAM ok for small tests. |
| [x] | DSI host + OTM8009A init sequence (video mode) | PAC | Port from C BSP; factor `otm8009a.rs`. |
| [x] | LTDC layer setup (FB addr, stride, fmt) | PAC | Start RGB565 FB to save RAM. |
| [x] | Backlight PWM + panel RESET GPIO | HAL TIM/GPIO | Optional TE line for vsync. |
| [x] | `Stm32h747iDiscoDisplay<B: Blitter>` glue | sections A/C | Compose selected blitter. |
| [x] | Feature flag: `stm32h747i_disco` | Cargo features | Gate no‑std deps/panic handler. |

---

## E) FT5336 Touch (I²C + EXTI)

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | I²C init @ 400 kHz | `stm32h7xx-hal` I2C | Use board pins. |
| [x] | EXTI on INT line (optional) | HAL EXTI | Or poll in `poll()`. |
| [x] | Minimal FT5336 driver: read points | none | Convert to `Event` (down/move/up). |
| [x] | `Stm32h747iDiscoInput` integration | platform input | Coordinate flip/rotation config. |

---

## F) Desktop Simulator: **winit + wgpu** Backend

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Replace/minimize `pixels/minifb` usage | `winit`, `wgpu` | `winit` window + `wgpu` swapchain. |
| [x] | `WgpuBlitter` implementing `Blitter` | `wgpu` | Use render pass + textured quads or compute. |
| [x] | Upload tile/rect to texture; blit/blend in shader | `wgpu` | Textures updated and blended via render pipelines. |
| [x] | Present @ vsync; map keyboard/mouse → `InputDevice` | `winit` | DPI scaling; sRGB swapchain. |
| [x] | Headless mode to dump PNGs for CI | `image` | Golden‑image regression tests. |

---

## G) SPI Panel Example (ST7789) to Prove Portability

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | `st7789` driver via `embedded-hal` | `embedded-hal` | Reuse `CpuBlitter`. |
| [ ] | DMA SPI flush path | HAL DMA | Optional: double‑buffer lines. |

---

## H) Integration & CI

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Cargo features matrix (`cpu`, `dma2d`, `wgpu`) | Cargo | Make backends swappable. |
| [ ] | CI jobs: host tests + wgpu offscreen + size report | GitHub Actions | Keep current size checks. |
| [ ] | Example: `examples/sim` uses `wgpu` | F) | Keybindings: toggle dirty‑rect debug. |
| [ ] | Example: `examples/STM32H747I-DISCO` uses DMA2D | C/D/E | Shares app code with sim (refactor). |

---

## I) Docs & Diffs

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | `#![doc = include_str!(…)]` for public APIs | none | Mirrors project style. |
| [ ] | Developer doc: “Choosing a blitter/backend” | none | When to pick which, memory tradeoffs. |
| [ ] | Image diff harness (sim output vs golden) | `image`, `assert_cmd` | Thresholded RGBA delta.


---

## J) Plugins & Widgets – Blitter Integration

| Done | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Integrate `fontdue` text rasteriser into `BlitterRenderer` | `fontdue` | Cache glyphs as `Surface`s; support CPU/WGPU/DMA2D paths. |
| [x] | Connect image decoders (`png`, `jpeg`, `gif`, `apng`) to produce blitter surfaces | `png`, `jpeg`, `gif`, `apng` | Decode to `Surface` and call `blit()`/`blend()`; handle animation frames. |
| [x] | Render `QrWidget` via blitter pipeline | `qrcode` | Generate QR bitmap, upload as `Surface`, avoid direct framebuffer writes. |
| [x] | Bridge `rlottie` frames to blitter surfaces | `rlottie` | Convert vector frames to `Surface`; allow GPU acceleration. |
| [x] | Treat `CanvasWidget` buffers as blitter surfaces | `embedded-canvas` | Incrementally flush dirty regions through blitter. |
| [x] | Route higher-level widgets (pinyin IME, FATFS file picker, NES demo) through canvas/blitter stack | `pinyin`, `fatfs-embedded`, `yane` | Ensure their rendering paths remain backend-agnostic. |
