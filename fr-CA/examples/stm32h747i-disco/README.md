```markdown
<!--
examples/stm32h747i-disco/README.md - STM32H747I-DISCO board demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démo STM32H747I-DISCO
---
Démontre rlvgl sur la carte de découverte STM32H747I-DISCO en utilisant des pilotes d'affichage et de tactile de remplacement.

## Liens Rapides
- Options de démarrage et flux double-cœur : voir `BOOT.md`
- Carte mémoire et régions : voir `MEMORY.md`
- Comportement et drapeaux de génération BSP STM32 : voir `docs/STM_BSP_GENERATION.md`

## Génération BSP
Le répertoire `bsp` est produit par `rlvgl-creator` et démontre
le "clock gating" (gestion de l'horloge) sensible au bus. Les GPIO et les
activations de périphériques ciblent automatiquement le `AHB4ENR` du H7
et les registres APB associés.

```rust
use crate::bsp::{hal, pac};

let dp = pac::Peripherals::take().unwrap();
hal::init_board_hal(&dp);
```

## Prérequis
- Cible Rust `thumbv7em-none-eabihf`
- Chaîne d'outils croisée `arm-none-eabi`

## Compilation
```bash
rustup target add thumbv7em-none-eabihf
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,qrcode,png,jpeg,fontdue" \
    --target thumbv7em-none-eabihf
```

Alternativement, utilisez les raccourcis du Makefile de niveau supérieur :

```
make gen-stm32h747i-disco-bsp   # Régénérer le BSP (par défaut SMPS/VOS1)
make build-disco                # Compiler l'exemple CM7
make build-disco-cm4            # Compiler l'exemple CM4
make build-disco-all            # Tout compiler
make openocd                    # Démarrer OpenOCD (stlink + stm32h7x)
make openocd-erase              # Effacement massif (DANGER)
```

Notes :
- Le `build.rs` de l'espace de travail déploie le `memory.x` de cet exemple dans
  le répertoire de construction de Cargo et passe automatiquement `-Tmemory.x`
  à l'éditeur de liens sur les cibles embarquées. Aucun `.cargo/config.toml`
  global n'est requis.
- L'option `backlight_pwm` active le PWM TIM8 sur `PJ6` pour le rétroéclairage
  LCD. La compilation par défaut utilise un simple basculement GPIO haut/bas
  pour le démarrage.

## Flashage
```bash
cargo objcopy --bin rlvgl-stm32h747i-disco \
    --target thumbv7em-none-eabihf --release \
    -- -O binary firmware.bin
st-flash write firmware.bin 0x08000000
```

## Tests Manuels
1. Réinitialisez la carte et confirmez que l'interface utilisateur de la démo correspond à la disposition du simulateur.
2. Touchez les widgets pour vous assurer que les événements tactiles se propagent correctement.

## État de l'Affichage (Démarrage)

- Horloge pixel : 32 MHz (PLL3R) — valeur par défaut conservatrice ; ajuster plus tard.
- Synchronisations LTDC (typique OTM8009A 800×480) :
  - HSW=20, HBP=140, HFP=20
  - VSW=4,  VBP=34,  VFP=10
- Couche 1 : tampon d'image RGB565 ; DMA2D prévu pour les blits/remplissages.
- Notes :
  - Ces valeurs sont étiquetées dans `platform/src/stm32h747i_disco.rs::configure_ltdc_timing()`
    pour faciliter les ajustements lors du réglage.
  - L'initialisation du panneau DSI est ébauchée ; les dessins LTDC sont en cours.

## Tactile (FT5336)

- Bus I²C : I2C4
  - PD12 = I2C4_SCL (AF4, drain ouvert, pull-up)
  - PD13 = I2C4_SDA (AF4, drain ouvert, pull-up)
- Interruption : PK7 = TOUCH_INT
- Propriété : CM4 initialise I2C4 et interroge le FT5336 ; CM7 exécute le travail d'affichage.
- Une initialisation I2C4 basée sur PAC pour CM4 sera ajoutée ; le support FT5336
  utilise un adaptateur embedded-hal 1.0.

## Rétroéclairage et Réinitialisation (Temporaire)

- Repli GPIO du rétroéclairage : PJ6 (haut = allumé). Le démarrage PWM est
  optionnel sur TIM8 (PJ6 supporte TIM8 CH1/CH2 ; acheminé vers LCD_BL_CTRL).
- Réinitialisation du panneau : PG3 (LCD_RESET sur MB1166). Le démarrage initial
  peut basculer cela via GPIO ; ajouter des retards conformes à la fiche
  technique.

## Facultatif : Actifs SD

- Activez l'adaptateur FATFS no_std et le périphérique de bloc SD lors de la
  compilation. Pour une démo de liste minimale au démarrage, activez également
  `sd_assets_demo` :

```bash
cargo build --bin rlvgl-stm32h747i-disco \
    --features "stm32h747i_disco,fatfs_nostd,sd_assets_demo" \
    --target thumbv7em-none-eabihf --release
```

- Le pilote `DiscoSdBlockDevice` (SDMMC1 + DMA + hygiène du cache D) est
  disponible via les fonctionnalités ci-dessus. Un adaptateur `fatfs` léger est
  inclus dans le crate de la plateforme (`sd_fatfs_adapter`). Avec
  `sd_assets_demo`, le firmware tentera de monter et de lister `/assets` au
  démarrage et d'afficher quelques noms.

### Indicateurs à l'écran

- `asset: <name>` : FAT monté et `/assets` contient des entrées ; jusqu'à 4 sont affichées.
- `SD: no assets` : FAT monté mais `/assets` (ou racine) est vide.
- `SD: mount/list failed` : Le montage FAT ou la liste du répertoire a échoué
  (vérifiez les broches/l'horloge/la carte SD).
```
