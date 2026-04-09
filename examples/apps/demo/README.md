<!--
README.md - Publish-facing overview for the rlvgl-app-demo crate.
-->

# rlvgl-app-demo
Package: `rlvgl-app-demo`

`rlvgl-app-demo` is the packaged demo application for the `rlvgl` workspace. It
implements the `Application` trait and is intended to be hosted by simulator or
hardware runtimes that want a small, representative UI to boot into.

## What It Demonstrates

- button, label, and container composition using the stock widget set
- localized UI text through `rlvgl-i18n`
- optional plugin/media paths such as GIF, PNG, JPEG, and QR code demos when
  those features are enabled
- a `no_std` application state model that can still light up richer host-only
  integrations on simulator builds

## Feature Flags

- `dylib`: prepare the crate for dynamic loading scenarios
- `gif`: enable the GIF-backed demo path
- `png`: enable the PNG-backed demo path on non-embedded targets
- `jpeg`: enable the JPEG-backed demo path on non-embedded targets
- `qrcode`: enable the QR demo path on non-embedded targets
- `fontdue`: forward font support needed by the demo host stack

## Typical Use

Most users will not depend on this crate directly. Instead, it is pulled into
the workspace's simulator or runtime binaries to provide a ready-made demo
screen for testing rendering, interaction, localization, and plugin features.

If you do want to host it yourself, construct `DemoApp` and hand it to a host
that understands the `rlvgl-core` application contract.

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../../../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
