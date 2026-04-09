```markdown
# Notas de inicio (bring-up) de STM32H747I-DISCO (CM7)

Este archivo resume el estado esencial, las decisiones y los próximos pasos para el inicio (bring-up) del CM7 del objetivo STM32H747I-DISCO en este repositorio.

## Objetivo, compilación, depuración
- Objetivo: STM32H747I-DISCO (núcleo CM7)
- Tarea de compilación: `build-disco (cm7)` con características:
  - `stm32h747i_disco_cm7,dma2d,backlight_pwm,pac_sdram_init,sdram_ramtest`
- Inicio de VSCode: "CM7 attach (OpenOCD externo)"
  - `runToEntryPoint: main`
  - Sin comandos de semihosting posteriores al lanzamiento

## Instantánea de la secuencia de arranque
La ruta CM7 completa consistentemente los pasos iniciales de arranque:

1. Espera activa corta para que los depuradores puedan conectarse antes de los efectos secundarios del periférico.
2. Las regiones MPU (DTCM/ITCM/SRAM/SRAM4/SRAM3/SRAM2/SRAM1 + SDRAM) se programan antes de cualquier acceso al registro RCC o FMC.
3. Los GPIO de SDRAM se fuerzan a AF12/VeryHigh y el reloj del núcleo FMC se habilita; la secuencia de comandos SDRAM se ejecuta directamente a través del PAC mientras el dispositivo aún está en el árbol de reloj de reinicio.
4. PWR SMPS + VOS1 se configuran una vez que la SDRAM está estable.
5. HAL RCC configura PLL1/PLL3 y vuelve a habilitar FMC para el tiempo de ejecución normal, seguido de la inicialización del panel/retroiluminación.

## Relojes (actual)
- HSE = 25 MHz; SYSCLK = 400 MHz (PLL1)
- LTDC: PLL3R ≈ 32 MHz (ruta de reloj de píxel)
- Reloj del núcleo FMC (post-HAL) = PLL2R 150 MHz → SDCLK ≈ 75 MHz (divisor /2)
- AHB (HCLK) = 200 MHz a través de HPRE=/2

## Estado de SDRAM (FMC)
- Perfil del dispositivo: IS42S32800G (32 MiB, x32) con SDCLK ahora a ~75 MHz (núcleo FMC 150 MHz / 2).
- La inicialización impulsada por PAC ahora ocurre antes de que el HAL toque RCC/PWR:
  - GPIO D/E/F/G/H/I enrutados a AF12 + velocidad VeryHigh antes de cualquier cambio de reloj.
  - Relojes FMC habilitados a través de `AHB3ENR` y `C1_AHB3ENR`.
  - SDCR1/SDTR1/SDCMR/SDRTR escritos directamente; el sondeo BUSY verifica que cada comando se complete a la velocidad de reloj más alta.
- Verificado: la instrumentación MPU aún registra el estado de la región (`MPU_TRACE`/`MPU_DUMP`); con el SDCLK de 75 MHz (PLL2R 150 MHz con divisor /2), el asignador/pruebas de SDRAM permanecen estables después del inicio.
- Registro: el semihosting permanece deshabilitado en la ruta crítica de SDRAM para evitar bloqueos de SWD.

## Optimización / Paso a paso
- Perfil de desarrollo: `opt-level = 0` para el espacio de trabajo; `stm32h7xx-hal` permanece sin optimizar para la depuración.
- Eliminado el giro de adjuntar al inicio (sin demora inicial prolongada).

## Observaciones conocidas
- El paso a paso debe aterrizar después de cada escritura PAC `SDCMR`; detenerse antes de que BUSY se borre puede dejar el controlador esperando indefinidamente.
- El sondeo del visor de periféricos aún puede competir con la inicialización de SDRAM; manténgalo cerrado mientras se ejecuta paso a paso la secuencia.
- El botón de pausa de VS Code puede emitir `reset halt`; use `monitor halt` en su lugar y confíe en las migas de pan `.noinit` para la reconstrucción posterior al reinicio.
- La vista de desensamblaje sigue siendo invaluable al verificar las escrituras de registros PAC sin procesar.

## Estrategia de paso a paso mínima y confiable
1) Coloque puntos de interrupción en el asistente de PAC justo después de cada escritura `SDCMR` en `configure_fmc_sdram`.
2) Después de que el punto de interrupción se active, lea:
   - `SDCMR @ 0x5200_4140` para confirmar MODE/CTBx.
   - `SDSR @ 0x5200_4158` y espere a que BUSY=0 antes de continuar.
3) Continúe a través de ClkEnable → retardo → PALL → AR(8) → LoadMode → programación de SDRTR.
4) Si aún aparecen fallas de MPU, compile temporalmente con `--features skip_sdram_mpu`; una vez que MPU se complete, inspeccione `MPU_TRACE`/`MPU_DUMP` para el estado final de la región.

## Propuestas para avanzar
- Captura de fallas: agregue manejadores de excepciones CM7 para interrumpir en el lugar en caso de fallas.
  - Implemente `HardFault`, `BusFault`, `UsageFault` con bucles `bkpt()` para que las fallas nunca vuelvan a reiniciar.
- Retardo del contador de ciclos DWT (opcional): use CYCCNT para retrasos deterministas e inmunes al depurador en lugar de giro asm (ambos están bien; CYCCNT facilita el ajuste).
- Esperas entre comandos: si algún `SDSR.BUSY` no se borra, extienda el bucle activo o agregue un breve retardo entre las escrituras `SDCMR` en `configure_fmc_sdram()`.
- Verificación cruzada de CubeIDE: genere la inicialización de FMC/SDRAM para H747I-DISCO y refleje los valores exactos de SDCR/SDTR/SDCMR/SDRTR para una comparación directa.
- Verificación de alcance (hardware): verifique SDCLK en PG8 y que SDCKE1 se active antes de PALL (PH7).
- Asignación de memoria: mantenga el heap/stack predeterminado en DTCM e introduzca un segundo asignador respaldado por `SDRAM`.
  - Defina una sección de salida `.sdram_heap` en `memory.x`.
  - Respaldelo con un estático de Rust usando `#[link_section = ".sdram_heap"]` e inicialice un `Heap`/bump allocator dedicado.
  - Proteja el asignador de SDRAM detrás de un mutex para que los componentes de alto impacto opten explícitamente.

### Manejadores de excepciones
Aún se recomienda: agregue manejadores `HardFault`, `BusFault`, `UsageFault` que se repitan en `bkpt()` para que las fallas no reinicien el CM7. Cuando se activen, lea `0xE000_ED28`, `0xE000_ED2C`, `0xE000_ED34`, `0xE000_ED38` para identificar la causa.

## Referencia rápida (Direcciones)
- Base FMC: `0x5200_4000`
  - `BCR1..`: `0x5200_4000`
  - `SDCR1/2`: `0x5200_4080`
  - `SDTR1/2`: `0x5200_4104`
- `SDCMR`: `0x5200_4140`
- `SDSR`: `0x5200_4158`
- `MPU_TRACE`: `0x2001_0030`
- `MPU_DUMP`: `0x2001_0034` (pares de RBAR/RASR escritos durante el inicio de MPU)

Comprobaciones típicas mientras se avanza paso a paso:
- Después de cada escritura `SDCMR`, lea `SDSR` y confirme que BUSY se borra (comando completado) antes de emitir el siguiente.
- Verifique que `BCR1.FMCEN` esté configurado por HAL (`memory_controller_enable()`).
- Confirme que los campos `SDCR` coincidan con el CAS/ancho/bancos/col/fila deseados; `SDTR` cumple con el tiempo en SDCLK.

## Valores predeterminados actuales en el código
- FMCSEL: `PLL2R` @ `100 MHz`
- `max_sd_clock_hz: 75_000_000`
- `hclk: 200_000_000` (HPRE=/2)
- Velocidades de pines: VeryHigh para todos los pines FMC
- Sin registro de semihost de SDRAM; sin giro de inicio inicial

## Qué necesita Codex (Esenciales)
- Confirmación de en qué etapa `configure_fmc_sdram` podría quedarse atascado `SDSR.BUSY` (después de ClkEnable, PALL, AR(8) o LoadMode).
- Instantáneas de registro de un solo disparo alrededor de cada comando:
  - `p/x *(u32*)0x52004140` (SDCMR) y `p/x *(u32*)0x52004158` (SDSR).
  - `p/x *(u32*)0x52004080`/`0x52004084` (SDCR1/2) y `0x52004104`/`0x52004108` (SDTR1/2).
- Contexto de falla si algo falla: CFSR/HFSR/MMFAR/BFAR + migas de pan `.noinit`.
- Las capturas de alcance de PG8 (SDCLK) y PH7 (SDCKE1) siguen siendo útiles para la validación de tiempos.
- Volcado de registro de CubeIDE (si está disponible) para verificación cruzada de valores SDCR/SDTR/SDCMR/SDRTR.

## Hoja de trucos de GDB Stepping
- Detenerse justo después de cada escritura `SDCMR` dentro de `configure_fmc_sdram`.
  1) Deje que el STR se ejecute, luego `p/x $lr` / `tbreak *$lr` según sea necesario para volver a la función que lo llamó.
  2) Inspeccione `SDCMR`/`SDSR`; espere a que BUSY se borre antes de continuar.
- Si debe ejecutar el STR paso a paso, `x/8i $pc`, `ni` a través del almacenamiento, luego `finish` de nuevo al asistente.
- Prefiera `monitor halt` para pausar; `interrupt` a menudo emite un reinicio. Si ocurre un reinicio, recupere el contexto de `MPU_TRACE`/`MPU_DUMP`.

## Resumen de pines de hardware (FMC SDRAM)
- Reloj/Habilitar: PG8 (SDCLK), PH7 (SDCKE1), PH6 (SDNE1)
- Control: PF11 (SDNRAS), PG15 (SDNCAS), PH5 (SDNWE)
- Dirección del banco: PG4 (BA0), PG5 (BA1)
- Dirección: PF0..PF5 (A0..A5), PF12..PF15 (A6..A9), PG0..PG2 (A10..A12)
- Carriles de byte: PE0 (NBL0), PE1 (NBL1), PI4 (NBL2), PI5 (NBL3)
- Datos: PD14..PD15, PD0..PD1, PE7..PE15, PD8..PD10, PH8..PH15, PI0..PI3, PI6, PI7, PI9, PI10
- Todos los anteriores configurados en AF12 + Speed::VeryHigh en el código

## Próximas acciones (Mínimas)
1) Agregue manejadores de excepciones CM7 para capturar fallas inmediatamente.
2) Siga validando cada comando SDRAM con puntos de interrupción después de `SDCMR` y ajuste el sondeo BUSY si es necesario.
3) Refleje los tiempos de CubeIDE si surgen diferencias de hardware; extienda el retardo entre comandos según sea necesario.
4) Considere restaurar un `opt-level` modesto una vez que el inicio permanezca estable.
5) Continúe capturando `MPU_TRACE`/`MPU_DUMP`; sobreviven a los reinicios y confirman el estado de MPU.

## Inicio de SDRAM (PAC Init desplegado)

Contexto: La ejecución paso a paso del asistente HAL era frágil bajo el depurador, por lo que la característica `pac_sdram_init` ahora despliega la secuencia de inicialización con escrituras explícitas de PAC.

- Característica: `pac_sdram_init` (predeterminada en la compilación CM7).
- Secuencia (base FMC 0x5200_4000):
  - Habilite los relojes FMC a través de `AHB3ENR.FMCEN` y `C1_AHB3ENR.FMCEN` (antes de HAL RCC).
  - `BCR1.FMCEN = 1` (habilitar controlador).
  - Programe `SDCR1` para IS42S32800G: NC=9, NR=12, MWID=32-bit, NB=4, CAS=3, SDCLK=/2, RBURST=1, RPIPE=0.
- Programe los tiempos `SDTR1` (@ ~75 MHz SDCLK): TMRD=2 ciclos (valor de escritura 1), TXSR=7 ciclos (valor 6), TRAS=5 ciclos (valor 4), TRC=7 ciclos (valor 6), TWR=2 ciclos (valor 1), TRP=2 ciclos (valor 1), TRCD=2 ciclos (valor 1).
  - Emita comandos a través de `SDCMR` con sondeo BUSY entre cada uno:
    1) Habilitar reloj (MODE=1, CTB1=1).
    2) Precargar todo (MODE=2, CTB1=1).
    3) Auto-refresco ×8 (MODE=3, NRFS=7 codificando 8 ciclos).
    4) Cargar registro de modo (MODE=4, MRD=0x0230, CTB1=1).
- Programe `SDRTR` para ~7.81 µs a 75 MHz: COUNT ≈ 566 (escriba `COUNT<<1`).

Observado (bien): `SDCMR.MODE=4` (LoadMode) y `SDSR.BUSY=0` post-secuencia. La ruta PAC regresa a la función que lo llamó de manera confiable.

### Región MPU para SDRAM (M7)
Una vez que HAL habilita la MPU, la SDRAM externa debe estar cubierta por una región MPU o la primera lectura puede causar MemManage/BusFault. Instalamos una región MPU de SDRAM inmediatamente después de la inicialización de PAC:

- Base de la región: `0xC000_0000`; tamaño: 32 MiB (campo SIZE = 24)
- Atributos: Memoria normal, no cacheable (TEX=1, C=0, B=0), Compartible=1, AP=Acceso completo
- MPU habilitada con PRIVDEFENA, más barreras DSB/ISB

Resultado: las primeras lecturas de SDRAM ya no fallan; las pruebas rápidas tienen éxito.

## Configuración de depuración de VS Code (Cortex-Debug)

Aprendizajes clave para estabilizar la conexión/ejecución en H7 con OpenOCD:
- Evite emitir `monitor reset halt` después de conectar; fuerza a reexaminar y aterriza en Reset (PC=0x08000298), borrando el estado de FMC.
- Prefiera una conexión pura que se ejecute inmediatamente:
  - Agregue una configuración con `request: "attach"`, `servertype: "external"`, y `postAttachCommands: ["continue"]`.
- Cargue las macros de GDB de forma segura:
  - `preLaunchCommands`: `set mem inaccessible-by-default off`, `add-auto-load-safe-path ${workspaceFolder}/.gdbinit`, `source ${workspaceFolder}/.gdbinit`
- Mantenga la vista de registros periféricos cerrada mientras inicia FMC/SDRAM; el sondeo frecuente puede causar errores de "Busy" de SWD.
- Las configuraciones de lanzamiento establecen `objdumpPath` + `showDisassembly: "always"`; la vista de desensamblaje mostrará instrucciones incluso cuando no haya símbolos.

Entrada de lanzamiento mínima de adjuntar+ejecutar (extracto):

```
{
  "name": "CM7 attach + run (no reset)",
  "type": "cortex-debug",
  "request": "attach",
  "servertype": "external",
  "gdbTarget": "localhost:3333",
  "preLaunchCommands": [
    "set mem inaccessible-by-default off",
    "add-auto-load-safe-path ${workspaceFolder}/.gdbinit",
    "source ${workspaceFolder}/.gdbinit"
  ],
  "postAttachCommands": ["continue"]
}
```

### Invocación de OpenOCD (recomendado)
- Use SWD lento + conecte bajo reinicio durante el inicio:
  - `transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init`

## Macros y tácticas de GDB

- Ayudantes `.gdbinit` (en la raíz del repositorio): `faultregs`, `sdramregs`, `wait_busy_clear`, `lrtrap_safe`.
- lrtrap vs BKPT en línea:
  - `lrtrap` es efectivo en sitios de llamada reales; `bkpt` en línea no tiene LR que pueda tbreak; use `set $pc = $pc + 2` para saltar.
- Los watchpoints en `SDCMR @ 0x5200_4140` son útiles, pero pueden causar problemas; prefiera la ruta PAC + volcados `sdramregs` en su lugar.

## Pasos de validación (rápido)
- Después de que se ejecute la inicialización de PAC, verifique rápidamente la SDRAM a través de la consola de depuración:
  - `set {unsigned int}0xC0000000 = 0xDEADBEEF`
  - `x/wx 0xC0000000` → `0xDEADBEEF`
- Comprobaciones de estado de FMC:
  - `x/wx 0x52004140` (SDCMR) → MODE=4 después de LoadMode
  - `x/wx 0x52004158` (SDSR) → Bit 5 de BUSY = 0
  - `x/wx 0x52004080` (SDCR1), `0x52004104` (SDTR1) → valores no predeterminados

## Rutas HAL vs PAC

- `pac_sdram_init` es la ruta predeterminada y ahora se ejecuta antes de cualquier cambio de reloj/energía de HAL; se ha validado de principio a fin.
- El asistente HAL anterior (`hal_sdram`) sigue estando disponible para experimentación, pero ya no forma parte de la compilación estándar de CM7.

## Peculiaridades conocidas (OpenOCD/H7)
- "read_memory ... 0x5C001004 ... examine-end failed": ruido inofensivo al adjuntar; evite reinicios/exámenes frecuentes desde GDB.
- "target not halted ... resume was requested": ocurre si se presiona Continuar mientras ya se está ejecutando; solo Pausar → Continuar.
- Si la conexión es inestable, reinicie OpenOCD con SWD lento y conecte bajo reinicio, luego adjunte con la configuración sin reinicio anterior.
```
```
