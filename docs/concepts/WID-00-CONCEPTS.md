# WID-00 — Editable Text Input Concepts

**Status:** Ratified 2026-06-11. Normative for the WID initiative
(editable text Input: edit buffer, caret, key routing).

Requesting ticket: "rlvgl WID — editable text Input (edit buffer,
caret, key routing)" (2026-06-11, wave 2, parallel — a downstream
consumer ships an app-side shim writing to an app-owned buffer rendered
via Label in the meantime and adopts this when published).

## 0. Authority Policy

| Concern | Owner | WID relationship |
|---|---|---|
| `Input` / `Textarea` public surface | `ui/src/input.rs` | WID makes them editable; existing constructors, `set_text`, `on_change`, and style accessors keep their semantics. |
| Key vocabulary | `core/src/event.rs:43-114` (`KeyDown`/`KeyUp`, `Key`) | WID adds `Key::Backspace` (the vocabulary gap; §5.4). Registration policy: Specification Required, same treatment INPUT-00 §5.3 recorded for `Event`. |
| playit key wire grammar | `playit/src/protocol.rs` (`KD:<key>`/`KU:<key>`), `KeySpec` | WID adds `KeySpec::Backspace` (`"Backspace"` / `"BS"`); everything else unchanged. |
| Text rendering | `rlvgl_widgets::label::Label` → `Renderer::draw_text` (backend metrics opaque) | WID renders through the same path; caret geometry uses configurable nominal metrics (§6.3) because the trait exposes no glyph extents. |
| Focus | nobody (no focus concept exists in `core/`) | WID explicitly does NOT introduce one: `set_active(bool)` is the routing surface (§7); a focus manager is named follow-up scope. |

If a WID phase changes a frozen decision in §5–§8, §15 MUST be amended
first in a separate change.

## 1. Purpose

Make `rlvgl-ui`'s `Input` (single-line) and `Textarea` (multi-line)
genuinely editable: an internal edit buffer that consumes `KeyDown`
when active — character insert, backspace delete, Enter
(submit / newline) — with a visible caret, change/submit callbacks,
and max-length + accepted-charset hooks (digits-plus-`*`/`#` dialpad
fields vs free text). Touch applications compose their own on-screen
keyboards from Grid+Buttons; this ticket is only the field the keys
land in.

## 2. Problem Statement

Evidence (all rlvgl-internal):

- `ui/src/input.rs` (172 ln): `Input` and `Textarea` wrap
  `rlvgl_widgets::label::Label`; the only mutation path is
  programmatic `set_text(&mut self, &str)`. No `Event::KeyDown`
  handling, no caret, no backspace semantics — they are labels with a
  change callback, not inputs.
- `core/src/event.rs:43-114` already carries `KeyDown { key }` /
  `KeyUp { key }` with `Key::Character(char)`, `Key::Enter`,
  `Key::Escape`, etc. — the event vocabulary exists (minus backspace);
  no widget consumes it for editing.
- No focus concept exists to route keys to one input among several
  (`core/` has no focus manager).

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Edit buffer** | The widget-owned `String` holding current content; ASCII-bounded in v1 (§5.2), so byte index == char index. | WID |
| **Caret** | Insertion position as a char index into the edit buffer (`0..=len`). For `Textarea`, derived (row, col) against `'\n'`-split lines. | WID |
| **Active** | The state in which a field consumes `KeyDown` (§7). Toggled only by `set_active(bool)`. | WID |
| **Accepted charset** | Predicate `Fn(char) -> bool` filtering insertions *after* the v1 ASCII bound (§5.2/§8.2). | WID |
| **Nominal char metrics** | `char_width` / `line_height` used for caret geometry in lieu of backend glyph extents (§6.3). | WID |
| `Input` / `Textarea` / `Label` / `Key` | As defined in `ui/src/input.rs` / `widgets/src/label.rs` / `core/src/event.rs:118`; `Key` gains the additive `Backspace` variant. | repo |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Edit semantics (insert/backspace/Enter/arrows, caret clamping) | `ui/src/input.rs` — shared edit core |
| Key vocabulary | `core/src/event.rs` — `Key::Backspace` lands beside the existing set |
| Wire encoding for backspace | `playit/src/protocol.rs` — `KD:Backspace` / `KD:BS` |
| Buffer/caret truth tables | `ui/src/input.rs` unit tests |
| End-to-end key→pixels truth | disco-sim playit test (`sim.input` surface, §8.4) |

## 5. Frozen Decisions — Edit Model

1. **Buffer and caret.** Each editable field owns a `String` buffer
   and a caret char-index `0..=len`. Every successful edit (insert,
   backspace, newline) fires `on_change(&buffer)`. Failed edits
   (rejected char, full buffer, backspace at 0) change nothing — no
   callback, caret untouched.
2. **ASCII v1 bound.** Insertable characters are printable ASCII
   `0x20..=0x7E` (matching the creator font pipeline's packed-ASCII
   output), pre-filtered before the accepted-charset hook. No
   IME/Unicode composition, no selection/clipboard (ticket non-goals).
   `set_text` remains unrestricted (programmatic path, unchanged
   semantics); the caret clamps to the new length.
3. **Key handling while active** (consumed → `handle_event` returns
   `true`):
   - `Character(c)`: insert at caret if the ASCII bound, charset hook,
     and max-length all pass; caret advances past the insertion.
   - `Backspace`: delete the char before the caret; no-op at 0.
   - `Enter`: `Input` fires `on_submit(&buffer)` (buffer untouched, no
     `on_change`); `Textarea` inserts `'\n'` (an ordinary edit).
   - `ArrowLeft` / `ArrowRight`: move the caret (clamped). Consumed so
     an active field doesn't fight app-level arrow navigation.
   - Everything else (including `KeyUp`, `Escape`, `ArrowUp/Down` in
     v1): not consumed, passes to the application.
   While **inactive**: no key is consumed, nothing mutates.
4. **`Key::Backspace` lands in core**, `KeySpec::Backspace` in playit
   (parse `"Backspace"` / `"BS"`, format `"Backspace"`). Registration
   policy for `Key`: **Specification Required** (same de-facto
   extensible-enum treatment INPUT-00 §5.3 recorded for `Event`; both
   enums grew variants across prior 0.x releases).

## 6. Frozen Decisions — Rendering

1. **Label remains the text path**: the edit core pushes the buffer
   into the wrapped `Label` after every edit, so colors/style/draw
   flow exactly as today. `Textarea` draws `'\n'`-split lines itself
   (one `draw_text` per line at `line_height` pitch) since backend
   `draw_text` has no newline semantics.
2. **Caret style: solid vertical line caret**, 2 px wide, one
   `line_height` tall, in the style's `border_color`. Drawn only while
   active. **No blink in v1** — caret blink would make rendering a
   function of tick phase and break the D-dump-equality test pattern
   (type "H" → dump A; type "I", backspace → dump must equal A);
   deferred behind the dirty-rect/anim seam (§14).
3. **Nominal char metrics.** The `Renderer` trait exposes no glyph
   extents, so caret x = `bounds.x + caret_col * char_width`, y =
   `bounds.y + caret_row * line_height`; `char_width` default **8**,
   `line_height` default **16** (matching the fontdue backend's 16 px
   nominal line, `TEXT_NOMINAL_LINE_PX`), configurable via
   `with_char_metrics(char_width, line_height)`. Documented
   limitation: proportional backend fonts make the caret approximate;
   monospace/bitmap-font consumers get exact placement. Glyph-accurate
   caret needs renderer metrics — deferred (§14, Coupled).

## 7. Frozen Decisions — Key Routing

1. **Explicit activation, no focus framework.** `set_active(bool)` /
   `is_active()` is the entire routing surface; applications toggle it
   from their own focus bookkeeping. With several fields, the
   application keeps at most one active — the framework does not
   enforce or track this. A focus manager is **named follow-up scope**
   (§14), not part of this ticket.
2. **Consumption contract**: an active field returns `true` from
   `handle_event` exactly for the keys in §5.3 — so tree dispatch
   stops at the field for editing keys, and applications can rely on
   unconsumed keys (e.g. `Escape`) for their own wiring (deactivation,
   navigation).

## 8. Frozen Decisions — Hooks & Observability

1. **Max length**: `with_max_len(usize)`; inserts beyond it are
   rejected edits (§5.1). Default: unlimited.
2. **Accepted charset**: `with_accept(impl Fn(char) -> bool)` runs
   after the ASCII bound; rejection is a failed edit — buffer and
   caret provably untouched (acceptance test). Example shipped in
   tests: dialpad `c.is_ascii_digit() || c == '*' || c == '#'`.
3. **Submit**: `Input::on_submit(impl FnMut(&str))` fires on `Enter`
   while active.
4. **Sim observability**: the disco-sim runtime owns a tagged
   `sim.input` node (an `rlvgl_ui::Input` pushed into the root tree at
   startup, inactive, in a free screen region) plus the extension
   commands `XI1` / `XI0` toggling `set_active`. The demo app is
   untouched — the editable-field surface is a sim-runtime concern,
   following the `active_crawl` precedent. Keys reach it through the
   existing `KD:`/`KU:` pipeline; pixel truth is asserted by D-dump
   equality inside the field's bounds.

## 9. (Reserved)

## 10. Reconciliation vs. Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `Label` (`widgets/src/label.rs`) | Unchanged; remains the render delegate. The downstream consumer's app-side shim (Label-wrapper + own buffer) is exactly the shape WID absorbs into the widget. |
| Demo `ControllerState::handle_key` navigation | Untouched. Sim KeyDowns still reach the controller after tree dispatch (the sim's `post_dispatch` is unconditional); the `sim.input` test avoids nav-bound keys (`Enter`, arrows trigger nav regardless — a known sim-runtime behavior, not a WID defect). |
| `DoubleTapRecognizer`/`DragRecognizer` chains | Orthogonal: keys don't traverse pointer recognizers (pass-through arms). |
| rlvgl-ui STATUS doc | The ticket asks for an rlvgl-ui STATUS note; recorded via this doc + the §15 entry (ui has no STATUS.md; `docs/concepts/` is the initiative ledger). |
| Creator font pipeline (packed-ASCII) | Cited as the v1 charset rationale only; no code coupling. |

## 11. Non-Goals

- No on-screen keyboard widget (consumers compose Grid+Buttons; a
  generic OSK is a separate ticket).
- No IME/Unicode composition, no selection, no clipboard — ASCII
  `0x20..=0x7E` bounds v1.
- No mandatory focus framework (§7.1).
- No caret blink, no glyph-accurate caret in v1 (§6.2/§6.3).

## 12. Acceptance Checklist

- [ ] Synthetic `KeyDown` streams produce expected buffer/caret states
      (insert, backspace, Enter, arrows, boundary cases).
- [ ] Inactive inputs ignore keys; `set_active` switches the recipient
      between two fields.
- [ ] `on_change` fires once per successful edit; `on_submit` fires on
      Enter (single-line); rejected edits fire neither.
- [ ] Charset hook rejects out-of-set characters with buffer and caret
      provably unchanged; max-length enforced.
- [ ] playit `KD:`/`KU:` script types "HI" then `Backspace` into the
      active `sim.input`; the rendered field equals the type-"H"-only
      render (D-dump equality, headless).
- [ ] Existing rlvgl-ui tests stay green, unmodified.
- [ ] Published in a crates.io 0.2.x release.

## 13. Files Cited

- `ui/src/input.rs` — display-only Input/Textarea (the gap)
- `widgets/src/label.rs` — render delegate
- `core/src/event.rs:43-114` — KeyDown/KeyUp/Key vocabulary
- `playit/src/protocol.rs:74` — `parse_key` (wire grammar)
- `core/src/renderer.rs` — `TEXT_NOMINAL_LINE_PX` (nominal metrics
  anchor)
- `examples/disco-sim/src/main.rs` — runtime-owned overlay precedent
  (`active_crawl`), extension command seam

## 14. Unblocks / Deferred

- **Unblocks now**: downstream OSK/dialpad consumers (field side);
  retiring the consumer's Label-wrapper shim.
- **Deferred — Safe**: a focus manager (explicitly out of ticket
  scope; revisit when ≥2 in-repo consumers juggle multiple fields);
  `ArrowUp/Down` row movement in `Textarea`; `Escape`-to-deactivate
  convention.
- **Deferred — Coupled**: caret blink (needs the anim/dirty-rect seam
  so blink doesn't force full repaints or break dump determinism);
  glyph-accurate caret placement (needs text-extent metadata on the
  `Renderer` trait — same seam REND-00 §14 noted for `draw_text`
  cropping).

## 15. Change Log

- **2026-06-11** — WID-00 drafted and ratified. Edit model §5 (ASCII
  v1, consumption contract); line caret without blink §6 (determinism
  rationale); `set_active` routing, focus manager deferred §7;
  `Key::Backspace` + `KeySpec::Backspace` vocabulary additions;
  sim-owned `sim.input` observability surface §8.4.
