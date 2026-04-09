<!--
README.md - Publish-facing overview for the rlvgl-core crate.
-->

# rlvgl-core

Package: `rlvgl-core`

`rlvgl-core` is the foundation crate for the `rlvgl` workspace. It defines the
runtime model used by widgets, applications, renderers, and platform backends.

## Main Areas

- widget tree and composition via `Widget` and `WidgetNode`
- event dispatch and application integration via `event` and `application`
- renderer-facing drawing abstractions in `renderer`
- style, theme, animation, and font utilities
- optional plugin modules for assets and integrations such as `png`, `jpeg`,
  `gif`, `qrcode`, `fontdue`, `lottie`, `canvas`, `fatfs`, `nes`, and `apng`

## Target Model

`rlvgl-core` is `no_std` by default. Host-side decoders and similar integrations
are feature-gated, and image/QR decoding features are automatically kept off on
`target_os = "none"` where those dependencies are not intended to be pulled
into embedded binaries.

## Where It Is Used

- `rlvgl` re-exports it as part of the top-level toolkit
- `rlvgl-widgets` builds concrete widgets on top of it
- `rlvgl-platform` provides hardware and simulator backends for it
- `rlvgl-ui` layers higher-level components and theming on top
- `rlvgl-app-demo` uses it as the application/runtime contract

## Typical Use

Reach for `rlvgl-core` when you are building custom widgets, writing a renderer
or display backend, or integrating the toolkit into a new runtime. Most end
applications will use it indirectly through `rlvgl`, `rlvgl-widgets`, or
`rlvgl-ui`.

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
