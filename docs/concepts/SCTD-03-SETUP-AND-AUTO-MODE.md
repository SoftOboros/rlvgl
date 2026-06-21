# SCTD-03 — Setup Screen, Auto Mode, and Selector Recomposition

Status: **RATIFIED** (2026-06-21)
Family: SCXML Tutorial Demo (SCTD). Builds on SCTD-00 (concepts) and
SCTD-02 (FireBeetle-P4 interactive). Governs the `rlvgl-app-sctd-demo`
crate and its FireBeetle-P4 host glue.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, MAY, RECOMMENDED are per
RFC 2119 / 8174.

## §0 Authority policy

- Selector geometry constants (`STRIP_X_OFFSET`, `STRIP_ICON_SIZE`,
  `STRIP_MARGIN_TOP`, `STRIP_GAP`) remain **owned by SCTD-00 §6.2**; used
  without modification.
- The generated machine public API (`Machine::new/start/step/run/get_var/
  current_state/get_child_state`) is **owned by the iState codegen**; used
  without modification (SCTD-00 §6.9 — UI MUST NOT reach past it).
- The Setup screen, the `Auto` model, and the new selector composition are
  **owned by SCTD-03** (this doc); they do not exist in the repo yet.

## §1 Purpose

Replace the three-machine selector (faithful DP / Media Player /
interactive DP) with a **Setup-driven** model discovered through live use
of the SCTD-02 build: a ⚙ Setup entry that configures two runnable
machines (DP, MP), where DP's classic timer behaviour becomes a per-run
`Auto` toggle rather than a separate machine.

## §2 Problem statement (informative)

On the bench, the faithful DP (selector slot 0) and the interactive DP
(slot 2) are two icons that share one glyph (`selector.rs:66`
`icons: [ICON_DP, ICON_MEDIA, ICON_DP]`) and are told apart only by the
highlight — confusing. The faithful machine auto-runs but takes no input;
the interactive machine takes input but does not auto-run. A user wants
**one** dining-philosophers view that can both auto-run *and* accept manual
inserts, plus a place to configure it and the media player. The footer
also advertises keyboard controls that do not exist on a touch-only board
(`lib.rs:810-818`).

## §3 Canonical glossary

- **Setup screen** — *Owned by SCTD-03.* The view shown when selector slot
  0 (⚙) is active: a top line of two separated touch **tabs** (`DP`, `MP`)
  and, below, the config body for the active tab.
- **Tab** — *Owned by SCTD-03.* A touch target on the Setup screen's top
  line selecting which machine's configuration is shown. Exactly one active.
- **Auto (DP)** — *Owned by SCTD-03.* A boolean DP run option. When **on**,
  a host timer auto-arrives and auto-departs philosophers and advances the
  think→hungry→eat cycle (classic autonomous table); when **off**, none of
  that fires. **In both states the on-screen buttons remain live** and
  insert events.
- **Auto-Ready (MP)** — *Owned by SCTD-03.* A boolean MP config option;
  when on, the host fires `Inp.Media.Ready` + `Inp.Media.ValidSource` on
  launch so MP opens past the idle screen.
- **Run view** — *Owned by SCTD-03.* The view shown when slot 1 (DP) or
  slot 2 (MP) is active: the machine panel (+ philosophers table for DP).
- **Faithful DP / Interactive DP** — *As defined in
  `sctd-demo/src/lib.rs`; retired by SCTD-03 §10:* merged into one DP
  adapter gated by `Auto`.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| Selector composition + order | SCTD-03 §5 (supersedes SCTD-02 §6.7) |
| `Auto` semantics | SCTD-03 §6 |
| Setup screen layout + tab set | SCTD-03 §7 |
| DP / MP config surface | SCTD-03 §8 |
| Status-line formatting | SCTD-03 §9 |
| Machine public API | iState codegen (unmodified) |
| Selector geometry constants | SCTD-00 §6.2 (unmodified) |

## §5 Frozen decision — selector composition (Standards Action)

The selector SHALL have exactly three slots in this order:

| Slot | Entry | Icon | Selecting it shows |
|---|---|---|---|
| 0 | **Setup** | ⚙ gear (new asset) | the Setup screen |
| 1 | **DP** | `ICON_DP` | DP run view |
| 2 | **MP** | `ICON_MEDIA` | MP run view |

`MACHINE_COUNT` stays `3`, but slot 0 is a **screen**, not a machine.
Boot default selection SHALL be **slot 1 (DP)**.

## §6 Frozen decision — Auto model (Specification Required)

- The single DP adapter SHALL expose `set_auto(bool)` and `auto() -> bool`.
- `tick()` SHALL fire the auto-arrive / auto-depart / cycle behaviour **iff
  `auto()`**; otherwise `tick()` is a no-op for table population (Pause and
  Speed still apply).
- Manual buttons (`Arrive`, `Depart`, `Panic`, `Reset`) SHALL dispatch
  regardless of `Auto`.
- Auto cadence (INV-SCTD03-1): auto-arrive biases the table toward full
  with occasional auto-departs, interleaved with the eat cycle, paced by
  the Speed setting. Cadence constants are tunable; the demo MUST remain
  human-watchable per SCTD-02 INV-SCTD02-2.

## §7 Frozen decision — Setup screen (Specification Required)

- Top line: two tabs `[ DP ] [ MP ]`, visually **separated**, each its own
  touch hit-rect; exactly one active (default `DP`). Tapping a tab switches
  the body without leaving the Setup screen.
- Body renders the active tab's config controls (§8) as finger-sized touch
  targets, reusing the SCTD-02 button-rect + `PressRelease` hit-test
  pattern (`machine_panel.rs`).

## §8 Frozen decision — config surface (Specification Required)

- **DP tab:** an `Auto` **checkbox** (default **on**) and a **Speed**
  selector `x0.5 / x1 / x2` (default `x1`). Pause stays a run-view button.
- **MP tab:** a **Default source** selector `USB / SD / AUX` (default
  `USB`) and an **Auto-Ready on open** checkbox (default **on**). No
  repeat / mute controls in v1.
- Config is applied when the corresponding machine is launched from its
  selector slot. Changing config while a machine is already running takes
  effect on next launch (or Reset).

## §9 Frozen decision — status-line formatting (Expert Review)

- A state summary containing ` | ` SHALL render the part before and after
  the separator on **separate lines**, and the `|` SHALL be dropped.
- Per-seat state abbreviations stay **3 char** (`thk / hun / wai / EAT`);
  SCTD-03 does not widen them.

## §10 Reconciliation vs. existing primitives

- `DiningPhilosophersAdapter` (faithful) is **retired**; its always-seated
  auto-run behaviour is reproduced by the single DP adapter with `Auto` on.
- `InteractiveDiningPhilosophersAdapter` becomes the **sole DP adapter**,
  gaining `set_auto`/`auto` and the auto-arrive/depart timer in `tick()`.
- `MediaPlayerAdapter` gains config seeding (source + Auto-Ready) at launch.
- SCTD-02 §6.7 (selector order DP/MP/Interactive) is **superseded** by §5;
  an SCTD-02 §15 amendment SHALL record this before code lands.

## §11 Non-goals

- No new generated machines; reuse `dining_philosophers_interactive` +
  `media_player`. The `dining_philosophers` (faithful) crate MAY remain
  vendored but is no longer wired into the demo.
- No MP repeat/mute config in v1. No keyboard control on the board (the
  desktop-sim key paths in `lib.rs` stay for the sim only).

## §12 Acceptance checklist (normative)

A conforming SCTD-03 build:
- (a) selector is `[⚙ Setup, DP, MP]` per §5; boots to DP.
- (b) selecting ⚙ shows the Setup screen with separated `DP`/`MP` tabs that
      switch on tap (§7).
- (c) DP tab toggles `Auto` and Speed; with `Auto` on the table auto-runs,
      with it off it does not, and buttons insert in both (§6, §8).
- (d) MP tab sets default source + Auto-Ready; launching MP honours them (§8).
- (e) status summaries render two lines with the `|` dropped (§9).
- (f) the footer no longer advertises non-existent keyboard controls.
- (g) selector/count tests updated to the §5 composition; workspace tests +
      clippy pass per the rlvgl pre-publish gates.

## §13 Files cited

- `examples/apps/sctd-demo/src/{lib.rs,selector.rs,machine_panel.rs,assets.rs}`
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs`
- `docs/concepts/SCTD-00-CONCEPTS.md`, `docs/concepts/SCTD-02-FIREBEETLE-P4-INTERACTIVE.md`

## §14 Unblocks

Implementation PR `SCTD03a:` (selector + Setup + Auto + config + cosmetics).

## §15 Change log

- 2026-06-21 — Initial draft. Decisions in §5–§9 from the bench session
  (Setup gear; DP/MP tabs; `Auto` checkbox with live buttons; MP
  default-source + Auto-Ready; two-line status, 3-char abbrevs kept).
- 2026-06-21 — RATIFIED. Owner accepted §5–§9 and the defaults (boot→DP,
  Auto on, Speed x1, MP source USB, Auto-Ready on). SCTD-02 §15 supersede
  amendment recorded (same date), before behaviour code. Execution may now
  cite this document as `SCTD03a:`.
