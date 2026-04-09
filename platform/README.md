<!--
README.md - Publish-facing overview for the rlvgl-platform crate.
-->

# rlvgl-platform
Package: `rlvgl-platform`

`rlvgl-platform` contains the backend and integration layer for `rlvgl`. It
bridges the core widget/runtime model to real hardware targets and to simulator
backends used during desktop development.

## What It Provides

- display and input abstractions used by hosts and board ports
- blitters, surfaces, and dirty-region restoration via the compositor
- simulator-facing modules such as app loading and renderer backends
- embedded integrations for the STM32H747I-DISCO path, including display,
  touch, QSPI, SD, DMA2D, and optional audio support

## Common Feature Groups

- `simulator`: desktop integration and dynamic app loading
- `stm32h747i_disco`: board-specific hardware support for the flagship demo
  target
- `dma2d`, `audio`, `sd_storage`, `fatfs_nostd`, `splash`: optional embedded
  subsystems layered onto that board support
- passthrough asset features such as `png`, `jpeg`, `gif`, `qrcode`, `apng`,
  `fontdue`, `lottie`, and `canvas`

## stm32h747i_disco backend

The optional `stm32h747i_disco` feature enables the embedded board support used
by the main hardware demo path in this repository. That includes the display
stack, FT5336 touch controller support, storage helpers, and optional
DMA2D/audio paths depending on the features you enable.

See [README-VENDOR.md](./README-VENDOR.md) for the vendor support policy.

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
