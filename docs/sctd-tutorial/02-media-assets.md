<!--
02-media-assets.md - Tutorial Chapter 2: converting Bolero artwork to RLE blobs
for firmware embedding, with license attribution and transparency notes.
-->

# Chapter 2 — Converting the media assets

**←** [Chapter 1 — The state charts](01-the-state-charts.md) **·** [Index](README.md) **·** [Chapter 3 — The QML screen → a Rust widget tree](03-qml-to-rlvgl.md) **→**

---

## Attribution — please read this first

The icons and background image used in this chapter come from a third-party
open-source project, and you need to honor its license if you redistribute
them.

> **SCXML Tutorial** by **Alexander Zhornyak** —
> <https://github.com/Alexzhornyak/SCXML-Tutorial>
> Licensed under the **BSD 3-Clause License**, Copyright © 2017
> Alexander Zhornyak. All original content remains the property of its
> author; no endorsement or affiliation is implied.

The Bolero media-player example is example #7 in that tutorial — the *Qt QML
SCXML Infotainment Radio Bolero Simulator*, an original work by Alexander
Zhornyak. The play/pause icons, the repeat-mode icons, the shuffle icons, the
mute icon, the source icons, and the panel background you are about to convert
are all derived from that tutorial.

**What the BSD 3-Clause license requires of you:**

- Keep the copyright notice (reproduced above) intact whenever you redistribute
  these files or any derivative of them.
- Do not use Alexander Zhornyak's name to endorse or promote products derived
  from this work without prior written permission.

The BSD 3-Clause license text ships alongside the tutorial assets in the
reference demo at
`examples/apps/sctd-demo/LICENSES/SCXML-TUTORIAL-BSD-3-Clause.txt`; carry that
file with any bundle you distribute.

rlvgl itself is MIT-licensed (Copyright © 2025 SoftOboros). The vendored
tutorial artwork keeps its upstream BSD 3-Clause terms — the two licenses
coexist cleanly, but they are distinct, and only the artwork is BSD-3.

---

## The asset set

The Bolero artwork consists of two size classes:

**48-pixel transport controls** (icons placed on the player buttons):

| PNG stem | Purpose |
|----------|---------|
| `ImgPlay_48` | Play button |
| `ImgPause_48` | Pause button (swapped in when playing) |
| `ImgRewindBack_48` | Skip backward |
| `ImgRewindForward_48` | Skip forward |
| `ImgMediaNoRepeat_48` | Repeat off |
| `ImgMediaTrackRepeat_48` | Repeat one track |
| `ImgMediaFolderRepeat_48` | Repeat folder |
| `ImgShuffleOff_48` | Shuffle off |
| `ImgShuffleOn_48` | Shuffle on |
| `ImgMute` | Mute indicator |

**128-pixel source-selector icons** (the AUX / SD / USB buttons):

| PNG stem | Purpose |
|----------|---------|
| `ImgAUX_128` | AUX input |
| `ImgSD_128` | SD card |
| `ImgUSB_128` | USB stick |

**Panel background:**

| PNG stem | Purpose |
|----------|---------|
| `ImgBoleroBackground` | Full-panel background image |

These start as PNGs in the tutorial source tree. Once converted, the blobs
live at `examples/apps/sctd-demo/assets/bolero/*.rle` and are embedded by
`examples/apps/sctd-demo/src/qt_assets.rs`.

---

## The conversion command

The core of this chapter is one `rlvgl-creator` command per icon:

```bash
rlvgl-creator compress ImgPlay_48.png ImgPlay_48.rle --transparent-key
```

Run it for each PNG in the table above. The output is an **RLEC blob** — an
RLE-compressed **RGB565** image that firmware can decode quickly on an MCU
without an external image library.

### Why RGB565, and what `--transparent-key` does

RGB565 packs each pixel into 16 bits (5 red, 6 green, 5 blue). It is compact
and fast to blit on memory-constrained hardware. The trade-off is that RGB565
**has no alpha channel** — the format cannot store per-pixel transparency.

That is a problem for icons: the transport controls are glyphs on a transparent
background, and without transparency they will draw as solid rectangles instead
of clean icons.

`--transparent-key` solves this with a sentinel approach:

1. **At encode time:** any source pixel whose alpha value is below 128 (that is,
   more transparent than opaque) is mapped to **magenta (#FF00FF)** instead of
   being blended into the background. All opaque pixels are converted to their
   nearest RGB565 colour normally.
2. **At render time:** the image helper that the generated widget tree uses
   (`qt_image`) detects magenta pixels and treats them as transparent, skipping
   them during blitting.

The result is **1-bit (hard-edged) transparency** — a pixel is either fully
drawn or fully skipped. There is no smooth alpha blending. For icon-style
artwork with clean silhouettes this is perfectly adequate, and it keeps the
blob format uniform and simple.

Use `--transparent-key` for every icon. The background image
(`ImgBoleroBackground`) is fully opaque, so the flag is harmless there — no
source pixels fall below the alpha threshold, so no magenta appears in the
output — but including it keeps your conversion script uniform.

---

## Converting all the icons at once

If you have the PNGs in a single directory, a shell loop is the quickest path:

```bash
for png in *.png; do
    stem="${png%.png}"
    rlvgl-creator compress "$png" "${stem}.rle" --transparent-key
done
```

Each `.rle` file ends up alongside its source PNG. Move the blobs into
`assets/bolero/` in your crate (that is where the reference demo keeps them).

---

## Embedding the blobs

Once you have the `.rle` files, pull them into Rust with `include_bytes!` and
expose them as `pub static` slices. The reference demo does this in
`examples/apps/sctd-demo/src/qt_assets.rs`. The pattern looks like this:

```rust
/// `ImgPlay_48` — vendored Bolero artwork.
pub static IMG_IMGPLAY_48: &[u8] = include_bytes!("../assets/bolero/ImgPlay_48.rle");

/// `ImgPause_48` — vendored Bolero artwork.
pub static IMG_IMGPAUSE_48: &[u8] = include_bytes!("../assets/bolero/ImgPause_48.rle");

// … one line per blob …
```

The symbol names follow the convention `IMG_<STEM_UPPER>` — the PNG file stem
converted to uppercase, prefixed with `IMG_`. For example, `ImgPlay_48.png`
becomes `IMG_IMGPLAY_48`. This naming is not arbitrary: the `rlvgl-creator qt
emit` code generator (Chapter 3) uses the same `asset_symbol` convention when
it emits `Image` widget references, so the names line up automatically. You do
not have to wire the names by hand.

---

## Sanity-checking a converted blob

Before embedding, you can round-trip any blob back to a PNG to verify the
conversion:

```bash
rlvgl-creator decompress ImgPlay_48.rle ImgPlay_48_check.png
```

Open the output and confirm the icon looks right. If magenta pixels appear where
you expected the icon's opaque body, the source PNG may have semitransparent
edges that fell below the 128 threshold — that is usually acceptable for icon
artwork. If the opaque areas look wrong, check that the source PNG was not
already pre-multiplied.

---

## What comes next

With the blobs in place, the whole asset side is done. Chapter 3 takes the QML
screen description for the Bolero media player and runs `rlvgl-creator qt emit`
on it, generating a Rust widget tree that references these same blob symbols —
`IMG_IMGPLAY_48`, `IMG_IMGPAUSE_48`, and so on — wherever the QML has an
`<Image source="...">` element.

Chapter 4 then adds the reactive wiring: the play blob swaps to the pause blob
when the state machine transitions to its *playing* state, the three repeat-mode
icons cycle as the machine steps through its repeat states, and so on. The blobs
themselves never change — the bindings just tell each `Image` widget which one
to display at any given moment.

---

**←** [Chapter 1 — The state charts](01-the-state-charts.md) **·** [Index](README.md) **·** [Chapter 3 — The QML screen → a Rust widget tree](03-qml-to-rlvgl.md) **→**
