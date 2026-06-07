<!--
04b-aesthetics.md — AM-04b: aesthetics pass proper. SVG / PNG asset
sources, rlvgl-creator rasterisation, asset-aware widget draw paths.
Replaces the AM-04b-stub deferral in `13-asset-hooks.md`.
-->

**[← Prev AM-04a](04-skins.md) · [Index](README.md) · [Next → AM-05 LedBargraph](05-led-bargraph.md)**

# AM-04b — Aesthetics: Visual Primitives + Creator Rasterisation

This chapter is the §0 concepts gate for the **aesthetics pass** of the
audio-meters initiative. It replaces the `AM-04b-stub` deferral in
[`13-asset-hooks.md`](13-asset-hooks.md) §"What's next (the aesthetics
pass)" with a ratified specification: SVG / PNG sources, a
build-time rasteriser shipped as a new `rlvgl-creator` subcommand,
asset-aware widget draw paths on both runtimes, and at least 2-3
alternate-look skin variants ("modern flat", "vintage analog") shipped
under `assets/audio-meters/skins/`.

Subsequent `AM-04b[a-z]:` execution PRs cite this doc as their
authority. Per the spec-before-code discipline in `CLAUDE.md`, no
execution PR rides on an unratified amendment to §5-§9 frozen
decisions; vocabulary or invariant changes ratify here first.

The §3 glossary, §5-§9 frozen decisions, §10 reconciliation table,
and §12 acceptance checklist are normative. The remaining sections
are informative.

## §0 — Authority policy

| Concept / artifact | Owner | AM-04b relationship |
|---|---|---|
| Runtime Rust `SkinAssets` struct + `Skin.assets` field | `widgets/src/meters/skin.rs` | Used without modification. AM-04b populates slots; it does **not** widen or rename them without a §15 amendment first. |
| Runtime TS `SkinAssets` interface | `audio-meters-widgets/ts/src/skin.ts` | Used without modification. The slot set MUST stay set-equal with Rust (see §5 INV-A4). |
| JSON `assets` block | `assets/audio-meters/schema/skin.schema.json` | Used without modification. The schema declared the block in AM-04a and the slot set is already enumerated; AM-04b authors files that reference it. |
| Procedural widget draw paths | `widgets/src/meters/{bargraph,needle,numeric,lufs_gauge}.rs` (Rust); `audio-meters-widgets/ts/src/{led-bargraph,needle-vu,numeric-peak,lufs-gauge}-core.ts` (TS) | Extended: each gains an asset-aware branch keyed off `self.skin.assets.any_present()`. State computation paths are unchanged (INV-A5). |
| Build-time rasteriser CLI surface | `src/bin/creator/` (`rlvgl-creator meters from-yaml` subcommand) | Owned by AM-04b; does not exist yet. Slots in next to existing chipdb / BSP generators (`Command::Bsp { cmd: BspCommand }`). |
| Per-target rasterised asset blobs | `<consumer-crate>/src/meters_assets_generated/` (codegen target, never hand-edited) | Owned by AM-04b. |
| RLE on-flash format for embedded targets | `rlvgl-decomp/` (existing decoder) | Consumed unmodified — the rasteriser produces RLE blobs the existing decoder accepts, when the consumer opts in to `--format rle`. |
| IEC 60268-10 / IEC 60268-17 / AES17 / ITU-R BS.1770-4 colour and metering authorities | External standards | Informative for the aesthetics pass. None of them prescribe a colour palette; they prescribe ballistics and scale ranges, both already frozen in AM-00 / AM-03. |
| Vendor "house style" references (BBC, Dolby, SSL, Studer, Neumann, etc.) | Their respective brand guidelines | Informative-only. AM-04b ships **generic** alternate looks; no vendor trademark, logo, or proprietary colour value is copied. The initiative owner MAY ship a separate, vendor-licensed asset pack outside this repo. |

The aesthetics pass does **not** introduce a new normative external
standard. It is bound by the standards AM-00 §0 cites; visual choices
(specific RGB values, gradient stops, art style) are not standardised
and live in the §10 art-direction note as `Owned by AM-04b[execution-letter]`.

## §1 — Purpose

Close the only remaining audio-meters-initiative deferral.

Replace the AM-04b-stub forward-compatibility scaffolding in
[`13-asset-hooks.md`](13-asset-hooks.md) with a concrete pipeline:

1. **SVG and PNG sources** under `assets/audio-meters/{svg,png}/`,
   referenced by filename from skin JSON files' `assets` block.
2. **A new `rlvgl-creator meters from-yaml` (or equivalent)
   subcommand** that rasterises SVG sources, optionally re-encodes
   PNG sources, emits `pub static <SKIN_ID>_ASSETS: SkinAssets =
   ...;` Rust source, and wires each generated `SkinAssets` constant
   into the corresponding skin preset.
3. **Asset-aware draw paths** on each of the four first-party
   widgets (`LedBargraph`, `NeedleVu`, `NumericPeak`, `LufsGauge`),
   keyed off `self.skin.assets.any_present()` so existing
   procedural-render skins are unchanged.
4. **A TS-side loader pattern** so the same skin JSON files drive
   the browser custom elements, with browser-native SVG handling
   (no TS-side rasterisation).
5. **At least 2-3 alternate-look skin variants** shipped under
   `assets/audio-meters/skins/`, exercising the asset path
   end-to-end on both runtimes.

The technical contract is the deliverable of this concepts doc. The
**visual direction** — palettes, art style, vintage faceplate
imagery, modern-flat geometry — is an art-direction concern handled
in execution PRs as `Owned by AM-04b[execution-letter]` decisions
by the initiative owner.

## §2 — Problem statement

[`13-asset-hooks.md`](13-asset-hooks.md) §"What's next (the
aesthetics pass)" enumerates the work that the stub deferred:

> 1. Author SVG / PNG sources under `assets/audio-meters/{svg,png}/`.
>    The skin JSON's `assets` block references them by filename.
> 2. Extend `rlvgl-creator` with a `meters from-yaml` (or similar)
>    subcommand that:
>    - Rasterises SVGs at the appropriate target sizes / colour formats.
>    - Optionally re-encodes PNGs to RLE for embedded targets.
>    - Emits `pub static <SKIN_ID>_ASSETS: SkinAssets = SkinAssets {
>      led_segment_on_png: Some(include_bytes!(...)), ... };` and
>      wires the `assets` field of the corresponding skin to point at it.
> 3. TS bundler config — let webpack/vite/rollup import the SVG /
>    PNG files directly as data URLs or static asset URLs; the
>    custom-element loaders (already shipped in AM-05/06/07/08d) read
>    them via `fetch`.
> 4. Widget rendering paths — each first-party widget grows an
>    asset-aware draw path keyed off `self.skin.assets.any_present()`.
> 5. Skin variants — author 2-3 alternate looks (modern flat,
>    vintage analog) leveraging the asset pipeline.

That five-item list is the inherited scope contract. AM-04b ratifies
it as the spec deliverable, plus the cross-cutting invariants the
stub could not pin without an execution plan.

The inherited scope from [`04-skins.md`](04-skins.md) §"Non-goals
(deferred to AM-04b)" also stands:

> - SVG / PNG primitive sources under `assets/audio-meters/{svg,png}/`.
> - `rlvgl-creator meters from-yaml` (or equivalent) subcommand for
>   rasterisation per target.
> - TS bundler-side direct ESM import of SVG / PNG.
> - Compile-time codegen of `const SkinStatic` modules for embedded
>   consumers.

The current surface area that will gain conditional branches is
cited at:

- `widgets/src/meters/bargraph.rs:151` — `LedBargraph::draw` paints
  `fill_rect(bounds, bg)` plus per-segment `fill_rect` calls. The
  asset-aware branch substitutes `led_segment_on_png` /
  `led_segment_off_png` blits per cell.
- `widgets/src/meters/needle.rs:130` — `NeedleVu::draw` paints
  `fill_rect(bounds, bg)` plus needle line and pivot dot. The
  asset-aware branch substitutes a `faceplate_png` blit (background)
  and a rotated `needle_svg` blit.
- `widgets/src/meters/numeric.rs:125` — `NumericPeak::draw` paints
  `fill_rect(bounds, bg)` plus two text lines. The asset-aware
  branch overlays an optional `bezel_svg`.
- `widgets/src/meters/lufs_gauge.rs:148` — `LufsGauge::draw` paints
  `fill_rect(bounds, bg)` plus three text lines. Same `bezel_svg`
  overlay pattern.

None of those four widgets are blocked on widget-API changes; the
asset-aware branches are purely additive at the draw-method level.

The first symptom this spec exists to prevent — beyond reopening the
deferral — is two-runtime drift on which pixels light up at a given
dBFS. INV-A5 ("bit-identical state computation; assets only change
pixel sourcing") is the technical anchor.

## §3 — Canonical glossary

Each term is defined once. Code definitions are cited with the
relationship marker from `CLAUDE.md` § "Spec-Before-Code Planning
Discipline" — *used without modification* / *adapted: <delta>* /
*Owned by AM-04b; does not exist in repo yet*.

- **`SkinAssets`** — Optional graphical primitives bound to a
  [`Skin`]. As defined in
  [`widgets/src/meters/skin.rs:204`](../../widgets/src/meters/skin.rs)
  (`pub struct SkinAssets { led_segment_on_png, led_segment_off_png,
  needle_svg, bezel_svg, faceplate_png: Option<&'static [u8]> }`);
  used without modification. AM-04b populates slots but does not
  rename, widen, or remove them without a §15 amendment first.

- **`Skin.assets`** — The `SkinAssets` field on the runtime
  [`Skin`] struct. As defined in
  [`widgets/src/meters/skin.rs:265`](../../widgets/src/meters/skin.rs);
  used without modification. AM-04a / AM-08* skins set this to
  `SkinAssets::EMPTY`; AM-04b populates it for the asset-bearing
  variants.

- **TS `SkinAssets`** — TS-runtime mirror. As defined in
  [`audio-meters-widgets/ts/src/skin.ts:89`](../../audio-meters-widgets/ts/src/skin.ts)
  (`export interface SkinAssets { led_segment_on_png?: string;
  led_segment_off_png?: string; needle_svg?: string; bezel_svg?:
  string; faceplate_png?: string; faceplate_svg?: string; }`);
  used without modification, with the slot-set-equality caveat
  in §5 INV-A4.

- **`SkinAssets::any_present`** — Fast-path predicate. As defined
  in [`widgets/src/meters/skin.rs:234`](../../widgets/src/meters/skin.rs);
  used without modification. The asset-aware widget branch keys off
  this predicate (§5 INV-A5).

- **`MeterRasterizer`** — Owned by AM-04b; does not exist in repo
  yet. The host-side build-time component invoked by the
  `meters from-yaml` subcommand. Reads a skin JSON file, resolves
  its `assets` block to source files under
  `assets/audio-meters/{svg,png}/`, rasterises SVG at the requested
  target sizes / formats, re-encodes PNG as required, and emits a
  generated Rust source file containing `pub static
  <SKIN_ID>_ASSETS: SkinAssets = SkinAssets { ... };` plus an
  optional wired-up `pub static <SKIN_ID>: Skin = ...;` overlay.
  Crate boundary: lives next to the existing
  `src/bin/creator/bsp_gen.rs`, conditionally compiled under the
  `creator` cargo feature so the embedded build profile does not
  pull in the rasteriser dependencies.

- **`meters from-yaml` subcommand** — Owned by AM-04b; does not
  exist yet. The user-facing CLI surface that drives the
  `MeterRasterizer`. Name parallels the existing `bsp from-yaml`
  subcommand (`Command::Bsp { cmd: BspCommand::FromIoc }` —
  [`src/bin/creator/cli.rs:614`](../../src/bin/creator/cli.rs)).
  Input: a YAML manifest naming the skins to rasterise and the
  per-skin target output format (RLE / RGB565 / RGBA8 / passthrough
  PNG). Output: one generated Rust source file per skin, plus an
  optional `mod.rs` index.

- **`AssetSource`** — Owned by AM-04b. A single SVG / PNG file
  under `assets/audio-meters/{svg,png}/`. Plain SVG only (no
  script, no foreign-object — §5 INV-A1); 8-bit-per-channel PNG
  only (§5 INV-A2). Filenames are skin-local (the JSON `assets`
  block names them relative to the skins directory, resolution to
  the source directory is the rasteriser's job).

- **`AssetSlot`** — Owned by AM-04b. The enumerated set of slots
  on `SkinAssets`: `led_segment_on_png`, `led_segment_off_png`,
  `needle_svg`, `bezel_svg`, `faceplate_png`. Frozen in §5
  with Standards Action registration policy. The TS interface
  includes a sixth `faceplate_svg` slot for native-SVG browser
  use; the Rust side rasterises both `faceplate_svg` and
  `faceplate_png` into the single `faceplate_png` runtime slot
  (§5 INV-A4 set-equality is preserved at the rendered-pixel
  level, not at the bytes-on-disk level).

- **`AssetVariant`** — Owned by AM-04b. A named alternate look
  shipped as a complete skin under `assets/audio-meters/skins/`.
  Initial set: at least one `modern_flat_<family>` and one
  `vintage_analog_<family>` (e.g. `vintage_analog_vu_needle`,
  `modern_flat_digital_bargraph`). The concrete identifiers, art
  direction, and palette choices are `Owned by
  AM-04b[execution-letter]` decisions; this concepts doc fixes
  only the count (≥3) and the fact that each variant is a
  complete skin file, not a delta over an existing skin.

- **Asset-aware draw path** — Owned by AM-04b. The conditional
  branch in each widget's `draw()` method, keyed off
  `self.skin.assets.any_present()`. When `false`, the existing
  procedural branch runs unchanged. When `true`, the widget
  consults individual slots (e.g.
  `self.skin.assets.led_segment_on_png.is_some()`) and substitutes
  asset blits for the corresponding `fill_rect` calls. The
  asset-aware branch MUST NOT mutate widget state (peak hold,
  ballistic, cached reading) — that lives in the shared `update`
  path (§5 INV-A5).

- **Procedural draw path** — As-of-AM-04a draw model that
  synthesises pixels from `palette` + `layout` + `secondary` only.
  Cited as the "procedural-render" baseline in
  [`04-skins.md`](04-skins.md) §"Validator contract" item 9 and
  [`13-asset-hooks.md`](13-asset-hooks.md) §"What's next" item 4.

- **Pre-rasterisation** — The build-time conversion of SVG sources
  into target-format byte slices that the runtime can consume
  without a parser. Anchored on §5 INV-A6: "runtime SVG parsing in
  `no_std` is out of scope". The runtime byte slice format is the
  consumer's choice (RLE blob via existing
  [`rlvgl-decomp`](../../rlvgl-decomp/README.md); RGB565 strip;
  PNG that the platform PNG decoder handles).

- **Skin-local bounds** — The widget's `bounds` rectangle at
  draw time. Rasterisation target sizes derive from this rectangle,
  not from the skin JSON (§5 INV-A3). Skins declare *which* slot
  is populated; widgets pass *what size* at draw time.

- **TS asset loader** — Owned by AM-04b. The browser-side analogue
  of the rasteriser, but with no rasterisation: the TS runtime
  resolves the skin JSON's `assets` filenames against a bundler-
  configured asset root (webpack `asset/resource`, vite `?url`,
  rollup `@rollup/plugin-url`, or equivalent), then uses
  `fetch()` + `<img>` or `<canvas>`'s native SVG path. The TS
  side **does not** rasterise SVG — browsers handle that natively.

## §4 — Source-of-truth map

One owner per concept. Implementations in other layers reference,
never restate.

| Concept / artifact | Owner | Mirrored in |
|---|---|---|
| `SkinAssets` Rust struct shape | `widgets/src/meters/skin.rs` (already shipped AM-04b-stub) | TS interface in `audio-meters-widgets/ts/src/skin.ts` (set-equal slot set, INV-A4). |
| `Skin.assets` field on runtime `Skin` | `widgets/src/meters/skin.rs` (already shipped AM-04b-stub) | Optional `assets?: SkinAssets` field on TS `Skin` interface. |
| SVG `AssetSource` files | `assets/audio-meters/svg/` | Rasterised at build time on the Rust side; consumed natively at runtime by the browser. |
| PNG `AssetSource` files | `assets/audio-meters/png/` | Optionally re-encoded at build time on the Rust side; consumed directly via `fetch` + `<img>` on the browser side. |
| Skin JSON `assets` block | Individual skin files under `assets/audio-meters/skins/*.json`; schema at `assets/audio-meters/schema/skin.schema.json` (declared since AM-04a) | Loaded as `Skin.assets?` on TS, projected into `SkinAssets` on Rust via codegen. |
| Build-time rasteriser | `src/bin/creator/` (`meters from-yaml` subcommand; module name `src/bin/creator/meters_gen.rs` recommended but not normative) | Browser side has no rasteriser — `<img src=...>` handles SVG. |
| Per-skin generated Rust source | `<consumer-crate>/src/meters_assets_generated/<skin_id>.rs` (e.g. inside the example crate or a downstream BSP) | None — TS does not need codegen; bundler does the equivalent at build time. |
| TS bundler asset wiring | `audio-meters-widgets/ts/<bundler-config>` (per-bundler; recipes in `audio-meters-widgets/ts/README.md`) | None — Rust does not need a bundler; codegen does the equivalent. |
| Asset-aware draw path (Rust) | Each widget's `draw()` method in `widgets/src/meters/{bargraph,needle,numeric,lufs_gauge}.rs` | TS-side widget cores in `audio-meters-widgets/ts/src/*-core.ts`. |
| Asset-aware draw path (TS) | Each widget's `draw()` method (`*-core.ts`) | Rust widgets, same INV-A5 bit-identical state requirement. |
| `MeterColor` palette → asset tint mapping | This doc §7 | Rasteriser tints `led_segment_on_png` per zone at build time; runtime widget uses the appropriate pre-tinted variant per segment. Browser side does the equivalent via CSS `filter: hue-rotate` or per-zone pre-baked PNGs (bundler choice). |
| Alternate-look skin variant set ("modern flat", "vintage analog") | `assets/audio-meters/skins/*.json` (new files) | Validators on both runtimes (already shipped AM-04a). |

INV-T1 ("two-runtime visual parity at the segment-state level") is
enforced by the same parity-fixture pattern that anchors L0
ballistics (`audio-meters-core/fixtures/`). See §12 (c) for the
acceptance gate.

## §5 — Frozen decision: `AssetSlot` enum

Registration policy: **Standards Action** (per `CLAUDE.md`
§ "Frozen enumerations — registration policy"). Adding a slot
ripples through both runtimes' `SkinAssets` structs, the JSON
schema, the rasteriser, and every consuming widget's draw path —
it MUST ratify here with a §15 amendment before any execution PR
lands.

| Identifier | Source format(s) | Target format(s) on Rust runtime | TS handling |
|---|---|---|---|
| `led_segment_on_png` | PNG (8-bit-per-channel); SVG MAY be authored and rasterised to PNG at build time | RLE / RGB565 / RGBA8 / PNG (consumer choice via subcommand flags) | PNG via `fetch` + `<img>`; SVG via `fetch` + `<img>` natively |
| `led_segment_off_png` | PNG (8-bit-per-channel); SVG MAY be authored and rasterised | Same as `led_segment_on_png` | Same as `led_segment_on_png` |
| `needle_svg` | SVG (plain; no script, no foreign-object) | RLE / RGB565 / RGBA8 pre-rasterised at the **maximum expected** widget size; widget downscales at blit time (§5 INV-A3) | SVG via `fetch` + `<img>`; rotated at runtime via `canvas.rotate()` |
| `bezel_svg` | SVG (plain) | Same as `needle_svg` | Same as `needle_svg` |
| `faceplate_png` | PNG (8-bit-per-channel); SVG MAY be authored | Same as `led_segment_on_png` | PNG via `fetch` + `<img>`; SVG variant via `faceplate_svg` slot (TS-only) |

The TS interface additionally permits `faceplate_svg` (already
shipped in `audio-meters-widgets/ts/src/skin.ts:95`). On the Rust
side the rasteriser folds an authored `faceplate_svg` source into
the `faceplate_png` runtime slot at build time; both runtimes end
up with one faceplate per skin (INV-A4 set-equality at the
*rendered-pixel-slot* level).

### Invariants

- **INV-A1** — SVG sources MUST be plain SVG: no `<script>`, no
  `<foreignObject>`, no XLink to remote URIs, no external `<image
  href=...>` references. The rasteriser MUST reject sources that
  violate this and MUST exit non-zero with the offending file
  named. Rationale: build-time trust boundary; the rasteriser
  runs with developer privileges, and an SVG-with-script in a
  third-party skin pack is a code-execution vector.

- **INV-A2** — PNG sources MUST be 8-bit-per-channel (sRGB or no
  colour-profile). 16-bit-per-channel sources MUST be rejected
  with an explanatory error. Rationale: the embedded target's
  decoder paths assume 8bpc; silently downsampling is a footgun.

- **INV-A3** — Asset rasterisation target sizes derive from the
  **consuming widget's bounds at draw time**, not from the skin
  JSON. Skins declare *which* slot is populated; widgets pass
  bounds at draw time. Build-time rasterisation produces an
  asset at the **maximum expected size** (the rasteriser default
  is a per-widget-family ceiling: 96×384 px for bargraph LED
  segments, 512×512 for needle / bezel, 1024×512 for faceplate;
  subcommand flags override). The runtime widget MAY downscale
  via `fill_rect`-tiled-blit or platform-specific blit; it MUST
  NOT upscale. Upscale is rejected at draw time with a fallback
  to procedural rendering for that frame (no panic on embedded).

- **INV-A4** — The `AssetSlot` set MUST stay set-equal across
  Rust and TS at the **rendered-pixel-slot** level (five slots:
  `led_segment_on_png`, `led_segment_off_png`, `needle_svg`,
  `bezel_svg`, `faceplate_png`). TS MAY additionally accept
  `faceplate_svg` as an author-time alternate source for the
  `faceplate_png` rendered slot; this is the only permitted
  asymmetry. Adding a slot is Standards Action (§15 amendment
  first). A parity test (`tests/asset_slots_match_ts.rs` or
  similar — name owned by execution PR) MUST enforce the
  set-equality.

- **INV-A5** — Asset-aware and procedural draw paths MUST be
  **bit-identical at the visual-state layer**. Specifically,
  for the same skin, scale, and `(dbfs, dt)` input sequence:
  same number of lit cells (bargraph), same needle angle
  (needle), same formatted text and peak-hold value
  (numeric / LUFS gauge), same zone-colour assignment per cell.
  The asset path only changes **pixel sourcing**, not state
  computation. Enforcement: shared draw-op recorder in widget
  unit tests verifies the count-and-position of segment blits
  against the count-and-position of `fill_rect` calls in the
  procedural reference (§12 (c)).

- **INV-A6** — Runtime SVG parsing in `no_std` Rust is **out of
  scope** (§11 non-goal #2). All SVG handling happens at build
  time on the host. The runtime sees only pre-rasterised byte
  slices in a format the platform already understands (RLE via
  `rlvgl-decomp`, RGB565 strip, or PNG via the platform's PNG
  decoder).

- **INV-A7** — Cross-runtime visual parity at the segment-state
  level (i.e. *which* slot is consulted per segment, not exact
  pixel values; pixel-exact parity is impossible because browser
  rasterisers differ from `tiny-skia`). The parity fixture
  pattern from L0 ballistics extends to widget draw paths via
  a shared "draw-op JSON" recorded by both runtimes and compared
  in CI.

- **INV-A8** — JSON `assets` block format: filenames are
  resolved relative to the **skin file's parent directory**
  (i.e. `assets/audio-meters/skins/`), and a leading path
  component MAY be `../svg/`, `../png/`, or any path within the
  `assets/audio-meters/` tree. The rasteriser MUST reject paths
  that escape `assets/audio-meters/` (no `..` past the root, no
  absolute paths). Rationale: build-time trust boundary; skin
  packs MUST be portable across machines.

- **INV-A9** — Alternate-look skin variants MUST ship as
  **complete skin files** under `assets/audio-meters/skins/`,
  not as deltas over existing skins. Each is a separate JSON
  document validated by the same `skin.schema.json`. Rationale:
  variant inheritance / delta-over-base would require a second
  schema concept that the audio-meters initiative has explicitly
  not adopted; one-skin-one-file keeps the loader logic
  identical between asset-bearing and procedural variants.

## §6 — Frozen decision: rasteriser CLI surface

Registration policy: **Specification Required** (per-chapter
walkthrough update for new flags; no §15 amendment unless the
subcommand name or its required-args set changes).

The CLI surface mirrors the existing `bsp from-yaml` pattern in
[`src/bin/creator/cli.rs:614`](../../src/bin/creator/cli.rs) so
that the operator experience is consistent across creator
subcommands.

```text
rlvgl-creator meters from-yaml <yaml> [options]

Required:
  <yaml>                 Path to a meters-assets manifest YAML file.

Options:
  --out <dir>            Output directory for generated Rust source.
                         Default: `./src/meters_assets_generated/`.
  --format <fmt>         Target runtime format: rle | rgb565 | rgba8 | png.
                         Default: rle.
  --max-size <wxh>       Override the default rasterisation ceiling
                         (see §5 INV-A3). Format: `<w>x<h>`.
  --skin <id>            Restrict to one skin id (default: all skins
                         listed in the manifest).
  --include-skin-const   In addition to `<SKIN_ID>_ASSETS`, emit a
                         `<SKIN_ID>` const with `assets:` wired to
                         the generated `SkinAssets` constant.
                         Default: off (compose at app level).
```

The manifest YAML schema is owned by AM-04b (Specification
Required). A starter form:

```yaml
# assets/audio-meters/meters_assets.yaml — example only
skins:
  - id: vintage_analog_vu_needle
    format: rle
  - id: modern_flat_digital_bargraph
    format: rgba8
    max_size: 128x512
```

Subcommand dispatch wiring goes into the existing `Command` enum
([`src/bin/creator/cli.rs:103`](../../src/bin/creator/cli.rs)) as
a new `Meters { cmd: MetersCommand }` arm; the execution PR owns
the exact module / function names. Conditional compilation under
the `creator` cargo feature is required so the rasteriser's
host-only dependencies (see §6.1) do not pollute the embedded
build profile.

### §6.1 — Rasteriser dependency budget

TBD: confirm `no_std` compatibility, license, and host-tool /
embedded split for each candidate before adopting. Candidates
mentioned in adjacent rlvgl-creator code paths:

- **`resvg` / `usvg`** — SVG parsing + rendering. TBD: license
  (currently MPL-2.0 last I checked); host-only acceptable.
- **`tiny-skia`** — software rasteriser used by `resvg`. TBD:
  confirm host-tool acceptability; not needed at runtime since
  INV-A6 forbids runtime SVG parsing.
- **`image`** — PNG / JPEG decode + encode. TBD: confirm
  `cargo doc` clean, no `pulldown-cmark` collision; already used
  elsewhere in `rlvgl-creator` so likely fine.
- **`rlvgl-decomp`** — existing RLE encoder/decoder. Reused
  unmodified.

The execution PR owns the final dependency choice and pins
versions in the workspace `Cargo.toml`. This concepts doc does
not pin a specific crate.

## §7 — Frozen decision: palette / asset tint relationship

Registration policy: **Standards Action**.

The procedural draw path looks up a per-segment zone colour from
the bound `Scale.zones` (`MeterColorId::{Safe,Nominal,Caution,Hot,Over}`)
and resolves the identifier to a concrete `Color` via
`Skin.palette` (already shipped, AM-00 §7 + AM-04a §"palette").

The asset-aware draw path has two acceptable tinting strategies;
both runtimes MUST agree per-skin on which one the skin's
`led_segment_on_png` is intended for. The mode is **implicit in
the asset content**, not declared in JSON:

- **Mode T1: per-zone pre-baked PNGs.** The skin author / the
  rasteriser pipeline produces one `led_segment_on_<zone>.png`
  per zone identifier, packed into a single sprite-sheet PNG.
  The widget selects the per-zone region of the sheet at draw
  time. The `led_segment_on_png` slot points at the sheet; the
  layout is the rasteriser-defined sprite-sheet convention.

- **Mode T2: greyscale-with-runtime-tint.** The skin's
  `led_segment_on_png` is a greyscale mask; the widget tints it
  per segment using `Skin.palette.color(zone_id)` at draw time
  (multiply-blend on RGB565 / RGBA8; LUT-based tint on RLE). The
  TS browser side does the equivalent via `canvas` global
  composite-operation or pre-baked-per-paint variants.

The choice is per-skin; AM-04b's initial 2-3 variants MAY use
either, but each variant MUST document its choice in its skin
JSON's top-level free-form `description` field (already
permitted by the schema as an unenforced informational field).
Bit-identical-visual-state (INV-A5) is satisfied by either mode
because **lit-cell count** is independent of tint mode.

A future amendment MAY promote the tint mode to a declared
JSON field if maintenance burden grows.

## §8 — Frozen decision: widget-draw-path branching shape

Registration policy: **Standards Action**.

For each of the four widgets, the `draw()` method gains a
single conditional at the top:

```rust
// Pseudocode — owned by execution PR.
fn draw(&self, renderer: &mut dyn Renderer) {
    if self.skin.assets.any_present() {
        self.draw_asset_aware(renderer);
    } else {
        self.draw_procedural(renderer);
    }
}
```

`draw_procedural()` is the current as-of-AM-04a body, extracted
without semantic change. `draw_asset_aware()` is new and
implements the slot-blit sequence per widget family (see §10
for the per-widget contract).

Constraints:

- **INV-W1** — Neither branch reads from nor writes to widget
  ballistic / peak-hold / reading state. Both branches are pure
  functions of `(self.bounds, self.skin, self.reading_db,
  self.peak_db)` (and `self.peak_age_s` for bargraph /
  numeric). State update lives in `update()` (concepts §9).

- **INV-W2** — The asset-aware branch MUST NOT panic on a
  non-fatal asset error (missing slot, oversize bounds — INV-A3
  upscale rejection). On `no_std` targets it falls back to the
  procedural branch for that frame; on host it MAY additionally
  emit a `debug_assert!` or log entry, but never a panic.

- **INV-W3** — No `unsafe` is introduced into widget draw
  paths as part of AM-04b. The runtime asset format
  (`&'static [u8]`) is interpreted via existing safe decoders
  (`rlvgl-decomp` for RLE, the platform's PNG decoder for PNG,
  `core::slice::from_raw_parts` is not used). §12 (g) gates this.

## §9 — Frozen decision: TS loader pattern

Registration policy: **Specification Required**.

The TS side does not get a rasteriser. The browser handles SVG
natively via `<img>` and `<canvas>`; PNG decoding is built-in.
The pattern is:

1. The `<rlvgl-led-bargraph>` (and friends) custom element loads
   skin JSON via `fetch(src-skin)` (already shipped, AM-05/06).
   The new behaviour: if the parsed skin has a non-empty
   `assets` block, the element resolves each filename against
   a bundler-configured root.

2. Bundler-root resolution. The element accepts an optional
   `asset-root` attribute (`<rlvgl-led-bargraph asset-root="/assets/audio-meters/">`),
   defaulting to the skin file's directory. The resolved URL is
   passed to `fetch` and the returned blob is decoded by the
   browser into an `ImageBitmap` (or `<img>` for SVG that
   needs CSS-driven rotation).

3. The widget's `*-core.ts` draw method gains the same
   conditional shape as the Rust widget (§8): if any asset slot
   is non-empty, take the asset-aware path; else procedural.
   The asset-aware path uses `canvas.drawImage` /
   `canvas.rotate` / `canvas.globalCompositeOperation` per §7.

4. Bundler integration is **out-of-band** — the audio-meters-widgets
   library does not depend on a specific bundler. The library's
   README documents three recipes (webpack `asset/resource`,
   vite `?url`, rollup `@rollup/plugin-url`), but the runtime
   accepts URLs from any source.

5. Demo: `audio-meters-widgets/ts/demo/` (already shipped for
   AM-05/06/07/08d) gains at least one new HTML example
   exercising an asset-bearing skin (§12 (d)).

## §10 — Reconciliation with adjacent layers

| Adjacent artifact | Reconciliation |
|---|---|
| [`04-skins.md`](04-skins.md) — Skin descriptor schema | **No schema change.** The `assets` block has been declared in `skin.schema.json` since AM-04a. AM-04b authors files that populate it. AM-04a's §"Non-goals (deferred to AM-04b)" list is closed by this chapter. |
| [`05-led-bargraph.md`](05-led-bargraph.md) — `LedBargraph` widget | Gains §8 conditional in `draw()`. Asset-aware branch: per cell, if `led_segment_on_png` (lit) / `led_segment_off_png` (unlit) is populated, blit the slot at the segment rect; else `fill_rect` per current procedural path. Peak-hold pip uses `peak_hold` colour (procedural) over the lit-state asset, since AM-04b does not introduce a peak-pip asset slot. AM-08b ticks path is unchanged (ticks are rendered via `Renderer::fill_rect` + `Renderer::draw_text`; tick assets are out-of-scope §11). |
| [`06-needle-vu.md`](06-needle-vu.md) — `NeedleVu` widget | Gains §8 conditional. Asset-aware branch: blit `faceplate_png` at `self.bounds` (replaces background fill), then blit rotated `needle_svg` (the SVG was pre-rasterised at build time per §5 INV-A3; rotation at draw time uses the platform's blit-with-rotation hook or a software fallback). Pivot dot is drawn procedurally on top. Tick rendering (when `show_ticks` is enabled) remains procedural. |
| [`07-numeric-peak.md`](07-numeric-peak.md) — `NumericPeak` widget | Gains §8 conditional. Asset-aware branch: optional `bezel_svg` overlay around the text region. Text rendering remains procedural (no glyph asset slot in AM-04b; future amendment MAY add). The numeric format itself is unchanged — INV-A5 requires identical text output. |
| [`11-lufs-gauge.md`](11-lufs-gauge.md) — `LufsGauge` compound widget | Same pattern as `NumericPeak`: optional `bezel_svg` overlay; text rendering remains procedural; three-line layout unchanged. INV-A5 requires identical `last_m` / `last_s` / `last_i` reading values and zone-colour assignments. |
| [`08-ticks-labels.md`](08-ticks-labels.md) — Tick / label rendering | Unchanged. Ticks and labels are not part of the AM-04b asset slot set. §11 explicitly defers tick assets to a future amendment. |
| [`09-stereo.md`](09-stereo.md) — Stereo composition | Unchanged. `StereoPair<W: MeterWidget>` is asset-mode-transparent — it composes whatever the inner widget draws, asset-aware or procedural. |
| [`13-asset-hooks.md`](13-asset-hooks.md) — AM-04b-stub | This chapter ratifies the stub's §"What's next" list. The stub's `SkinAssets` type, `Skin.assets` field, and `SkinAssets::any_present()` helper are reused without modification. The stub's "stub-by-design" non-goals (real SVG/PNG, rasteriser, asset-aware draw paths, additional presets) are exactly the AM-04b deliverable. The execution PR that closes AM-04b SHOULD update `13-asset-hooks.md`'s chapter title / change log to reference AM-04b as the closer (but this concepts doc does not perform that edit — it is an execution-PR action). |
| [`rlvgl-creator`](../creator/CLI.md) — Creator CLI | New `meters from-yaml` subcommand. CLI surface frozen in §6; manifest YAML schema is Specification Required. Wiring goes into `src/bin/creator/cli.rs` as a `Command::Meters { cmd: MetersCommand }` arm, conditional under the `creator` cargo feature. |
| [`rlvgl-decomp`](../../rlvgl-decomp/README.md) — RLE decoder | Reused unmodified. RLE is one of the runtime asset formats (`--format rle` on the subcommand). |
| TS runtime (`audio-meters-widgets/ts/`) | New asset loader pattern (§9). No new build-time tools required; bundler config is per-bundler and documented in the library README. |
| Art-direction (palettes, art style, vintage faceplate imagery, modern-flat geometry) | `Owned by AM-04b[execution-letter]` — the initiative owner decides per execution PR. This concepts doc fixes only the count of alternate-look variants (≥3 — §5 INV-A9) and that they ship as complete skin files. |

## §11 — Non-goals

The following are explicitly out of scope for AM-04b. Each is
classified per `CLAUDE.md` § "Initiative retrospective" §5
("Deferred work reclassification") as **Coupled** (re-openable
once the named assumption is revisited) or **Abandoned**
(deliberately killed, with resurrection-prevention note).

- **GLSL / shader-based draw paths.** Coupled. Couples to the
  Renderer trait surface; would require a `Renderer::draw_with_shader`
  hook that doesn't exist. Re-open when the renderer grows
  GPU-shader support (probably never on embedded; possibly on
  Tauri / host targets).

- **Runtime SVG parsing in `no_std`.** Abandoned (anchors INV-A6).
  The embedded targets do not have the working memory or the
  decoder maturity. SVG rasterisation is build-time-only. Resurrection
  prevention: any future "but tiny-skia is no_std now" pitch MUST
  also account for typical SVG complexity in real skin packs
  (gradients, masks, filters) which dwarfs the `no_std` decoder
  surface; revisit only with a fresh memory budget and a worked
  example.

- **Animated SVG / SMIL / Lottie.** Coupled. The audio-meters
  widgets already animate procedurally at the ballistic time
  constant; layering a second animation source would require
  reconciling two time-domain pipelines. Re-open when there's a
  specific user demand (e.g. an analog meter with a needle-overshoot
  visual flourish).

- **Per-frame asset blending.** Abandoned. Crossfading between
  two skins at runtime is a designer-UI concern, not a metering
  concern. Resurrection prevention: do this in app code with two
  widgets and a fade overlay; the meter widget itself stays
  single-skin.

- **Multi-resolution asset packs (one rasterisation per build).**
  Coupled. The rasteriser produces one asset per slot per skin
  per build invocation; the consumer chooses the target size via
  `--max-size`. A future amendment MAY add multi-resolution
  output (e.g. 1× / 2× / 4× variants) if a Retina-class display
  use case emerges.

- **3D bezel rendering / pseudo-3D lighting effects.** Coupled.
  The faceplate / bezel slots are 2D pre-rasterised assets;
  any pseudo-3D look comes from the **art** in the SVG / PNG,
  not from runtime lighting math. Re-open if a 3D-capable
  renderer trait surface lands.

- **Tick / label asset slots.** Coupled. Ticks and labels
  currently render via `Renderer::fill_rect` and
  `Renderer::draw_text`. A `tick_strip_png` slot is a plausible
  amendment but is **not** part of AM-04b's slot set. Re-open
  when one of the alternate-look variants demonstrably needs
  tick artwork that procedural rendering cannot match (e.g. a
  hand-painted scale arc on a vintage analog skin).

- **Per-asset accessibility (alt text, contrast variants,
  high-contrast palettes).** Coupled. A separate future
  AM-NN chapter ("Audio meters accessibility") would own this.
  AM-04b's §14 explicitly unblocks that future chapter.

- **Vendor brand-pack support (BBC, Dolby, SSL, Studer
  faceplates).** Coupled to licensing. AM-04b ships generic
  alternate looks under the repo's existing license; vendor
  packs go in a separate distribution outside this repo.

> **Likely-to-be-demoted candidates.** The four-non-goals tagged
> *Coupled* above are the items the initiative owner is most
> likely to want re-opened — particularly **tick / label asset
> slots** (cosmetic continuity with vintage-analog variants),
> **multi-resolution asset packs** (Retina / 2× / 4× DPI), and
> **accessibility variants**. The §11 wording is deliberately
> "deferred / re-openable", not "abandoned", for those three.

## §12 — Acceptance checklist

A conforming AM-04b deployment MUST satisfy **all** of the
following. The execution PR set is expected to be lettered
(`AM-04b-a`, `AM-04b-b`, ...) and individual checklist items
MAY ship in sequence; the chapter is ratified only when all
items below are green.

- [ ] (a) **Rasteriser subcommand wired.** `cargo run --features
      creator --bin rlvgl-creator -- meters from-yaml <skin>.yaml
      --out <dir>` produces `pub static <SKIN_ID>_ASSETS:
      SkinAssets = SkinAssets { led_segment_on_png:
      Some(include_bytes!(...)), ... };` and (with
      `--include-skin-const`) wires it into a `<SKIN_ID>: Skin`
      whose `assets:` field references the generated constant.

- [ ] (b) **At least one preset migrated.** At least one preset
      under `widgets/src/meters/presets.rs` (or a new generated
      sibling file) flips from `assets: SkinAssets::EMPTY` to a
      populated `SkinAssets` literal embedding rasterised PNG
      bytes via `include_bytes!`. The original procedural preset
      MAY remain as a separate skin id; the migration is
      **additive** to the AM-04a preset set, not a replacement.

- [ ] (c) **Per-widget asset-aware unit tests.** Each of the
      four widgets (`bargraph`, `needle`, `numeric`,
      `lufs_gauge`) has at least one unit test that drives a
      synthetic dBFS sequence and renders to a host-side
      framebuffer (or `RecordingRenderer`-style draw-op recorder
      — pattern used in
      [`widgets/src/meters/bargraph.rs:325`](../../widgets/src/meters/bargraph.rs))
      with the asset-aware path active. Output compares against
      a committed golden artefact (PNG checked into the repo, or
      a JSON draw-op trace — execution PR picks the pattern).
      INV-A5 (bit-identical state) verified by re-running the
      same input against the procedural path and asserting
      identical lit-cell count / needle angle / numeric text.

- [ ] (d) **TS demo: asset-bearing skin.** At least one custom-
      element demo under `audio-meters-widgets/ts/demo/` (or
      `examples/`) renders an asset-bearing skin and is exercised
      by the existing `npm run demo` workflow. Visual smoke is
      manual; structural smoke (asset URL fetched, image element
      mounted) is automated in the headless test suite.

- [ ] (e) **2-3 alternate-look skin variants.** At least three
      complete skin files under `assets/audio-meters/skins/`,
      named for their look (suggested forms: `modern_flat_*`,
      `vintage_analog_*`). Each is a complete JSON document
      validated by the existing `skin.schema.json` validator on
      both runtimes. Each MUST exercise at least one of the
      `AssetSlot` enum values being non-empty. The set MUST
      include at least one bargraph variant and at least one
      needle variant.

- [ ] (f) **Cross-runtime slot-set parity.** The
      `widgets/src/meters/skin.rs::SkinAssets` and
      `audio-meters-widgets/ts/src/skin.ts::SkinAssets` types
      remain set-equal on the rendered-pixel-slot level (the
      five-slot set: `led_segment_on_png`, `led_segment_off_png`,
      `needle_svg`, `bezel_svg`, `faceplate_png`). The TS-only
      `faceplate_svg` author-time alternate is permitted (INV-A4).
      A test enforcing this asserts at CI time.

- [ ] (g) **No `unsafe` in widget draw paths.** AM-04b
      introduces no new `unsafe` blocks in
      `widgets/src/meters/*.rs`. The rasteriser's host-side code
      MAY use `unsafe` for performance (image decode primitives),
      but the embedded-target draw paths stay safe. Phase 2.5 of
      `/pre-publish` does not currently scan `widgets/`, so this
      gate is enforced by execution-PR review against
      `git diff --stat widgets/src/meters/`.

- [ ] (h) **Schema unchanged.** `assets/audio-meters/schema/skin.schema.json`
      is **not** modified by AM-04b execution PRs. The schema
      already declares the `assets` block since AM-04a. Any
      schema modification would be a §15 amendment first per
      Standards Action.

- [ ] (i) **`13-asset-hooks.md` reclassified.** A subsequent
      execution PR (after AM-04b lands fully) MAY update
      `13-asset-hooks.md` to mark the stub as superseded and
      add a "see AM-04b" pointer. This concepts doc does
      **not** perform that edit; it is an execution-PR action.

## §13 — Files cited

Authoritative artifacts referenced by this concepts doc. Paths
are relative to the repo root.

- [`CLAUDE.md`](../../CLAUDE.md) — § "Spec-Before-Code Planning
  Discipline", § "Initiative retrospective" (for §11
  classification), § "Frozen enumerations — registration policy"
  (for §5 Standards Action).
- [`docs/audio-meters/README.md`](README.md) — initiative
  landing page; chapter list with AM-04b deferred.
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) — §0-§9
  ratified vocabulary that AM-04b extends.
- [`docs/audio-meters/04-skins.md`](04-skins.md) — §"Non-goals
  (deferred to AM-04b)" closed by this chapter.
- [`docs/audio-meters/05-led-bargraph.md`](05-led-bargraph.md) —
  first widget gaining an asset-aware branch.
- [`docs/audio-meters/06-needle-vu.md`](06-needle-vu.md) —
  needle widget asset branch (faceplate + rotated needle).
- [`docs/audio-meters/07-numeric-peak.md`](07-numeric-peak.md) —
  numeric widget asset branch (optional bezel overlay).
- [`docs/audio-meters/08-ticks-labels.md`](08-ticks-labels.md) —
  ticks remain procedural (§11 non-goal).
- [`docs/audio-meters/11-lufs-gauge.md`](11-lufs-gauge.md) —
  LUFS gauge asset branch (optional bezel overlay).
- [`docs/audio-meters/13-asset-hooks.md`](13-asset-hooks.md) —
  AM-04b-stub; §"What's next" is the inherited contract.
- [`docs/concepts/DCB-00-CONCEPTS.md`](../concepts/DCB-00-CONCEPTS.md) —
  §0-§15 reference shape.
- [`assets/audio-meters/schema/skin.schema.json`](../../assets/audio-meters/schema/skin.schema.json) —
  canonical JSON schema; `assets` block already declared.
- [`widgets/src/meters/skin.rs`](../../widgets/src/meters/skin.rs) —
  runtime `SkinAssets`, `Skin.assets`, `SkinAssets::EMPTY`,
  `SkinAssets::any_present`.
- [`widgets/src/meters/presets.rs`](../../widgets/src/meters/presets.rs) —
  preset constants; (b) target.
- [`widgets/src/meters/bargraph.rs`](../../widgets/src/meters/bargraph.rs) —
  `LedBargraph::draw` at line 151.
- [`widgets/src/meters/needle.rs`](../../widgets/src/meters/needle.rs) —
  `NeedleVu::draw` at line 130.
- [`widgets/src/meters/numeric.rs`](../../widgets/src/meters/numeric.rs) —
  `NumericPeak::draw` at line 125.
- [`widgets/src/meters/lufs_gauge.rs`](../../widgets/src/meters/lufs_gauge.rs) —
  `LufsGauge::draw` at line 148.
- [`audio-meters-widgets/ts/src/skin.ts`](../../audio-meters-widgets/ts/src/skin.ts) —
  TS `SkinAssets` interface, line 89.
- [`audio-meters-widgets/ts/src/led-bargraph-core.ts`](../../audio-meters-widgets/ts/src/led-bargraph-core.ts) —
  TS bargraph draw core; asset-aware branch target.
- [`audio-meters-widgets/ts/src/needle-vu-core.ts`](../../audio-meters-widgets/ts/src/needle-vu-core.ts) —
  TS needle draw core.
- [`audio-meters-widgets/ts/src/numeric-peak-core.ts`](../../audio-meters-widgets/ts/src/numeric-peak-core.ts) —
  TS numeric draw core.
- [`audio-meters-widgets/ts/src/lufs-gauge-core.ts`](../../audio-meters-widgets/ts/src/lufs-gauge-core.ts) —
  TS LUFS gauge draw core.
- [`src/bin/creator/cli.rs`](../../src/bin/creator/cli.rs) —
  CLI dispatch; `Command::Bsp { cmd: BspCommand }` is the
  pattern to mirror at line 614.
- [`src/bin/creator/bsp_gen.rs`](../../src/bin/creator/bsp_gen.rs) —
  existing generator; AM-04b rasteriser slots next to it.
- [`docs/creator/CLI.md`](../creator/CLI.md) — creator CLI
  reference; `meters from-yaml` to be documented here at
  AM-04b execution time.
- [`rlvgl-decomp/README.md`](../../rlvgl-decomp/README.md) —
  RLE decoder; reused unmodified.

## §14 — Unblocks

Ratification of this doc unblocks:

- **AM-04b execution PRs** (`AM-04b-a:` through `AM-04b-<n>:`)
  — rasteriser subcommand, asset-aware widget draw paths, TS
  loader, alternate-look skin variants. Closing all §12
  acceptance items closes AM-04b and the entire audio-meters
  initiative's `v0.5 open-work list` (per user-memory
  `project_app_schema_status.md` and
  `project_audio_meters_architecture.md`).
- **Future AM-NN: accessibility variants.** The §11 deferred
  "per-asset accessibility (alt text, contrast variants)"
  becomes addressable once the asset slot mechanism is in
  place. Authors can ship a `*_highcontrast` skin variant
  with its own asset set without further infrastructure work.
- **Future AM-NN: tick / label asset slots.** §11 deferred but
  re-openable. The Standards Action gate on `AssetSlot`
  ensures this is a controlled extension.
- **Audio-meters retrospective.** Closing AM-04b satisfies the
  "every named phase shipped or closed-with-deferral"
  trigger in `CLAUDE.md` § "Initiative retrospective". The
  retrospective lives at
  `docs/audio-meters/AM-RETROSPECTIVE.md` (file not yet
  created — also an execution-PR action), capturing the
  spec-vs-implementation divergences from this five-item
  pass and feeding §1-§7 lessons forward.

## §15 — Change log

- **2026-05-11 — Ratified (owner: Ira Abbott).** Doc *shape*
  ratified. `AM-04b[a-z]:` execution PRs MAY now cite
  §-numbers as frozen authority. Art direction, palette
  choices, and concrete variant identifiers remain
  `Owned by AM-04b[execution-letter]` — execution-PR
  decisions by the initiative owner, not ratified here.
  Five cross-doc edits recommended-but-not-performed in the
  drafting pass (stub-supersede pointer in `13-asset-hooks.md`,
  footnote in `04-skins.md`, README chapter-table flip,
  `docs/creator/CLI.md` `meters from-yaml` stub, forward-pointers
  in 05/06/07/11) are unblocked for follow-up.

- **2026-05-11 — DRAFT — awaiting ratification.** Initial
  concepts doc for AM-04b (aesthetics pass proper). Replaces
  the AM-04b-stub deferral in `13-asset-hooks.md` with a
  ratifiable spec covering: SVG / PNG `AssetSource` sources,
  the build-time `MeterRasterizer` shipped as
  `rlvgl-creator meters from-yaml`, asset-aware widget draw
  paths on both runtimes (Rust + TS) gated on
  `SkinAssets::any_present()`, the TS bundler-driven loader
  pattern (no TS-side rasteriser), and ≥3 alternate-look skin
  variants ("modern flat", "vintage analog") shipped as
  complete skin files. Frozen decisions: `AssetSlot` enum
  (Standards Action, five-slot set + TS-only `faceplate_svg`
  author-time alternate), CLI surface
  (Specification Required), palette/tint mode duality
  (Standards Action, implicit-in-content selection between
  Mode T1 sprite-sheet and Mode T2 greyscale-runtime-tint),
  widget draw-path branching shape (Standards Action), and
  the TS loader pattern (Specification Required). Invariants:
  INV-A1 (no script/foreign-object SVG), INV-A2 (8bpc PNG),
  INV-A3 (widget bounds drive rasterisation size, not skin
  JSON), INV-A4 (set-equal slot set across runtimes,
  rendered-pixel-slot level), INV-A5 (bit-identical visual
  state across asset-aware and procedural branches), INV-A6
  (no runtime SVG parsing in `no_std`), INV-A7 (cross-runtime
  segment-state parity), INV-A8 (skin-local asset path
  resolution, no escape past `assets/audio-meters/`), INV-A9
  (alternate-look variants ship as complete skin files, not
  deltas), INV-W1-W3 (widget draw-path discipline: stateless
  branches, non-fatal asset errors, no new `unsafe`).
