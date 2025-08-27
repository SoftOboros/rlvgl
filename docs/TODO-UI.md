<!--
docs/TODO-UI.md - rlvgl – UI Workstream TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – UI Workstream TODO

This file tracks the tasks for building the high-level `rlvgl-ui` crate.

## Phase 1 – LVGL-Compatible Style & Theme
- [x] Audit LVGL style APIs
- [x] StyleBuilder (padding, margin, bg, text, border, radius)
- [x] Part/State helpers
- [x] Token structs (Spacing, Colors, Radii, Fonts)
- [x] Legacy theme bridge (material, mono)
- [x] Demo + CI tests
- [x] Tag v0.1.0

## Phase 2 – rlvgl-ui Core
- [x] Layout helpers (HStack, VStack, Grid, Box)
- [x] Event hooks (on_click, on_change)
- [x] Icon font integration
- [x] Optional macro DSL (view!) behind feature flag
- [x] Publish rlvgl-ui v0.1

## Phase 3 – Chakra-Inspired Components
 - [x] Button / IconButton
 - [x] Text / Heading
 - [x] Input / Textarea
 - [x] Checkbox
 - [x] Switch
- [x] Radio
- [x] Badge / Tag / Alert
 - [x] Modal / Drawer / Toast
- [ ] Storybook-style demo app
- [ ] Release v0.2 and draft 1.0

---

MIT-licensed: MIT.
