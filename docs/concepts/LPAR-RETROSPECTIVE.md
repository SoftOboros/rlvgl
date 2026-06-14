# LPAR Retrospective - parity initiative closure

**Status:** Drafted 2026-06-14. Initiative-completion retrospective for the
LPAR (LVGL parity) initiative on the `v0.2.4` line.

This is a historical artifact for future multi-wave UI/runtime work. The
normative contracts remain in `LPAR-00-CONCEPTS.md` through
`LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md`.

## 1. Outcome snapshot

LPAR landed the tree-resident runtime model that the backlog needed:
`ObjectNode` now carries flags/states, focus, scroll, animation, style, and
layout state without replacing the existing widget trait shape. The draw stack
has defaulted capability methods for shaped text, masks, gradients, shadows,
and image descriptors, so old renderers still compile while richer renderers
can opt in. Widget coverage now spans primitive, control, selection/navigation,
data-rich, canvas/media, property, and observer surfaces.

The LPAR-16 fixture set closes the main conformance loop: deterministic runtime
fixtures, exact geometry fixtures, pixel goldens for the widget waves, feature
gates, no-std compile checks, simulator example coverage, documentation build,
and release bookkeeping.

## 2. Divergence log

The largest planning divergence was text rendering. LPAR-08 correctly shipped
font metrics, wrapping, shaped extents, and clipped glyph-coverage plumbing, but
the original shaped-text default path was only an extent visualizer. Real
end-to-end glyph rendering still depends on renderer/font coverage adapters and
widget migration, especially `Label`.

LPAR-10 also exposed a layout-positioning gap before implementation: clipping
with `effective_bounds` does not reposition a child. The fix was to reuse the
proven `ClipRenderer::with_offset` translation path and add a default-no-op
`Widget::set_bounds` for resize-aware widgets.

LPAR-16 exposed release-discipline drift rather than feature drift: crate
versions and internal dependency constraints lagged the `0.2.4` release target,
the concepts README still described LPAR-16 as drafted, and the changelog had
no LPAR entry.

## 3. Gates that worked

The strongest gates were the small, phase-local fixtures. Pixel-golden tests
found renderer routing mistakes cheaply, and the exact geometry tests made
layout regressions easy to identify. The no-std target check caught host/embedded
cfg mistakes in the STM32H747I-DISCO crate. Playit automation gave useful
end-to-end coverage once the drag test asserted the simulator status path
instead of relying on a stale pixel proxy.

## 4. Gates that were too weak

`cargo doc --workspace --no-deps` is only a pass/fail build gate today. It still
allows many broken/private intra-doc links because rustdoc warnings are not
promoted to errors. `make build-disco` similarly proves the embedded artifact
builds, but it is not warning-clean. Future release work should decide whether
these warnings remain accepted debt or become explicit gates.

The simulator example requirement was underspecified enough that a legacy demo
build could have been mistaken for LPAR parity coverage. The cleanup slice added
an explicit LPAR parity composition to remove that ambiguity.

## 5. Deferred items

LPAR-09 remains partially open for FATFS-over-`SimBlockDevice`; it needs a real
`FatfsAssetSource` bridge and a std-only `rlvgl-fs-sim` integration path.

LPAR-15 optional media widgets (`Lottie`, `DashLottie`, `Texture3d`) remain
deferred until their renderer/runtime surfaces exist. They should not be added
to the LPAR-16 pixel-golden obligation before that substrate is real.

End-to-end shaped glyph rendering is still open: the font metric backends and
coverage rows exist, but widgets mostly continue through legacy text paths.

## 6. Forward constraints

Do not add a second text measurement stack. Span, table, textarea, label, and
future rich-text widgets must keep using `core::font` measurement and wrapping.

Do not add widget-local layout solvers when `ObjectNode::layout_state` can carry
the state. Widget `set_bounds` overrides are for local geometry recomputation,
not for creating a parallel layout system.

Do not mark a phase conformance-complete from implementation alone. Each phase
needs an explicit LPAR-16 fixture row or a named, dated deferral.

## 7. Release notes for next wave

The next high-leverage work is text completion: connect real glyph coverage to
`draw_text_shaped`, migrate `Label`, and add a simulator-visible text fixture.
After that, close LPAR-09's FATFS asset-source bridge and make the rustdoc
warning backlog either clean or explicitly non-blocking.

## 8. Evidence

Key validation commands from the cleanup slice:

- `cargo fmt --all -- --check`
- `RUSTFLAGS="" cargo clippy --workspace -- -D warnings`
- `RUSTFLAGS="" cargo test --workspace`
- `RUSTFLAGS="" cargo test -p rlvgl-core --all-features`
- `RUSTFLAGS="" cargo test -p rlvgl-widgets --no-default-features`
- `RUSTFLAGS="" cargo test -p rlvgl-widgets --features lpar_arclabel`
- `RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-core -p rlvgl-widgets`
- `RUSTFLAGS="" cargo doc --workspace --no-deps`
- `make build-disco`

## 9. Change log

- **2026-06-14** — Drafted at LPAR initiative completion (every named phase
  shipped or closed-with-deferral; the LPAR-16 §6 fixture ledger satisfied
  except the deferred-Coupled LPAR-09 FATFS prong). Section-shape note (per the
  CLAUDE.md allowance to "amend the §1–§7 shape with a named justification"):
  this retrospective frames the canonical **§3 Refactor points** and **§4
  Mitigation patterns** as "Gates that worked / too weak" (the LPAR initiative's
  inflection points were overwhelmingly *gate* decisions — which fixtures to
  trust, which to harden), and the canonical **§7 Provenance hooks** as the §8
  Evidence command list plus the inline `LPAR-NN`/commit references throughout.
  The load-bearing **§2 Divergence log** and normative **§6 Forward
  constraints** retain their canonical meaning.
