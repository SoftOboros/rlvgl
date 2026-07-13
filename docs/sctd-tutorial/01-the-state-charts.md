<!--
01-the-state-charts.md - Chapter 1 of the State Chart → Reactive UI tutorial.
Covers the two worked state charts (Dining Philosophers and Bolero media player),
converting them to Rust state-machine crates via iState, the generated machine API,
and the In() normalization pattern.
-->

# Chapter 1 — The state charts

**←** [Index](README.md) **·** [Chapter 2 — Converting the media assets](02-media-assets.md) **→**

---

This chapter is about the "brain" half of the pipeline.  By the end you will
have two ready-to-compile Rust state-machine crates — one per demo screen —
downloaded from [iState](https://softoboros.com/istate). You can generate in
the browser or through the iState MCP tools. You will not write the generated
Rust by hand; the tool writes it for you.

## The two state charts

The demo has two runnable machine screens behind the Setup selector shell.
Each runnable screen is driven by a separate state machine, and the machines
have nothing to do with each other except that both live in the same embedded
binary.

### Dining Philosophers

The classic concurrency puzzle: five philosophers sit at a round table.
Each thinks for a while, then gets hungry, picks up both forks next to them,
eats, puts the forks down, and goes back to thinking.  The catch: each fork is
shared between two adjacent philosophers, so only one of them can hold it at a
time.  If every philosopher simultaneously picks up their left fork and waits
for their right one, you get deadlock — everyone waits forever.

As a state chart, each philosopher is a parallel region cycling through
`Thinking → Hungry → Eating`.  Each fork is its own parallel region with a
`Free / Taken` state.  Transitions between `Hungry` and `Eating` are guarded:
the philosopher can only enter `Eating` when both adjacent forks are `Free`.

The *interactive* variant used in the demo adds host-injected lifecycle
events: philosophers can arrive or depart mid-session (so the table size
changes), the host can send a "break deadlock" nudge that releases a blocked
fork, and a "reset" event returns everyone to `Thinking`.  These exist so a
human at a touch screen can observe — and escape — a deadlock in real time.

Why a state chart for this?  Because the concurrency structure — one
independent region per philosopher, one per fork, transitions guarded by
adjacent state — is exactly what parallel regions in SCXML express naturally.
You do not write a lock or a semaphore; you describe the legal configurations
and the machine enforces them.

### Bolero media player

The second screen is modelled on the Škoda *Bolero* infotainment
head-unit — a car media player with transport controls, a mute button, a
repeat-mode button, a shuffle button, and an audio-source selector (USB / SD /
AUX).  The original state chart comes from the
[SCXML Tutorial by Alexander Zhornyak](https://github.com/Alexzhornyak/SCXML-Tutorial)
(BSD 3-Clause License).

The structure is a set of **orthogonal (parallel) regions** that run
concurrently inside a `mediaPlayerRun` superstate:

| Region | States |
|--------|--------|
| `playbackRegion` | `mediaStopped` · `mediaPlaying` · `mediaPaused` |
| `muteRegion` | `muteOff` · `muteOn` |
| `shuffleRegion` | `mediaPlayMixModeOff` · `mediaPlayMixModeOn` |
| `repeatRegion` | `mediaRepeatOff` · `mediaRepeatTrack` · `mediaRepeatFolder` |
| source | tracked by a datamodel variable `s_source` (see below) |

Each region runs independently.  Pressing play/pause steps `playbackRegion`.
Pressing mute steps `muteRegion` — regardless of what the playback region is
doing.  Pressing repeat cycles `repeatRegion` through its three states.

This is why state charts suit this problem: you express the behaviour of each
control once, in its own region, and the machine composes them without extra
glue code.

The key insight for Chapter 4 is that **the UI reads these regions as named
states**.  When `mediaPlaying` is active, you show the pause icon.  When
`muteOn` is active, the mute indicator lights up.  When `mediaRepeatTrack` is
active, the repeat button shows the single-track icon.  The names are the
contract between machine and screen.

## Converting with iState

[iState](https://softoboros.com/istate) is a hosted state-chart tool that takes
SCXML or scjson and produces a ready-to-compile Rust crate implementing the
state machine. The browser and MCP service use the same generation boundary.

### Browser workflow

The general workflow:

1. **Open** [softoboros.com/istate](https://softoboros.com/istate) in a browser.
2. **Import your SCXML file.**  Use the import or upload option and point it at
   your `.scxml` document.  For this tutorial, the source files are in
   `examples/apps/sctd-demo/machines/` — use the `source/` subdirectory
   inside each machine's folder.
3. **Review the chart.**  iState displays the state hierarchy, parallel
   regions, transitions, guards, and datamodel variables.  Check that the
   import looks right before generating.
4. **Export the Rust crate.**  Trigger the code-generation export.  iState
   produces a self-contained Rust crate — a `Cargo.toml` plus a `src/lib.rs`
   — and offers it as a download.
5. **Drop the crate into your workspace.**  Unzip the download into your
   project tree and add it to your workspace's `Cargo.toml`.  The finished
   demo keeps each machine under
   `examples/apps/sctd-demo/machines/<name>/`.

Repeat for each chart.  The Dining Philosophers and the Bolero media player
are independent crates; both live side by side in the demo.

### MCP workflow

Use MCP when generation needs to be repeatable from an agent session or when
you are producing several target languages from the same chart. The iState MCP
surface exposes four operations:

| Tool | Purpose |
|---|---|
| `istate_upload_xml` | Upload the authoritative SCXML document and return its document identity |
| `istate_codegen_create` | Start a code-generation job; request `target_langs=["rust"]` for this tutorial |
| `istate_codegen_status` | Poll the job until it succeeds or returns a diagnostic |
| `istate_codegen_download` | Download the generated artifact bundle |

The exact argument envelope is shown by the MCP client when the tools are
connected, but the sequence is stable:

1. Read the complete SCXML source as text and call `istate_upload_xml`. Give
   the document a durable slug that identifies the application and chart.
2. Pass the returned document identity to `istate_codegen_create` with
   `target_langs=["rust"]`.
3. Poll `istate_codegen_status` using the returned job identity. Do not infer
   success from elapsed time; preserve any generator diagnostic with the run
   record.
4. Call `istate_codegen_download` only after success, unpack the Rust bundle
   into a temporary directory, and run its vector tests there.
5. Replace the vendored machine crate as one generated unit. Keep the source
   SCXML, generator provenance, and any emitted self-manifest alongside it.

Do not patch generated `src/lib.rs` to make a failing chart compile. Correct
the SCXML or the generator, rerun the MCP sequence, compare the new bundle,
and then update the vendored output. Hand-maintained packaging metadata should
be clearly separated; the reference machine manifests document which fields
are restored after generation.

The same MCP workflow can request other admitted targets. For example, the
SCTD HDL probe uses `target_langs=["verilog", "vhdl"]`; that output is a
separate projection and does not replace the Rust machine used by this demo.
See [`examples/apps/sctd-demo/hdl/README.md`](../../examples/apps/sctd-demo/hdl/README.md)
for its contract and limitations.

> The finished generated crates are already present in the reference
> implementation at
> [`examples/apps/sctd-demo/machines/`](../../examples/apps/sctd-demo/machines/).
> If you want to follow along without running iState right now, you can read
> the chapter against those files and come back to generate your own later.

## The generated machine API

The generated crate exposes a single `Machine` struct.  The API is
string-based: events are plain `&str` names, and active states are queried by
name.  There are no enums to import or match on — you send the event name that
the SCXML document declares, and you ask whether a named state is currently
active.

Here is the full public surface you will use:

```rust
/// Construct a new machine (datamodel initialized, not yet started).
pub fn new() -> Machine

/// Enter the initial configuration and run onentry actions.
pub fn start(&mut self)

/// Deliver one event to the machine.
/// Returns nothing — read state via is_active() or active_states() after the call.
pub fn step(&mut self, event_name: &str, event_data: Value)

/// The first active leaf state.  For a parallel machine, use active_states() instead.
pub fn current_state(&self) -> &str

/// All active leaf states — one per active parallel region.
pub fn active_states(&self) -> &[String]

/// True if the named state is currently active (as a leaf or an ancestor of one).
pub fn is_active(&self, state_id: &str) -> bool

/// Read a datamodel variable by name.
pub fn get_var(&self, name: &str) -> Value
```

`Value` is a tagged enum the generated crate defines:

```rust
pub enum Value {
    Number(f64),
    Int(i64),
    Str(String),
    Bool(bool),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Undefined,
}
```

It has helpers like `is_truthy()` (ECMAScript semantics) and
`to_display_string()`.  You mostly pass `Value::Undefined` as the event
payload unless your chart reads `_event.data` in a guard — the Bolero chart
does not.

### A short usage example

```rust
use media_player_norm::{Machine, Value};

// 1. Construct and start
let mut player = Machine::new();
player.start();

// At this point the machine is in mediaPlayerIdle — waiting for a source.

// 2. Connect a source and signal ready
player.step("Inp.Media.Ready", Value::Undefined);
player.step("Inp.Media.ValidSource", Value::Undefined);

// Now in mediaPlayerRun > mediaStopped (plus the parallel regions)

// 3. Press play
player.step("Inp.Media.PlayPause", Value::Undefined);

// 4. Check what's active
assert!(player.is_active("mediaPlaying"));
assert!(player.is_active("muteOff"));     // still un-muted
assert!(player.is_active("mediaRepeatOff")); // no repeat

// 5. Toggle mute
player.step("Inp.Media.Mute", Value::Undefined);
assert!(player.is_active("muteOn"));

// 6. Read a datamodel variable
let source = player.get_var("s_source");
// Value::Str("USB") — default source
```

`is_active` is the key predicate.  In Chapter 4 you will wire it directly to
widget visibility and image-source bindings: when `mediaPlaying` is active,
show the pause icon; when `muteOn` is active, show the mute overlay; and so on.

## The In() gotcha — and how to fix it

SCXML includes a built-in predicate called `In(stateId)` that a guard
expression can call to test whether a named state in *another* parallel region
is currently active.  You might write something like:

```xml
<transition event="Inp.Media.Play" cond="!In('muteOn')" target="mediaPlaying"/>
```

This is perfectly legal SCXML.  The problem is that `In()` crosses a boundary:
it jumps from the expression evaluator into the state-machine runtime.  The
constrained ECMAScript evaluator that iState targets for embedded use does not
support `In()` — it cannot resolve it without hooking back into the runtime,
which would break the embedded-IR architecture.  If your chart uses `In()`,
iState will flag those guards as blocked and stub them with safe no-op
placeholders.

The Bolero chart originally used `In()` for several guards (mute state,
repeat mode, source availability).  The normalized form used throughout this
tutorial replaces every `In()` check with a **datamodel variable** that
mirrors the relevant region's state:

| Variable | What it mirrors | Kept in sync by |
|----------|-----------------|-----------------|
| `s_mute` | `muteOn` / `muteOff` | transitions that enter/leave `muteOn` |
| `s_repeat` | repeat mode | transitions that enter each repeat state |
| `s_source` | active source | transitions that handle `Inp.Media.Source.*` |

When the machine enters `muteOn`, a transition action sets `s_mute = true`.
When it exits back to `muteOff`, another action sets `s_mute = false`.  A
transport guard that used to read `!In('muteOn')` now reads `!s_mute`.  The
behaviour is identical; the expression evaluator can handle it.

The practical rule: **if your chart uses `In()`, replace each occurrence with
a boolean or string variable that the relevant transitions keep in sync.**
This is a one-time normalization.  Once the variables are in the datamodel,
guards across all regions can read them freely.

The normalized Bolero source file is at
`examples/apps/sctd-demo/machines/media-player/source/media_player_normalized.scxml`
— the provenance comment at the top of that file lists every `In()` that was
removed and which variable replaced it.  Use it as a reference if you are
normalizing your own chart.

---

## What's next

Chapter 2 converts the Bolero artwork — the play/pause, mute, repeat, shuffle,
and source icons — from PNG files into compact RLE blobs you can embed
directly in firmware.

**←** [Index](README.md) **·** [Chapter 2 — Converting the media assets](02-media-assets.md) **→**
