<!--
Plan for aligning CubeMX `.ioc` board overlays with canonical IR and Rust init templates.
-->
# STM32 Board IR Alignment Plan

## Current gap
- `st_extract_af.py`/`st_ioc_board.py` emit pin → signal → AF maps only.
- Templates require per-pin context: port/index, class, mode, pull, speed, otype, EXTI, etc.

## Plan
0. [ ] **MCU cleanup** – `stm32_xml_scraper.py` should skip or delete MCUs lacking pin definitions so subsequent `.ioc` conversions do not abort.
1. [ ] **Pin context builder** – Parse `.ioc` keys (`Signal`, `Mode`, `GPIO_PuPd`, `GPIO_Speed`, `GPIO_OType`, `GPIO_Label`) and merge with canonical MCU JSON to emit per-pin objects.
2. [ ] **Lookup normalisation** – Centralise maps translating Cube strings to MODER/OTYPER/OSPEEDR/PUPDR bits and HAL enum names. Embed derived bitfields and HAL strings into each pin context. Pins missing an AF store `null` so templates can skip AFR writes.
3. [ ] **Board overlay emission** – Store the canonical pin context per board under `boards/<board>.json` so all boards share the same schema.
4. [ ] **HAL template rules** – Generate `into_alternate`, `into_push_pull_output`, etc., using normalised speed/pull/otype helpers.
5. [ ] **PAC template rules** – Emit register writes for MODER/OTYPER/OSPEEDR/PUPDR and AFR; include EXTI routing when `is_exti` is true.
6. [ ] **EXTI derivation** – Compute `exti_port_index`, `exti_rising`, and `exti_falling` from `.ioc` mode strings for interrupt-capable pins.
7. [ ] **Tests** – Snapshot tests for `.ioc` → canonical context plus HAL and PAC template smoke tests.

## Canonical pin context
Each pin in a board overlay follows a single JSON schema built from `.ioc` keys and MCU AF lookups:

```
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

## Lookup tables
Normalised maps translate Cube strings into register bits and HAL enum names:

- `MODE_TO_MODER` – e.g. `GPIO_AF_PP` → `0b10`
- `PULL_TO_BITS` – `GPIO_PULLUP` → `0b01`
- `SPEED_TO_BITS` – `GPIO_SPEED_FREQ_HIGH` → `0b10`
- `OTYPE_TO_BIT` – `GPIO_OType_OD` → `1`
- `HAL_SPEED` – `GPIO_SPEED_FREQ_VERY_HIGH` → `VeryHigh`
- `HAL_PULL` – `GPIO_PULLDOWN` → `PullDown`

## Template classes
Rendering rules depend on the `class` field:

- **Peripheral** – configure alternate function mode and apply `otype`, `pull`, and `speed`; set AFR slot to `af`.
- **GPIO** – drive MODER/PUPDR/OSPEEDR/OTYPER; when `is_exti` is true, route the pin through `SYSCFG.EXTICR` and configure `RTSR`/`FTSR` based on `exti_rising`/`exti_falling`.
- **System** – treat like `Peripheral` for signals such as `RCC_MCO` or debug pins unless explicitly overridden.

HAL templates use `into_alternate`, `into_push_pull_output`, or `into_analog` with helpers like `map_speed_to_hal`. PAC templates emit explicit register writes for each field and conditionally program AFR and EXTI registers.

## HAL template pattern
Enable each used GPIO port once and configure pins using the canonical context:

```rust
let mut rcc = dp.RCC.constrain();
let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);

// USART1_TX on PA9
let pa9 = gpioa.pa9.into_alternate::<{ pins["PA9"].af }>();
pa9.set_speed(Speed::{ pins["PA9"].hal_speed });
pa9.internal_pull_up({ pins["PA9"].pull == "GPIO_PULLUP" });
pa9.set_open_drain({ pins["PA9"].otype == "GPIO_OType_OD" });

// Analog input on PA3
let pa3 = gpioa.pa3.into_analog();
```

## PAC template pattern
Write registers directly using precomputed bitfields:

```rust
// Enable GPIOC
dp.RCC.ahb2enr.modify(|_, w| w.gpiocen().set_bit());

// Configure PC12 as Alternate Function with AF12
let n = 12;
dp.GPIOC.moder.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0b11 << (n * 2))) | (0b10 << (n * 2)))
});
dp.GPIOC.afrh.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0xF << ((n % 8) * 4))) | ((12u32 & 0xF) << ((n % 8) * 4)))
});
```

EXTI-capable pins also route through `SYSCFG.EXTICR` and configure `RTSR`/`FTSR` according to `exti_rising`/`exti_falling`.
