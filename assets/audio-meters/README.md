# Audio Meters Asset Package

Cross-runtime source-of-truth for the rlvgl audio-meters initiative. The
files here are consumed by both rlvgl (Rust, no_std) and the TypeScript
companion (`@rlvgl/audio-meters-core` and downstream widget packages).

## Layout

```
assets/audio-meters/
  schema/
    scale.schema.json    # JSON Schema for scale descriptors (AM-03)
  scales/
    *.json               # Canonical scale set (concepts §6)
  svg/                   # Vector primitives (AM-04)
  png/                   # Raster sprites (AM-04)
  skins/                 # Skin descriptors binding scale + ballistic + assets (AM-04)
```

## Adding a scale

1. Drop a new `*.json` file under `scales/` matching `schema/scale.schema.json`.
2. The `id` field MUST equal the filename stem.
3. Validation runs in both runtimes:
   - Rust: `cargo test -p rlvgl-audio-meters-core --test scales`
   - TS: `npm test --prefix audio-meters-core/ts`
4. Adding a *new* scale is **Specification Required** (concepts §6) — no
   §15 change-log amendment needed unless the new scale changes the
   schema. Document the scale in the relevant chapter walkthrough.

## Adding visual assets (AM-04)

TBD. Source SVGs go under `svg/`, source PNGs under `png/`. Skins under
`skins/` reference them by filename. The rlvgl-creator pipeline
rasterises SVGs into per-target RLE blobs at build time; the TS bundler
imports them directly as ESM static assets.

## Reference

See [`docs/audio-meters/00-concepts.md`](../../docs/audio-meters/00-concepts.md)
for the §3 glossary, §5 ballistic enum, §6 scale enum, §7 colour enum,
§8 schema, and §9 widget update contract that all assets here are built
against.
