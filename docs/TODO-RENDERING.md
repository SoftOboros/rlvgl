# rlvgl – Rendering Workstream TODO

This file tracks tasks for enhancing the rendering pipeline so widgets can draw multiple layers with alpha blending. All color values must carry RGBA data from source to display; if every layer is transparent at a pixel, the color from the lowest layer remains visible.

## Alpha-enabled Rendering
- [x] Extend `Color` from RGB to RGBA so widgets can express opacity.
- [ ] Add alpha-aware blend methods to `Renderer` and update backends.
- [ ] Define widget layering/compositing semantics so higher layers blend over lower ones.
- [ ] Propagate RGBA colors through style and fill APIs across widgets and backends.

---

*Last updated 2025-08-06*
