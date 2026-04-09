# rlvgl-playit

A mini-playwright test driver for [rlvgl](https://github.com/softoboros/rlvgl).

`rlvgl-playit` lets you drive an rlvgl UI from an external host — injecting
input events, querying widget state by tag, and inspecting framebuffer pixels —
over any byte-oriented transport (serial UART, stdin/stdout, TCP, or
in-process).

## Features

- **Input injection** — send any `Event` variant (tap, pointer, key, tick,
  multi-touch) through the widget tree, by coordinate or by widget tag.
- **Gesture routing** — optionally pipe raw events through a gesture
  recognition pipeline (`EventPipeline` trait) so `PointerDown`/`Up` sequences
  produce the same `PressRelease` / `DoubleTap` events as real hardware.
- **Event recording** — capture a timed input session with per-tick deltas and
  dump it for replay.  Record from live hardware, replay in CI.
- **Widget tagging** — address widgets by name instead of pixel coordinates.
  Tags are `Option<&'static str>` on `WidgetNode`, zero-cost when unused.
- **Widget queries** — ask for bounds, existence, or child count of a tagged
  widget.
- **Framebuffer inspection** — dump ARGB8888 pixel windows, synchronised to
  display present count.
- **`no_std` by default** — runs on bare-metal targets (STM32, etc.) with no
  allocator required.
- **Transport-agnostic** — implement the `PlayitTransport` trait (two methods)
  to plug in any byte stream.
- **Text wire protocol** — human-readable, backward-compatible with the
  original serial commands (`T`, `D`, `?`), and extensible for new input types.

## Quick start

Add the dependency:

```toml
[dependencies]
rlvgl-playit = "0.1"
```

### Embedded (serial transport)

```rust,ignore
use rlvgl_playit::{PlayitExecutor, PlayitTransport, StatusData, NullPipeline};

struct SerialTransport;

impl PlayitTransport for SerialTransport {
    fn read_byte(&mut self) -> Option<u8> {
        serial::pop_rx()
    }
    fn write_bytes(&mut self, bytes: &[u8]) {
        serial::write_bytes(bytes);
    }
}

let mut executor = PlayitExecutor::new(SerialTransport);
let mut pipeline = NullPipeline;  // or a real GesturePipeline

// In your main loop:
let status = StatusData { tick_count, present_count };
executor.poll(&mut root, &status, Some(&fb_reader), &mut pipeline, |ext| {
    // handle app-specific extension commands
});
```

### Gesture routing

```rust,ignore
use rlvgl_playit::EventPipeline;
use rlvgl::platform::gesture::TapRecognizer;

struct GesturePipeline { tap: TapRecognizer }

impl EventPipeline for GesturePipeline {
    fn process(&mut self, event: Event) -> (Option<Event>, Option<Event>) {
        (self.tap.process(&event), None)
    }
    fn tick(&mut self) -> (Option<Event>, Option<Event>) {
        (self.tap.tick(), None)
    }
}
```

Now `PD100,200` followed by `PU100,200` produces a debounced `PressRelease`
that buttons actually respond to.

### Widget tagging

```rust,ignore
use rlvgl_core::WidgetNode;

let node = WidgetNode::new(my_widget)
    .with_tag("settings_btn");
```

Then from the test host:

```text
QE:settings_btn        -> EXISTS:1
QB:settings_btn        -> BOUNDS:10,20,80,40
T@settings_btn:40,30   -> OK
```

### Event recording and replay

```text
-> RS                   <- REC:recording
  (user interacts with the screen)
-> RE                   <- REC:START,4
                        <- @0 PD100,200
                        <- @3 PM105,205
                        <- @6 PU110,210
                        <- @12 PD300,150
                        <- REC:END
```

The `@<delta>` prefix is the tick count since the previous event.  A host
replay script converts these to wall-clock delays and sends the commands back.

## Wire protocol

Commands are single lines terminated by `\n` or `\r\n`.

| Command | Wire format | Description |
|---------|-------------|-------------|
| Tap | `T<x>,<y>` | Inject `PressRelease` |
| Press down | `TD<x>,<y>` | Inject `PressDown` |
| Double tap | `TT<x>,<y>` | Inject `DoubleTap` |
| Tick | `TK` | Inject `Tick` |
| Pointer down | `PD<x>,<y>` | Inject `PointerDown` |
| Pointer up | `PU<x>,<y>` | Inject `PointerUp` |
| Pointer move | `PM<x>,<y>` | Inject `PointerMove` |
| Multi-touch | `MT<n>:<id>,<s>,<x>,<y>;...` | Inject `Touch` (s=D/U/C) |
| Key down | `KD:<key>` | Inject `KeyDown` (Enter, Escape, a, F5, ...) |
| Key up | `KU:<key>` | Inject `KeyUp` |
| Tagged tap | `T@<tag>:<x>,<y>` | Inject `PressRelease` to tagged widget |
| Dump pixels | `D<x>,<y>,<w>,<h>[,<frames>]` | Dump ARGB hex from framebuffer |
| Query bounds | `QB:<tag>` | Get widget bounds |
| Query exists | `QE:<tag>` | Check if widget exists |
| Query children | `QC:<tag>` | Count children of widget |
| Record start | `RS` | Start recording input events |
| Record stop | `RE` | Stop recording and dump entries |
| Record dump | `RD` | Dump entries without stopping |
| Status | `?` | Get tick/present telemetry |
| Extension | `X<payload>` | App-defined (e.g. toggle animations) |

## Feature flags

| Flag | Effect |
|------|--------|
| *(default)* | `no_std`, no allocator — core types, protocol, traits |
| `alloc` | Enables heap-backed helpers |
| `std` | Enables `alloc` plus standard-library integrations |

## License

MIT
