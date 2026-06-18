<!--
LPAR-09-ASSET-FILESYSTEM.md — LVGL parity asset and filesystem sources concepts.
-->

# LPAR-09 — Asset and Filesystem Sources

**Status:** Ratified 2026-06-12. Normative for LPAR-09 asset and filesystem
source implementation.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Draw/image substrate:
[LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md).
Invalidation: [LPAR-03-INVALIDATION-DISPLAY.md](LPAR-03-INVALIDATION-DISPLAY.md).

## 0. Authority Policy

| Concern | Owner | LPAR-09 relationship |
|---|---|---|
| Existing `core::fs` asset abstraction | `core/src/fs.rs` (feature-gated `fs`) | `core/src/fs.rs` is the canonical definition of `FsError`, `BlockDevice`, `AssetError`, `AssetRead`, `AssetSource`, `AssetIter`, and `AssetManager`. LPAR-09 reconciles with and EXTENDS these types. It MUST NOT replace or break them. |
| `ImageData`/`ImageDescriptor`/`CacheHandle` | `core/src/image.rs` | LPAR-08 owns the descriptor and handle shape. LPAR-09 adds the reserved `#[non_exhaustive]` `ImageData::Asset` variant and the concrete eviction policy. Any addition to `ImageData` requires a LPAR-08 §15 amendment. |
| Existing decode plugins | `core/src/plugins/{png,jpeg,gif,apng,qrcode}.rs`, `core/src/plugins/fatfs.rs` | These plugins convert source bytes to pixel data. LPAR-09 owns source lookup; the plugins are CONSUMED by LPAR-09 after bytes are retrieved from a source, not replaced. |
| LVGL filesystem reference | `lvgl/src/misc/lv_fs.c`, `lv_fs.h` @ LPAR-01 §2 pin | Source reference for drive-letter semantics, `lv_fs_drv_t` registration, and `lv_image_src_t` source-type enum (`LV_IMAGE_SRC_VARIABLE`, `LV_IMAGE_SRC_FILE`, `LV_IMAGE_SRC_SYMBOL`). Rust API differs where documented. |
| LVGL image decoder reference | `lvgl/src/draw/lv_image_decoder.h` @ LPAR-01 §2 pin | Reference for the open/get-area/close decoder lifecycle and `lv_image_cache_data_t` cache key. LPAR-09 maps these concepts onto the Rust plugin model. |
| BlockDevice implementors | `platform/src/stm32h747i_disco_sd.rs` (`DiscoSdBlockDevice`), `fs-sim/src/lib.rs` (`SimBlockDevice`) | These are the two concrete `BlockDevice` implementations; both import from `rlvgl_core::fs`. The FATFS adapter (`platform/src/sd_fatfs_adapter.rs`) wraps a `&mut BlockDevice` for the `fatfs` crate. LPAR-09 MUST NOT break these call sites. |
| `no_std + alloc` contract | `core/` crate manifest, `core/src/lib.rs:12` | `core/` is `no_std` by default; the `fs` feature is a compile-time opt-in. All new `core/` types in LPAR-09 MUST compile under `no_std + alloc`. Host-filesystem / simulator sources MUST be gated behind `std` or a named feature. |
| Invalidation planner | `core/src/invalidation.rs`, LPAR-03 §7 | Deferred asset loads that change visible pixels (decoded image becomes available) MUST report dirty rects through `InvalidationList`. LPAR-09 introduces no second repaint path. |

If LPAR-09 changes a frozen decision in §5–§11, §15 MUST be amended first in
a separate docs change. The `ImageData::Asset` variant addition is a Standards
Action and requires a LPAR-08 §15 amendment filed, accepted, and merged before
any code that constructs `ImageData::Asset` lands.

## 1. Purpose

Define how assets — image files, font files, arbitrary binary resources — are
located, opened, loaded, and cached from the four source kinds that rlvgl
targets: embedded static bytes, FATFS/block-device, simulator (host filesystem),
and in-RAM memory buffers. This phase:

- Reconciles the existing `core::fs` asset abstraction with the LPAR-09 source
  model, extending it additively rather than replacing it.
- Adds the `ImageData::Asset` variant that LPAR-08 reserved as `#[non_exhaustive]`,
  allowing `ImageDescriptor` to hold a reference to a source-backed handle
  without eagerly decoding the full pixel buffer.
- Freezes the cache eviction policy that LPAR-08 deferred to this phase,
  including the `CacheHandle`-to-decoded-pixels mapping, slot-count bound, and
  LRU order.
- Establishes an LVGL-informed path/addressing model for the four source kinds
  without requiring runtime drive-letter dispatch when a typed enum suffices.
- States feature-gating rules and `no_std` / `std` boundaries clearly so
  embedded builds do not silently acquire filesystem dependencies.

LPAR-09 is a prerequisite for LPAR-12 (`ImageButton` needs a source-backed image
path), LPAR-15 (`Canvas`, `AnimImage`, `Lottie`/media needs full source pipeline),
and any widget that loads assets at runtime rather than linking them as statics.

## 2. Problem Statement

Evidence in the current tree:

### 2.1 `core::fs` asset abstraction exists but has no consumers above `BlockDevice`

`core/src/fs.rs` (the `fs` feature gate) defines `FsError`, `BlockDevice`,
`AssetError`, `AssetRead`, `AssetSource`, `AssetIter`, and `AssetManager<S>`.
Grep for `AssetSource`, `AssetManager`, `AssetRead` outside `core/src/fs.rs`
returns zero hits. The abstraction was built correctly but has never been
wired to a concrete source implementation above the `BlockDevice` level.

`platform/src/sd_fatfs_adapter.rs:18` imports only `BlockDevice` from
`rlvgl_core::fs`. `fs-sim/src/lib.rs:12` imports only `BlockDevice` and
`FsError`. Neither crate creates an `AssetSource` or an `AssetManager`.

The practical asset path in the current tree is: `include_bytes!` for embedded
static assets (found in `examples/stm32h747i-disco/assets/disco-assets/src/lib.rs`,
`examples/stm32h747i-disco/src/main.rs`, `examples/stm32h747i-disco/src/star_crawl.rs`,
`platform/src/pixels_renderer.rs`, `platform/src/blit.rs`, and elsewhere),
and free functions in `core/src/plugins/fatfs.rs` (`list_dir`, `file_exists`,
`read_file`, `read_file_range`) operating directly on a `Read + Write + Seek`
image — bypassing `AssetSource` entirely. The `AssetSource` trait was intended
to sit above these but has never been instantiated.

LPAR-09 is primarily a **reconciliation and wiring** phase: the plumbing
exists at the bottom (`BlockDevice`, `FatfsBlockStream`, `SimBlockDevice`,
`mount_and_list_assets`); the upper connection to `AssetSource` and the bridge
into `ImageDescriptor` are what is missing.

### 2.2 No `ImageData::Asset` variant bridges source lookup to the image pipeline

`core/src/image.rs` defines `ImageData` as `#[non_exhaustive]` with three
variants: `Borrowed(&[u8])`, `BorrowedColors(&[Color])`, `Owned(Vec<u8>)`.
The LPAR-08 spec (§5.G, §8 conflict table, §9 registration policy) explicitly
states: "Adding `AssetHandle` is owned by LPAR-09 and requires a §15 amendment
to LPAR-08." As of ratification of LPAR-08, no `AssetHandle`/`Asset` variant
exists. A widget that needs to blit a file-backed image cannot currently express
that through `ImageDescriptor` — it must either decode eagerly (heap spike) or
use a parallel path that bypasses the descriptor.

### 2.3 Cache eviction policy is undefined

`core/src/image.rs:232-243` defines `ImageCache<'a>` with `get`/`put`. No
concrete implementation exists. The cache slot count, eviction order (LRU,
FIFO), and behavior when an `AssetHandle`-backed entry is evicted (reload vs.
drop) are undefined. LPAR-08 §5.G explicitly deferred eviction to LPAR-09.

### 2.4 Simulator asset path handling is informal

`fs-sim/src/lib.rs` (`SimBlockDevice`) provides a `BlockDevice` over a host
file, enabling the FATFS path on the simulator. There is no `std`-backed
`AssetSource` that resolves a plain path string like `"fonts/DejaVuSans.ttf"`
to a host filesystem file, which would allow simulator builds to load assets
dynamically without pre-building a FAT image. The disco-assets build.rs
(`examples/stm32h747i-disco/assets/disco-assets/build.rs:30`) auto-generates
`include_bytes!` constants as a workaround, but this approach bakes files into
the binary rather than loading them at runtime.

### 2.5 No LVGL-like path addressing model

LVGL uses drive letters (`A:`, `S:`) to dispatch path strings to registered
`lv_fs_drv_t` drivers (`lvgl/src/misc/lv_fs.h:138`: "path beginning with the
driver letter (e.g. S:/folder/file.txt)"). LVGL's image widget accepts a `const
void *` source and resolves it through `lv_image_src_get_type`, yielding
`LV_IMAGE_SRC_VARIABLE` (C pointer to image descriptor), `LV_IMAGE_SRC_FILE`
(path string), or `LV_IMAGE_SRC_SYMBOL` (UTF-8 symbol string).

rlvgl has no analog. There is no typed addressing model that lets a widget
declare "I want image at path X from source Y" without either hard-coding the
source type or requiring a runtime string prefix.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **`AssetSource`** | As defined in `core/src/fs.rs:64`; used without modification. The canonical source trait over which LPAR-09 lays the path-addressing and source-kind model. | `core/src/fs.rs` (existing) |
| **`AssetRead`** | As defined in `core/src/fs.rs:49`; used without modification. Streaming byte reader with `read`, `seek`, `len`. | `core/src/fs.rs` (existing) |
| **`AssetManager<S>`** | As defined in `core/src/fs.rs:87`; extended. Typed loading helper that wraps an `AssetSource`. LPAR-09 adds image-load and font-load helpers to it. | `core/src/fs.rs` (existing), extended by LPAR-09 |
| **`BlockDevice`** | As defined in `core/src/fs.rs:22`; used without modification. Sector-based block storage; consumed by `FatfsBlockStream` to reach a FAT volume. | `core/src/fs.rs` (existing) |
| **Source kind** | One of four compile-time-distinct asset source categories: `Embedded`, `Fatfs`, `Simulator`, `Memory`. See §5.B. | LPAR-09 |
| **`AssetPath`** | A typed value that names an asset within a specific source kind: `AssetPath::Embedded(&str)`, `AssetPath::Fatfs(&str)`, `AssetPath::Sim(&str)` (std-gated), `AssetPath::Memory(&str)`. The drive-letter dispatch analog for Rust. See §5.B. | LPAR-09 |
| **`AssetHandle`** | An opaque token combining a `CacheHandle` (LPAR-08) and an `AssetPath`, representing a source-backed image that has been registered with the cache but whose decoded pixels may or may not be present. Added as `ImageData::Asset(AssetHandle)` variant. See §5.C. | LPAR-09 |
| **`ImageData::Asset`** | The reserved `#[non_exhaustive]` variant of `core::image::ImageData` that holds an `AssetHandle`. LPAR-08 mandated its existence as owned by LPAR-09; adding it requires a LPAR-08 §15 Standards Action amendment. | LPAR-09 (addition); LPAR-08 (registration gatekeeper) |
| **Embedded source** | Static asset bytes linked into the binary via `include_bytes!`. Lookup is a `match` on a `&str` symbol name against a static table generated at build time. Infallible once compiled; `AssetError` cannot occur. | LPAR-09 |
| **FATFS source** | Assets on a FAT volume reached through `BlockDevice` + `FatfsBlockStream` + `fatfs` crate. Fallible (device errors, file-not-found). `no_std + alloc`, gated behind the `fatfs` feature. | LPAR-09 |
| **Simulator source** | Assets on the host filesystem, resolved via `std::fs::File`. Fallible. `std`-only; gated behind a `sim` or `std` feature. Intended for simulator builds only. | LPAR-09 |
| **Memory source** | Assets in a statically or heap-allocated `&[(name, &[u8])]` map. Useful for in-test asset injection and for RAM-resident pre-loaded content. `no_std + alloc`. | LPAR-09 |
| **`ImageCache` concrete** | The concrete `no_std + alloc` implementation of `ImageCache<'a>` from LPAR-08: a fixed-size LRU ring buffer keyed by `CacheHandle`, evicting the least-recently-used decoded entry when full. | LPAR-09 |
| **Cache key** | The `(AssetPath, PixelFormat)` tuple identifying an entry in the image cache. Matches LVGL's `lv_image_cache_data_t` intent: cache key is the source identity, not the pixel address. | LPAR-09 |
| **Decode plugin boundary** | The point at which source bytes pass from LPAR-09's lookup path to a codec plugin (`png::decode`, `jpeg::decode`, etc.) that returns pixel data. LPAR-09 owns everything up to this boundary; the decode plugins are existing code consumed after the bytes are in hand. | LPAR-09 (source side); existing plugins (decode side) |
| **`lv_fs_drv_t`** | LVGL's per-driver registration struct (letter + callbacks); adapted here as an `AssetSource` trait object registered in an `AssetRegistry`. Drive letters are not adopted verbatim; see §5.B. | LVGL reference; adapted |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| `FsError`, `BlockDevice`, `AssetError`, `AssetRead`, `AssetSource`, `AssetIter`, `AssetManager<S>` | `core/src/fs.rs` (existing; LPAR-09 extends `AssetManager`) |
| `SimBlockDevice` (block-device over host file) | `fs-sim/src/lib.rs` |
| `DiscoSdBlockDevice` (SDMMC block device) | `platform/src/stm32h747i_disco_sd.rs` |
| `FatfsBlockStream` + `mount_and_list_assets` (fatfs adapter) | `platform/src/sd_fatfs_adapter.rs` |
| `core/src/plugins/fatfs.rs` (std-only FAT helpers: `list_dir`, `read_file`, etc.) | `core/src/plugins/fatfs.rs` |
| `ImageDescriptor`, `ImageData` (`#[non_exhaustive]`), `PixelFormat`, `CacheHandle`, `ImageCache<'a>` | `core/src/image.rs` |
| `AssetPath` enum (new — typed source-kind addressing) | Future `core/src/fs.rs` or `core/src/asset.rs` |
| `AssetHandle` (new — source-backed image token) | Future `core/src/asset.rs` or `core/src/image.rs` |
| `AssetRegistry` (new — multi-source dispatcher) | Future `core/src/asset.rs` |
| Concrete `ImageCache` implementation (LRU ring) | Future `core/src/image_cache.rs` or `core/src/asset.rs` |
| `AssetManager<S>` image-load / font-load helpers (new) | `core/src/fs.rs` (additive impl block) |
| Embedded source (`EmbeddedAssetSource`, build-generated table) | Future `core/src/asset.rs`; build.rs conventions in `examples/stm32h747i-disco/assets/disco-assets/` (existing pattern) |
| Simulator source (`SimAssetSource`) | Future `core/src/asset.rs` or `fs-sim/` (std-gated) |
| Memory source (`MemoryAssetSource`) | Future `core/src/asset.rs` |
| LVGL drive-letter reference | `lvgl/src/misc/lv_fs.h` @ LPAR-01 §2 |
| LVGL image source-type reference | `lvgl/src/draw/lv_image_decoder.h:34-38` |
| Invalidation dirty-rect reporting | `core/src/invalidation.rs` + LPAR-03 §7 |

## 5. Frozen Decisions

### 5.A — Reconcile `core::fs`: Extend, Do Not Replace

**`AssetSource`, `AssetRead`, `AssetError`, `AssetManager<S>`, and `BlockDevice`
are the canonical lower-half traits and MUST be preserved without modification.**

Evidence for the extend-not-replace call:

- `AssetSource` / `AssetManager` / `AssetRead` have zero external consumers
  (grep for all three outside `core/src/fs.rs` returns empty — confirmed above).
  They are not yet locked by compatibility pressure, but they were designed
  correctly for the role LPAR-09 needs.
- `BlockDevice` has three concrete implementations: `DiscoSdBlockDevice`
  (`platform/src/stm32h747i_disco_sd.rs:33`), `SimBlockDevice`
  (`fs-sim/src/lib.rs:12`), and the unnamed test devices in `platform/src/blit.rs:1472`.
  Both platform crates import from `rlvgl_core::fs`. A rename or restructure
  would break two external crates.
- `FatfsBlockStream` (`platform/src/sd_fatfs_adapter.rs:18`) wraps a
  `&mut BlockDevice` and provides `Read + Write + Seek` for the `fatfs` crate
  in `no_std + alloc`. This is the only bridge between `BlockDevice` and the
  FAT filesystem layer. It is correct and working; LPAR-09 reuses it.

**What LPAR-09 adds above the existing lower half:**

1. A typed `AssetPath` enum (§5.B) providing the source-kind and path without
   runtime drive-letter strings.
2. An `AssetRegistry` that holds one or more `Box<dyn AssetSource>` entries
   and dispatches `open(path)` calls by matching the source kind in `AssetPath`.
   This is the LVGL `lv_fs_drv_t` analog in typed form.
3. Concrete `AssetSource` implementations for each of the four source kinds
   (§5.E), wiring `AssetRead` over the appropriate backend.
4. Additive helper methods on `AssetManager<S>` for typed image and font loading.
5. The `AssetHandle` type and `ImageData::Asset` variant (§5.C).
6. The concrete `ImageCache` implementation (§5.D).

**Nothing in `core/src/fs.rs` changes.** The `AssetManager`, `AssetSource`,
and `AssetRead` traits remain as currently defined. New functionality is
entirely additive: new `impl AssetManager<S>` methods, new implementors of
`AssetSource`, and new types in a new `core/src/asset.rs` module.

**Deprecation-in-place precedent does NOT apply here.** No existing API overlaps
in a way that requires deprecation. The free functions in `core/src/plugins/fatfs.rs`
(`list_dir`, `read_file`, etc.) are `std`-only helpers operating on arbitrary
`Read + Write + Seek` images; they remain useful for test utilities and do not
conflict with the `AssetSource` abstraction. They are NOT deprecated.

### 5.B — Source Kinds and Path / Drive-Convention Model

**The source kind set is frozen. Registration policy: Standards Action.**

`AssetPath` is owned by the registry (it is interned behind an `AssetHandle`
token, §5.C), so its runtime variants hold owned `String`s — they are not
constrained to `'static` or borrowed lifetimes:

| Source kind | Rust variant | `no_std` | Feature gate | Notes |
|---|---|---|---|---|
| `Embedded` | `AssetPath::Embedded(&'static str)` | Yes | `fs` (already gates `core::fs`) | `include_bytes!`-backed statics; symbol name is compile-time `'static`; infallible lookup |
| `Fatfs` | `AssetPath::Fatfs(String)` | Yes (`no_std + alloc`) | `fatfs` | Runtime path; via `BlockDevice` + `FatfsBlockStream` + `fatfs` crate |
| `Simulator` | `AssetPath::Sim(String)` | No (`std`-only) | `sim` or `std` | Runtime host filesystem path; simulator builds only |
| `Memory` | `AssetPath::Memory(String)` | Yes (`no_std + alloc`) | `fs` | Runtime key into an in-RAM `&[(name, &[u8])]` map; test injection and RAM-resident data |

Adding a new source kind to this set requires a §15 Standards Action amendment.
Examples of future candidates: `LittleFs`, `Spiffs`, `Network` (deferred-Coupled
— see §14).

**LVGL drive-letter scheme vs typed enum — frozen decision:**

LVGL uses single uppercase letters (`A:`, `S:`, etc.) prepended to path strings,
dispatched at runtime through a linked list of `lv_fs_drv_t` structs
(`lvgl/src/misc/lv_fs.h`). This design enables runtime driver registration
without compile-time knowledge of source kinds, at the cost of string parsing
and a runtime search.

**LPAR-09 DOES NOT adopt LVGL's drive-letter syntax.** The reason is that the
Rust type system provides a better encoding: `AssetPath` is a data enum whose
variant encodes the source kind and whose inner `&str` is the path within that
source. There is no string to parse, no runtime dispatch list, and no
possibility of silently routing a path to the wrong source kind. The `AssetPath`
variant IS the drive letter, typed.

The `AssetRegistry` implements the `lv_fs_drv_t`-like role: it holds up to
`ASSET_REGISTRY_MAX_SOURCES` (initial value: 4; Expert Review to increase)
`Box<dyn AssetSource>` slots, one per registered source kind. Dispatch is by
variant match, not by string prefix.

**Path string format:** within each source kind, paths are `/`-separated,
UTF-8, with no leading `/` required (consistent with `AssetSource::open`'s
existing `"fonts/regular.bin"` example in `core/src/fs.rs:66`). For `Embedded`
sources, the path is the Rust symbol name used to look up the static byte
slice in the generated table (see §5.E). For `Fatfs`, the path is passed to
the FAT driver after stripping any leading `/`. For `Sim`, the path is a
relative or absolute host filesystem path.

**LVGL `LV_IMAGE_SRC_VARIABLE` / `LV_IMAGE_SRC_FILE` / `LV_IMAGE_SRC_SYMBOL`
mapping:**

| LVGL source kind | rlvgl analog |
|---|---|
| `LV_IMAGE_SRC_VARIABLE` | `ImageData::Borrowed` or `ImageData::BorrowedColors` (already defined in LPAR-08) |
| `LV_IMAGE_SRC_FILE` | `ImageData::Asset(AssetHandle)` with `AssetPath::Fatfs` or `AssetPath::Sim` variant |
| `LV_IMAGE_SRC_SYMBOL` | `ImageData::Asset(AssetHandle)` with `AssetPath::Embedded` variant, where the `&str` is the symbol name |

### 5.C — Asset→Image Bridge: `ImageData::Asset` Variant

**LPAR-08 made `ImageData` `#[non_exhaustive]` specifically to allow LPAR-09 to
add this variant without a breaking change. Adding it requires a LPAR-08 §15
Standards Action amendment.**

The new variant:

```
// Inside core::image::ImageData<'a>
// (addition requires LPAR-08 §15 Standards Action amendment)
AssetHandle(crate::asset::AssetHandle),
```

Where `AssetHandle` is an **opaque registry token**, not a path-bearing
struct:

```
/// An opaque token identifying a source-backed image registered with the
/// asset registry. The decoded pixels may or may not be resident in the
/// cache at any moment; callers MUST call `AssetRegistry::resolve_image`
/// (which has the registry, and thus the path and cache) to ensure decoded
/// data is present before calling `renderer.blit_image`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle(u32);
```

The registry owns a table of `AssetHandle -> (AssetPath, Option<CacheHandle>)`.
`AssetRegistry::register(path: AssetPath) -> AssetHandle` interns a path and
returns its token; `resolve_image` looks the token up, decodes (or returns
the cached entry), and updates the cache slot.

**Why the handle is an opaque id rather than carrying its `AssetPath`:**
the alternative — embedding the `AssetPath` (and a `CacheHandle`) in a `Copy`
struct — forces `AssetPath` to be `Copy`/`'static`, which is wrong for the
runtime FATFS/simulator file paths that `LV_IMAGE_SRC_FILE` targets (a file
browser opening a user-selected path needs an owned, runtime `String`). The
opaque-id design lets the **registry** own the paths (owned, any lifetime)
while the handle stays a `Copy` `u32` that any widget can hold. The
"self-contained reload" benefit of embedding the path is illusory: a widget
must call `resolve_image(&registry, handle)` to decode anyway, so the
registry — and therefore the reload address — is always in hand. Eviction
drops only the decoded pixels; the registry's `AssetPath` enables reload
(§5.D).

**LPAR-08 / LPAR-09 boundary (normative):**

- LPAR-08 owns: `ImageDescriptor`, `ImageData` (variants `Borrowed`,
  `BorrowedColors`, `Owned`), `PixelFormat`, `BlitOpts`, `CacheHandle`,
  and the `ImageCache<'a>` trait.
- LPAR-09 owns: the `ImageData::Asset(AssetHandle)` variant (added here, gated
  by §9 registration policy); `AssetHandle`; `AssetPath`; the concrete
  `ImageCache` implementation; source lookup for all four source kinds; and
  the `AssetRegistry`.

**Widget behavior with `ImageData::Asset`:**

Widgets that hold an `ImageDescriptor` with `ImageData::Asset` MUST call
`AssetRegistry::resolve_image(&mut self, handle: AssetHandle) ->
Result<&ImageDescriptor<'_>, AssetError>` before the draw call. `resolve_image`
checks the cache; on a miss it opens the asset via the registered source, passes
the bytes to the appropriate decode plugin, stores the result, and returns the
decoded descriptor. The widget then calls `renderer.blit_image(dest, descriptor,
opts)` as normal. This maps to LVGL's `lv_image_decoder_open` / `get_area` /
`close` lifecycle, but without separate open/close state.

**Decode plugin boundary (normative):**

`AssetRegistry::resolve_image` obtains bytes by calling `AssetSource::open(path)`
and draining the `AssetRead` stream. It then selects a decode plugin based on
path extension or magic bytes:

| Extension / magic | Plugin |
|---|---|
| `.png` / `\x89PNG` | `core::plugins::png::decode(&[u8])` |
| `.jpg` / `.jpeg` / `\xff\xd8` | `core::plugins::jpeg::decode(&[u8])` |
| `.gif` / `GIF8` | `core::plugins::gif` |
| `.raw` / `.rle` | Direct bytes wrapped in `ImageData::Borrowed` or decoded inline |
| `.bin` (font) | Handled by `AssetManager::load_packed_font`, not by image decode |

LPAR-09 does NOT implement new codec logic. It calls the existing decode plugins
after obtaining bytes from the source. These plugins are `std`-only today
(using `std::io::Cursor`); making them `no_std + alloc` compatible is
deferred-Safe (see §14).

**Infallible vs fallible decode:**

- `Embedded` sources are byte-exact (bytes were verified at build time by the
  `include_bytes!` invocation); decode failure is still possible (corrupt
  format) and returns `AssetError::Decode(...)`.
- `Fatfs`, `Sim`, `Memory` sources may return `AssetError::Fs(FsError)` if the
  underlying read fails before decode begins.

### 5.D — Cache Policy

**LPAR-08 defined `CacheHandle(u32)` and `ImageCache<'a>` trait. LPAR-09
provides the concrete implementation.**

**Concrete `ImageCache` shape:**

A `SlotCache<N>` (type alias `ImageCacheN = SlotCache<N>`) backed by a
fixed-size array of N `Option<(CacheHandle, timestamp, ImageDescriptor<'static>)>`
entries where:

- `N` is a const generic, caller-chosen at construction.
- `timestamp` is a monotonic `u32` counter advanced on each `get` hit or `put`.
- Eviction policy: **LRU** — on a `put` when all N slots are occupied, the
  entry with the smallest `timestamp` is overwritten.
- `get` bumps the matched entry's `timestamp` (timestamp = current counter; counter increments).
- `CacheHandle` values are assigned sequentially from a `u32` counter on each
  `put`; they are never reused within a session (handle 0 is reserved as
  invalid/null).

**Cache key and `AssetHandle`-backed eviction:**

When an `ImageData::Asset`-backed descriptor is evicted, the slot is freed and
the `CacheHandle` becomes invalid. The widget's `AssetHandle` still contains
the `AssetPath`, so the next `resolve_image` call will reload and re-decode the
asset. This is consistent with LVGL's cache model, which drops decoded buffers
and reloads from the filesystem on pressure.

**`no_std + alloc`:** The `SlotCache<N>` uses a fixed-size array; N is a const
generic. No heap allocation in the cache itself. Decoded `ImageDescriptor::Owned`
entries use `alloc::vec::Vec` for pixel data (already the case for `ImageData::Owned`).
Decoded `ImageDescriptor::Borrowed` entries reference static byte slices that
are not managed by the cache.

**Reconciliation with LPAR-08 `CacheHandle`:** LPAR-08 defines `CacheHandle`
as `CacheHandle(u32)` and the `ImageCache<'a>` trait with `get` / `put`. LPAR-09
provides the concrete `SlotCache<N: usize>` that implements `ImageCache<'static>`.
The trait and handle type are unchanged.

**Determinism for LPAR-16:** the LRU eviction order is fully deterministic given
a fixed sequence of `get`/`put` calls. Tests that need a reproducible cache state
use a fresh `SlotCache<N>` and drive calls in a fixed order.

**Feature gating:** `SlotCache` is `no_std + alloc`, available behind the `fs`
feature. Simulator and FATFS integration that feeds decoded data into the cache
additionally requires `fatfs` or `sim` features respectively.

### 5.E — Per-Source-Kind Contracts

#### Embedded source (`AssetPath::Embedded`)

- **Lookup:** a statically-initialized `&[(&'static str, &'static [u8])]` table,
  typically auto-generated by a build.rs script (following the pattern at
  `examples/stm32h747i-disco/assets/disco-assets/build.rs:30`). The table is
  passed to `EmbeddedAssetSource::new(table)` at startup.
- **`AssetSource::open`** for `Embedded`: searches the table for an exact string
  match on the path component. Returns a `Box<dyn AssetRead>` wrapping the
  static slice. **Always succeeds** if the symbol exists in the table; returns
  `AssetError::Fs(FsError::NoSuchFile)` if not.
- **Error semantics:** `FsError::Device` MUST NOT occur (no device behind a
  static slice). `FsError::NoSuchFile` is the only expected failure.
- **`no_std + alloc`:** Yes. The table is `&'static`, the `AssetRead` impl is
  allocation-free on the read path.

#### FATFS source (`AssetPath::Fatfs`)

- **`FatfsAssetSource`** holds a `&mut dyn BlockDevice` and mounts a FAT volume
  via `FatfsBlockStream` + `fatfs::FileSystem`. Because `fatfs::FileSystem` takes
  ownership of the stream, the source opens a new stream for each `open` call
  (same pattern as `mount_and_list_assets` in `platform/src/sd_fatfs_adapter.rs:141`).
- **`AssetSource::open`** for `Fatfs`: creates a `FatfsBlockStream`, opens the
  FAT volume read-only, opens the named file, reads it into a `Vec<u8>`, and
  returns a `Box<dyn AssetRead>` wrapping the vec.
- **`AssetSource::list`** for `Fatfs`: uses `mount_and_list_assets` or an
  equivalent internal helper; fills `AssetIter`.
- **Error semantics:** all `FsError` variants are possible: `Device` (SDMMC
  failure), `InvalidPath` (malformed path), `NoSuchFile` (missing file).
- **`no_std + alloc`:** Yes, behind the `fatfs` feature.
- **Relationship to `core/src/plugins/fatfs.rs`:** the existing `read_file`
  and `list_dir` free functions remain available for test and tooling use. They
  are not deprecated; the `FatfsAssetSource` is a compatible wrapper around the
  same `fatfs` crate APIs.

#### Simulator source (`AssetPath::Sim`)

- **`SimAssetSource`** holds an optional `std::path::PathBuf` prefix. `open`
  resolves `prefix / path` on the host filesystem via `std::fs::File`.
- **`AssetSource::open`** for `Sim`: opens the resolved host path and returns a
  `Box<dyn AssetRead>` over the `std::fs::File` (via an `AssetRead` adapter).
- **Error semantics:** host-filesystem errors map to `FsError::Device` (I/O
  failures) or `FsError::NoSuchFile` (ENOENT).
- **`no_std`:** No. This source is unconditionally `std`-only and is gated behind
  a `sim` or `std` feature. It MUST NOT compile on `target_os = "none"`.
- **Relationship to `SimBlockDevice`:** `SimBlockDevice` (in `fs-sim/`) provides
  a `BlockDevice` over a FAT image file — still useful for testing the FATFS path
  on a simulator. `SimAssetSource` is a different and complementary abstraction:
  it bypasses the FAT image entirely and reads host files directly. Both coexist.

#### Memory source (`AssetPath::Memory`)

- **`MemoryAssetSource`** holds a `&'static [(  &'static str, &'static [u8])]`
  or an `alloc::vec::Vec<(&'static str, &'static [u8])>` (runtime-populated
  variant). Intended for test asset injection and for cases where assets are
  pre-loaded into RAM from a FATFS or network source before the widget tree
  starts.
- **`AssetSource::open`** for `Memory`: string-matches path against the table
  and returns a `Box<dyn AssetRead>` over the byte slice. Semantics identical
  to the embedded source but with a runtime-populated table.
- **Error semantics:** identical to `Embedded` (no device; `NoSuchFile` on miss).
- **`no_std + alloc`:** Yes.

### 5.F — Invalidation, Lifecycle, and Deferred Decode

**Async/streaming decode is deferred-Coupled (see §14).** v1 is synchronous:
`resolve_image` blocks until decoding is complete. A future `resolve_image_async`
entry point would return `Poll::Pending` and report a dirty rect via
`InvalidationList` when the decoded descriptor becomes available. The `AssetHandle`
and `AssetPath` design accommodate this without structural change; the bridge to
`InvalidationList` (LPAR-03 §7) is named here.

**For v1 (synchronous):** `resolve_image` returns `Ok(&ImageDescriptor)` or an
`AssetError`. The caller reports the affected widget's rect as dirty before the
call (the standard draw-cycle path) and renders the decoded descriptor in the
same frame. No additional invalidation notification is needed for synchronous
decode.

**LPAR-03 integration (normative):** if a deferred decode implementation lands
in a future sub-letter, the decode completion callback MUST call
`InvalidationList::mark_dirty(widget_bounds)`. It MUST NOT post a second
repaint via any other path.

**Asset lifecycle (v1):**

1. Widget constructs an `ImageDescriptor` with `ImageData::Asset(handle)`.
2. Widget's `draw` method calls `AssetRegistry::resolve_image(handle)`.
3. On cache hit: returns `&ImageDescriptor` with decoded pixel data. Draw proceeds.
4. On cache miss: opens source, reads bytes, calls decode plugin, stores in cache,
   returns decoded `&ImageDescriptor`. Draw proceeds.
5. If the decoded entry is evicted between frames: step 3 is a miss again; decode
   repeats. This is correct; draw does not fail, it just re-decodes.

### 5.G — `no_std` / `alloc` / `std` Split (normative)

| Component | `no_std + alloc` | `std` required | Feature gate |
|---|---|---|---|
| `AssetSource` trait, `AssetRead` trait, `AssetError`, `FsError`, `AssetManager<S>` | Yes | No | `fs` |
| `BlockDevice` trait | Yes | No | `fs` |
| `EmbeddedAssetSource` | Yes | No | `fs` |
| `MemoryAssetSource` | Yes | No | `fs` |
| `FatfsAssetSource` | Yes | No | `fatfs` |
| `SimAssetSource` | No | Yes | `sim` or `std` |
| `SlotCache<N>` (concrete `ImageCache`) | Yes | No | `fs` |
| `AssetPath` enum | Yes | No | `fs` |
| `AssetHandle` | Yes | No | `fs` |
| `AssetRegistry` | Yes | No | `fs` |
| Decode plugins (`png`, `jpeg`, `gif`) | No (currently `std::io::Cursor`) | Yes | `png`, `jpeg`, `gif` |
| Font-load helper (`load_packed_font`) | Yes | No | `fs` |

The decode plugins currently use `std::io::Cursor`; making them `no_std + alloc`
compatible is deferred-Safe. Until that lands, `resolve_image` with a non-embedded
source that produces PNG/JPEG data is limited to `std` builds. LPAR-09 MUST NOT
silently expand the `no_std` boundary by routing decoded pixels through a `std`-only
codec in an `no_std`-advertised path; it MUST either gate the relevant source-kind
paths behind `std` feature checks or require callers to provide a decoder callback.

Raw-format assets (`.raw`, `.rle` — used extensively in the disco example via
`include_bytes!`) bypass codec plugins entirely and wrap bytes directly in
`ImageData::Borrowed`; these are `no_std`-safe.

**Registration policy for the `fs` feature scope:** the `fs` feature in
`core/Cargo.toml:50` is currently an empty feature flag that enables
`pub mod fs` in `core/src/lib.rs:50`. LPAR-09 expands its scope to also enable
`pub mod asset` (the new `AssetPath`, `AssetHandle`, `AssetRegistry`, and
`SlotCache` types). The `fs` feature remains a single compile-time opt-in;
its expanded scope is recorded in the §15 change log.

## 6. Source-of-Truth Map (Canonical)

| Concept | Canonical artifact |
|---|---|
| `FsError`, `BlockDevice`, `AssetError`, `AssetRead`, `AssetSource`, `AssetManager<S>` | `core/src/fs.rs` |
| `AssetPath`, `AssetHandle`, `AssetRegistry`, `EmbeddedAssetSource`, `MemoryAssetSource` | Future `core/src/asset.rs` |
| `FatfsAssetSource` | Future `core/src/asset.rs` (behind `fatfs` feature) |
| `SimAssetSource` | Future `core/src/asset.rs` or `fs-sim/src/asset.rs` (behind `sim`/`std` feature) |
| `SlotCache<N>` (concrete `ImageCache`) | Future `core/src/image_cache.rs` or `core/src/asset.rs` |
| `ImageData::Asset(AssetHandle)` variant (new) | `core/src/image.rs` (added after LPAR-08 §15 amendment) |
| `DiscoSdBlockDevice` | `platform/src/stm32h747i_disco_sd.rs` |
| `SimBlockDevice` | `fs-sim/src/lib.rs` |
| `FatfsBlockStream`, `mount_and_list_assets` | `platform/src/sd_fatfs_adapter.rs` |
| Decode plugins | `core/src/plugins/{png,jpeg,gif,apng,qrcode}.rs` |
| LVGL reference | `lvgl/src/misc/lv_fs.h`, `lvgl/src/draw/lv_image_decoder.h` @ LPAR-01 §2 |
| Invalidation dirty reports | `core/src/invalidation.rs` + LPAR-03 §7 |

## 7. Dependency Analysis

| Dependency | Reason | Blocks if missing |
|---|---|---|
| LPAR-08 ratification | Defines `ImageDescriptor`, `ImageData` (`#[non_exhaustive]`), `CacheHandle`, and `ImageCache<'a>` trait. LPAR-09 adds the `Asset` variant and the concrete cache implementation. | `ImageData::Asset` and `SlotCache` |
| LPAR-08 §15 Standards Action amendment for `ImageData::Asset` | Required before any code that constructs `ImageData::Asset` lands. | `ImageData::Asset` variant |
| LPAR-03 invalidation planner | Deferred async decode must report dirty rects through `InvalidationList`. | Async decode variant (deferred) |
| `core/src/fs.rs` `BlockDevice` + `AssetSource` (existing) | LPAR-09 layers above these. They are not changed but their full contract is exercised for the first time. | All source implementations |
| `platform/src/sd_fatfs_adapter.rs` `FatfsBlockStream` (existing) | `FatfsAssetSource` reuses this adapter. | FATFS source implementation |
| Decode plugins (`core/src/plugins/png.rs`, etc.) | `resolve_image` calls these after obtaining bytes. | Image decode |
| LPAR-01 baseline pin | Identifies the LVGL reference for path conventions and image source types. | Parity claim validity |
| LPAR-12 (`ImageButton`) | Consumes `ImageData::Asset` and `AssetRegistry::resolve_image`. Cannot complete without LPAR-09. | LPAR-12 acceptance |
| LPAR-15 (`Canvas`, `AnimImage`) | Needs full source/cache pipeline. Cannot complete without LPAR-09. | LPAR-15 acceptance |

## 8. Conflict Analysis

| Conflict | Risk | LPAR-09 resolution |
|---|---|---|
| **Existing `core::fs::AssetSource` vs LPAR-09 model** (named LPAR-00 §9) | Introducing a new asset model without reconciling the existing one would leave a dead abstraction and break future consumers who read the docs expecting `AssetSource` to be the entry point. | §5.A: `AssetSource` is the canonical lower-half trait. LPAR-09 extends it with concrete implementations and wires it upward via `AssetRegistry`. No replacement. No deprecation. |
| **`ImageData` `#[non_exhaustive]` extension — non-breaking** (LPAR-08 §8 and §9) | Adding `ImageData::Asset` is a non-breaking addition to a `#[non_exhaustive]` enum — existing match arms with `_` compile without change. But it is a Standards Action and requires a LPAR-08 §15 amendment before code lands. | §5.C: Standards Action process is mandatory. The amendment records the variant shape, the `AssetHandle` type, and the `AssetRegistry::resolve_image` contract. |
| **Decode-plugin boundary: LPAR-09 source vs plugin decode** | If decode plugins are invoked inside `AssetSource::open`, the source is now responsible for knowing the format. If decode is in `resolve_image`, the registry must select the plugin. | §5.C, §5.E: `AssetSource::open` returns raw bytes only; format detection and plugin dispatch live in `AssetRegistry::resolve_image`. No codec logic in any `AssetSource` implementation. |
| **FATFS adapter reuse** | `platform/src/sd_fatfs_adapter.rs` is a concrete `BlockDevice` adapter; it is not an `AssetSource`. Re-wrapping it as one must not change its no_std/fatfs compile contract. | §5.E: `FatfsAssetSource` is a NEW type that wraps `&mut dyn BlockDevice` internally using `FatfsBlockStream`. The existing `FatfsBlockStream` and `mount_and_list_assets` are consumed, not modified. |
| **Simulator host-FS feature gating** | If `SimAssetSource` is not feature-gated, it silently introduces `std` dependencies into `core/`, which is `no_std` by default. | §5.G: `SimAssetSource` is gated behind a `sim` or `std` feature. It MUST NOT compile on `target_os = "none"`. |
| **Cache eviction determinism vs LPAR-16** | Non-deterministic eviction (e.g., a time-based or random policy) would make visual goldens frame-dependent. | §5.D: LRU with a monotonic `u32` counter; fully deterministic given a fixed call sequence. LPAR-16 tests use a fresh `SlotCache<N>` per fixture. |
| **LVGL drive-letter scheme vs typed Rust registry** | Adopting LVGL's string-prefix dispatch would require runtime string parsing and a linked-list registry, adding latency and preventing the type system from rejecting malformed paths. | §5.B: `AssetPath` enum with source-kind variants. Drive letters are not adopted. The LVGL `lv_fs_drv_t` role is served by `AssetRegistry`. |
| **Media widgets (LPAR-15) depending on this** | LPAR-15 (`Canvas`, `AnimImage`) needs the full source/cache pipeline. If LPAR-09 leaves the `ImageData::Asset` bridge incomplete, LPAR-15 cannot proceed. | §5.C: `AssetHandle` and `resolve_image` are defined in LPAR-09. LPAR-15 is blocked on LPAR-09 ratification (per LPAR-00 §8 dependency table row "Asset/image substrate before media widgets"). |
| **Decode plugins currently `std`-only** | `core/src/plugins/png.rs` uses `std::io::Cursor`; same for `jpeg`. Using these from an `no_std` `FatfsAssetSource` would silently pull in `std`. | §5.G: LPAR-09 MUST NOT route no_std source paths through std-only codecs without explicit feature gating. The `resolve_image` dispatch path gates codec calls behind the same feature flags that guard the plugins themselves. Making plugins no_std is deferred-Safe. |
| **`core/src/plugins/fatfs.rs` free functions vs `FatfsAssetSource`** | Both operate on FAT volumes. Keeping both risks confusion about which is the "right" path. | §5.A, §5.E: free functions (`list_dir`, `read_file`, etc.) remain as test/tooling utilities over arbitrary `Read + Write + Seek` images. `FatfsAssetSource` is the production path. The two are complementary, not competing. No deprecation. |

## 9. Frozen Enum Registration Policy

| Enum | Policy | Notes |
|---|---|---|
| `AssetPath` source kind variants | Standards Action | Cross-phase contract (block-device abstraction, cache, image pipeline, platform backends). Adding a variant (e.g. `LittleFs`, `Network`) requires a §15 amendment. Initial: `Embedded`, `Fatfs`, `Simulator`, `Memory`. |
| `ImageData` variants | Standards Action (inherited from LPAR-08 §9) | LPAR-09 adds `Asset(AssetHandle)` as its one Standards Action. Any further `ImageData` additions require LPAR-08 §15 amendments. |
| `AssetError` variants | Specification Required | Error variants are phase-local and do not cross phase boundaries in v1. Adding `AssetError::Decode(DecodeError)` (new for decode failures) requires a phase-doc entry here, not a LPAR-08 amendment. Initial: `Fs(FsError)`, `Decode(DecodeError)` (new). |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship | Decision |
|---|---|---|
| `core/src/fs.rs` `AssetSource`, `AssetRead`, `AssetError`, `AssetManager<S>`, `BlockDevice` | **Extended** by LPAR-09. All types preserved; new `AssetManager<S>` helper methods added; new implementors (`EmbeddedAssetSource`, `FatfsAssetSource`, `SimAssetSource`, `MemoryAssetSource`) added. | Additive; no breaking change. |
| `platform/src/sd_fatfs_adapter.rs` `FatfsBlockStream` + `mount_and_list_assets` | **Consumed** by `FatfsAssetSource` internally. Not modified. | As-is; new consumer only. |
| `platform/src/stm32h747i_disco_sd.rs` `DiscoSdBlockDevice` | **Consumed** as the `BlockDevice` argument to `FatfsAssetSource`. Not modified. | As-is. |
| `fs-sim/src/lib.rs` `SimBlockDevice` | **Coexists** with `SimAssetSource`. They serve different access patterns: `SimBlockDevice` for testing the FATFS path over an image file; `SimAssetSource` for direct host-FS access. | Both preserved; `SimAssetSource` is additive. |
| `core/src/plugins/fatfs.rs` free functions | **Coexist** as test/tooling utilities. Not deprecated. | As-is. |
| `core/src/image.rs` `ImageData` | **Extended** with `Asset(AssetHandle)` variant after LPAR-08 §15 amendment. All existing variants (`Borrowed`, `BorrowedColors`, `Owned`) unchanged; `#[non_exhaustive]` already present. | Standards Action amendment required; extension is then non-breaking. |
| `core/src/image.rs` `CacheHandle`, `ImageCache<'a>` | **Implemented** by `SlotCache<N>` (LPAR-09). Trait unchanged. | `SlotCache<N>` is a new `impl ImageCache<'static>`. |
| Decode plugins (`core/src/plugins/{png,jpeg,gif,apng,qrcode}.rs`) | **Consumed** by `AssetRegistry::resolve_image`. Not modified. | As-is; LPAR-09 is a new caller of existing functions. |
| `examples/stm32h747i-disco/assets/disco-assets/build.rs` | **Pattern reused** by `EmbeddedAssetSource` build conventions. The generated `include_bytes!` + symbol table pattern is the reference implementation for embedded source setup. | Existing pattern elevated to a documented convention; no changes to example code required. |
| LVGL `lv_fs.h` drive-letter model | **Adapted** as `AssetPath` enum + `AssetRegistry`. Drive letters are not adopted; the Rust type system provides the equivalent dispatch without string parsing. | Documented difference (§5.B). |
| LVGL `lv_image_src_t` source types | **Mapped** to `ImageData` variants (§5.B table). `LV_IMAGE_SRC_VARIABLE` → `Borrowed`/`BorrowedColors`; `LV_IMAGE_SRC_FILE` → `Asset` with Fatfs/Sim path; `LV_IMAGE_SRC_SYMBOL` → `Asset` with Embedded path. | Reference-adapted; no C ABI. |

## 11. Non-Goals

- No runtime drive-letter string parsing or LVGL-compatible path string format.
- No streaming / async decode in v1. Deferred-Coupled; see §14.
- No network source kind in v1. Deferred-Coupled; see §14.
- No write/save support. `AssetSource::open` is read-only. `BlockDevice` has
  `write_blocks` for raw device access; FAT-level file write is not exposed
  through `AssetSource`.
- No changes to `FsError`, `BlockDevice`, `AssetRead`, `AssetSource`, or
  `AssetManager<S>` beyond additive helper methods on `AssetManager`.
- No modification of the decode plugins (`png`, `jpeg`, etc.). Making them
  `no_std + alloc`-compatible is deferred-Safe.
- No removal of `core/src/plugins/fatfs.rs` free functions. They remain as-is.
- No `SimBlockDevice` changes (it serves the FAT-image testing path, which is
  independent of `SimAssetSource`).
- No C ABI compatibility with `lv_fs_drv_t` or `lv_image_decoder_t`.
- No new font codec; font loading via `AssetManager::load_packed_font` reads raw
  `.bin` bytes and constructs a `PackedFont`. Fontdue TTF loading is
  `std`-only and continues to use `include_bytes!` or a `SimAssetSource`-backed
  path on the simulator.
- No LVGL canvas widget (LPAR-15 scope), no `AnimImage` (LPAR-15 scope).
- No media widget implementation (LPAR-11 through LPAR-15 scope).

## 12. Acceptance Checklist

LPAR-09 implementation is complete only when:

- [ ] LPAR-08 §15 Standards Action amendment for `ImageData::Asset(AssetHandle)`
      filed, accepted, and merged before any code constructs `ImageData::Asset`.
- [ ] `AssetPath` enum defined in `core/src/asset.rs` (or `core/src/fs.rs`)
      behind the `fs` feature; all four variants present and documented.
- [ ] `AssetHandle { cache: CacheHandle, path: AssetPath }` struct defined;
      `Copy + Clone + Eq + Hash`.
- [ ] `AssetRegistry` struct defined; holds up to `ASSET_REGISTRY_MAX_SOURCES`
      `Box<dyn AssetSource>` slots; `register(source) -> Result<(), RegistryError>`;
      `resolve_image(handle) -> Result<&ImageDescriptor<'_>, AssetError>`.
- [ ] `EmbeddedAssetSource::new(table: &'static [(&'static str, &'static [u8])])` 
      compiles; `AssetSource::open` returns borrowed slice for found entries,
      `AssetError::Fs(FsError::NoSuchFile)` for missing entries.
- [ ] `MemoryAssetSource` compiles; `no_std + alloc`; same contract as embedded.
- [ ] `FatfsAssetSource` wraps `&mut dyn BlockDevice`; gated behind `fatfs`
      feature; existing `FatfsBlockStream` and `mount_and_list_assets` are
      consumed, not forked.
- [ ] `SimAssetSource` compiles behind `sim`/`std` feature; does NOT compile
      with `target_os = "none"`.
- [ ] `SlotCache<N>` implements `ImageCache<'static>` (or lifetime-erased
      variant); LRU eviction; deterministic; `no_std + alloc`; N is const generic.
- [ ] `AssetManager<S>` gains at least one typed helper:
      `load_packed_font(path) -> Result<PackedFont, AssetError>`.
- [ ] `ImageData::Asset(AssetHandle)` variant present in `core/src/image.rs`;
      `as_bytes()` / `as_color_slice()` match arms updated; `byte_len()` for
      `Asset` variant returns `0` (unresolved, not in-memory).
- [ ] `resolve_image` correctly dispatches to decode plugins by extension/magic;
      result stored in `SlotCache` via `ImageCache::put`; decoded `ImageDescriptor`
      returned by reference.
- [ ] Decode plugin dispatch gates plugin calls behind their feature flags;
      no `std`-only codec invoked in an `no_std`-compiled path.
- [ ] Existing `BlockDevice` implementors (`DiscoSdBlockDevice`,
      `SimBlockDevice`) compile without modification.
- [ ] All new `core/` types compile under `no_std + alloc` (except `SimAssetSource`).
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and
      `cargo clippy --workspace -- -D warnings` pass.
- [ ] LPAR-16 conformance fixtures: embedded source lookup (hit + miss),
      FATFS source open over a `SimBlockDevice`-backed FAT image, `SlotCache`
      LRU eviction sequence (deterministic), `ImageData::Asset` round-trip
      through `resolve_image` with a `MemoryAssetSource`.
- [ ] Public APIs in publishable crates have doc comments.
- [ ] `docs/CHANGELOG.md` and crate manifests updated for publishable crates
      touched by this phase.

## 13. Files Cited

- `core/src/fs.rs` — `FsError` (:10), `BlockDevice` (:22), `AssetError` (:43),
  `AssetRead` (:49), `AssetSource` (:64), `AssetIter` (:76), `AssetManager<S>` (:87).
- `core/src/image.rs` — `PixelFormat` (:16), `ImageData` (`#[non_exhaustive]`) (:43),
  `ImageDescriptor` (:99), `CacheHandle` (:218), `ImageCache<'a>` (:237).
- `core/src/lib.rs` — `no_std` declaration (:12), `fs` feature gate (:49-50),
  `image` module declaration (:54).
- `core/Cargo.toml` — `fs = []` feature (:50), `fatfs` feature (:47).
- `core/src/plugins/fatfs.rs` — `list_dir` (:18), `file_exists` (:43),
  `read_file` (:54), `read_file_range` (:71). (std-only; preserved as-is.)
- `core/src/plugins/png.rs` — `decode(&[u8])` (:12). (std-only via `Cursor`.)
- `core/src/plugins/jpeg.rs` — `decode(&[u8])` (:9). (std-only via `Cursor`.)
- `platform/src/sd_fatfs_adapter.rs` — `FatfsBlockStream` (:23),
  `mount_and_list_assets` (:141).
- `platform/src/stm32h747i_disco_sd.rs` — `DiscoSdBlockDevice` (:40).
- `fs-sim/src/lib.rs` — `SimBlockDevice` (:15).
- `examples/stm32h747i-disco/assets/disco-assets/src/lib.rs` — embedded
  `include_bytes!` static table pattern (:14-42).
- `examples/stm32h747i-disco/assets/disco-assets/build.rs` — build-time
  static table generation (:30).
- `examples/stm32h747i-disco/src/main.rs` — `include_bytes!` for icons/fonts
  (:2709-2730). (Current production embedded asset pattern.)
- `platform/src/pixels_renderer.rs` — `include_bytes!` font (:16).
- `platform/src/blit.rs` — `include_bytes!` font (:30).
- `lvgl/src/misc/lv_fs.h` — `lv_fs_drv_t` (:69), drive letter in path (:138),
  `lv_fs_drv_register` (:118), `lv_fs_get_drv` (:125).
- `lvgl/src/misc/lv_fs.c` — drive-letter dispatch (:65-85), file cache (:109-126).
- `lvgl/src/draw/lv_image_decoder.h` — `lv_image_src_t` enum (:34-38),
  `lv_image_decoder_open_f_t` (:55), `lv_image_decoder_add_to_cache` (:182).
- `lvgl/src/lv_conf_internal.h` — per-driver letter config
  (`LV_FS_FATFS_LETTER`, `LV_FS_STDIO_LETTER`, etc. :2836-2961).
- `docs/concepts/LPAR-00-CONCEPTS.md` §9 — named conflict "Existing asset
  pipeline and plugin source conventions"; §8 dependency row "Asset/image
  substrate before media widgets".
- `docs/concepts/LPAR-01-BASELINE.md` §5 — "Asset source conventions: Partial
  (Embedded assets, FATFS, simulator path handling)".
- `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` §5.G — LPAR-08/09 boundary
  statement; `CacheHandle`; `ImageData` `#[non_exhaustive]`; §8 conflict entry
  for LPAR-09; §9 `ImageData` Standards Action policy; §11 non-goal ("No asset
  source lookup … LPAR-09 scope").

## 14. Unblocks / Deferred Work

### Unblocks after ratification

- LPAR-08 §15 amendment for `ImageData::Asset(AssetHandle)`.
- `EmbeddedAssetSource`, `FatfsAssetSource`, `SimAssetSource`, `MemoryAssetSource`
  concrete implementations.
- `SlotCache<N>` concrete `ImageCache` implementation.
- `AssetRegistry::resolve_image` wiring decode plugins to source lookup.
- LPAR-12 `ImageButton` using `ImageData::Asset` for file-backed images.
- LPAR-15 `Canvas` and `AnimImage` source/cache pipeline.
- LPAR-16 asset source fixtures.

### Deferred — Safe

- **`no_std + alloc` decode plugins:** making `png`, `jpeg`, `gif` work without
  `std::io::Cursor`. Requires replacing `Cursor` with a `core::io::Cursor`
  equivalent or a custom `Read` impl over `&[u8]`. Orthogonal; does not
  require changing `AssetSource`, `AssetRegistry`, or `SlotCache`. The
  source lookup path is correct; only the codec step needs the change.
- **Streaming / chunk-read decode:** breaking large file reads into chunks to
  avoid a full-buffer `Vec<u8>` allocation before decode. Requires a streaming
  codec API; no current plugin supports it. The `AssetRead::seek` method is
  already present for random access. Orthogonal to the source lookup contract.
- **Bilinear / Lanczos image scaling via asset pipeline:** deferred at LPAR-08
  §14; no change needed in LPAR-09.
- **Font load helpers beyond `load_packed_font`:** TTF loading via fontdue
  (`std`-only, already feature-gated), bitmap font from raw bytes. All fit
  the `AssetManager<S>` helper pattern.
- **`AssetIter` fleshed out:** `AssetIter` is a no-op placeholder
  (`core/src/fs.rs:76-83`); `Item = ()`. Making it return directory entry
  names is orthogonal to the image pipeline and can be added additively.
- **Symbolic asset names (LVGL symbol analog):** LVGL `LV_IMAGE_SRC_SYMBOL`
  provides a UTF-8 string that maps to a glyph in a symbol font. A future
  `AssetPath::Symbol(&str)` variant could serve the same role for icon fonts.
  Safe to add as a Standards Action after this phase ships.

### Deferred — Coupled

- **Async / deferred decode:** `AssetRegistry::resolve_image_async` that
  returns `Poll::Pending` and posts a dirty rect on completion. Coupled to
  LPAR-03 `InvalidationList` callback model and to the LPAR-06 timer/task
  infrastructure. Must be revisited with the async executor assumptions made
  in the FreeRTOS and Zephyr ports (CLAUDE.md project memory: FreeRTOS port
  status, Zephyr port working status).
- **Network source kind (`AssetPath::Network`):** requires `std`, a TCP/HTTP
  client, and a connection lifecycle. Coupled to connectivity assumptions and
  to the async decode model. Must be evaluated against the LPAR-Core `no_std`
  commitment before a Standards Action amendment adds the variant.
- **Write / save support:** writing assets back through `AssetSource`
  (FATFS write path, host-FS write). Coupled to `BlockDevice::write_blocks`
  lifetime model and to the FatFS adapter's read-only mount assumption
  (`FsOptions::read_only(true)` in `mount_and_list_assets`). Must not be
  introduced silently; a separate §15 amendment required.

### Deferred — Abandoned

None at this time.

## 15. Change Log

- **2026-06-12** — LPAR-09 drafted from code evidence: `core/src/fs.rs`,
  `core/src/image.rs`, `platform/src/sd_fatfs_adapter.rs`,
  `fs-sim/src/lib.rs`, `core/src/plugins/{fatfs,png,jpeg}.rs`,
  `examples/stm32h747i-disco/assets/disco-assets/`, LVGL
  `lv_fs.h` / `lv_image_decoder.h` @ LPAR-01 §2 pin. Not ratified.
- **2026-06-12** — Reviewer fix folded in, then ratified by owner instruction
  ("proceed down the LPAR worklist"). §5.C `AssetHandle` changed from a
  `Copy` struct embedding `{ cache: CacheHandle, path: AssetPath }` to an
  **opaque registry token** (`AssetHandle(u32)`), with the registry owning the
  interned `AssetPath`s. The embedded-path design forced `AssetPath` to be
  `Copy`/`'static`, which is wrong for runtime FATFS/simulator file paths
  (`LV_IMAGE_SRC_FILE`); the opaque-id design lets the registry own owned
  `String` paths while the handle stays a `Copy u32`, and the cited
  "self-contained reload" benefit was illusory since `resolve_image(&registry,
  handle)` needs the registry to decode anyway. §5.B runtime path variants
  changed to owned `String`. The reserved `ImageData::Asset` variant was
  registered via a LPAR-08 §15 Standards Action amendment (filed first).
  Evidence verified: `AssetSource`/`AssetManager`/`AssetRead` have zero
  consumers (safe to extend); `BlockDevice` is locked by the platform SD
  adapters. Implementation unblocked.
