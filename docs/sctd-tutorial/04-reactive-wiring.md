<!-- Chapter 4 of the SCTD tutorial — the climax chapter. Joins the iState-generated
     machine crate from Chapter 1 to the widget tree from Chapter 3 so the UI reacts. -->

# Chapter 4 — Wiring it reactive

**←** [Chapter 3 — The QML screen → a Rust widget tree](03-qml-to-rlvgl.md) **·** [Index](README.md) **·** [Chapter 5 — Build and run it](05-build-and-run.md) **→**

---

You now have two independent pieces: the machine crate from Chapter 1 (which
knows what is true — playing? muted? which repeat mode?) and the widget tree
from Chapter 3 (which knows how things look, but nothing about logic). This
chapter connects them.

The connection has three directions:

- **Machine → pixels.** When the machine enters a new state, bound widgets
  update their artwork automatically.
- **Pixels → machine.** When the user taps a transport button, the machine
  advances.
- **External data → pixels.** A caption label whose text comes from outside the
  machine — a media-player object, a live track title — also updates, on a
  separate cadence.

All three are driven by a small generated layer that `rlvgl-creator` produces
when you pass it one extra flag.

---

## 4.1 Re-emitting with the machine attached

In Chapter 3 you ran:

```bash
rlvgl-creator qt emit FrameMedia.qml out/ --target rlvgl
```

That produced a static widget tree. The reactive bindings were not generated
because the emitter had no way to know which machine crate the QML predicates
referred to.

Adding `--scxml-context` gives it that information:

```bash
rlvgl-creator qt emit FrameMedia.qml out/ \
    --target rlvgl \
    --scxml-context scxmlBolero=media_player
```

The flag says: _QML expressions qualified by `scxmlBolero.` resolve against the
`media_player` crate_ (the one iState generated in Chapter 1). With it, the
emitter finds every `Image` whose `source:` switches on a `scxmlBolero.<state>`
predicate and every `visible:` binding tied to one, and lowers them into typed
Rust structures.

The signature of `build_screen` changes accordingly:

```rust
// Without --scxml-context: returns (WidgetNode, Rc<RefCell<ScreenState>>)
// With    --scxml-context: returns the machine and binding list too
pub fn build_screen(
    bounds: Rect,
) -> (
    WidgetNode,
    Rc<RefCell<ScreenState>>,
    Rc<RefCell<Machine>>,
    Vec<Binding>,
)
```

The `Machine` is the iState-generated type from `media_player::Machine`. The
`Vec<Binding>` is the set of reactive connections between machine states and
widgets. Without `--scxml-context`, you get neither; you get the static tree
from Chapter 3.

---

## 4.2 Machine → pixels: the binding types

The emitter inspects every QML `Image` and `Text` element that carries a
`scxmlBolero.`-qualified expression and lowers it into one of four binding
variants, all collected into a sealed `Binding` enum.

### Predicate binding — two-way artwork flip

QML pattern:
```qml
source: scxmlBolero.mediaPlaying ? "ImgPause_48.png" : "ImgPlay_48.png"
```

The emitter lowers this to a `Binding::Predicate` holding the `Image` widget
handle, the state name (`"mediaPlaying"`), and both artwork buffers decoded from
the RLE blobs (Chapter 2). On refresh it calls `machine.is_active("mediaPlaying")`
and sets the image pixels to whichever branch is true.

The play/pause button (`__rep_btn_1` in the generated tree) is the canonical
example. At rest the machine is stopped — `is_active("mediaPlaying")` is false —
so the Play icon is shown. After a PlayPause event drives the machine into
`mediaPlaying`, `is_active` returns true and the Pause icon appears. No
conditional code in your app; the binding handles it.

### Visibility binding — show or hide a widget

QML pattern:
```qml
visible: scxmlBolero.muteOn
```

The emitter lowers this to a `Binding::Visibility` holding the `Image` handle
and the state name. On refresh it calls `machine.is_active("muteOn")` and calls
`set_hidden(!active)` on the widget.

The mute icon (`imgMute`) is the canonical example. When the machine is not in
`muteOn` the widget is hidden; when it is, the icon appears.

### Chained predicate — first-active-wins among several states

QML pattern:
```qml
source: scxmlBolero.mediaRepeatTrack  ? "ImgMediaTrackRepeat_48.png"
      : scxmlBolero.mediaRepeatFolder ? "ImgMediaFolderRepeat_48.png"
      :                                 "ImgMediaNoRepeat_48.png"
```

The emitter lowers this to a `Binding::Chain` holding the image, a list of
`(state_id, artwork)` arms, and a default artwork. On refresh it walks the arms
in order and stops at the first one where `is_active` returns true, then sets
the pixels to that arm's artwork — or to the default if none match.

The repeat-mode icon is the canonical example. Three states, three artworks, one
default; the icon cycles through NoRepeat, TrackRepeat, and FolderRepeat as the
machine advances.

### Collecting them all

```rust
pub enum Binding {
    Label(LabelBinding),
    Predicate(PredicateBinding),
    Visibility(VisibilityBinding),
    Chain(PredicateChainBinding),
    ExternalText(ExternalTextBinding),  // covered in Section 4.4
}
```

All four machine-driven variants are refreshed by a single call:

```rust
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    machine: &Rc<RefCell<Machine>>,
    bindings: &[Binding],
)
```

This function is **not** called automatically. The emitter generates it as a
pure function you call yourself — once at construction to seed the initial
artwork, and again after every `machine.step(...)`. The `ExternalText` variant
is a no-op in `refresh_bindings`; it runs on its own cadence described in Section 4.4.

---

## 4.3 Pixels → machine: the tap table

The emitter does not make the widget tree call the machine directly. Instead it
emits a static table listing every tappable button:

```rust
// From media_player_gen.rs — generated, do not edit
pub const BUTTON_TAP_EVENTS: &[(&str, &str)] = &[
    ("__rep_btn_0",          "MediaFunc.Reverse"),
    ("__rep_btn_1",          "MediaFunc.Play"),
    ("__rep_btn_2",          "MediaFunc.Forward"),
    ("repeatBtn",            "MediaFunc.Repeat"),
    ("__btn_mediafunc_scan", "MediaFunc.Scan"),
    ("__btn_mediafunc_shuffle", "MediaFunc.Shuffle"),
];
```

Each row is a widget tag (used to look up the widget's bounding box in the
generated tree) paired with the raw QML button-event string that the original
`submitBtnSetupEvent(...)` handler sent. The emitter stays app-agnostic: it does
not know what `"MediaFunc.Play"` means to your machine. That translation belongs
to the consumer.

In the demo, the skin owns a `MEDIA_FUNC_MAP` that maps the QML event string to
the machine input string:

```rust
// From media_player_skin.rs
const MEDIA_FUNC_MAP: &[(&str, &str)] = &[
    ("MediaFunc.Play",    "Inp.Media.PlayPause"),
    ("MediaFunc.Repeat",  "Inp.Media.Repeat"),
    ("MediaFunc.Shuffle", "Inp.Media.Shuffle"),
    ("MediaFunc.Reverse", "Inp.Media.Prev"),
    ("MediaFunc.Forward", "Inp.Media.Next"),
];
```

At construction the skin joins these two tables against the actual widget bounds
in the tree to build a resolved tap-target list:

```rust
let tap_targets: Vec<(Rect, &'static str)> =
    media_player_gen::BUTTON_TAP_EVENTS
        .iter()
        .filter_map(|(tag, qml_event)| {
            let bounds = find_bounds_by_tag(&node, tag)?;
            let machine_event = MEDIA_FUNC_MAP
                .iter()
                .find(|(q, _)| q == qml_event)
                .map(|(_, ev)| *ev)?;
            Some((bounds, machine_event))
        })
        .collect();
```

Buttons with no entry in `MEDIA_FUNC_MAP` (here, `MediaFunc.Scan`) are dropped
silently. Buttons whose tag is absent from the built tree are also dropped. The
result is a flat list of `(pixel bounds, machine event string)` pairs that the
event handler can check in a tight loop.

On a pointer-up inside a button's bounds, the consumer looks up the event, calls
`machine.step`, and calls `refresh_bindings` — all in one short method:

```rust
// From media_player_skin.rs
fn step_event(&self, event: &str) {
    self.machine
        .borrow_mut()
        .step(event, media_player::Value::Undefined);
    media_player_gen::refresh_bindings(&self.state, &self.machine, &self.bindings);
}

fn handle_event(&mut self, event: &Event) -> bool {
    if !self.visible { return false; }
    if let Event::PressRelease { x, y } | Event::PointerUp { x, y } = *event {
        for (b, ev) in &self.tap_targets {
            if x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height {
                self.step_event(ev);
                return true;
            }
        }
    }
    self.node.borrow_mut().dispatch_event(event)
}
```

The machine decides what the tap means. The play button does not know it is a
play button — it is a pixel region that sends `"Inp.Media.PlayPause"` to a
machine that decides whether that means start playing or stop.

---

## 4.4 External text: live data that is not in the machine

Some labels on the screen do not read machine state at all. The source-caption
label (`textSource`) shows the currently-playing track title, which the original
QML reads from an external Qt object:

```qml
text: audioPlayer.currentPlayUrlFileName
```

The state machine does not know the track name; it only knows transport state
(playing, paused, etc.). The emitter lowers this to an `ExternalTextBinding`:

```rust
// From media_player_gen.rs
pub const EXTERNAL_TEXT_BINDINGS: &[(&str, &str)] = &[
    ("textSource", "audioPlayer.currentPlayUrlFileName"),
];

pub fn apply_external_text(
    bindings: &[Binding],
    resolve: impl Fn(&str) -> Option<String>,
)
```

The table surfaces the widget tag and the verbatim QML key string. The function
walks the `Binding::ExternalText` entries in the binding list and calls
`resolve(key)` for each; if the resolver returns `Some(value)`, it calls
`set_text` on the label. The emitter does not bake in the value.

The consumer owns the resolver. In the demo, the skin holds a static map and a
small function:

```rust
// From media_player_skin.rs
const EXTERNAL_TEXT_SOURCES: &[(&str, &str)] =
    &[("audioPlayer.currentPlayUrlFileName", "Bolero - Ravel.mp3")];

fn resolve_external_text(key: &str) -> Option<alloc::string::String> {
    use alloc::string::ToString;
    EXTERNAL_TEXT_SOURCES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}
```

A real media player would return the live track title from its player object
here instead of a fixed string. The emitter does not care either way.

`apply_external_text` runs on a **separate cadence** from `refresh_bindings`.
`refresh_bindings` runs when the machine steps (machine-driven). External text
runs when the external source may have changed — every frame in a live player,
once at startup in the demo where the source is fixed. They are independent.

---

## 4.5 The consumer-owned principle

If you look at what the emitter actually generated, the pattern is consistent:

| Emitter produces | Consumer supplies |
|---|---|
| `BUTTON_TAP_EVENTS` — widget tag + raw QML event string | `MEDIA_FUNC_MAP` — QML event → machine input string |
| `EXTERNAL_TEXT_BINDINGS` — widget tag + verbatim QML key | `resolve_external_text` — key → live value |
| `refresh_bindings(state, machine, bindings)` | The call sites (after `step`, at startup) |
| `apply_external_text(bindings, resolve)` | The call sites and the resolver function |

The emitter emits *tables of typed handles* — which widget, which key, which
event string. The consumer supplies *meaning* — what a QML event does to the
machine, what a QML key resolves to at runtime.

This separation is what lets the same emitter serve any app. The QML and the
emitter know the screen structure; only the consumer knows the app.

---

## 4.6 Putting it together

Here is the complete wiring sequence, drawn directly from
[`examples/apps/sctd-demo/src/media_player_skin.rs`](../../../../examples/apps/sctd-demo/src/media_player_skin.rs):

```rust
pub fn new(bounds: Rect) -> Self {
    // 1. Build the widget tree, the ScreenState, the Machine, and the
    //    Binding list all in one call. build_screen() also calls
    //    machine.start() internally.
    let (node, state, machine, bindings) = media_player_gen::build_screen(bounds);

    // 2. Resolve tap targets: join BUTTON_TAP_EVENTS + MEDIA_FUNC_MAP
    //    against the built tree to get (pixel bounds, machine event) pairs.
    let tap_targets: Vec<(Rect, &'static str)> =
        media_player_gen::BUTTON_TAP_EVENTS
            .iter()
            .filter_map(|(tag, qml_event)| {
                let bounds = find_bounds_by_tag(&node, tag)?;
                let machine_event = MEDIA_FUNC_MAP
                    .iter()
                    .find(|(q, _)| q == qml_event)
                    .map(|(_, ev)| *ev)?;
                Some((bounds, machine_event))
            })
            .collect();

    // 3. Seed the machine past its initial idle state so the transport
    //    controls are live (the machine starts in mediaPlayerIdle and
    //    needs a Ready + ValidSource event to reach mediaStopped).
    {
        let mut m = machine.borrow_mut();
        m.step("Inp.Media.Ready",       media_player::Value::Undefined);
        m.step("Inp.Media.ValidSource", media_player::Value::Undefined);
    }

    // 4. Apply the initial machine-driven artwork (Play icon at rest).
    media_player_gen::refresh_bindings(&state, &machine, &bindings);

    // 5. Apply the consumer-owned external-text resolver (the source
    //    caption). This runs independently of the machine.
    media_player_gen::apply_external_text(&bindings, resolve_external_text);

    Self { bounds, node: RefCell::new(node), state, machine, bindings, tap_targets, visible: false }
}
```

After construction, the per-tap path is four lines:

```rust
// Inside handle_event, on PressRelease / PointerUp:
for (b, ev) in &self.tap_targets {
    if x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height {
        self.machine.borrow_mut().step(ev, media_player::Value::Undefined);
        media_player_gen::refresh_bindings(&self.state, &self.machine, &self.bindings);
        return true;
    }
}
```

That is the whole reactive loop.

---

## What you can see now

With the wiring in place, the media-player screen is fully reactive:

- Tap the center transport button: the Play icon flips to Pause (the
  `Binding::Predicate` over `mediaPlaying` swaps the artwork).
- Tap again: Pause flips back to Play.
- Trigger the mute event: the mute icon appears in the header strip
  (the `Binding::Visibility` over `muteOn` calls `set_hidden(false)`).
- Tap the repeat button: the icon cycles through NoRepeat, TrackRepeat,
  FolderRepeat and back to NoRepeat (the `Binding::Chain` walks the arm
  list first-active-wins).
- Tap the shuffle button: the shuffle icon swaps between on and off
  (a second `Binding::Predicate` over `mediaPlayMixModeOn`).
- The source caption reads "Bolero - Ravel.mp3" — written once at
  construction by `apply_external_text`.

The machine decides every one of those outcomes. The emitter generated the
wiring. Your consumer code supplied the vocabulary maps and the resolver. None
of those three pieces knows about the others' internals.

---

**←** [Chapter 3 — The QML screen → a Rust widget tree](03-qml-to-rlvgl.md) **·** [Index](README.md) **·** [Chapter 5 — Build and run it](05-build-and-run.md) **→**
