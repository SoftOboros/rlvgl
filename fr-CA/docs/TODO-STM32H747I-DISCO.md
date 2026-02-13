```markdown
<!--
TODO-STM32H747I-DISCO.md - Liste de contrôle et plan de travail pour le matériel réel.
-->

# Tâches à effectuer pour le démarrage du matériel STM32H747I-DISCO

Ce document répertorie le travail restant à effectuer pour exécuter la démo `rlvgl`
sur le matériel réel STM32H747I-DISCO (noyau M7). Les éléments sont regroupés par sous-système et
ordonnés approximativement des prérequis de démarrage aux fonctionnalités de plus haut niveau.

## Démarrage, Liaison et Horloges

- Script de construction pour le script de liaison :
  - Statut : fait. Le `build.rs` de l'espace de travail copie maintenant
    `examples/stm32h747i-disco/memory.x` dans le `OUT_DIR` de la construction, émet
    `cargo:rustc-link-search`, et `cargo:rustc-link-arg=-Tmemory.x` pour la
    cible embarquée. Cela suit les directives du projet "Exemples de scripts de liaison"
    tout en évitant toute hypothèse globale de `.cargo/config.toml`.
    Si l'exemple est un jour divisé en son propre crate, reproduire cette logique minimale
    dans un `build.rs` local.
- Horloges système et PLL :
  - Analyser les paramètres du PLL à partir du `.ioc` (déjà pris en charge dans l'IR) et générer
    une configuration d'horloge minimale suffisante pour l'horloge pixel LTDC et les noyaux I2C/SDMMC.
  - Programmer les PLL et les multiplexeurs de noyau via PAC/HAL lors de l'initialisation de la carte.

## SDRAM Externe (FMC)

- Implémenter l'initialisation du contrôleur SDRAM (chronométrage, registres de mode, rafraîchissement) :
  - Configurer les broches FMC et le chronométrage pour la SDRAM embarquée.
  - Exécuter la séquence d'initialisation JEDEC SDRAM et définir le taux de rafraîchissement.
  - Vérifier que la base du framebuffer à `0xC000_0000` est inscriptible et stable.

## Affichage (LTDC + DSI + Panneau)

- Chronométrage LTDC et configuration des couches :
  - Programmer les largeurs de synchronisation, les marges arrière/avant et la polarité pour le panneau 800×480.
  - Configurer le pas de la couche 1, le format de pixel (RGB565), le mélange et activer le rechargement.
- Démarrage de la liaison MIPI-DSI :
  - Développer `platform::otm8009a` pour inclure la séquence complète d'initialisation du panneau (format,
    alimentation, gamma) plutôt que la veille/affichage minimal actuel.
  - Configurer les paramètres du mode vidéo de l'hôte DSI et démarrer la liaison.
- Chemin de vidage :
  - Implémenter `DisplayDriver::flush` pour transférer les modifications dans la SDRAM et/ou déclencher
    un rechargement LTDC. Envisager l'accélération DMA2D si disponible (fonctionnalité optionnelle).

## Rétroéclairage et Réinitialisation du Panneau

- PWM du rétroéclairage :
  - Progression : un rétroéclairage de secours HAL‑GPIO existe dans l'exemple et une
    fonctionnalité `backlight_pwm` gère un chemin PWM HAL TIM8 avec un adaptateur `SetDutyCycle`
    embedded‑hal 1.0. Une rampe douce de luminosité au démarrage est implémentée dans
    le démarrage de l'affichage. Suivant : envisager de faire du PWM le défaut.
- GPIO de réinitialisation du panneau :
  - Progression : `PG3` (LCD_RESET) est piloté via HAL GPIO dans l'exemple avec un
    délai de base entre bas/haut, avant l'initialisation DSI. Suivant : remplacer
    le délai de cycle grossier par un délai basé sur un temporisateur qui correspond
    au chronométrage de la fiche technique.

## Tactile (FT5336)

- Câblage I2C4 réel :
  - Confirmer que le `.ioc` a I2C4 SCL/SDA sur `PD12/PD13` (AF4, drain ouvert, pull-ups).
  - Statut : fait. L'aide à l'initialisation HAL existe
    (`platform::stm32h747i_disco::init_touch_i2c`) et mappe PD12/PD13 à AF4
    drain ouvert avec une vitesse de bus de 400 kHz.
  - Supprimer le shim de compatibilité I2C temporaire 0.2→1.0 une fois que la plateforme/HAL
    convergera sur embedded‑hal 1.0 pour I2C.
- Ligne d'interruption (facultatif) :
  - Câbler FT5336 INT sur `PK7` comme entrée et utiliser le chemin `new_with_int` pour réduire
    le sondage.

## Carte SD (facultatif)

- Valider `DiscoSdBlockDevice` par rapport au média réel :
  - Progression : `platform::DiscoSdBlockDevice` est implémenté en utilisant HAL SDMMC1
    avec une maintenance D‑Cache explicite et une taille de bloc de 512 octets. Suivant : valider
    sur le matériel et intégrer `fatfs` derrière la fonctionnalité `fs` dans l'exemple.
  - Liste de contrôle :
    - Configurer GPIO : `PC8..PC12` → AF12, `PD2` → AF12 ; très haute vitesse, pull‑ups.
    - Horloge : Activer l'horloge de noyau `SDMMC1` (PLL2 `Q` recommandé), activer DMA.
    - Initialisation HAL : construire `stm32h7xx_hal::sdmmc::Sdmmc` avec des flux DMA RX/TX.
    - Envelopper comme `DiscoSdBlockDevice` et monter via `fatfs` (adaptateur) pour lister `/assets`.
  - Suivi : ajouter une petite démo sur l'appareil qui monte, liste `/assets` et rend
    une ligne de texte ou une image comme test de fumée.

### Esquisse de démarrage SDMMC1 (HAL)

```rust
// GPIO & clocks (abbrev.)
let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
let gpiod = dp.pd2.split(ccdr.peripheral.PD2);
let _d0 = gpioc.pc8.into_alternate::<12>();
let _d1 = gpioc.pc9.into_alternate::<12>();
let _d2 = gpioc.pc10.into_alternate::<12>();
let _d3 = gpioc.pc11.into_alternate::<12>();
let _ck = gpioc.pc12.into_alternate::<12>();
let _cmd = gpiod.pd2.into_alternate::<12>();

// DMA + SDMMC1
let mut sd = stm32h7xx_hal::sdmmc::Sdmmc::new(
    dp.SDMMC1,
    (/* d0..d3, ck, cmd pins */),
    ccdr.peripheral.SDMMC1,
    &ccdr.clocks,
);
sd.init_card(/* 4-bit, freq */).unwrap();

// Block device and FAT mount (adapter layer required)
let mut dev = rlvgl::platform::DiscoSdBlockDevice::new(sd);
  // TODO: monter avec adaptateur fatfs et lister /assets
```

### Dépannage SD

- Horloge : assurez-vous que l'horloge de noyau SDMMC1 est alimentée par le PLL2 (par exemple, PLL2Q) à un
  débit raisonnable. Si elle est trop faible, la carte peut expirer ; si elle est trop élevée, l'initialisation échoue.
- GPIO AF & pulls : PC8..PC12 et PD2 doivent être AF12, très haute vitesse ; activer les pull‑ups
  si nécessaire (47 kΩ externes généralement présents sur les cartes).
- Effets D‑Cache : les données obsolètes ou les erreurs CRC signifient souvent un manque de maintenance du cache.
  Le `DiscoSdBlockDevice` nettoie/invalide déjà ; évitez les tampons supplémentaires que DMA
  ne peut pas voir.
- Largeur du bus : commencer en 1‑bit, puis passer à 4‑bit après que la carte signale son support.
- Format de la carte : utiliser MBR + FAT32. Éviter exFAT. Assurez-vous que les secteurs logiques sont de 512 octets.
- Alimentation/câblage : vérifier le rail 3.3 V et l'insertion de la microSD. Réinsérer la carte.
- Pilote de noyau occupé : après des erreurs, redémarrer complètement la carte pour récupérer
  la machine d'état de la carte.

## Suivis du Générateur BSP

- Entrées de régénération :
  - S'assurer que rlvgl-creator utilise toujours la base de données canonique STM32
    (`rlvgl-chips-stm`) pour la résolution AF. Aucune utilisation de `stm32_af.json` ne demeure.
- Sortie HAL/PAC :
  - Après avoir intégré les ressources de la DB canonique (`RLVGL_CHIP_SRC`), régénérer le
    BSP H747I-DISCO et vérifier les AF (I2C4 sur `PD12/PD13` → AF4, etc.).

## Tests et CI

- Vérifications côté hôte :
  - Maintenir un état propre `cargo fmt` / `clippy` avec toutes les combinaisons de fonctionnalités.
- Constructions croisées :
  - Ajouter une tâche CI pour construire `rlvgl-stm32h747i-disco` pour
    `thumbv7em-none-eabihf` en utilisant le script de liaison géré par `build.rs` de l'exemple.
- Tests de fumée sur cible (manuel/matériel) :
  - Vérifier le rétroéclairage, la couleur de l'écran clair et les événements tactiles via UART.
  - Capturer une courte exécution de démonstration et comparer les séquences d'événements attendues.

## Terminé / Récemment Mis en Œuvre

- Le créateur résout maintenant les fonctions alternatives à partir de la base de données STM32 canonique ;
  `--af` et `stm32_af.json` sont supprimés de CLI/docs/scripts.
- L'exemple gagne un chemin pour initialiser I2C4 via HAL et faire le pont vers
  embedded‑hal 1.0 pour le pilote tactile (couche de compatibilité temporaire).
- Gestion du script de liaison : le `build.rs` de l'espace de travail prépare le
  `memory.x` de l'exemple dans `OUT_DIR` et passe `-Tmemory.x` à l'éditeur de liens pour les cibles
  embarquées.
- Câblage d'exemple pour la réinitialisation du panneau sur `PG3` implémenté ; le contrôle du rétroéclairage
  fonctionne via un secours HAL‑GPIO, avec un chemin PWM TIM8 sous le contrôle de `backlight_pwm`.
- Échafaudage du périphérique de bloc SD implémenté pour SDMMC1 avec DMA et hygiène du cache.

## Polissage HAL/BSP restant et prochaines étapes

- Modèle HAL (H7) :
  - Garder `.set_speed(Speed::VeryHigh)` chaîné à `.into_alternate::<AF>()` sur une seule instruction (éviter les lignes commençant par `.` ).
  - Ne pas émettre d'importations par port (`gpioa::*`, etc.) ; seulement `use stm32h7xx_hal::{gpio::Speed, pac, prelude::*};` et `use stm32h7xx_hal::rcc;`.
  - Assurer la signature `configure_pins_hal(dp, ccdr)` pour H7 et utiliser `dp.GPIOx.split(ccdr.peripheral.GPIOX)`.
- Régénération BSP : exécuter `scripts/gen-example-bsp.sh` et vérifier que le fichier régénéré `examples/stm32h747i-disco/bsp/hal.rs` compile et passe `cargo fmt --check`.
- Pin‑mux d'exemple : passer au mux HAL (`bsp_hal::configure_pins_hal(&dp, &ccdr)`), en abandonnant le secours PAC temporaire une fois que le fichier régénéré compile proprement.
- Résolution AF : confirmer PD12/PD13 → I2C4 AF4 (DB canonique) ; supprimer le secours une fois que la base de données fournit définitivement les AF pour H747.
- Rétroéclairage + réinitialisation :
  - Remplacer le rétroéclairage GPIO temporaire par TIM8 (PJ6) HAL PWM ; ajouter un petit adaptateur `SetDutyCycle` embedded‑hal 1.0 sur le canal HAL PWM.
  - Garder la réinitialisation du panneau sur PG3 avec des délais conformes ; passer au GPIO HAL après que le mux compile.
- CI/formatage : réexécuter `cargo fmt --all -- --check` et corriger les nids de blancs ou de retour à la ligne résiduels des modèles afin que les fichiers générés restent propres avec rustfmt.
```
