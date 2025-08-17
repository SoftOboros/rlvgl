# rlvgl Demo
---
Demonstrates core widgets alongside plugin features such as QR code generation
and PNG/JPEG image decoding.

## Usage

Run the simulator with a custom screen resolution using:

```bash
cargo run --bin rlvgl-sim -- --screen=800x480
```

Omit `--screen` to use the default 320x240 resolution. Pass a file path as an
additional argument to export a single frame to a PNG instead of launching the
interactive window.

## rlvgl-creator workflows

`rlvgl-creator` manages assets for rlvgl projects and can be combined with the
simulator when developing interfaces. Common flows include:

### Initialize a new asset pack

```bash
rlvgl-creator init
rlvgl-creator add-target host vendor
```

`init` creates `icons/`, `fonts/`, and `media/` directories alongside a
`manifest.yml`. `add-target` records where converted assets should be written
for the host simulator.

### Import and convert assets

Place source files under the asset directories and scan them:

```bash
rlvgl-creator scan
rlvgl-creator convert
```

`scan` hashes assets and updates the manifest; `convert` normalizes images to
raw RGBA and refreshes the manifest.

### Preview and scaffold

```bash
rlvgl-creator preview
rlvgl-creator scaffold assets-pack
```

`preview` writes 64×64 thumbnails under `thumbs/`. `scaffold` generates a
dual-mode assets crate that can be embedded or vendored into the simulator.

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
