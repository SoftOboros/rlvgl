<!--
Plan para alinear las superposiciones de placa `.ioc` de CubeMX con el IR canónico y las plantillas de inicialización de Rust.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Plan de Alineación de IR de Placa STM32

## Brecha actual
- El importador actual solo emite mapas de pin → señal → AF.
- Las plantillas requieren contexto por pin: puerto/índice, clase, modo, pull, velocidad, tipo de salida (otype), EXTI, etc.

## Plan
0. [ ] **Limpieza de MCU** – `stm32_xml_scraper.py` debe omitir o eliminar los MCUs que carecen de definiciones de pin para que las conversiones `.ioc` posteriores no se aborten.
1. [ ] **Constructor de contexto de pines** – Analizar las claves `.ioc` (`Signal`, `Mode`, `GPIO_PuPd`, `GPIO_Speed`, `GPIO_OType`, `GPIO_Label`) y fusionar con el JSON canónico del MCU para emitir objetos por pin.
2. [ ] **Normalización de búsqueda** – Centralizar los mapas que traducen las cadenas de Cube a bits MODER/OTYPER/OSPEEDR/PUPDR y nombres de enumeración HAL. Incrustar los campos de bits derivados y las cadenas HAL en cada contexto de pin. Los pines a los que les falta una AF almacenan `null` para que las plantillas puedan omitir las escrituras AFR.
3. [ ] **Emisión de superposición de placa** – Almacenar el contexto canónico de pines por placa en `boards/<board>.json` para que todas las placas compartan el mismo esquema.
4. [ ] **Reglas de plantilla HAL** – Generar `into_alternate`, `into_push_pull_output`, etc., utilizando ayudantes normalizados de velocidad/pull/otype.
5. [ ] **Reglas de plantilla PAC** – Emitir escrituras de registro para MODER/OTYPER/OSPEEDR/PUPDR y AFR; incluir el enrutamiento EXTI cuando `is_exti` sea verdadero.
6. [ ] **Derivación EXTI** – Calcular `exti_port_index`, `exti_rising`, y `exti_falling` a partir de las cadenas de modo `.ioc` para pines capaces de interrupción.
7. [ ] **Pruebas** – Pruebas de instantáneas para `.ioc` → contexto canónico más pruebas de humo de plantilla HAL y PAC.

## Contexto canónico de pines
Cada pin en una superposición de placa sigue un único esquema JSON construido a partir de claves `.ioc` y búsquedas de AF de MCU:

```json
{
  "name": "PC12",
  "port": "C",
  "index": 12,
  "class": "Peripheral|GPIO|System|Raw",
  "sig_full": "SDMMC1_CK",
  "instance": "SDMMC1",
  "signal": "CK",
  "af": 12,                    // null if no alternate function
  "mode": "GPIO_AF_PP",
  "pull": "GPIO_NOPULL",
  "speed": "GPIO_SPEED_FREQ_VERY_HIGH",
  "otype": "GPIO_OType_PP",
  "label": "SDMMC1_CK",
  "is_exti": false,
  "exti_line": null,
  "exti_port_index": null,
  "exti_rising": false,
  "exti_falling": false,
  "moder_bits": 0b10,
  "pupd_bits": 0b00,
  "speed_bits": 0b11,
  "otype_bit": 0,
  "hal_speed": "VeryHigh",
  "hal_pull": "None"
}
```

## Tablas de búsqueda
Los mapas normalizados traducen las cadenas de Cube a bits de registro y nombres de enumeración HAL:

- `MODE_TO_MODER` – p. ej. `GPIO_AF_PP` → `0b10`
- `PULL_TO_BITS` – `GPIO_PULLUP` → `0b01`
- `SPEED_TO_BITS` – `GPIO_SPEED_FREQ_HIGH` → `0b10`
- `OTYPE_TO_BIT` – `GPIO_OType_OD` → `1`
- `HAL_SPEED` – `GPIO_SPEED_FREQ_VERY_HIGH` → `VeryHigh`
- `HAL_PULL` – `GPIO_PULLDOWN` → `PullDown`

## Clases de plantilla
Las reglas de renderizado dependen del campo `class`:

- **Peripheral** – configurar el modo de función alternativa y aplicar `otype`, `pull` y `speed`; establecer el slot AFR en `af`.
- **GPIO** – controlar MODER/PUPDR/OSPEEDR/OTYPER; cuando `is_exti` es verdadero, enrutar el pin a través de `SYSCFG.EXTICR` y configurar `RTSR`/`FTSR` según `exti_rising`/`exti_falling`.
- **System** – tratar como `Peripheral` para señales como `RCC_MCO` o pines de depuración, a menos que se anule explícitamente.

Las plantillas HAL utilizan `into_alternate`, `into_push_pull_output`, o `into_analog` con ayudantes como `map_speed_to_hal`. Las plantillas PAC emiten escrituras de registro explícitas para cada campo y programan condicionalmente los registros AFR y EXTI.

## Patrón de plantilla HAL
Habilitar cada puerto GPIO usado una vez y configurar los pines usando el contexto canónico:

```rust
let mut rcc = dp.RCC.constrain();
let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);

// USART1_TX en PA9
let pa9 = gpioa.pa9.into_alternate::<{ pins["PA9"].af }>();
pa9.set_speed(Speed::{ pins["PA9"].hal_speed });
pa9.internal_pull_up({ pins["PA9"].pull == "GPIO_PULLUP" });
pa9.set_open_drain({ pins["PA9"].otype == "GPIO_OType_OD" });

// Entrada analógica en PA3
let pa3 = gpioa.pa3.into_analog();
```

## Patrón de plantilla PAC
Escribir registros directamente utilizando campos de bits precalculados:

```rust
// Habilitar GPIOC
dp.RCC.ahb2enr.modify(|_, w| w.gpiocen().set_bit());

// Configurar PC12 como función alternativa con AF12
let n = 12;
dp.GPIOC.moder.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0b11 << (n * 2))) | (0b10 << (n * 2)))
});
dp.GPIOC.afrh.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0xF << ((n % 8) * 4))) | ((12u32 & 0xF) << ((n % 8) * 4)))
});
```

Los pines con capacidad EXTI también se enrutan a través de `SYSCFG.EXTICR` y configuran `RTSR`/`FTSR` según `exti_rising`/`exti_falling`.
