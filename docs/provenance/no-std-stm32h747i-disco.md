<!--
no-std-stm32h747i-disco.md - Provenance list for the STM32H747I-DISCO no_std build profile.
-->

# STM32H747I-DISCO no_std Provenance

This document records the Cargo packages resolved for the primary no_std
STM32H747I-DISCO CM7 build profile.

Scope command:

```sh
RUSTFLAGS="-C target-cpu=cortex-m7" cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio \
  --no-default-features \
  --locked
```

Provenance extraction commands:

```sh
cargo tree \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio \
  --no-default-features \
  --locked

cargo metadata \
  --format-version 1 \
  --filter-platform thumbv7em-none-eabihf \
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio \
  --no-default-features \
  --locked
```

The table separates firmware target dependencies from host-side build and
proc-macro dependencies. Host-side entries are used to build the artifact but
are not linked into the target firmware.

| Package | Version | Scope | Source | License | Repository |
|---|---:|---|---|---|---|
| bare-metal | 0.2.5 | target | crates.io | MIT OR Apache-2.0 | https://github.com/japaric/bare-metal |
| bare-metal | 1.0.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/bare-metal |
| bitfield | 0.13.2 | target | crates.io | MIT OR Apache-2.0 | https://github.com/dzamlo/rust-bitfield |
| bitflags | 2.10.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| byteorder | 1.5.0 | target | crates.io | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| cast | 0.3.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/japaric/cast.rs |
| cc | 1.2.34 | build host | crates.io | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| cortex-m-rt-macros | 0.7.5 | proc-macro host | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/cortex-m |
| cortex-m-rt | 0.7.5 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/cortex-m |
| cortex-m | 0.7.7 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/cortex-m |
| critical-section | 1.2.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/critical-section |
| embedded-alloc | 0.5.1 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-alloc |
| embedded-display-controller | 0.2.0 | target | crates.io | MIT/Apache-2.0 | https://github.com/richardeoin/embedded-display-controller |
| embedded-dma | 0.2.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-dma |
| embedded-hal | 0.2.7 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-hal |
| embedded-hal | 1.0.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-hal |
| embedded-io | 0.6.1 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/embedded-hal |
| embedded-sdmmc | 0.9.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded-community/embedded-sdmmc-rs |
| embedded-storage | 0.3.1 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded-community/embedded-storage |
| fugit | 0.3.7 | target | crates.io | MIT OR Apache-2.0 | https://github.com/korken89/fugit |
| gcd | 2.3.0 | target | crates.io | MIT/Apache-2.0 | https://github.com/frewsxcv/rust-gcd |
| hash32 | 0.3.1 | target | crates.io | MIT OR Apache-2.0 | https://github.com/japaric/hash32 |
| heapless | 0.8.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/heapless |
| itoa | 1.0.15 | build host | crates.io | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| libm | 0.2.15 | target | crates.io | MIT | https://github.com/rust-lang/compiler-builtins |
| linked_list_allocator | 0.10.5 | target | crates.io | Apache-2.0/MIT | https://github.com/phil-opp/linked-list-allocator |
| memchr | 2.7.5 | build host | crates.io | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| nb | 0.1.3 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/nb |
| nb | 1.1.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/nb |
| panic-halt | 1.0.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/korken89/panic-halt |
| paste | 1.0.15 | proc-macro host | crates.io | MIT OR Apache-2.0 | https://github.com/dtolnay/paste |
| portable-atomic | 1.11.1 | target | crates.io | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| proc-macro2 | 1.0.101 | proc-macro host | crates.io | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| quote | 1.0.40 | proc-macro host | crates.io | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| rlvgl-app-disco-demo | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-audio-meters-core | 0.2.0 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-core | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-decomp | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-example-disco | 0.2.0 | target root | workspace | MIT (workspace) | https://github.com/softoboros/rlvgl |
| rlvgl-i18n | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-platform | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-playit | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-ui | 0.2.0 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rlvgl-widgets | 0.2.2 | target | workspace | MIT | https://github.com/softoboros/rlvgl |
| rustc_version | 0.2.3 | build host | crates.io | MIT/Apache-2.0 | https://github.com/Kimundi/rustc-version-rs |
| ryu | 1.0.20 | build host | crates.io | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| sdio-host | 0.9.0 | target | crates.io | MIT OR Apache-2.0 | https://github.com/jkristell/sdio-host |
| semver-parser | 0.7.0 | build host | crates.io | MIT/Apache-2.0 | https://github.com/steveklabnik/semver-parser |
| semver | 0.9.0 | build host | crates.io | MIT/Apache-2.0 | https://github.com/steveklabnik/semver |
| serde_json | 1.0.143 | build host | crates.io | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| serde | 1.0.219 | build host | crates.io | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| shlex | 1.3.0 | build host | crates.io | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex |
| stable_deref_trait | 1.2.0 | target | crates.io | MIT/Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| stm32-fmc | 0.3.2 | target | crates.io | MIT/Apache-2.0 | https://github.com/stm32-rs/stm32-fmc |
| stm32h7 | 0.15.1 | target | crates.io | MIT/Apache-2.0 | https://github.com/stm32-rs/stm32-rs |
| stm32h7xx-hal | 0.16.0 | target | crates.io | 0BSD | https://github.com/stm32-rs/stm32h7xx-hal |
| syn | 2.0.106 | proc-macro host | crates.io | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| unicode-ident | 1.0.18 | proc-macro host | crates.io | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| vcell | 0.1.3 | target | crates.io | MIT OR Apache-2.0 | https://github.com/japaric/vcell |
| void | 1.0.2 | target | crates.io | MIT | https://github.com/reem/rust-void.git |
| volatile-register | 0.2.2 | target | crates.io | MIT OR Apache-2.0 | https://github.com/rust-embedded/volatile-register |

## Non-Cargo Inputs To Review

The selected build profile also embeds or stages local non-Cargo inputs:

| Input | Local path | Usage in this profile | Provenance note |
|---|---|---|---|
| Linker script | `examples/stm32h747i-disco/memory.x` | Staged by `examples/stm32h747i-disco/build.rs` for the CM7 link | Workspace-authored unless otherwise noted in file history |
| Display/media assets | `examples/stm32h747i-disco/assets/media/`, `examples/stm32h747i-disco/assets/icons/` | Embedded with `include_bytes!` from the Disco example and Disco app | `assets/manifest.yml` records SHA-256 hashes for source raw assets and marks them MIT; generated `.rle` assets should be traced back to those raw inputs |
| Bitmap fonts | `examples/stm32h747i-disco/assets/fonts/` and `assets/fonts/DejaVuSans*.ttf` | Embedded with `include_bytes!` in platform and example rendering paths | Font provenance should be reviewed separately; DejaVu fonts normally carry their own font license rather than the project MIT license |
| Built-in translations | `i18n/build.rs` output | Build script embeds generated `translations.bin` into `rlvgl-i18n` | Workspace-generated from `i18n` sources |

Vendored source trees present in the repository are not part of this selected
Cargo profile unless their features are enabled:

| Tree | Local path | When used | Provenance note |
|---|---|---|---|
| FreeRTOS kernel | `examples/stm32h747i-disco/freertos/Source/` | Only with the `freertos` feature | License text is `examples/stm32h747i-disco/freertos/Source/LICENSE.md` |
| STM CubeMX/CMSIS/HAL reference project | `examples/stm32h747i-disco/DiscoBiscuit/` | Reference/generated project material; not compiled by the selected Rust-only profile | License texts are under `DiscoBiscuit/Drivers/**/LICENSE.txt` |
| LVGL C submodule | `lvgl/` | Reference only per project policy; not linked into Rust library | Upstream LVGL repository: https://github.com/lvgl/lvgl |
| STM32 Open Pin Data | `chips/stm/STM32_open_pin_data/` | Used by BSP/chip database generation, not by this firmware build profile directly | See `NOTICES.md` for pinned commit and BSD-3-Clause notice |

## Review Notes

- The Cargo package list contains 61 resolved packages for this profile.
- No Cargo package in the resolved target tree had a missing license or
  repository after applying the workspace repository fallback for local crates.
- For export-control evidence, keep the scope command and `Cargo.lock` together;
  changing feature flags materially changes this list.
- The `freertos` profile needs a separate provenance pass because it compiles
  C sources through the `cc` crate and links `libfreertos.a`.
