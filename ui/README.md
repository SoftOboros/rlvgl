<!--
README.md - Publish-facing overview for the rlvgl-ui crate.
-->

# rlvgl-ui
Package: `rlvgl-ui`

`rlvgl-ui` is the higher-level component crate in the `rlvgl` workspace. It
builds on top of `rlvgl-core` and `rlvgl-widgets` to provide themed controls,
layout helpers, style builders, and overlay utilities for application code.

## What It Provides

- ready-to-use components such as `Alert`, `Badge`, `Bar`, `Button`,
  `ButtonMatrix`, `Checkbox`, `Drawer`, `Input`, `Led`, `List`, `Modal`,
  `Progress`, `Radio`, `Select`, `Spinner`, `Switch`, `Tabs`, `Tag`, `Text`,
  and `Toast`
- direct access to mature LVGL-parity widgets such as `Calendar`,
  `MessageBox`, `Spinbox`, `Table`, and `Window`
- layout helpers including `HStack`, `VStack`, `Grid`, and `BoxLayout`
- geometry helpers including `rect`, `origin_rect`, `RectProps`, and
  `Theme::control_rect`
- a theme/token layer with `Theme`, `Tokens`, `Style`, `StyleBuilder`, and
  fluent `StyleProps` / `ThemeProps`
- overlay and transient UI helpers such as `EventWindow`
- an optional `view` feature for view-oriented helpers

## Naming and Props

The UI crate uses canonical Rust names with Chakra-inspired vocabulary:

- app-facing wrappers use common component names where they clarify intent,
  such as `Select` over the lower-level dropdown and `Progress` over the
  progress bar
- LVGL-specific controls keep their LVGL-oriented names, such as `Bar`,
  `ButtonMatrix`, `Led`, and `Spinbox`
- type names use `PascalCase`; methods use `snake_case`
- fluent style props use short names such as `bg`, `rounded`,
  `border_width`, and `opacity`
- themed props use `ColorScheme`, `Variant`, and `ComponentSize`; variants
  include `Solid`, `Subtle`, `Outline`, and `Ghost`
- state builders use `with_*` when the natural name is already the getter,
  such as `with_value`, `with_options`, and `with_active_tab`
- `BoxLayout` remains the layout box name to avoid conflicting with
  `alloc::boxed::Box`

For components with accent parts, such as `Progress`, `Bar`, `Spinner`,
`Led`, `Select`, `Tabs`, and `ButtonMatrix`, the inherent `themed(...)` method
maps the same resolved theme style onto fills, indicators, selected rows, or
tab states. For simpler components, importing `ThemeProps` provides a generic
`themed(...)` style-only prop.

Rect construction follows the same compact style:

- use `rect(x, y, width, height)` for explicit geometry
- use `origin_rect(width, height)` for origin-based children
- import `RectProps` for fluent adjustments such as `at`, `size`, `height`,
  `translate`, `inset`, and `min_size`
- use `Theme::control_rect(x, y, width, size)` when a component should use the
  theme's recommended minimum height for a `ComponentSize`

## How It Fits

- `rlvgl-core` supplies the runtime primitives: events, rendering, styles,
  applications, and plugin hooks
- `rlvgl-widgets` supplies lower-level widget implementations
- `rlvgl-ui` composes those pieces into a more ergonomic application-facing API

This crate is `no_std` by default and is intended to work on both embedded
targets and simulator builds.

## Features

- `view`: enables the optional `view` module

## Typical Use

Use `rlvgl-ui` when you want to build application screens from themed controls
instead of wiring raw widget types and style values by hand. It is a good fit
for demo shells, control panels, settings screens, and overlay-driven UIs where
you want a consistent look across simulator and hardware targets.

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
