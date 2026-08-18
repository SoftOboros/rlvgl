<!--
README.md - Wayland native backend SBC family index.
-->

# WLD — Native Wayland Backend

**Status:** Draft initiative opened 2026-08-18. No implementation is
authorized. Target release line: rlvgl v0.2.7.

WLD specifies a native, `std`-only Wayland display and input backend for
`rlvgl-platform`. It is deliberately independent of the concurrent MPY and
CPython binding initiatives: those families may consume neutral frame and
platform contracts, but WLD does not own interpreter integration, Python
buffer leases, or Stage/Actor protocol semantics.

## Phase map

| Phase | Scope | Status |
|---|---|---|
| [WLD-00](WLD-00-CONCEPTS.md) | Authority, profiles, invariants, dependency boundaries, and phase plan | Draft; all five PCDNs resolved, owner ratification pending |
| [WLD-01](WLD-01-SESSION-SHM-PRESENTATION.md) | Wayland session, XDG lifecycle, SHM buffers, damage, pacing, and release-safe presentation | Draft; blocked by WLD-00 |
| [WLD-02](WLD-02-INPUT-CONFORMANCE-RELEASE.md) | Seat input, cancellation policy, compositor conformance, performance evidence, docs, and v0.2.7 release closure | Draft; blocked by WLD-00 and WLD-01 |

## Authority and evidence

- [Family errata](ERRATA.md)
- [Pinned LPAR baseline](../concepts/LPAR-01-BASELINE.md)
- [Existing display authority](../concepts/LPAR-03-INVALIDATION-DISPLAY.md)
- [Existing input authority](../concepts/LPAR-04-EVENT-FOCUS-INPUT.md)
- [LVGL parity backlog item 58](../todo/TODO-LVGL-PARITY.md)
- Vendored reference implementation: `lvgl/src/drivers/wayland/`

Ratification and implementation are separate gates. WLD-00 may establish the
family boundary without authorizing WLD-01 or WLD-02 behavior. Shipping in
v0.2.7 requires both implementation phases to be ratified, executed, and
closed with their stated evidence.
