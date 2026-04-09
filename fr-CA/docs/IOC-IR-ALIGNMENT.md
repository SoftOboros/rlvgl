<!--
Plan pour l'alignement des superpositions de cartes CubeMX `.ioc` avec les modèles d'initialisation IR et Rust canoniques.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Plan d'alignement IR de la carte STM32

## Écart actuel
- L'importateur actuel n'émet que des mappages broche → signal → AF.
- Les modèles nécessitent un contexte par broche : port/index, classe, mode, pull, vitesse, otype, EXTI, etc.

## Plan
0. [ ] **Nettoyage MCU** – `stm32_xml_scraper.py` devrait ignorer ou supprimer les MCU sans définitions de broches afin que les conversions `.ioc` ultérieures n'échouent pas.
1. [ ] **Constructeur de contexte de broche** – Analyser les clés `.ioc` (`Signal`, `Mode`, `GPIO_PuPd`, `GPIO_Speed`, `GPIO_OType`, `GPIO_Label`) et fusionner avec le JSON MCU canonique pour émettre des objets par broche.
2. [ ] **Normalisation des recherches** – Centraliser les mappages traduisant les chaînes Cube en bits MODER/OTYPER/OSPEEDR/PUPDR et en noms d'énumérations HAL. Intégrer les champs de bits dérivés et les chaînes HAL dans chaque contexte de broche. Les broches sans AF stockent `null` afin que les modèles puissent ignorer les écritures AFR.
3. [ ] **Émission de superposition de carte** – Stocker le contexte de broche canonique par carte sous `boards/<board>.json` afin que toutes les cartes partagent le même schéma.
4. [ ] **Règles de modèle HAL** – Générer `into_alternate`, `into_push_pull_output`, etc., en utilisant des aides normalisées de vitesse/pull/otype.
5. [ ] **Règles de modèle PAC** – Émettre des écritures de registre pour MODER/OTYPER/OSPEEDR/PUPDR et AFR ; inclure le routage EXTI lorsque `is_exti` est vrai.
6. [ ] **Dérivation EXTI** – Calculer `exti_port_index`, `exti_rising`, et `exti_falling` à partir des chaînes de mode `.ioc` pour les broches compatibles avec les interruptions.
7. [ ] **Tests** – Tests instantanés pour `.ioc` → contexte canonique plus les tests de fumée des modèles HAL et PAC.

## Contexte de broche canonique
Chaque broche dans une superposition de carte suit un schéma JSON unique construit à partir des clés `.ioc` et des recherches MCU AF :

```json
{
  "name": "PC12",
  "port": "C",
  "index": 12,
  "class": "Peripheral|GPIO|System|Raw",
  "sig_full": "SDMMC1_CK",
  "instance": "SDMMC1",
  "signal": "CK",
  "af": 12,                    // null si pas de fonction alternative
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

## Tables de recherche
Les mappages normalisés traduisent les chaînes Cube en bits de registre et en noms d'énumérations HAL :

- `MODE_TO_MODER` – ex. `GPIO_AF_PP` → `0b10`
- `PULL_TO_BITS` – `GPIO_PULLUP` → `0b01`
- `SPEED_TO_BITS` – `GPIO_SPEED_FREQ_HIGH` → `0b10`
- `OTYPE_TO_BIT` – `GPIO_OType_OD` → `1`
- `HAL_SPEED` – `GPIO_SPEED_FREQ_VERY_HIGH` → `VeryHigh`
- `HAL_PULL` – `GPIO_PULLDOWN` → `PullDown`

## Classes de modèles
Les règles de rendu dépendent du champ `class` :

- **Périphérique** – configurer le mode de fonction alternative et appliquer `otype`, `pull`, et `speed` ; définir l'emplacement AFR à `af`.
- **GPIO** – gérer MODER/PUPDR/OSPEEDR/OTYPER ; lorsque `is_exti` est vrai, router la broche via `SYSCFG.EXTICR` et configurer `RTSR`/`FTSR` en fonction de `exti_rising`/`exti_falling`.
- **Système** – traiter comme `Périphérique` pour des signaux tels que `RCC_MCO` ou des broches de débogage, sauf si explicitement remplacé.

Les modèles HAL utilisent `into_alternate`, `into_push_pull_output`, ou `into_analog` avec des aides comme `map_speed_to_hal`. Les modèles PAC émettent des écritures de registre explicites pour chaque champ et programment conditionnellement les registres AFR et EXTI.

## Modèle de modèle HAL
Activer chaque port GPIO utilisé une fois et configurer les broches en utilisant le contexte canonique :

```rust
let mut rcc = dp.RCC.constrain();
let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);

// USART1_TX sur PA9
let pa9 = gpioa.pa9.into_alternate::<{ pins["PA9"].af }>();
pa9.set_speed(Speed::{ pins["PA9"].hal_speed });
pa9.internal_pull_up({ pins["PA9"].pull == "GPIO_PULLUP" });
pa9.set_open_drain({ pins["PA9"].otype == "GPIO_OType_OD" });

// Entrée analogique sur PA3
let pa3 = gpioa.pa3.into_analog();
```

## Modèle de modèle PAC
Écrire directement les registres en utilisant des champs de bits précalculés :

```rust
// Activer GPIOC
dp.RCC.ahb2enr.modify(|_, w| w.gpiocen().set_bit());

// Configurer PC12 comme fonction alternative avec AF12
let n = 12;
dp.GPIOC.moder.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0b11 << (n * 2))) | (0b10 << (n * 2)))
});
dp.GPIOC.afrh.modify(|r, w| unsafe {
    w.bits((r.bits() & !(0xF << ((n % 8) * 4))) | ((12u32 & 0xF) << ((n % 8) * 4)))
});
```

Les broches compatibles EXTI sont également routées via `SYSCFG.EXTICR` et configurent `RTSR`/`FTSR` selon `exti_rising`/`exti_falling`.
