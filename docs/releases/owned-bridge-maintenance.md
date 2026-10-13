<!--
owned-bridge-maintenance.md — Current Unreleased candidate, native API and release boundaries.
-->
# Unreleased owned-bridge maintenance

Status as of **2026-10-13**. This is the current source/API guide for the accepted
maintenance candidate. Released v0.2.7 / A2 remains unchanged. Package allocations
below do not assert registry availability or select a future release tag.
The [dated family state](v0.2.7-successor-family-state-20261013.md) records the
remaining initiative decisions and evidence. Historical plans and failed results
retain their original scope; this guide does not ratify a phase or close a gate.

## Source and package identity

| Component | Candidate version or source | Meaning |
| --- | --- | --- |
| `rlvgl` | 0.2.8 | Selective umbrella-package allocation |
| `rlvgl-platform` | 0.2.7 | Selective allocation including the accepted one-line compile-diagnostic correction; typed front-buffer write prohibition remains |
| `ratatui-rlvgl` | 0.1.2 | Owned carrier at `crates/ratatui-rlvgl` |
| `rlvgl-core` | 0.2.7 | Existing allocation and published payload remain unchanged |
| Official Ratatui | Release 0.30.2; commit `e665c36cb14752a61cd777fbd06dbef8474f2add` | Unmodified upstream source authority |
| `ratatui-core` | Exactly 0.1.2 | Native primitive crate from that upstream release; its package version differs from the Ratatui release version |

The other published package allocations and existing dependency minima are
retained. Publication selects the changed bridge → platform → umbrella cohort;
it does not bump every workspace package or dependency requirement.

Current source consumers use `ratatui_core` for `Terminal`, `Buffer`, `Rect`,
styles, text and native widget traits, and `ratatui_rlvgl` for the backend, view
and owned compatibility. Keep **one native core type identity** across both.
The source carrier uses exact native-core and local core paths in its
[manifest](../../crates/ratatui-rlvgl/Cargo.toml); the
[tutorial](../ratatui-tutorial/README.md) distinguishes source recipes from
registry recipes usable only after publication. A forked facade or the old
`vendor/ratatui/ratatui-rlvgl` location is not the current carrier.

## Rendering, compatibility and portability

The retained backend and `RatatuiView` bridge native cells and RLVGL surfaces.
The owned [`compat` module](../../crates/ratatui-rlvgl/src/compat.rs) supplies this
explicit subset over native core primitives:

| API | Supported scope |
| --- | --- |
| `CellEffect`, `Shadow`, `WithShadow` | Shared effects with retained thread/panic bounds; shadow rendering before the wrapped widget |
| `Borders`, `Panel` | Public `NONE`/`ALL` borders, background/plain border, left title and bounded inner geometry |
| `TextPanel` | Left-aligned hard lines with native styles and clipping; no wrapping, scrolling or `.block(...)` API |
| `BarValueLabel` | Display-column-centered label over caller-owned bar content; does not draw a bar body or replace upstream `BarChart` |

This is not whole-facade or full Block/Paragraph/BarChart equivalence. Compose a
`Panel` and a `TextPanel` explicitly. Display-width-aware label placement does
not add CJK glyphs or expand the curated font repertoire.

On targets without pointer atomics, enable the bridge's `portable-atomic`
feature. It forwards to `ratatui-core/portable-atomic` and selects the owned
portable Arc implementation; omission is a compile-time error. Existing
board-specific single-core assumptions are not portable hardware guarantees.
This migration does not modify upstream Ratatui or grant new unsafe-code scope.

The bridge package carries the exact existing regular, bold, oblique and
bold-oblique packed font binaries, plus the
[font notice](../../crates/ratatui-rlvgl/FONT-NOTICE.txt). No regeneration, default,
repertoire or license change is implied. `RatatuiView::new` defaults to the
packed family: pair it with `CellMetrics::packed_terminal()` (14×21 cells).
For the bitmap baseline, explicitly select `RatatuiTerminalFont::Bitmap6x10`
and its 12×20 cell metrics. The immutable bridge 0.1.1 missing-font archive
remains historical evidence; the successor package must independently prove
its include set and extracted consumer behavior.

## Native Stage and Endpoint snapshots

These are **A2 native APIs carried forward**, not new bridge functionality.
Authoritative signatures and result types are in
[`Endpoint`](../../core/src/endpoint.rs),
[`StageRegistry`](../../core/src/actor.rs),
[`direction`](../../core/src/direction.rs) and
[`subscription`](../../core/src/subscription.rs).

For an endpoint-owned Stage, use this bounded flow:

1. Call `endpoint.snapshot_begin(stage_id)` to obtain a `SnapshotToken`.
   A Stage has one active cursor; an existing cursor yields `Busy`.
2. Call `endpoint.snapshot_read(stage_id, token, max_records,
   max_text_bytes_per_record, max_style_values_per_record,
   max_subscriptions_per_record)` for each page. Records follow deterministic
   root order/pre-order. A page carries its frozen `revision`, zero-based
   `sequence`, bounded `records` and `ended` flag.
3. Inspect each record's truncation/redaction metadata while consuming it.
   Continue until `ended` is true; that final page already releases the cursor.
4. Use `endpoint.snapshot_end(stage_id, token)` only when aborting a still-live
   cursor early. Ending an already released or invalid token yields
   `InvalidCursor`.

The four bounds are independent:

| Bound | Interpretation |
| --- | --- |
| `max_records` | Page record count; zero is `InvalidLimit` |
| `max_text_bytes_per_record` | Aggregate retained property-text budget for that actor; a property exceeding it is explicitly redacted, not silently shortened |
| `max_style_values_per_record` | Retained sparse MPY-owned style prefix; `total_style_values` and `styles_truncated` describe the complete count and omission |
| `max_subscriptions_per_record` | Active neutral subscription prefix in registration order; `total_subscriptions` and `subscriptions_truncated` remain truthful even when the bound is zero |

Zero style/subscription bounds are valid. The aggregate `actor.truncated` flag
includes subscription truncation. Counts describe the complete matching set,
not just retained rows. Subscription metadata contains neutral identities and
opaque callback tokens; it does not expose or dereference VM callback objects.

Every projected actor/subscription record is prepared before the page's single
cursor commit. A preparation/registry error leaves its position and sequence
unchanged: after addressing the cause, retry the same token if the Stage is
unchanged. This does not promise recovery from a faulted endpoint or teardown.
If the owning Stage revision changed, the next read returns
`Stale { starting, current }` and releases the stale cursor. Discard accumulated
pages and begin a fresh snapshot; do not mix revisions. An invalid token cannot
be resumed. Endpoint admission also rejects faulted or teardown-pending state.

Stage-only `snapshot_read(token, max_records, max_text_bytes_per_record)` and
`snapshot_read_with_style_limit(...)` retain their actor-only return types;
they do not add endpoint subscription rows.

For removal, call
`endpoint.unsubscribe(stage_id, actor_identity, subscription_id)` with the
exact identities. `Removed(TeardownReport)` removes the record and reports its
callback token once. `AlreadyRemoved` recognizes the same completed removal
within the bounded tombstone window; an unrecognized/stale identity fails with
`StaleSubscription`. Successful first removal advances the owning Stage revision
and stales its active snapshot. Repeated or failed removals leave that revision
unchanged, and other Stages remain isolated. The native report does not itself
establish language-adapter callback release or wire/interpreter conformance.

## Packaging and publishing are different gates

Gate P packages the **full 31-crate publish order**, with `rlvgl-core` immediately
before the owned bridge. It discovers the bridge once through root metadata.
The finite preparation contexts enumerate 48 root members and 42 official
native-workspace members; these are not additional publish-list entries or
permission to admit future descriptors automatically. The current authority is
[CRATES-CI](../crates-ci/CRATES-CI-00-CONCEPTS.md).

The publisher selects **three changed packages** in the existing full order:
`ratatui-rlvgl` 0.1.2 → `rlvgl-platform` 0.2.7 → `rlvgl` 0.2.8. Full packaging
still verifies unchanged versions with P-DIFF. Progressive package verification
uses extracted predecessors; final consumers use extracted archives, with no
workspace/source fallback. Include, metadata, resolution, order, finite-context,
withdrawal and source/lock-drift guards remain required.

Before a successor release, the maintainer sequence remains:

1. Review and adopt the governing documentation through its exact protected PR,
   then the coherent implementation/docs/index change through the separate
   protected route. Keep the adopted governing postimages unchanged.
2. Freeze a genuine clean source ref, pins, locks, metadata and artifacts; pass
   all applicable format, pre-commit and pre-publish checks for those inputs.
3. Run actual full clean-ref Linux/hosted Gate P on that exact ref, with normal
   assets, all 31 packages, all three existing packaged consumers and the owned
   bridge boundary. Retain earlier successes and failures at their actual refs.
4. Obtain the separate release/tag and publication disposition. The publisher
   rechecks successful same-ref Gate P, exact clean HEAD and order before
   authentication. Gate R then exercises the actual registry consumers.
5. Qualify parent-pin adoption separately against the published/source release
   and consumer evidence. Existing parent pins remain unchanged until that gate.

`ALLOW_DIRTY`, `ONLY_CRATES` and `SKIP_ASSET_PREP` are diagnostic iteration knobs,
not release recipes. A local source check, subset archive, old A2 provider run or
documentation review does not substitute for a new same-ref Gate P/Gate R.

## Remaining family and hardware qualification

Current native snapshots do not close Python/MicroPython projections, wire/Batch
dispatch, the whole cross-driver corpus or board transport. MPY-06/07 remain
accepted but unratified; MPY-08/09 and staging 01B remain Draft under their own
evidence and owner processes. Module/actor identity presence is not proof that
every runtime-backed operation is available.

Producer source work covers accepted existing-Map/bounded-Array writes and
Python RHS/computed-key failure behavior within its stated grammar limits.
Source allocation 0.4.3 and candidate proof do not promote the production scjson
0.4.2 requirements or establish deployed producer/MCP artifacts. Package-first
promotion and the remaining integration vectors stay separate.

Clock-source admission preserves unknown HSE frequency; it does not choose an
oscillator or qualify board behavior. RLVGL retains identity-only admission and
existing reporting. CCPS temperature validation and prototype wiring remain
specific CCPS dependencies; PTN/PCM health reporting is not silicon identity.
Embedded flash/transport, live Wayland compositor, soak/performance and complete
family conformance require their own evidence. The dated family summary retains
these open boundaries; this guide selects no new hardware policy or phase status.
