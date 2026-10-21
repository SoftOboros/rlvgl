#!/usr/bin/env bash
# The exact release supplements previously inline in publish.yml.
set -euo pipefail
RLVGL_LINT_STRICT=1 RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile
RUSTFLAGS="" cargo test -p rlvgl-chips-esp
RUSTFLAGS="" cargo test -p rlvgl --test esp_ir_roundtrip --features creator
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_cli --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo check -p rlvgl-example-beetle-esp32c3 --features esp_hal --target riscv32imc-unknown-none-elf
RUSTFLAGS="" cargo check -p rlvgl-example-beetle-esp32c3 --features bsp_pac --target riscv32imc-unknown-none-elf
RUSTFLAGS="" cargo clippy -p rlvgl-example-beetle-esp32c3 --features esp_hal --target riscv32imc-unknown-none-elf -- -D warnings
RUSTFLAGS="" cargo clippy -p rlvgl-example-beetle-esp32c3 --features bsp_pac --target riscv32imc-unknown-none-elf -- -D warnings
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32p4_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32p4_cli --features creator
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32p4_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c6_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c6_cli --features creator
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c6_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c5_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c5_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32h2_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32h2_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c61_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c61_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl-chips-silabs
RUSTFLAGS="" cargo test -p rlvgl --test bsp_silabs_slstk3701a_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_silabs_slstk3701a_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl-chips-ti
RUSTFLAGS="" cargo test -p rlvgl --test bsp_ti_cc1352r_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_ti_cc1352r_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo test -p rlvgl-chips-microchip
RUSTFLAGS="" cargo test -p rlvgl --test bsp_microchip_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_microchip_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo check --manifest-path examples/launchxl-cc1352r1/Cargo.toml --target thumbv7em-none-eabihf
RUSTFLAGS="" cargo check --manifest-path examples/slstk3701a/Cargo.toml --target thumbv7em-none-eabihf
RUSTFLAGS="" cargo check --manifest-path examples/feather-m4-express/Cargo.toml --target thumbv7em-none-eabihf
RUSTFLAGS="" cargo check -p rlvgl-platform --target aarch64-unknown-linux-gnu --features linux_fbdev
RUSTFLAGS="" cargo check -p rlvgl-platform --target armv7-unknown-linux-gnueabihf --features linux_fbdev
RUSTDOCFLAGS="--cfg docsrs --cfg nightly" cargo doc --all-features --no-deps --target x86_64-unknown-linux-gnu
