<!--
docs/CROSS-TESTING.md - Requisitos de vinculador de pruebas de destino cruzado y guía de pruebas nativas.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Pruebas de destino cruzado

La ejecución de pruebas para destinos integrados como `thumbv7em-none-eabihf` requiere un vinculador compatible. Por defecto, `cargo test` invoca `arm-none-eabi-gcc`, lo que falla si la cadena de herramientas GCC no está presente. Para evitar esta dependencia, instale el componente `rust-lld` y configure Cargo para usarlo:

```bash
rustup component add rust-lld
```

Coloque el siguiente fragmento en `.cargo/config.toml` para seleccionar el vinculador para ese destino:

```toml
# .cargo/config.toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

Con esta configuración, las pruebas cruzadas se vinculan sin la cadena de herramientas GCC externa.

## Ejecuciones de pruebas nativas

La mayoría de las pruebas unitarias no dependen de destinos integrados y pueden ejecutarse en el host:

```bash
cargo test --workspace
```

Esto ejecuta las pruebas con el vinculador del host y omite el requisito del vinculador cruzado. Solo las pruebas de integración de hardware necesitan el destino integrado.

## Notas de CI

El flujo de trabajo de CI actual ejecuta pruebas solo en el destino del host, pero las compilaciones de destino cruzado deben asegurar que `rust-lld` esté disponible si se agregan pruebas. Instale el componente durante la configuración y reutilice la configuración anterior:

```yaml
- name: Install rust-lld
  run: rustup component add rust-lld
```

## Solución de problemas

- **`linker "rust-lld" not found`** – asegúrese de que el componente esté instalado con `rustup component add rust-lld`.
- **Las pruebas siguen invocando `arm-none-eabi-gcc`** – verifique que `.cargo/config.toml` contenga el bloque `[target.thumbv7em-none-eabihf]`.
- **Errores del vinculador sobre `memory.x`** – algunos ejemplos requieren un script de vinculación; compile con el `build.rs` de la placa o elimine la bandera `--target` para ejecutar en el host.

## Matices específicos de la placa

- **STM32H747I-DISCO** – Habilite la característica `stm32h747i_disco` y deje que el `build.rs` del ejemplo prepare `memory.x`. Compile o pruebe con:

  ```bash
  cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco" \
    --target thumbv7em-none-eabihf

  SD + FATFS smoke (adaptador no_std; CI marcado como allow-failure debido a `core_io` en rustc más reciente):

  cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,fatfs_nostd" \
    --target thumbv7em-none-eabihf
  ```

  Las pruebas solo de host pueden omitir la bandera `--target` para ejecutarse de forma nativa.
