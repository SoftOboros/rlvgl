<!--
TODO-LVGL-PARITY.md - Ordered backlog for outstanding LVGL parity work.
-->

# Outstanding LVGL Parity Items

Status: Promoted to spec-before-code initiative. The canonical phase,
wave, dependency, and conflict plan is
[`docs/concepts/LPAR-00-CONCEPTS.md`](../concepts/LPAR-00-CONCEPTS.md).
This file remains the raw ordered backlog that fed LPAR-00. Items appended
after that initiative are follow-up candidates and do not retroactively change
ratified LPAR contracts.

Reference baseline: the local C LVGL submodule under `lvgl/src`. This
list is ordered by dependency and expected implementation leverage: core
runtime contracts first, then layout/style/draw infrastructure, then
widget families, and finally verification and documentation.

Current rlvgl coverage already includes a Rust widget tree, renderer
trait, basic events, styles, themes, animation primitives, clipping,
scroll view, button, checkbox, click area, clock, container, image,
label, list, progress, radio, slider, switch, editable `Input` /
`Textarea`, UI wrappers, simulator/display backends, and selected media
plugins. Items below are the remaining LVGL-parity work or places where
the current implementation is intentionally narrower than LVGL.

1. [ ] Lock the parity baseline. Record the exact LVGL submodule commit, the LVGL major/minor target, enabled LVGL config options, and a current/partial/missing matrix.
2. [ ] Define Rust-vs-LVGL naming policy. Decide when modules keep LVGL names (`Arc`, `Bar`, `Roller`) and when rlvgl keeps Rust-native names (`Progress`) with documented aliases.
3. [ ] Complete `lv_obj`-style object semantics. Cover parent/child ownership, screen roots, sibling order, raise/lower, hidden/disabled/clickable flags, hit testing, and object deletion semantics.
4. [ ] Add LVGL-like invalidation propagation. Track dirty areas from child to screen, merge invalid rectangles, and separate invalidation from drawing.
5. [ ] Expand event dispatch parity. Add event codes for draw, size, value, focus, press, long press, scroll, gesture, delete, and custom user events.
6. [ ] Add event bubbling/trickling controls. Model target/current-target distinction, stop-propagation behavior, and parent notification rules.
7. [ ] Add focus groups. Provide LVGL-like focus next/previous, active object tracking, focus styling, and keypad/encoder routing.
8. [ ] Add input device abstraction parity. Normalize pointer, keypad, encoder, and button input sources behind a shared input-device layer.
9. [ ] Complete pointer gesture parity. Add long press, repeat press, release/cancel, scroll begin/end, fling/momentum, and gesture direction events.
10. [ ] Complete scroll container parity. Add scrollbar modes, scroll chaining, scroll snapping, scroll one/throw, nested scroll behavior, and scrollable flag handling.
11. [ ] Complete style cascade parity. Add selector matching across object part and state, local/shared style lists, inherited properties, removal/reset rules, and style refresh invalidation.
12. [ ] Expand style properties. Cover margins, padding, min/max dimensions, transforms, translate, rotation, scale, shadow, outline, line, arc, image recolor, text spacing, and blend properties.
13. [ ] Add style transitions. Support transition descriptors, delayed/repeated transitions, property filtering, and object-state-driven style animation.
14. [ ] Complete theme parity. Add default theme coverage, theme chaining, per-widget default styles, and deterministic theme application order.
15. [ ] Complete text and font parity. Add glyph metrics, font fallback, long-text modes, text alignment, recolor markup, selection metadata, bidi/RTL policy, and line wrapping compatible with LVGL labels.
16. [ ] Expand draw primitive parity. Add arcs, lines, polygons, gradients, shadows, masks, anti-aliasing policy, opacity blending, rounded-border details, and image transforms.
17. [ ] Complete clipping and mask parity. Move from rectangular parent clipping toward LVGL mask stack behavior where widgets need rounded, line, radius, or fade masks.
18. [ ] Complete display driver parity. Model display buffers, flush callbacks, color formats, rotation, full/partial refresh, double buffering, and flush completion semantics.
19. [ ] Define GPU accelerator parity hooks. Normalize DMA2D and future PXP, VG-Lite, OpenGL, or vendor draw backends behind feature-gated draw acceleration contracts.
20. [ ] Complete image asset parity. Cover LVGL-style image descriptors, file-backed images, cache behavior, color formats, alpha formats, indexed/palette formats, recolor, and scale/rotate draws.
21. [ ] Complete filesystem and asset source parity. Normalize embedded, FATFS, simulator, and memory-backed asset lookup behind LVGL-like source conventions.
22. [ ] Complete timer/task parity. Extend current tick-driven animation support with LVGL-like timers, callback lifecycle, pause/resume, repeat count, and per-object animation binding.
23. [ ] Complete layout sizing primitives. Add LVGL-compatible percent sizing, content sizing, min/max constraints, align-to-parent helpers, and object-size change events.
24. [ ] Implement flex layout parity. Cover row/column flow, wrap, grow/shrink, main/cross alignment, gaps, reverse flow, and nested flex behavior.
25. [ ] Implement grid layout parity. Cover tracks, fractions, spans, placement cells, alignment, gaps, percent/content sizing, and template-like declarations.
26. [ ] Add property/introspection parity. Provide a typed property layer for object/widget properties, style properties, generated bindings, tests, and creator/playit introspection.
27. [ ] Add observer/data-binding parity. Track whether LVGL observer/subject support should map into rlvgl core, ui, or creator-generated app state.
28. [ ] Add `Arc` widget parity. Implement value/range, rotation, modes, knob/main/indicator parts, and arc draw style properties.
29. [ ] Add `Bar` widget parity. Either rename/alias `Progress` or add a dedicated `Bar` with range, start value, modes, indicator part, and animation behavior.
30. [ ] Add `ButtonMatrix` widget parity. Implement matrix maps, control flags, one/toggle modes, checked buttons, disabled buttons, and key navigation.
31. [ ] Add `Calendar` widget parity. Implement date model, month navigation, highlighted days, header controls, and styling parts.
32. [ ] Add `Chart` widget parity. Implement line/bar/scatter modes, axes, ticks, series storage, update modes, cursors, and division lines.
33. [ ] Add `Dropdown` widget parity. Implement collapsed/open states, option list rendering, selected option, direction, symbol, and keyboard navigation.
34. [ ] Add `ImageButton` widget parity. Implement state-specific image sources, checked/pressed/disabled images, and image recolor/opacity behavior.
35. [ ] Add `Keyboard` widget parity. Implement reusable on-screen keyboard maps, modes, popovers, target textarea binding, and key event emission.
36. [ ] Add `LED` widget parity. Implement brightness, color, on/off/toggle helpers, and LVGL-style glow appearance.
37. [ ] Add `Line` widget parity. Implement point arrays, auto-size behavior, y-invert, style-driven width/color/rounded ends, and clipping.
38. [ ] Add `Menu` widget parity. Implement pages, sections, separators, sidebar/root pages, back behavior, and selectable rows.
39. [ ] Add `MessageBox` widget parity. Map existing `Modal`/`Alert` surfaces to LVGL-like message boxes with title, text, buttons, close behavior, and modal overlay.
40. [ ] Add `Roller` widget parity. Implement option list, visible row count, infinite mode, selected row styling, and encoder/key navigation.
41. [ ] Add `Scale` widget parity. Implement tick generation, labels, major/minor ticks, sections, needle/indicator integration, and radial/linear variants.
42. [ ] Add `Span` rich-text parity. Implement multi-style inline spans, line wrapping across spans, span-level style inheritance, and selection/cursor metadata if needed.
43. [ ] Add `Spinbox` widget parity. Implement numeric text editing, digit step selection, range, roll-over, increment/decrement helpers, and target keypad behavior.
44. [ ] Add `Spinner` widget parity. Implement animated arc spinner, configurable period, arc length, direction, and style parts.
45. [ ] Add `Table` widget parity. Implement rows/columns, cell text, merge/span policy, cell styling, selection, scrolling, and measurement.
46. [ ] Add `Tabview` widget parity. Implement tab bar placement, tab pages, active tab state, animated tab changes, and focus/key navigation.
47. [ ] Add `Textarea` parity beyond WID-01. Add selection, clipboard policy, password mode, placeholder, one-line mode, accepted-char helpers, cursor blink, cursor position APIs, and scroll-to-cursor.
48. [ ] Add `Tileview` widget parity. Implement tile grid navigation, valid tile positions, swipe/scroll transitions, active tile tracking, and nested scroll interaction.
49. [ ] Add `Window` widget parity. Implement header, title, control buttons, content area, close/minimize hooks, and style parts.
50. [ ] Add `Canvas` widget parity. Implement draw-to-canvas surface ownership, pixel format choices, blit-to-renderer behavior, and primitive drawing APIs.
51. [ ] Add media-specialized widget parity. Decide scope for `AnimImage`, `Lottie`, `3DTexture`, and `ArcLabel` based on current plugin support and embedded footprint.
52. [ ] Add LVGL example parity. Port a small, stable set of upstream examples into Rust examples that exercise each core widget family.
53. [ ] Add C-reference behavior tests. Extract deterministic behavior vectors from the LVGL submodule for widgets, style resolution, layout, events, and text wrapping.
54. [ ] Add visual golden coverage. Expand simulator/playit tests so each parity widget has at least one deterministic pixel dump or geometry assertion.
55. [ ] Add no-std and allocation policy gates. For every new parity widget, record whether it is `no_std`, `alloc`, or `std` only, and test the intended feature combination.
56. [ ] Add parity documentation. For each shipped widget, document LVGL behavior covered, intentional Rust API differences, unsupported LVGL features, and migration notes.
57. [ ] Add release tracking. Tie each parity item to a crate version and changelog entry when it lands, so downstream consumers can target the correct `0.x` release.
58. [ ] Add native Wayland display and input backend parity. Provide an `std`-only XDG-shell session with SHM buffers, compositor-paced and release-safe presentation, configure/resize/close lifecycle, and seat-derived pointer, keyboard, touch, and pointer-axis input. Keep DMA-BUF, fractional scaling, and multi-window support evidence-gated follow-ups. See the Draft [`WLD-00`](../wayland/WLD-00-CONCEPTS.md) initiative.
