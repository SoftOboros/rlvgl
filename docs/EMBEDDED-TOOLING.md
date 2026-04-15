<!--
  EMBEDDED-TOOLING.md — User installation and setup guide for embedded
  toolchains used by rlvgl examples. Covers ARM/STM32, ESP32 (RISC-V +
  Xtensa), and AVR. Includes Intel-macOS workarounds where upstream
  installers are broken.
-->

# Embedded Tooling Setup

This guide installs the toolchains needed to build and flash every
embedded target currently wired into `rlvgl`'s workspace. Follow only the
sections for targets you actually intend to build.

## Supported Targets

| Target                  | Architecture        | Rust target triple                  | Flasher       | Notes                                   |
| ----------------------- | ------------------- | ----------------------------------- | ------------- | --------------------------------------- |
| STM32H747I-DISCO (CM7)  | ARM Cortex-M7       | `thumbv7em-none-eabihf`             | probe-rs      | Primary board, full rlvgl demo.         |
| STM32H747I-DISCO (CM4)  | ARM Cortex-M4F      | `thumbv7em-none-eabihf`             | probe-rs      | Second core, MicroPython experiments.   |
| ESP32 (WROOM/WROVER)    | Xtensa LX6          | `xtensa-esp32-none-elf` (`+esp`)    | espflash      | DevKitS-R or DevKitC, serial flash.     |
| ESP32-S2                | Xtensa LX7          | `xtensa-esp32s2-none-elf` (`+esp`)  | espflash      | Xtensa via esp toolchain.               |
| ESP32-S3                | Xtensa LX7          | `xtensa-esp32s3-none-elf` (`+esp`)  | espflash      | Xtensa via esp toolchain.               |
| ESP32-C3                | RISC-V (IMC)        | `riscv32imc-unknown-none-elf`       | espflash      | Beetle ESP32-C3 example board.          |
| ESP32-C6                | RISC-V (IMAC)       | `riscv32imac-unknown-none-elf`      | espflash      | Beetle-DFR1172 companion.               |
| ESP32-P4                | RISC-V (IMAC)       | `riscv32imac-unknown-none-elf`      | espflash      | Beetle-DFR1172 main.                    |
| Arduino Uno (ATmega328) | AVR (8-bit)         | `avr-unknown-gnu-atmega328`         | avrdude       | Nightly Rust + `avr-hal` template.      |

ESP32-C3 and ESP32-C6/P4 use *different* RISC-V targets — install both
if you plan to build for any of them.

## Common Prerequisites

1. **rustup** (>= 1.29):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **Stable toolchain >= 1.90** (older stable will fail `cargo install
   cargo-generate` because `cargo-util-schemas` requires rustc 1.90+):
   ```bash
   rustup update stable
   ```
3. **Nightly toolchain** (required for AVR Rust, optional elsewhere):
   ```bash
   rustup toolchain install nightly
   rustup component add rust-src --toolchain nightly
   ```
4. **Homebrew** on macOS, the distro package manager on Linux. Windows
   users should use `winget` or `scoop` where indicated; specific
   Windows paths are out of scope for this guide.
5. **Git**, **Python 3.10+**, and a C toolchain already available via
   Xcode Command Line Tools on macOS or `build-essential` on Linux.

## ARM / STM32

Everything STM32 in the workspace uses the Cortex-M Rust target and
`probe-rs` for flashing and debug. No ESP or AVR pieces are needed.

```bash
rustup target add thumbv7em-none-eabihf
cargo install probe-rs --locked --features cli
```

`probe-rs` auto-detects the on-board ST-Link on the DISCO board. See
[MAKE.md](./MAKE.md) for the `make flash-disco` / `make probe-rs-gdb`
workflows that this toolchain unlocks.

## ESP32

There are two parallel Rust paths for Espressif parts:

* **RISC-V cores** (ESP32-C3, -C6, -P4) use upstream `rustup` targets.
* **Xtensa cores** (original ESP32, -S2, -S3) use a vendor-built fork
  distributed by `esp-rs`. This fork is installed via `espup` under a
  custom rustup toolchain named `esp`.

You need the toolchains for the cores you target and the **common
flasher** section once.

### RISC-V targets (C3, C6, P4)

```bash
rustup target add riscv32imc-unknown-none-elf      # C3
rustup target add riscv32imac-unknown-none-elf     # C6, P4
rustup target add riscv32imafc-unknown-none-elf    # reserved
```

That is all the compiler setup for RISC-V chips. The board examples
(`rlvgl-example-beetle-esp32c3` etc.) build against plain `+stable`.

### Xtensa targets (original ESP32, S2, S3)

Install **espup**:

```bash
cargo install espup --locked
```

Then install the Xtensa toolchain. The happy path is:

```bash
espup install
```

This pulls four things from GitHub Releases:

1. `xtensa-esp-elf` GCC (from `espressif/crosstool-NG`).
2. `xtensa-esp-elf-clang` LLVM (from `espressif/llvm-project`).
3. `rust-X.Y.Z-<host>.tar.xz` Xtensa Rust (from `esp-rs/rust-build`).
4. `rust-src-X.Y.Z.tar.xz` standard library source.

Files land under `~/.rustup/toolchains/esp/` and a symlink is created at
`~/.espup/esp-clang`.

#### Intel macOS workaround

As of 2026-04-15 `esp-rs/rust-build` **no longer publishes an
`x86_64-apple-darwin` asset** after `v1.90.0.0`. On Intel Macs you must
pin to 1.90.0.0:

```bash
espup install --toolchain-version 1.90.0.0 --skip-version-parse
```

If `espup install` fails mid-download with `HTTP GET Error: 400 Bad
Request`, `error sending request for url`, or `File exists (os error
17)` (an espup 0.16.0 bug where it trips over its own pre-existing
symlinks), install the three tarballs by hand:

```bash
# 1. Xtensa Rust
cd /tmp
wget https://github.com/esp-rs/rust-build/releases/download/v1.90.0.0/rust-1.90.0.0-x86_64-apple-darwin.tar.xz
wget https://github.com/esp-rs/rust-build/releases/download/v1.90.0.0/rust-src-1.90.0.0.tar.xz
tar -xf rust-1.90.0.0-x86_64-apple-darwin.tar.xz
tar -xf rust-src-1.90.0.0.tar.xz
mkdir -p ~/.rustup/toolchains/esp
cd rust-nightly-x86_64-apple-darwin && \
  ./install.sh --destdir="$HOME/.rustup/toolchains/esp" --prefix="" \
               --without=rust-docs --disable-ldconfig
cd ../rust-src-nightly && \
  ./install.sh --destdir="$HOME/.rustup/toolchains/esp" --prefix="" \
               --disable-ldconfig

# 2. Xtensa LLVM libs
cd /tmp
wget https://github.com/espressif/llvm-project/releases/download/esp-20.1.1_20250829/libs-clang-esp-20.1.1_20250829-x86_64-apple-darwin.tar.xz
mkdir -p ~/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829
tar -xf libs-clang-esp-20.1.1_20250829-x86_64-apple-darwin.tar.xz \
    -C ~/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829

# 3. xtensa-esp-elf GCC
cd /tmp
wget https://github.com/espressif/crosstool-NG/releases/download/esp-15.2.0_20250920/xtensa-esp-elf-15.2.0_20250920-x86_64-apple-darwin.tar.xz
mkdir -p ~/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920
tar -xf xtensa-esp-elf-15.2.0_20250920-x86_64-apple-darwin.tar.xz \
    -C ~/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920

# 4. esp-clang symlink
mkdir -p ~/.espup
ln -s ~/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib \
      ~/.espup/esp-clang
```

Upgrade-check before re-pinning: if esp-rs later restores Intel-macOS
builds, the asset list in `https://api.github.com/repos/esp-rs/rust-build/releases?per_page=10`
will again contain an `x86_64-apple-darwin.tar.xz` entry.

### Serial flasher tools

```bash
cargo install espflash --locked        # primary flasher, Rust
pipx install esptool                   # Python fallback, commonly needed
```

`espflash` handles `build -> convert -> flash -> monitor` in one
command; `esptool` is useful for chip-ID probes, fuse reads, and when
espflash's bootloader-entry handshake fails.

If `pipx` itself is unavailable (see **Known installation blockers**
below) you can install esptool directly with the Python.org / Homebrew
`pip3`:

```bash
/usr/local/bin/pip3 install --user esptool
```

The binary ends up at `~/Library/Python/3.12/bin/esptool` on macOS (or
the equivalent `~/.local/bin/esptool` on Linux) — add that directory to
your `PATH` (see below).

### ESP-IDF (C-first framework — optional)

Only install this if you plan to mix ESP-IDF C code into a project. The
Rust ecosystem does not require it. Standard recipe:

```bash
mkdir -p ~/esp && cd ~/esp
git clone --recursive https://github.com/espressif/esp-idf.git
./esp-idf/install.sh
source ./esp-idf/export.sh
```

## AVR (Arduino Uno, ATmega328P)

AVR support spans two overlapping universes:

* **C / Arduino** via `avr-gcc`, `avrdude`, and either the Arduino IDE
  or `arduino-cli`. This is the usual path for flashing Arduino
  sketches and for hand-written C.
* **Rust on AVR** via `avr-hal` + `ravedude` + nightly Rust with
  `-Z build-std`. Still rough, but works for the `examples/uno-blink`
  class of demos.

### avr-gcc and binutils

macOS via Homebrew's `osx-cross/avr` tap:

```bash
brew tap osx-cross/avr
brew install avr-gcc avr-binutils
```

Linux: `sudo apt install gcc-avr avr-libc binutils-avr` (Debian/Ubuntu)
or the distro equivalent.

### avrdude

On modern macOS (CLT 15.2+) or Linux:

```bash
brew install avrdude                   # macOS
sudo apt install avrdude               # Debian/Ubuntu
```

If the brew source build fails (older CLT — see the **Known installation
blockers** section), install `arduino-cli` and let it pull Arduino's
prebuilt avrdude:

```bash
brew install arduino-cli
arduino-cli core update-index
arduino-cli core install arduino:avr
```

That puts `avrdude` at
`~/Library/Arduino15/packages/arduino/tools/avrdude/8.0.0-arduino1/bin/avrdude`
along with its `avrdude.conf`. Add that `bin/` directory to `PATH`.

### ravedude (Rust AVR runner)

```bash
cargo install ravedude --locked
```

`ravedude` wraps avrdude with a USB-serial monitor that auto-matches
Arduino boards. `avr-hal`'s `cargo-generate` template wires it in as the
runner in `.cargo/config.toml`.

### cargo-generate (project templates)

```bash
cargo install cargo-generate --locked
```

Used by `avr-hal`'s `cargo generate` template command and by several
ESP-Rust project templates. Required stable rustc 1.90+.

## Shell Environment Setup

Add the following to `~/.zshrc` (or `~/.bashrc`) so the installed
binaries resolve and `esp-rs` libclang is found during `cargo build`:

```bash
# Python user scripts (esptool, pipx-managed tools)
export PATH="$HOME/Library/Python/3.12/bin:$PATH"     # macOS Python.org 3.12
# export PATH="$HOME/.local/bin:$PATH"                # Linux / pipx default

# Arduino-bundled avrdude (only if installed via arduino-cli)
export PATH="$HOME/Library/Arduino15/packages/arduino/tools/avrdude/8.0.0-arduino1/bin:$PATH"

# Xtensa GCC
export PATH="$HOME/.rustup/toolchains/esp/xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin:$PATH"

# esp-rs libclang (required for Xtensa bindgen)
export LIBCLANG_PATH="$HOME/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib/libclang.dylib"
# On Linux:
# export LIBCLANG_PATH="$HOME/.rustup/toolchains/esp/xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib/libclang.so"
```

Reload with `exec $SHELL -l`.

## Verification

Run each check for the targets you installed:

```bash
# rustup
rustup toolchain list           # should include stable, esp (if Xtensa)
rustup target list --installed  # should include your ESP / ARM targets

# ARM
cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco --features cm7

# Xtensa
rustup +esp --version           # should print "(1.90.0.0)" or current pin
xtensa-esp-elf-gcc --version

# RISC-V ESP
cargo check -p rlvgl-example-beetle-esp32c3 --features esp_hal \
  --target riscv32imc-unknown-none-elf

# Flashers
espflash --version
esptool version

# AVR
avr-gcc --version
avrdude -?                      # prints help banner with version
ravedude --version
```

An end-to-end ESP32 smoke test (DevKitC, C3-DevKitM, etc. — anything
with auto-reset wired):

```bash
espflash board-info --port /dev/cu.usbserial-XXXX
```

Carrier boards without auto-reset (for example the **ESP32-DevKitS-R**
used for WROVER flashing) need manual bootloader entry:

1. Hold **BOOT**.
2. Tap **EN** while still holding BOOT.
3. Release BOOT.
4. Re-run `espflash board-info ...`.

## Known Installation Blockers

The following issues surfaced during setup on an Intel macOS 13 machine
and are preserved here in case you hit them. None of them are blocking
on Apple-Silicon macOS or mainstream Linux.

### Xcode Command Line Tools too old (macOS)

Homebrew now classifies macOS 13 (Ventura) as Tier 3 and requires CLT
15.2+. CLT 14.x cannot build certain formulas from source. Symptoms:

> This build failure was expected, as this is not a Tier 1 configuration.

If `softwareupdate --list` shows no CLT update, run:

```bash
sudo rm -rf /Library/Developer/CommandLineTools
sudo xcode-select --install
```

If even that does not upgrade CLT (some older Mac/macOS combinations are
frozen at 14.3.1), the affected formulas must be installed through a
prebuilt channel instead of brew. Alternatives:

| Formula   | Workaround                                                   |
| --------- | ------------------------------------------------------------ |
| `pipx`    | `pip3 install --user <tool>` directly.                       |
| `avrdude` | Install via `arduino-cli core install arduino:avr`.          |
| Python    | Use a Homebrew cask (e.g. `brew install --cask python`) or   |
|           | the python.org installer, both of which ship prebuilt.       |

### `espup install` fails with HTTP 400

The `ureq` HTTP client inside `espup` 0.16.0 occasionally returns `400
Bad Request` or `error sending request for url` on older macOS
configurations where TLS libraries are stale. The underlying URLs are
reachable via `wget`. Use the **manual tarball recipe** in the Intel
macOS workaround section above.

### `espup install` claims a prior install exists but the dir is empty

If a previous espup run crashed mid-download, it may leave an empty
`esp-<version>_<date>/` directory under
`~/.rustup/toolchains/esp/xtensa-esp-elf/` or
`~/.rustup/toolchains/esp/xtensa-esp32-elf-clang/`. Subsequent espup
runs log:

> [warn]: Previous installation of LLVM exists in: ... Reusing this installation

and then fail downstream. Fix: remove the stub directories before
retrying espup, or just drop into the manual tarball recipe.

### `cargo install cargo-generate` requires rustc 1.90+

`cargo-util-schemas@0.10.2` bumped its MSRV past rustc 1.88. If your
stable toolchain is older, `rustup update stable` first, or pass
`--locked`:

```bash
cargo install cargo-generate --locked
```

## Tool Reference

| Tool            | Install command (happy path)            | Binary location (macOS default)                                                                                                | Purpose                         |
| --------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- |
| `rustup`        | `curl https://sh.rustup.rs \| sh`       | `~/.cargo/bin/rustup`                                                                                                           | Rust toolchain manager          |
| `probe-rs`      | `cargo install probe-rs`                | `~/.cargo/bin/probe-rs`                                                                                                         | SWD/JTAG flash + debug (STM32)  |
| `espup`         | `cargo install espup`                   | `~/.cargo/bin/espup`                                                                                                            | Xtensa toolchain installer      |
| `espflash`      | `cargo install espflash`                | `~/.cargo/bin/espflash`                                                                                                         | ESP32 serial flasher (Rust)     |
| `esptool`       | `pipx install esptool`                  | `~/Library/Python/3.12/bin/esptool` / `~/.local/bin/esptool`                                                                    | ESP32 serial flasher (Python)   |
| `cargo-generate`| `cargo install cargo-generate --locked` | `~/.cargo/bin/cargo-generate`                                                                                                   | Project templates               |
| `ravedude`      | `cargo install ravedude`                | `~/.cargo/bin/ravedude`                                                                                                         | AVR Rust runner (wraps avrdude) |
| `avr-gcc`       | `brew install osx-cross/avr/avr-gcc`    | `/usr/local/bin/avr-gcc`                                                                                                        | AVR C compiler                  |
| `avrdude`       | `brew install avrdude` or arduino-cli   | `/usr/local/bin/avrdude` or `~/Library/Arduino15/packages/arduino/tools/avrdude/8.0.0-arduino1/bin/avrdude`                     | AVR programmer                  |
| `arduino-cli`   | `brew install arduino-cli`              | `/usr/local/bin/arduino-cli`                                                                                                    | Arduino core/library manager    |
| Arduino IDE     | `brew install --cask arduino-ide`       | `/Applications/Arduino IDE.app`                                                                                                 | GUI Arduino IDE (optional)      |

## See Also

- [MAKE.md](./MAKE.md) — STM32 convenience targets (build, flash, GDB).
- [CROSS-TESTING.md](./CROSS-TESTING.md) — Host-side cross-target tests.
- [CHIP-SUPPORT.md](./CHIP-SUPPORT.md) — Which chips the chipdb/BSP
  generator supports.
- `CLAUDE.md` at the repo root — agent runbook with the exact build,
  flash, and `/pre-publish` invocations for this workspace.
