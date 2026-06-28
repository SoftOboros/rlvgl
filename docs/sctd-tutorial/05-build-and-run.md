<!-- Chapter 5 of the SCTD tutorial: building and running the finished demo on
     the desktop simulator and on the ESP32-P4. -->

# Chapter 5 — Build and run it

**←** [Chapter 4 — Wiring it reactive](04-reactive-wiring.md) **·** [Index](README.md) **·** [Done — back to the index](README.md) **→**

---

You have a generated state-machine crate, a generated widget tree, and the
bindings that join them. This chapter shows how to run the result — first
on your desktop to prove the logic and rendering before touching hardware,
then on the ESP32-P4 panel.

---

## Desktop verification

There is no separate compile step needed before running the desktop
simulator. The `rlvgl-example-disco-sim` workspace member includes a
purpose-built binary, `rlvgl-sctd-sim`, that mounts the SCTD demo app on
the same Wgpu-backed display host used by the rest of the disco simulator:

```sh
cargo build -p rlvgl-example-disco-sim --bin rlvgl-sctd-sim
cargo run   -p rlvgl-example-disco-sim --bin rlvgl-sctd-sim
```

The simulator window opens at 800 × 480 (the P4 panel's native resolution).
You should see the screen-selector shell on the right edge and the first
screen — Dining Philosophers — filling the left. Tap or click the
media-player item in the selector to switch screens.

On the media-player screen, verify that every binding from Chapter 4 is
live:

- **Play / Pause** — clicking the play button sends the `play` event; the
  icon flips to a pause symbol. Clicking again sends `pause`; it flips
  back.
- **Mute** — the mute icon is hidden when the machine is in an un-muted
  state; it reappears when you toggle mute.
- **Repeat** — each tap cycles the machine through its three repeat states
  (`off`, `one`, `all`); the repeat icon changes glyph to match.
- **Shuffle** — tapping the shuffle button fires a `shuffle` event into the
  machine and the button's visual state updates.
- **Source caption** — the label at the bottom of the screen shows the
  current track text, supplied via the `ExternalText` binding from the
  host application.

If a binding is absent or wrong, this is the cheapest place to debug it:
the host runs the same Rust code as the firmware, there is no cross
compilation involved, and `println!` works normally.

### Vector tests: proving the logic

The simulator proves the rendering path. The machine crates carry their own
deterministic vector suites that prove the logic in isolation — no display
required. Run them straight from the workspace:

```sh
cargo test --manifest-path \
  examples/apps/sctd-demo/machines/media-player/Cargo.toml

cargo test --manifest-path \
  examples/apps/sctd-demo/machines/dining-philosophers-interactive/Cargo.toml
```

These tests are the authoritative gate for the state-machine crates. They
exercise every state transition, guard, and data-model update that iState
generated, and they run in milliseconds on any host. If you modified a
generated machine crate — or regenerated it from an updated SCXML — run
these first.

To run all SCTD-related host tests at once (widget tree pixel tests, app
integration tests, and the machine vectors via the workspace member):

```sh
cargo test -p rlvgl-app-sctd-demo
```

Between the simulator run and the vector tests, you have two independent
checks: the logic is correct (vectors pass), and the rendering is correct
(the simulator shows what you expect). Both should be green before you
flash.

---

## On hardware — ESP32-P4

The firmware that hosts the Rust payload lives in
`examples/beetle-esp32p4-idf/`. It is a standard ESP-IDF CMake project.
C owns the hardware bring-up (DSI display, PSRAM, I2C touch controller),
and Rust owns the pixels. The CMake build system calls `cargo build`
automatically as an ExternalProject step, so a normal `idf.py build` builds
the Rust staticlib too.

Before building you need:

- **ESP-IDF** — sourced and on your `PATH`.
- **The RISC-V Rust target** — add it once with:
  ```sh
  rustup target add riscv32imafc-unknown-none-elf
  ```
- **`cargo` on `PATH`** — the CMake build script calls it directly.

Build and flash:

```sh
cd examples/beetle-esp32p4-idf
. $IDF_PATH/export.sh
idf.py set-target esp32p4
idf.py build
idf.py flash monitor
```

The `idf.py flash monitor` command flashes the firmware and opens the
serial console. You should see the IDF boot log, DSI bus lock confirmation,
and then the widget tree rendering loop begin.

### How the payload is selected

The Rust staticlib is built from the glue crate at
`examples/beetle-esp32p4-idf/components/rlvgl_app/rust/`. Its
`Cargo.toml` declares a `default = ["app_sctd"]` feature set, which pulls
in `rlvgl-app-sctd-demo` as the active payload. You can inspect or modify
this at:

```
examples/beetle-esp32p4-idf/components/rlvgl_app/rust/Cargo.toml
```

To swap back to the original disco-demo payload instead, build with:

```sh
idf.py -DRLVGL_PAYLOAD=disco build
```

### Isolation sdkconfigs

The `examples/beetle-esp32p4-idf/` README describes optional
`sdkconfig.defaults.*` overlays that let you wake the display bridge
without initializing the full DSI/DPI path — useful for diagnosing hardware
bring-up issues. You do not need them for a normal flash.

---

## What you should see

On the panel, the screen-selector shell appears on the right edge. The left
area shows the **Dining Philosophers** screen with the philosophers animated
around the table. Tap the media-player entry in the selector to switch to
the **Bolero media player**.

On the media-player screen:

- The **play icon** flips to a pause icon when the player is active, and
  back when paused.
- The **mute icon** appears and disappears in response to the mute button.
- The **repeat icon** cycles through its three-state glyph sequence each
  time you tap it.
- **Shuffle taps** register visually.
- The **source caption** at the bottom shows the current track text, fed
  from the host application through the `ExternalText` binding you wired
  in Chapter 4.

This is the full reactive loop you built: a tap on the screen sends an
event into the machine; the machine steps; the updated state flows back
through the bindings into the widget tree; the renderer writes updated
pixels to the framebuffer.

---

## Compare against the finished example

Everything you built in this tutorial is the same code that lives in the
reference implementation under:

- `examples/apps/sctd-demo/` — the shared, no-std UI controller (machine
  crates, widget tree, bindings, selector shell).
- `examples/beetle-esp32p4-idf/` — the ESP32-P4 firmware that hosts it.

The parts the tutorial deliberately left out — the screen-selector shell,
the Dining Philosophers table rasterizer, the setup screens — are ordinary
rlvgl UI code you can now read directly. None of them involve the
state-chart-to-reactive-UI pipeline; they use the same `rlvgl-core` widget
primitives you have already worked with.

---

## Where to go next

The two halves of this pipeline are independent by design; either one can
evolve without touching the other.

**Re-skin the view.** Edit the `.qml` file and re-run `rlvgl-creator qt
emit` (with `--scxml-context` to regenerate the bindings). The widget tree
and binding list regenerate; the machine crate is untouched.

**Re-model the logic.** Open the SCXML in iState, change the state chart,
and re-export the Rust crate. The machine crate regenerates; the widget
tree is untouched. Run the vector tests to confirm the new logic, then
rebuild the firmware.

**Add a new screen.** Generate a new machine crate and a new widget tree,
wire them together following the pattern from Chapter 4, and mount the new
screen in the selector shell.

---

Congratulations on finishing the tutorial. You started with a state chart
and a QML file and ended with a reactive embedded UI running on real
hardware. The two halves — the iState-generated machine crate and the
`rlvgl-creator`-generated widget tree — were built and verified
independently and joined by a small, generated set of bindings. That
separation is what makes the whole thing maintainable: change the brain
without touching the face, or change the face without touching the brain.

---

**←** [Chapter 4 — Wiring it reactive](04-reactive-wiring.md) **·** [Index](README.md) **·** [Done — back to the index](README.md) **→**
