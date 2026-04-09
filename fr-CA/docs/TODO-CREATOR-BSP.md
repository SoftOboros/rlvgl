```markdown
<!--
TODO-CREATOR-BSP.md - Task list for the BSP generator in rlvgl-creator.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# À faire - Créateur BSP

Ce fichier recense les tâches restantes pour le générateur de paquet de
support de carte (BSP) de `rlvgl-creator`. Le générateur fonctionne en deux
étapes :

1. **Importer** les fichiers de configuration du fournisseur (`.ioc`, `.mex`, etc.) dans une petite
   IR YAML indépendante du fournisseur décrivant les horloges, les groupes de broches, le DMA, les interruptions et
   les paramètres des périphériques.
2. **Générer** le code BSP Rust en rendant des modèles MiniJinja à partir de l'IR.

## Tâches

- [x] Implémenter un script Python dans `tools/afdb/` pour construire une base de données JSON
      de fonctions alternatives STM32.
- [x] Développer l'adaptateur CubeMX `.ioc` de STM32 pour couvrir la configuration du PLL et de l'horloge
      du noyau.
- [x] Ajouter des modèles de niveau classe pour l'instanciation USART, SPI et I2C en utilisant
      les numéros d'instance dérivés des noms de périphériques.
- [x] Refuser la configuration des broches réservées (SWD : `PA13`, `PA14`) sauf si
      une dérogation explicite est fournie.
- [ ] Fournir des adaptateurs pour des fournisseurs supplémentaires :
  - [x] Espressif
  - [x] Microchip
  - [x] Nordic
  - [x] NXP
  - [x] Renesas
  - [x] RP2040
  - [x] Silicon Labs
  - [x] TI
- [x] Documenter les aides de modèle et le schéma IR afin que les utilisateurs puissent fournir des modèles
      personnalisés.
- [x] Ajouter des tests unitaires qui capturent l'IR et la sortie générée pour des exemples de
      projets de fournisseurs.
- [x] Diviser le code généré en fonctions d'aide `enable_gpio_clocks`, `configure_pins` et
      `enable_peripherals`.
- [x] Fusionner les écritures RCC par registre pour émettre un seul appel de modification OR'd par
      bus.
- [x] Configurer les broches I2C en tant que drain ouvert avec pull-ups dans les modèles PAC.
- [x] Émettre des réglages de très haute vitesse pour les broches ULPI, SDMMC et SPI.
- [x] Limiter les blocs `unsafe` aux lignes `w.bits(...)` dans le code généré.
- [x] Sélectionner les noms de bus RCC par famille de MCU lors de l'activation des horloges.
- [x] Préfixer les en-têtes SPDX et de provenance à tous les fichiers générés.
- [x] Fournir des crochets de désinitialisation optionnels qui bloquent les horloges et libèrent les broches.
- [x] Autoriser les commutateurs de générateur tels que `--grouped-writes`, `--emit-hal`,
      `--emit-pac`, `--one-file`, `--per-peripheral` et `--with-deinit`.
- [x] Ajouter des attributs d'hygiène de compilation (`#![allow(non_snake_case)]` et
      `#[allow(clippy::too_many_arguments)]`) au code de liaison généré.
- [x] Protéger les modules par périphérique avec les fonctionnalités Cargo.
- [x] Désactiver les horloges DMA et les interruptions pendant la désinitialisation.
- [x] Réinitialiser les registres de configuration DMA et effacer les drapeaux d'interruption pour les flux
      et les canaux.
- [x] Émettre des déclarations `mod` parentes à fonctionnalité limitée pour les dispositions
      par périphérique.
- [x] Intégrer les contrôleurs BDMA et MDMA au nettoyage DMA.
- [x] Affiner la protection des horloges périphériques pour les sous-familles STM32 restantes.
- [x] Démontrer la génération de BSP sensible aux bus dans des exemples de cartes supplémentaires.
- [x] Élargir la couverture BDMA/MDMA pour d'autres variantes STM32 (F0, F1, F2, F3, U5, WB, WL).
- [x] Étendre les démos sensibles aux bus à davantage de cartes de découverte et d'évaluation, y compris H573I-DISCO et U599I-EVAL.
- [x] Polir la documentation du générateur avec des exemples de configuration avancés et des visites guidées.
- [x] Mapper les registres RCC spécifiques aux périphériques pour les familles STM32 restantes.
- [x] Couvrir les réinitialisations de registres DMA supplémentaires et les cas limites.
- [x] Documenter les cas limites restants et les pièges dans la référence CLI.

## Notes

- Aucune table par puce ne doit être maintenue; toutes les données d'instance sont dérivées
  programmatiquement des métadonnées du fournisseur.
- Garder l'IR petite et aligner les classes avec les traits `embedded-hal` pour rester
  indépendant du fournisseur.
```
