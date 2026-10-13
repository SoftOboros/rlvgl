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
- `wayland`: native Linux XDG-shell and bounded SHM presentation without an
  imposed event-loop framework
- `uefi`: UEFI GOP display + keyboard + serial transport for pre-OS runtimes
- `stm32h747i_disco`: board-specific hardware support for the flagship demo
  target
- `dma2d`, `audio`, `sd_storage`, `fatfs_nostd`, `splash`: optional embedded
  subsystems layered onto that board support
- passthrough asset features such as `png`, `jpeg`, `gif`, `qrcode`, `apng`,
  `fontdue`, `lottie`, and `canvas`

## Wayland backend

The optional `wayland` feature provides a session-owned native client for
Linux windowed and kiosk deployments. `WaylandSession::connect` performs only
bounded registry setup; applications then integrate its file descriptor and
`io_interest`/`dispatch_ready` methods with their own poller. Configure notices
must be explicitly accepted before presentation. The `WaylandDisplay` adapter
implements `DisplayDriver` over a complete private shadow frame, while frame
callbacks pace submission and `wl_buffer.release` exclusively controls SHM
slot reuse.

WLD-01 intentionally has no seat-input API. Pointer, keyboard, and touch
translation remain in the separately gated WLD-02 phase. See the
[WLD initiative](../docs/wayland/README.md) for the accepted lifecycle,
geometry, resource, and evidence contracts.

## UEFI backend

The optional `uefi` feature enables a pre-OS display and input backend
targeting UEFI GOP (Graphics Output Protocol). This allows rlvgl
applications to run directly from the UEFI shell or as UEFI boot
applications — no OS kernel required.

Modules:

- `uefi.rs` — GOP framebuffer display driver with software blitting,
  keyboard polling via SimpleTextInput (with synthesized KeyUp events),
  and `Screen`/`DisplayDriver` trait implementations.
- `uefi_serial_transport.rs` — `PlayitTransport` implementation over UEFI
  Serial I/O protocol, enabling playit test automation over a QEMU
  virtio-serial chardev exposed as a TCP socket on the host.

The `uefi-disco` example (`examples/uefi-disco/`) demonstrates the full
pipeline: disco-demo controller running on GOP with serial-based playit
test driving.

## stm32h747i_disco backend

The optional `stm32h747i_disco` feature enables the embedded board support used
by the main hardware demo path in this repository. That includes the display
stack, FT5336 touch controller support, storage helpers, and optional
DMA2D/audio paths depending on the features you enable.

The STM32H747I-DISCO demo runs on three platforms — all sharing the same
display init, touch I2C, and DMA2D engine code from this crate:

| Platform | Feature | Task model | Guide |
|----------|---------|------------|-------|
| Bare-metal | (default) | Cooperative main loop | [Vol II](../docs/disco-platform-guide/README.md) |
| FreeRTOS | `freertos` | Preemptive tasks with semaphores | [Vol IV](../docs/disco-freertos-guide/README.md) |
| Zephyr | `zephyr` | Zephyr threads with C FFI shell | [Vol V](../docs/disco-zephyr-guide/README.md) |

The `freertos` and `zephyr` features gate platform-specific code paths
(e.g., skipping the second PG3 reset pulse under Zephyr, CSleep LPENR
register variants for FreeRTOS/Zephyr low-power modes).

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
