<!--
  MAKE.md — Developer guide for Makefile convenience targets
  Covers available targets, typical flows, and prerequisites.
-->

# Utilisation du Makefile (Développeur)

Le dépôt inclut un Makefile léger avec des cibles pratiques pour accélérer les flux de travail courants du STM32H747I-DISCO : régénération des BSP, compilation des deux cœurs et gestion d'OpenOCD.

## Prérequis

- Cible Rust : `thumbv7em-none-eabihf`
  - `rustup target add thumbv7em-none-eabihf`
- Chaîne d'outils Arm pour le débogage/flash (par exemple, GNU Tools for Arm Embedded)
- OpenOCD installé et dans le PATH

## Cibles

- `make help`
  - Affiche un résumé des cibles disponibles.

- `make gen-stm32h747i-disco-bsp`
  - Régénère le BSP exemple à partir de `DiscoBiscuit.ioc`.
  - Utilise par défaut `STM32_PWR_SUPPLY=SMPS` et `STM32_PWR_SDLEVEL=VOS1`.
  - Utilise `examples/stm32h747i-disco/gen-bsp.sh` (opérateur idempotent ; régénère uniquement si nécessaire).

- `make build-disco`
  - Compile l'exemple CM7 : `rlvgl-stm32h747i-disco`.

- `make build-disco-cm4`
  - Compile l'exemple CM4 : `rlvgl-stm32h747i-disco-cm4`.

- `make build-disco-all`
  - Compile les deux exemples CM7 et CM4.

- `make openocd`
  - Démarre OpenOCD avec les scripts ST-Link + STM32H7 cibles standard et arrête le CPU.
  - Utilisez ceci avec la configuration VSCode "CM7 attach (external OpenOCD)".

- `make openocd-erase`
  - Effacement de masse via OpenOCD et sortie. À utiliser avec prudence.

## Flux typiques

1) Régénérer le BSP et compiler les deux cœurs

```
make gen-stm32h747i-disco-bsp
make build-disco-all
```

2) Déboguer à l'aide d'un OpenOCD externe (recommandé)

```
make openocd                       # terminal 1
# VSCode: lancer "CM7 attach (external OpenOCD)"   # terminal 2/VSCode
```

3) Mettre à jour les paramètres par défaut de l'alimentation du BSP (surcharger l'environnement)

```
STM32_PWR_SUPPLY=LDO STM32_PWR_SDLEVEL=VOS2 make gen-stm32h747i-disco-bsp
```

## Notes

- Le fichier `build.rs` de niveau supérieur met automatiquement en scène le script de liaison approprié pour chaque binaire exemple (CM7 utilise `memory.x`, CM4 utilise `memory_cm4.x`).
- L'espace de travail VSCode fournit deux profils de lancement ; consultez `examples/stm32h747i-disco/BOOT.md` pour les options de démarrage dual-core (A/B/C) et le handshake basé sur la boîte aux lettres.
