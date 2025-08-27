<!--
docs/CROSS-TESTING.md - Cross-target test linker requirements and native test guidance.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Cross-Target Testing

Running tests for embedded targets such as `thumbv7em-none-eabihf` requires a compatible linker. By default `cargo test` invokes `arm-none-eabi-gcc`, which fails if the GCC toolchain is missing. To avoid this dependency, install the `rust-lld` component and configure Cargo to use it:

```bash
rustup component add rust-lld
```

Place the following snippet in `.cargo/config.toml` to select the linker for that target:

```toml
# .cargo/config.toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

With this configuration, cross-tests link without the external GCC toolchain.

## Native test runs

Most unit tests do not rely on embedded targets and can run on the host:

```bash
cargo test --workspace
```

This runs tests with the host linker and skips the cross-linker requirement. Only hardware integration tests need the embedded target.

## CI notes

The current CI workflow executes tests on the host target only, but cross-target builds should ensure `rust-lld` is available if tests are added. Install the component during setup and reuse the configuration above:

```yaml
- name: Install rust-lld
  run: rustup component add rust-lld
```

## Troubleshooting

- **`linker "rust-lld" not found`** – ensure the component is installed with `rustup component add rust-lld`.
- **Tests still invoke `arm-none-eabi-gcc`** – verify `.cargo/config.toml` contains the `[target.thumbv7em-none-eabihf]` block.
- **Linker errors about `memory.x`** – some examples require a linker script; build with the board's `build.rs` or drop the `--target` flag to run on the host.

## Board-specific nuances

- **STM32H747I-DISCO** – Enable the `stm32h747i_disco` feature and let the example's `build.rs` stage `memory.x`. Build or test with:

  ```bash
  cargo build --bin rlvgl-stm32h747i-disco --features stm32h747i_disco --target thumbv7em-none-eabihf
  ```

  Host-only tests can omit the `--target` flag to run natively.
