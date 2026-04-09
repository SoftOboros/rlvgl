<!--
README.md - Publish-facing overview for the rlvgl-ui crate.
-->

# rlvgl-ui
Package: `rlvgl-ui`

`rlvgl-ui` is the higher-level component crate in the `rlvgl` workspace. It
builds on top of `rlvgl-core` and `rlvgl-widgets` to provide themed controls,
layout helpers, style builders, and overlay utilities for application code.

## What It Provides

- ready-to-use components such as `Alert`, `Badge`, `Button`, `Checkbox`,
  `Drawer`, `Input`, `Modal`, `Radio`, `Switch`, `Tag`, `Text`, and `Toast`
- layout helpers including `HStack`, `VStack`, `Grid`, and `BoxLayout`
- a theme/token layer with `Theme`, `Tokens`, `Style`, and `StyleBuilder`
- overlay and transient UI helpers such as `EventWindow`
- an optional `view` feature for view-oriented helpers

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
