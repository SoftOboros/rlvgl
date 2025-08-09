# rlvgl Demo
---
Demonstrates core widgets alongside plugin features such as QR code generation
and PNG/JPEG image decoding.
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
