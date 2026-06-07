<!--
examples/disco-sim/README.md - Host simulator for the shared disco demo runtime.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl Disco Simulator
---

`rlvgl-disco-sim` is a desktop wgpu-backed simulator that runs the same
shared `rlvgl-app-disco-demo` controller used by the STM32H747I-DISCO
firmware and the UEFI demo.  It exposes a playit TCP automation socket so
the same test suite drives every target.

## Build

The package is `rlvgl-example-disco-sim` and its binary is `rlvgl-disco-sim`.
This crate has no Cargo features today; everything pulls through the shared
`rlvgl-app-disco-demo` and `rlvgl-platform/simulator` dependencies.

| Method | Command |
| --- | --- |
| Make | `make build-disco-sim` |
| Cargo | `cargo build -p rlvgl-example-disco-sim --bin rlvgl-disco-sim` |

## Usage

| Flag | Effect |
| --- | --- |
| `--screen=WxH` | Override the default 800x480 framebuffer |
| `--headless[=path]` | Render once to ASCII (path optional) and exit |
| `--automation-headless` | Run the main loop without opening a window |
| `--playit-port[=N]` | Bind a playit TCP server (use 0 for auto-assign) |
| positional `file.png` | Capture a single frame to PNG and exit |

When `--playit-port` is set, the binary prints
`PLAYIT_READY tcp://127.0.0.1:<port>` to stdout once the socket is listening.
Combine with `--automation-headless` for CI runs.

## Tests

The full disco simulator playit test suite is run via:

```bash
make test-disco-sim
```

That target builds the binary, runs the `rlvgl-app-disco-demo` unit tests,
the Rust integration tests in `tests/playit_automation.rs`, and the Node.js
suite under `playit/node/test/`.

See [`OPTIONS.md`](./OPTIONS.md) for the (currently empty) feature reference.
