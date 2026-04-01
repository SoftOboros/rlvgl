```markdown
<!--
  MEMORY.md — STM32H747I‑DISCO ejemplo de diseño de memoria
  Describe cómo CM7/CM4 utilizan DTCM, D1/D2/D3 SRAM, y el buzón compartido.
-->

# Diseño de Memoria (Ejemplo STM32H747I‑DISCO)

Este documento explica cómo el ejemplo de doble núcleo particiona la memoria entre CM7 y CM4, cómo funciona el buzón compartido y cómo los scripts de enlazado y build.rs seleccionan las regiones correctas.

## Resumen

- CM7 utiliza DTCM para su pila/datos por defecto y puede colocar grandes búferes en D1 AXI SRAM.
- CM4 posee D2 SRAM (excepto un buzón reservado) y toda la D3 SRAM4 para uso de retención/bajo consumo.
- Un buzón de 1 KB en D2 SRAM3 es compartido por ambos núcleos para semáforos y datos de traspaso.

## Regiones

- DTCM (local de CM7)
  - Base: `0x2000_0000`, Tamaño: 128 KB
  - Utilizado por CM7 como `RAM` por defecto en `memory.x`.

- D1 AXI SRAM (dominio compartido, dividido 3/4:1/4)
  - Total: 512 KB en `0x2400_0000`.
  - Segmento CM7 `D1_CM7`: `0x2400_0000`..`0x2405_FFFF` (384 KB).
  - Segmento CM4 `D1_CM4`: `0x2406_0000`..`0x2407_FFFF` (128 KB).
  - Actualmente declaradas pero aún no utilizadas por secciones; reservadas para framebuffers, pools compartidos, etc.

- D2 SRAM (dominio CM4, buzón reservado)
- CM4 `RAM`: `0x3000_0000`..`0x3003_FFFF` (contabilizado como 255 KB de RAM general + 1 KB de buzón = 256 KB total).
  El buzón de 1 KB reside en `0x3004_7000` y debe ser accedido por separado de la ventana de RAM general.
  - Buzón (compartido): `0x3004_7000`..`0x3004_73FF` (1 KB) en ambos scripts de enlazado.

- D3 SRAM4 (retención de CM4)
  - `D3_CM4`: `0x3800_0000`..`0x3800_FFFF` (64 KB) declarado para retención/bajo consumo.

## Tabla de Regiones

| Región     | Propietario | Dominio | Base         | Tamaño | Propósito/Notas                                    |
|------------|-------------|---------|--------------|--------|----------------------------------------------------|
| RAM        | CM7         | DTCM    | `0x2000_0000`| 128 KB | Pila/datos por defecto de CM7 (TCM rápido, no compartido) |
| D1_CM7     | CM7         | D1 AXI  | `0x2400_0000`| 384 KB | Segmento CM7 AXI SRAM (grandes búferes, FB, etc.)   |
| D1_CM4     | CM4         | D1 AXI  | `0x2406_0000`| 128 KB | Segmento CM4 AXI SRAM                             |
| RAM (CM4)  | CM4         | D2      | `0x3000_0000`| 255 KB | Pila/datos por defecto de CM4 (RAM general)        |
| MAILBOX    | Compartido  | D2      | `0x3004_7000`| 1 KB   | Buzón entre núcleos; semáforo en `+0x000`         |
| D3_CM4     | CM4         | D3      | `0x3800_0000`| 64 KB  | Región de retención/bajo consumo                  |

## Buzón (Compartido)

- Rango de direcciones: `0x3004_7000`..`0x3004_73FF` (1 KB).
- Palabra de semáforo: `0x3004_7000` (desplazamiento 0), accedido como `AtomicU32`.
- Uso:
  - CM7 (propietario principal del reloj) llama a `signal_clocks_ready()` después de la inicialización de potencia/RCC.
  - CM4 llama a `wait_for_clocks()` para bloquear hasta que el semáforo esté configurado, luego procede.
- Bytes restantes: reservados para un futuro protocolo de traspaso (por ejemplo, configuración, razones de arranque).

## Binarios y Scripts de Enlazado

- Binario CM7: `rlvgl-stm32h747i-disco`
  - Script de enlazado: `examples/stm32h747i-disco/memory.x` (DTCM `RAM` por defecto, buzón declarado).
  - PAC/HAL importa el objetivo `stm32h747cm7`.

- Binario CM4: `rlvgl-stm32h747i-disco-cm4`
  - Script de enlazado: `examples/stm32h747i-disco/memory_cm4.x` (D2 `RAM`, buzón declarado).
  - PAC/HAL importa el objetivo `stm32h747cm4`.

- Selección de compilación:
  - `build.rs` de nivel superior copia el `memory*.x` correcto en `OUT_DIR/memory.x` basándose en `CARGO_BIN_NAME`.
  - `cargo:rustc-link-arg=-Tlink.x` se emite para que cortex‑m‑rt incluya el script escenificado.

## Alimentación y Relojes

- La inicialización de BSP (PAC/HAL) configura PWR en H7 con SCUEN gating, suministro (SMPS/LDO), SDLEVEL y escribe `D3CR.VOS`.
- VOS y el suministro se obtienen de CubeMX `.ioc` o se sobrescriben a través del entorno:
  - `STM32_PWR_SUPPLY=SMPS|LDO`
  - `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3`
- Los relojes aún no se emiten desde `.ioc`; el sistema arranca con los valores predeterminados de reinicio (~64 MHz) a menos que su aplicación configure RCC.

## Trabajo Futuro

- Definir las secciones `.axisram_cm7`, `.axisram_cm4` y `.retained_d3` y mapearlas a `D1_CM7`, `D1_CM4` y `D3_CM4`.
- Formalizar el protocolo del buzón (diseño de la estructura, versionado, campos de comando/ack).
- Emitir la configuración RCC/PLL para CM7 desde `.ioc` y bloquear CM4 hasta `signal_clocks_ready()`.
```
