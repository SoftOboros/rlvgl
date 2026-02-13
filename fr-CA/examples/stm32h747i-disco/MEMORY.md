```markdown
<!--
  MEMORY.md — Disposition de la mémoire exemple STM32H747I‑DISCO
  Décrit comment le CM7/CM4 utilise DTCM, D1/D2/D3 SRAM, et la boîte aux lettres partagée.
-->

# Disposition de la mémoire (Exemple STM32H747I‑DISCO)

Ce document explique comment l'exemple dual‑core partitionne la mémoire entre le CM7 et le CM4, comment la boîte aux lettres partagée fonctionne, et comment les scripts de liaison (linker scripts) et `build.rs` sélectionnent les bonnes régions.

## Vue d'ensemble

- Le CM7 utilise par défaut le DTCM pour sa pile/ses données et peut placer de grands tampons dans le D1 AXI SRAM.
- Le CM4 possède le D2 SRAM (sauf une boîte aux lettres réservée) et tout le D3 SRAM4 pour la rétention/utilisation à faible consommation.
- Une boîte aux lettres de 1 Ko dans le D2 SRAM3 est partagée par les deux cœurs pour les sémaphores et les données de transfert.

## Régions

- DTCM (local au CM7)
  - Base: `0x2000_0000`, Taille: 128 Ko
  - Utilisé par le CM7 comme `RAM` par défaut dans `memory.x`.

- D1 AXI SRAM (domaine partagé, divisé 3/4:1/4)
  - Total: 512 Ko à `0x2400_0000`.
  - Tranche CM7 `D1_CM7`: `0x2400_0000`..`0x2405_FFFF` (384 Ko).
  - Tranche CM4 `D1_CM4`: `0x2406_0000`..`0x2407_FFFF` (128 Ko).
  - Actuellement déclaré mais pas encore utilisé par des sections; réservé aux tampons d'affichage (framebuffers), pools partagés, etc.

- D2 SRAM (domaine CM4, boîte aux lettres réservée)
- `RAM` CM4: `0x3000_0000`..`0x3003_FFFF` (comptée comme 255 Ko de RAM générale + 1 Ko de boîte aux lettres = 256 Ko au total).
  La boîte aux lettres de 1 Ko se trouve à `0x3004_7000` et doit être accessible séparément de la fenêtre RAM générale.
  - Boîte aux lettres (partagée): `0x3004_7000`..`0x3004_73FF` (1 Ko) dans les deux scripts de liaison.

- D3 SRAM4 (rétention CM4)
  - `D3_CM4`: `0x3800_0000`..`0x3800_FFFF` (64 Ko) déclaré pour la rétention/faible consommation.

## Tableau des régions

| Région     | Propriétaire | Domaine | Base         | Taille   | Objectif/Notes                                |
|------------|--------------|---------|--------------|----------|-----------------------------------------------|
| RAM        | CM7          | DTCM    | `0x2000_0000`| 128 Ko   | Pile/données par défaut du CM7 (TCM rapide, non partagé) |
| D1_CM7     | CM7          | D1 AXI  | `0x2400_0000`| 384 Ko   | Tranche AXI SRAM du CM7 (grands tampons, FB, etc.) |
| D1_CM4     | CM4          | D1 AXI  | `0x2406_0000`| 128 Ko   | Tranche AXI SRAM du CM4                       |
| RAM (CM4)  | CM4          | D2      | `0x3000_0000`| 255 Ko   | Pile/données par défaut du CM4 (RAM générale) |
| MAILBOX    | Partagé      | D2      | `0x3004_7000`| 1 Ko     | Boîte aux lettres inter-cœur; sémaphore à `+0x000` |
| D3_CM4     | CM4          | D3      | `0x3800_0000`| 64 Ko    | Région de rétention/faible consommation       |

## Boîte aux lettres (partagée)

- Plage d'adresses: `0x3004_7000`..`0x3004_73FF` (1 Ko).
- Mot de sémaphore: `0x3004_7000` (décalage 0), accessible comme `AtomicU32`.
- Utilisation:
  - Le CM7 (propriétaire de l'horloge principale) appelle `signal_clocks_ready()` après l'initialisation de l'alimentation/RCC.
  - Le CM4 appelle `wait_for_clocks()` pour bloquer jusqu'à ce que le sémaphore soit activé, puis continue.
- Octets restants: réservés pour un futur protocole de transfert (par exemple, configuration, raisons de démarrage).

## Binaires et scripts de liaison

- Binaire CM7: `rlvgl-stm32h747i-disco`
  - Script de liaison: `examples/stm32h747i-disco/memory.x` (DTCM `RAM` par défaut, boîte aux lettres déclarée).
  - Les importations PAC/HAL ciblent `stm32h747cm7`.

- Binaire CM4: `rlvgl-stm32h747i-disco-cm4`
  - Script de liaison: `examples/stm32h747i-disco/memory_cm4.x` (D2 `RAM`, boîte aux lettres déclarée).
  - Les importations PAC/HAL ciblent `stm32h747cm4`.

- Sélection de la construction:
  - Le `build.rs` de niveau supérieur copie le bon `memory*.x` dans `OUT_DIR/memory.x` en fonction de `CARGO_BIN_NAME`.
  - `cargo:rustc-link-arg=-Tlink.x` est émis pour que cortex‑m‑rt inclue le script mis en scène.

## Alimentation et horloges

- L'initialisation BSP (PAC/HAL) configure le PWR sur le H7 avec le gating SCUEN, l'alimentation (SMPS/LDO), le SDLEVEL, et écrit `D3CR.VOS`.
- Le VOS et l'alimentation proviennent du `.ioc` de CubeMX ou sont remplacés via les variables d'environnement:
  - `STM32_PWR_SUPPLY=SMPS|LDO`
  - `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3`
- Les horloges ne sont pas encore émises depuis le `.ioc`; le système démarre avec les paramètres par défaut de réinitialisation (~64 MHz) à moins que votre application ne configure le RCC.

## Travaux futurs

- Définir les sections `.axisram_cm7`, `.axisram_cm4`, et `.retained_d3` et les mapper aux `D1_CM7`, `D1_CM4`, et `D3_CM4`.
- Formaliser le protocole de la boîte aux lettres (structure, versionnage, champs de commande/acquittement).
- Émettre la configuration RCC/PLL pour le CM7 à partir du `.ioc` et bloquer le CM4 jusqu'à `signal_clocks_ready()`.
```
