<!--
CHANGELOG.md - Notas sobre las versiones de la base de datos de chips y placas.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Registro de cambios

## Sin publicar
- DISCO: Se añadió un adaptador FATFS sin_std (`platform::sd_fatfs_adapter`) y cableado de ejemplo opcional
  para montar y listar `/assets` en STM32H747I-DISCO (`fatfs_nostd` +
  `sd_assets_demo`).
- Documentos DISCO: Se marcó como completado el manejo de scripts del enlazador, la inicialización I2C táctil completada, se añadieron
  notas sobre el aumento de la retroiluminación, lista de verificación de inicio de SDMMC y sección de resolución de problemas.
- README de ejemplo: Se aclararon las banderas de construcción y los indicadores en pantalla para el éxito/fracaso del montaje de SD.
- Crates iniciales de proveedores para placas STM, Nordic, Espressif, NXP, Silicon Labs, Microchip, Renesas, Texas Instruments y RP2040.
- Se añadió `tools/bump_vendor_versions.py` para aumentar las versiones de los crates después de regenerar los datos de los pines.
- Se documentó la integración del creador con los crates del proveedor para que las selecciones de placas reflejen las bases de datos incluidas.
- Se añadió `scripts/gen_ioc_bsps.sh` para convertir por lotes archivos CubeMX `.ioc` usando `rlvgl-creator`.
- `rlvgl-creator` ahora puede cargar definiciones canónicas de MCU junto con superposiciones de placas de archivos de proveedores.
- Se añadió `rlvgl-creator board from-ioc` para convertir proyectos CubeMX de usuario en superposiciones de placas.
- Se añadió la bandera `--allow-reserved` a `rlvgl-creator bsp from-ioc` para permitir los pines SWD `PA13`/`PA14`.
