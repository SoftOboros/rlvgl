<!--
02-splash-and-assets.md - Tutorial Chapter 2: splash image + creator CLI and UI.
-->

**[← Prev](01-hello-world.md) · [Index](README.md) · [Next →](03-desktop.md)**

# Chapter 2 — Splash Screen & the Asset Pipeline

## What you will add

A full-screen boot splash. The splash is an 800×480 image processed
by `rlvgl-creator` into rlvgl's runtime RLE format, embedded into
the firmware with `include_bytes!`, decoded into the SDRAM
framebuffer, and held for a couple of seconds before the Chapter 1
label draws over it.

Along the way you'll meet **both** sides of `rlvgl-creator` — the
CLI and the desktop UI — and learn which one to reach for when.

Features turned on:

- `splash` — pulls in `rlvgl-platform`'s RLE decode path and gates
  the splash blob + hold loop in `main.rs`.

## Before you start

- Chapter 1 works: the board boots and prints **Hello, rlvgl**.
- You have a source image for the splash. If you don't have one, copy
  the file the real crate ships with:
  [`examples/stm32h747i-disco/assets/media/splash.raw`](../../examples/stm32h747i-disco/assets/media/splash.raw).
  Its RLE-compressed sibling
  [`splash.rle`](../../examples/stm32h747i-disco/assets/media/splash.rle)
  is the file the firmware actually embeds.

## Meet `rlvgl-creator`

`rlvgl-creator` is the workspace's asset-and-BSP tool. It runs as
either a CLI (you pass a subcommand) or as a desktop UI (you run
with no arguments). This chapter uses it to convert a source
image into the runtime RLE format rlvgl decodes on-device.

The tutorial covers the minimum viable conversion. For the full
surface — manifest tracking, font packing, APNG, SVG at arbitrary
DPIs, scaffolded assets crates, BSP generation — read these once
and refer back as needed:

- [`src/bin/creator/README.md`](../../src/bin/creator/README.md) —
  one-page overview and command index.
- [`docs/CREATOR-CLI.md`](../CREATOR-CLI.md) — full CLI reference,
  including every subcommand flag and edge-case.
- [`docs/CREATOR-ASSET-PIPELINE.md`](../CREATOR-ASSET-PIPELINE.md) —
  epic-level design (manifest format, hashing, determinism).
- [`docs/CREATOR-UI-DESIGN.md`](../CREATOR-UI-DESIGN.md) — desktop
  UI interaction model.
- [`docs/IMAGE-COMPRESSION-FORMAT.md`](../IMAGE-COMPRESSION-FORMAT.md)
  — the actual on-device RLE format.

## Steps

### 1. Build `rlvgl-creator`

From the workspace root, following the table in the
[repo README](../../README.md#host-tools-and-simulators):

```bash
cargo build -p rlvgl --bin rlvgl-creator --features creator
```

Add `,creator_ui` if you plan to use the desktop UI below:

```bash
cargo build -p rlvgl --bin rlvgl-creator --features creator,creator_ui
```

### 2. Convert the splash image — pick CLI or UI

Both paths produce the same `.rle` file. Pick whichever fits the
moment; the output is interchangeable.

#### Path A — CLI

Mirror the workflow in [`CREATOR-CLI.md`](../CREATOR-CLI.md) §Quick
start workflow. Put the source image under an `assets/media/`
directory in your crate, then:

```bash
# one-time project init (creates icons/ fonts/ media/ manifest.yml)
cargo run --bin rlvgl-creator --features creator -- init

# hash + index the tree so the manifest knows about the file
cargo run --bin rlvgl-creator --features creator -- scan .

# convert PNG/JPEG media into rlvgl-consumable forms
cargo run --bin rlvgl-creator --features creator -- convert
```

`convert` emits `splash.rle` (and an uncompressed `splash.raw`)
next to the source. See [`CREATOR-CLI.md §convert`](../CREATOR-CLI.md#convert)
for options like target dimensions, colour depth, and dithering.

For an SVG source use the `svg` subcommand instead — see
[`CREATOR-CLI.md §svg`](../CREATOR-CLI.md#svg).

#### Path B — Desktop UI

Launch the UI from the repo root:

```bash
cargo run --bin rlvgl-creator --features creator,creator_ui -- ui
```

Then, following the interaction model in
[`CREATOR-UI-DESIGN.md`](../CREATOR-UI-DESIGN.md):

1. **File → Open Project** and point at the asset folder in your
   tutorial crate (the one with `manifest.yml`).
2. The **Asset Browser** pane lists everything `scan` discovered.
   If you added the source image after opening the project, click
   **Rescan** — this is the UI equivalent of the `scan` subcommand.
3. Select `splash.png` (or whatever you named it) in the browser.
   The right-hand pane shows a preview and format options.
4. Set the target format to **RLE** and the target dimensions to
   **800×480**. Click **Convert**. The UI reports the output path
   and size.
5. **File → Save Manifest** to persist the manifest updates.

The UI shells out to the same conversion code the CLI uses, so the
output byte-for-byte matches Path A.

### 3. Embed the RLE blob

Back in `examples/stm32h747i-disco/Cargo.toml`, add the `splash`
feature (the real crate declares it at
[`Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml) line 29):

```toml
[features]
# ...existing entries...
splash = ["rlvgl-platform/splash"]
```

In `src/main.rs`, declare the RLE blob near the top of the file.
This matches the real crate verbatim
([`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs)
lines 55–56):

```rust
#[cfg(feature = "splash")]
static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");
```

### 4. Decode the splash into the framebuffer

After `board::bring_up()` and before the widget draw loop, decode
the splash directly into the SDRAM framebuffer and hold it for a
moment so the user sees it. The real crate does this around
[`src/main.rs`](../../examples/stm32h747i-disco/src/main.rs) line
4531 (`SPLASH_RLE` is passed to the decoder, then the loop
sleeps a couple of seconds before widget rendering starts).

The shape is:

```rust
#[cfg(feature = "splash")]
{
    let rle = rlvgl_decomp::parse_rle_blob(SPLASH_RLE)
        .expect("splash RLE parse");
    rlvgl_platform::stm32h747i_disco::paint_rle(fb_addr, &rle);

    // Hold the splash ~2s so humans can see it.
    for _ in 0..120 {
        cortex_m::asm::delay(16_000_000 / 60); // ~one 60 Hz frame
    }
}
```

`rlvgl-decomp` is already a transitive dep of the real crate — add
it to `[dependencies]` if your skeleton doesn't have it yet:

```toml
rlvgl-decomp = { path = "../../rlvgl-decomp" }
```

The Chapter 1 widget draw loop runs immediately after. Because the
label writes into the same framebuffer, the splash stays visible
under the text until the loop starts.

## Verify

Build with the new feature flag:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,pac_sdram_init
```

Flash and watch the panel:

```bash
make flash-disco
```

Expected sequence:

1. Panel lights up showing the splash image (full 800×480).
2. After the 2-second hold, the Chapter 1 label appears centered
   on top of the splash.

If the splash looks corrupt (diagonal tearing, wrong colors), the
usual cause is a dimension or format mismatch during conversion —
re-run `convert` with the correct target size, or in the UI
confirm the **Target format: RLE 800×480** option before
re-exporting.

## Going deeper

- [`docs/IMAGE-COMPRESSION-FORMAT.md`](../IMAGE-COMPRESSION-FORMAT.md)
  — the exact bit layout of the RLE blob you just generated.
- [`docs/FILESYSTEM-ASSET-LOADING.md`](../FILESYSTEM-ASSET-LOADING.md)
  — how the finished demo loads assets from SD/QSPI instead of
  `include_bytes!` when those features are on.
- [`docs/CREATOR-TEMPLATES.md`](../CREATOR-TEMPLATES.md) — scaffolding
  a proper assets crate so multiple targets can share art.

---

**[← Prev](01-hello-world.md) · [Index](README.md) · [Next →](03-desktop.md)**
