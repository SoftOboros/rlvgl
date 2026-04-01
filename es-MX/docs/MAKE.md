<!--
  MAKE.md — Guía del desarrollador para los objetivos de conveniencia de Makefile
  Cubre los objetivos disponibles, los flujos típicos y los requisitos previos.
-->

# Uso de Makefile (Desarrollador)

El repositorio incluye un Makefile ligero con objetivos de conveniencia para acelerar los flujos de trabajo comunes de STM32H747I-DISCO: regeneración de BSP, construcción de ambos núcleos y gestión de OpenOCD.

## Requisitos previos

- Destino Rust: `thumbv7em-none-eabihf`
  - `rustup target add thumbv7em-none-eabihf`
- Cadena de herramientas Arm para depuración/flasheo (por ejemplo, GNU Tools for Arm Embedded)
- OpenOCD instalado y en el PATH

## Objetivos

- `make help`
  - Imprime un resumen de los objetivos disponibles.

- `make gen-stm32h747i-disco-bsp`
  - Regenera el BSP de ejemplo desde `DiscoBiscuit.ioc`.
  - Por defecto, `STM32_PWR_SUPPLY=SMPS` y `STM32_PWR_SDLEVEL=VOS1`.
  - Utiliza `examples/stm32h747i-disco/gen-bsp.sh` (idempotente; regenera solo si es necesario).

- `make build-disco`
  - Construye el ejemplo CM7: `rlvgl-stm32h747i-disco`.

- `make build-disco-cm4`
  - Construye el ejemplo CM4: `rlvgl-stm32h747i-disco-cm4`.

- `make build-disco-all`
  - Construye ambos ejemplos, CM7 y CM4.

- `make openocd`
  - Inicia OpenOCD con scripts estándar de ST-Link + destino STM32H7 y detiene la CPU.
  - Úselo con la configuración de VSCode "CM7 attach (external OpenOCD)".

- `make openocd-erase`
  - Borrado masivo a través de OpenOCD y salida. Usar con precaución.

## Flujos típicos

1) Regenerar BSP y construir ambos núcleos

```
make gen-stm32h747i-disco-bsp
make build-disco-all
```

2) Depurar usando OpenOCD externo (recomendado)

```
make openocd                       # terminal 1
# VSCode: launch "CM7 attach (external OpenOCD)"   # terminal 2/VSCode
```

3) Actualizar valores predeterminados de energía del BSP (anular entorno)

```
STM32_PWR_SUPPLY=LDO STM32_PWR_SDLEVEL=VOS2 make gen-stm32h747i-disco-bsp
```

## Notas

- El archivo `build.rs` de nivel superior auto-establece el script de enlace apropiado para cada binario de ejemplo (CM7 usa `memory.x`, CM4 usa `memory_cm4.x`).
- El espacio de trabajo de VSCode proporciona dos perfiles de lanzamiento; consulte `examples/stm32h747i-disco/BOOT.md` para las opciones de arranque de doble núcleo (A/B/C) y el protocolo de enlace basado en buzón.
