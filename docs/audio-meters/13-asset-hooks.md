<!--
13-asset-hooks.md - AM-04b-stub: forward-compat asset hooks for the
aesthetics pass. No graphics shipped here; the schema, runtime types,
and widget API are now ready for art to be plugged in without
disturbing committed contracts.
-->

**[← Prev AM-08e](12-lufs-gating.md) · [Index](README.md)**

# AM-04b-stub — Asset Hooks

Forward-compatibility scaffolding for the aesthetics pass. AM-04b
proper (SVG/PNG sources, rlvgl-creator rasterisation, asset-aware
widget rendering) is deferred per the user directive that aesthetics
land as a separate pass. This stub commits the *types* — runtime
`SkinAssets`, the `skin.assets` field on `Skin` — so the aesthetics
pass can populate them and add rendering paths without changing any
existing committed contract.

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/skin.rs` | New `SkinAssets` struct with `Option<&'static [u8]>` slots: `led_segment_on_png`, `led_segment_off_png`, `needle_svg`, `bezel_svg`, `faceplate_png`. New `SkinAssets::EMPTY` const for "no assets". New `Skin.assets: SkinAssets` field. |
| `widgets/src/meters/presets.rs` | Every existing skin preset gains `assets: SkinAssets::EMPTY`. |
| `audio-meters-widgets/ts/src/skin.ts` | New `SkinAssets` interface with optional filename strings (matching the JSON schema). New `Skin.assets` optional field. |

The schema-side `assets` field has been declared since AM-04a
(`assets/audio-meters/schema/skin.schema.json`). AM-04b-stub aligns
the runtime types with it.

## What's next (the aesthetics pass)

When the aesthetics pass starts:

1. **Author SVG / PNG sources** under
   `assets/audio-meters/{svg,png}/`. The skin JSON's `assets` block
   references them by filename.
2. **Extend `rlvgl-creator`** with a `meters from-yaml` (or similar)
   subcommand that:
   - Rasterises SVGs at the appropriate target sizes / colour formats.
   - Optionally re-encodes PNGs to RLE for embedded targets.
   - Emits `pub static <SKIN_ID>_ASSETS: SkinAssets = SkinAssets {
     led_segment_on_png: Some(include_bytes!(...)), ... };` and
     wires the `assets` field of the corresponding skin to point at
     it.
3. **TS bundler config** — let webpack/vite/rollup import the SVG /
   PNG files directly as data URLs or static asset URLs; the
   custom-element loaders (already shipped in AM-05/06/07/08d) read
   them via `fetch`.
4. **Widget rendering paths** — each first-party widget grows an
   asset-aware draw path keyed off `self.skin.assets.any_present()`:
   - Bargraph: blit `led_segment_on_png` per lit cell, `_off_png`
     per dark cell. Overlay zone tint with palette as needed.
   - Needle: rasterise / blit `needle_svg` rotated by
     `needle_angle_rad()`; layer over `faceplate_png`.
   - Numeric / LufsGauge: optional `bezel_svg` overlay around the
     text region.
5. **Skin variants** — author 2-3 alternate looks (modern flat,
   vintage analog) leveraging the asset pipeline.

None of the steps above modify the schema, the runtime `Scale` /
`Skin` type shape, or the `Widget` trait surface. The widget
constructors remain unchanged. Existing skins (AM-04a / AM-08*)
continue to work in their procedural-rendering form because their
`assets` field is `SkinAssets::EMPTY`.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `assets/audio-meters/schema/skin.schema.json` | The `assets` block has been declared optional since AM-04a. AM-04b-stub doesn't change the schema; it aligns the runtime types with what the schema already permits. |
| `meter_presets_match_json` | Currently does not check the `assets` field. When the aesthetics pass starts populating real skins, the test will be extended to compare per-filename. |
| `rlvgl-creator` | Out of scope for this stub. The codegen target is documented above; it slots in next to the existing chipdb / BSP generators. |
| `rlvgl-decomp` | Existing RLE decoder; reused by the asset pipeline when SVG → RLE is the chosen runtime format on embedded targets. |
| Widget rendering | Unchanged. AM-04b-stub does not add asset-aware draw paths. The `SkinAssets::any_present()` helper is exposed so the aesthetics pass can branch on it cleanly. |

## Acceptance checklist

- [x] `SkinAssets` struct + `EMPTY` const + `any_present` helper
      shipped in Rust.
- [x] `SkinAssets` interface shipped in TS.
- [x] All existing Rust skin presets carry `assets: SkinAssets::EMPTY`.
- [x] No widget code change; existing tests pass unmodified.
- [x] Cortex-M7 cross-compile clean.

## Non-goals (stub-by-design)

- Real SVG / PNG art under `assets/audio-meters/{svg,png}/`.
- `rlvgl-creator meters from-yaml` codegen.
- Asset-aware widget rendering paths.
- Additional skin presets exercising the assets.

All of those are the aesthetics pass — covered when it starts.

## Files cited

- `widgets/src/meters/skin.rs`
- `widgets/src/meters/presets.rs`
- `widgets/src/meters/mod.rs`
- `audio-meters-widgets/ts/src/skin.ts`
- `audio-meters-widgets/ts/src/index.ts`
- [`docs/audio-meters/04-skins.md`](04-skins.md)

## Change log

- **2026-04-26** — Initial ratification (AM-04b-stub). `SkinAssets`
  type + `skin.assets` runtime field shipped on both runtimes.
  Aesthetics pass can begin without re-touching the schema or the
  widget API.
