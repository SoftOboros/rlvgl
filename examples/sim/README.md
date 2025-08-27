<!--
examples/sim/README.md - Desktop simulator example.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl Demo
---
Demonstrates core widgets alongside plugin features such as QR code generation
and PNG/JPEG image decoding.

## Usage

Run the simulator with a custom screen resolution using:

```bash
cargo run --bin rlvgl-sim -- --screen=800x480
```

Omit `--screen` to use the default 320x240 resolution. By default the simulator
uses the CPU fallback blitter for rendering. Pass `--wgpi` to enable the wgpu
accelerated blitter instead. Provide a file path as an additional argument to
export a single frame to a PNG instead of launching the interactive window.

For asset management workflows using `rlvgl-creator`, see
[`README-CREATOR.md`](../../README-CREATOR.md).

## Limitations

On displays that exceed the GPU's maximum texture size, the simulator
renders to a smaller internal framebuffer and scales the result to fit the
window. This scaling can introduce letterboxing or reduced sharpness on
ultra-high-resolution monitors.

## Requirements
The rlvgl demo requires libgtk-3-dev and librlotte-dev for display and support of Lottie creation (Not implemented).

see [Dockerfile](../../Dockerfile) and [setup-ci-env.sh](../../scripts/setup-ci-env.sh) for understanding of 
the complate set of packages used to execute.

If unavalable, rlottie can be built from source as follows:
```bash
# Set install prefix path (modify as needed))
INSTALL_PREFIX="/opt/rlottie"

# Build and install rlottie locally
git clone https://github.com/Samsung/rlottie
cd rlottie && mkdir build && cd build
cmake .. \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DLIB_INSTALL_DIR=lib \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
make -j"$(nproc)"
make install
cd ../..

# Export environment variables to future steps
export PKG_CONFIG_PATH="$INSTALL_PREFIX/lib/pkgconfig"
export BINDGEN_EXTRA_CLANG_ARGS="-I$INSTALL_PREFIX/include"

```

---

## VS Code setup

### Plugins
- [CodeLLDB](https://github.com/vadimcn/codelldb)
- [rust-analyzer](https://rust-analyzer.github.io)
- Even Better TOML

### Launch Configuration
(.vscode/lanch.json)[../../../.vscode/launch.json] contains setting to execute under OSX on x86

```json
{
  "version": "0.2.0",
  "configurations": [

    {
      "name": "Debug sim",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/x86_64-apple-darwin/debug/rlvgl-sim",
      "args": [],
      "cwd": "${workspaceFolder}",
      "cargo": {
        "args": ["build", "--package=rlvgl-sim", "--target=x86_64-apple-darwin"]
      },
      "sourceLanguages": ["rust"]
    },
  ]
}
```

Change out the target string for your host platform.
