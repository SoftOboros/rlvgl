<!--
README.md - Progressive tutorial for turning a state chart + a Qt/QML
screen into a reactive rlvgl UI, using the worked SCTD demo (Dining
Philosophers + Skoda Bolero media player). Written for a reader outside
the Softoboros monorepo: the state-machine half is produced by the
hosted iState tool at https://softoboros.com/istate; the view half by
the public `rlvgl-creator` CLI. Each chapter ends with nav links.
-->

# State Chart → Reactive UI — A Progressive Tutorial

This series walks you through the whole pipeline that turns **a state
chart and a Qt/QML screen** into **a reactive embedded UI in Rust** —
and runs the result on a microcontroller. You build it the way the
finished demo was actually built, one stage at a time.

The worked example is the **SCTD demo**: two screens — the classic
**Dining Philosophers** concurrency puzzle and a **car-radio media
player** modelled on the Škoda *Bolero* infotainment example — sharing
one screen-selector shell, running live on an **ESP32-P4** panel (and on
your desktop in a simulator).

You will use two public tools and nothing else:

1. **[iState](https://softoboros.com/istate)** — a hosted web tool that
   converts a *state chart* (SCXML/scjson) into a ready-to-compile Rust
   **state-machine crate**. This is the "brain" of each screen.
2. **`rlvgl-creator`** — the command-line tool shipped with
   [rlvgl](https://github.com/softoboros/rlvgl) (this repository). It
   converts a *Qt/QML screen* into a Rust **widget tree**, and converts
   *image assets* into compact embedded blobs. This is the "face" of
   each screen.

The last stage — the one that makes the demo *reactive* — is wiring the
brain to the face so that machine state drives what you see, and taps on
the screen drive the machine.

> **You do not need access to any private repository to follow this.**
> iState runs in your browser; `rlvgl-creator` is open source; the state
> charts and artwork come from a third-party tutorial whose license is
> reproduced below. Everything here is reproducible from public tools.

## What you will build

| Stage | Result |
|-------|--------|
| End of Chapter 1 | A generated Rust **state-machine crate** for each screen, downloaded from iState — the Dining Philosophers logic and the Bolero media logic, driven entirely by events. |
| End of Chapter 2 | The Bolero **artwork** (play/pause, repeat, shuffle, mute, source icons) transcoded into compact RLE blobs you can embed in firmware. |
| End of Chapter 3 | A generated Rust **widget tree** for the media-player screen — every rectangle, label, and image from the QML, laid out and ready to draw. |
| End of Chapter 4 | The widget tree **wired** to the state machine: the play icon flips to pause when the machine is playing, the mute icon hides when un-muted, tapping a transport button steps the machine, and the source caption reads live track text. |
| End of Chapter 5 | The whole thing **running** — first in a desktop simulator, then flashed to an ESP32-P4 panel. |

## The big picture

Two independent halves are generated separately and then joined:

```
   STATE CHART  ────────►  iState (softoboros.com/istate)  ────►  Rust state-machine crate
   (SCXML/scjson)                                                  Machine::{step, is_active, get_var}
        the "brain"                                                            │
                                                                               │  --scxml-context
   QML SCREEN  ──────────►  rlvgl-creator qt emit  ──────────────►  Rust widget tree  ◄──┘
        the "face"                                                  build_screen() + Binding list
                                                                               ▲
   PNG ARTWORK ──────────►  rlvgl-creator compress  ───────────────────────────┘
                                                                  embedded RLE image blobs
```

- The **brain** knows *what is true* (is the player playing? is it
  muted? which repeat mode?) but nothing about pixels.
- The **face** knows *how things look* but nothing about logic.
- The **wiring** (Chapter 4) is a small, generated set of *bindings*
  that connect named machine states to widgets — plus a refresh call you
  make whenever the machine changes.

Keeping the two halves separate is the whole point: you can redraw the
screen without touching the logic, and re-model the logic without
touching the screen.

## Attribution — the state charts and artwork

The state charts and the media-player artwork used throughout this
tutorial come from a third-party open-source project:

> **SCXML Tutorial** by **Alexander Zhornyak** —
> <https://github.com/Alexzhornyak/SCXML-Tutorial>
> Licensed under the **BSD 3-Clause License**, Copyright © 2017
> Alexander Zhornyak. All original content remains the property of its
> author; no endorsement or affiliation is implied.

The "Bolero" media player is example #7 in that tutorial — the *Qt QML
SCXML Infotainment Radio Bolero Simulator*, an original work by
Alexander Zhornyak. The Dining Philosophers state chart and the
media-player state chart, the QML screen, and every play / pause /
repeat / shuffle / mute / source icon are derived from that tutorial and
remain under its BSD 3-Clause terms.

**If you reuse these assets, keep the copyright notice.** Chapter 2
shows exactly which files are involved and reproduces the license text
in full. rlvgl itself is MIT-licensed (Copyright © 2025 SoftOboros); the
vendored tutorial assets keep their upstream BSD 3-Clause license.

## Prerequisites

A browser and a Rust toolchain are enough for Chapters 1–4. Chapter 5
adds an optional hardware path.

- **A web browser** — for [softoboros.com/istate](https://softoboros.com/istate).
- **Rust toolchain** (`rustup`, stable) — to compile the generated
  crates and the demo.
- **`rlvgl-creator`** — build it once from this repo:
  ```bash
  cargo build --features creator --bin rlvgl-creator
  ```
  Every `rlvgl-creator …` command in this tutorial runs through that
  binary; the long form is
  `cargo run --features creator --bin rlvgl-creator -- <args>`.
- **(Chapter 5, optional) An ESP32-P4 board** + the ESP-IDF toolchain,
  if you want to flash the result. A desktop simulator path is given for
  readers without the hardware.

You do **not** need Qt installed. The `.qml` and `.scxml` source files
travel with the tutorial assets; `rlvgl-creator` and iState read them
directly.

## The finished example

Once you have followed the chapters, the complete reference
implementation lives in this repo at
[`examples/apps/sctd-demo/`](../../examples/apps/sctd-demo/) (the
shared, no-std UI controller) and
[`examples/beetle-esp32p4-idf/`](../../examples/beetle-esp32p4-idf/) (the
ESP32-P4 firmware that hosts it). The crate is at version **0.2.5** —
the first release able to produce this demo end-to-end. Compare your work
against it as you go.

## Chapters

| Ch | Title | What you produce | Tool |
|----|-------|------------------|------|
| [1](01-the-state-charts.md) | The state charts | A generated Rust state-machine crate per screen | iState |
| [2](02-media-assets.md) | Converting the media assets | Embeddable RLE image blobs (with transparency) | `rlvgl-creator compress` |
| [3](03-qml-to-rlvgl.md) | The QML screen → a Rust widget tree | A generated `build_screen()` widget module | `rlvgl-creator qt emit` |
| [4](04-reactive-wiring.md) | Wiring it reactive | Bindings joining machine state ↔ widgets | `rlvgl-creator qt emit --scxml-context` |
| [5](05-build-and-run.md) | Build and run it | The demo on desktop, then on an ESP32-P4 | `cargo` / `idf.py` |

Each chapter file starts and ends with a nav strip: **← Prev · Index ·
Next →**. Read top to bottom, or jump using the table above.

## What's out of scope

This tutorial teaches the *pipeline*, not every pixel of the finished
demo. The screen-selector shell, the Dining Philosophers table
rasterizer, the setup screens, and the desktop/board host glue are
present in the finished example but are ordinary rlvgl UI code; they are
not part of the state-chart-to-reactive-UI path and are left for you to
read in the reference crate. Each chapter points at the specific
reference file when it skips over something.

---

**Next →** [Chapter 1 — The state charts](01-the-state-charts.md)
