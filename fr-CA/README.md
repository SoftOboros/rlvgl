```markdown
<!--
README.md - Top-level overview and navigation for rlvgl.
-->
<p align="centre">
  <img src="./rlvgl-logo.png" alt="rlvgl" />
</p>

<span style="font-size:26px"><b>rlvgl</b></span> est une réimplémentation modulaire et idiomatique en Rust de LVGL (Light and Versatile Graphics Library).

rlvgl préserve le paradigme d'interface utilisateur basé sur les widgets de LVGL tout en éliminant la gestion de la mémoire de style C non sécurisée et l'état global. Cette bibliothèque est structurée pour prendre en charge les environnements `no_std`, les cibles embarquées (par exemple, STM32H7) et les backends de simulateur pour un prototypage rapide.

La version C de LVGL est incluse en tant que sous-module git pour référence et extraction de vecteurs de test, mais elle n'est ni liée ni compilée dans cette bibliothèque.

## Objectifs
Paquet: `rlvgl`
- Conserver l'architecture et le système de mise en page de LVGL
- Remplacer la gestion de la mémoire C par une propriété Rust idiomatique
- Prend en charge l'affichage et la saisie embarqués via `embedded-hal`
- Activer la hiérarchie des widgets, les styles et les événements à l'aide des traits Rust
- Utiliser les crates Rust existantes si possible (par exemple, `embedded-graphics`, `heapless`, `tinybmp`)

## Fonctionnalités
- Support `no_std` + alloueur
- Disposition modulaire basée sur les composants (noyau, widgets, plateforme)
- Simulable via un indicateur de fonctionnalité activé par `std`
- Backends d'affichage et de saisie enfichables
- Prise en charge optionnelle de Lottie via le crate `rlottie` pour la lecture dynamique.
  Les cibles embarquées doivent pré-rendre les animations en APNG pour une taille minimale.

## Structure du projet
- [core](./core/README.md) – Trait de base du widget, mise en page, distribution d'événements
- [widgets](./widgets/README.md) – Réimplémentations natifs en Rust des widgets LVGL
- [platform](./platform/README.md) – Traits d'affichage/saisie et adaptateurs HAL
- [ui](./ui/README.md) – Composants d'interface utilisateur de niveau supérieur
- [examples](./examples/README.md) – Exemples d'applications et de démos de cartes
- [docs](./docs/README.md) – Documentation du projet et listes de tâches
- [lvgl](./lvgl/README.md) – Sous-module C (référence uniquement)
- [chips/stm/bsps](./chips/stm/bsps/README.md) 🆕 – Stubs BSP STM32 générés

## Bases de données de puces du fournisseur

Les définitions de cartes spécifiques au fournisseur se trouvent dans les crates [`chipdb/`](./chipdb/README.md). L'assistant
`tools/gen_pins.py` agrège les entrées brutes du fournisseur en blocs JSON, tandis que
`tools/build_vendor.sh` orchestre la génération et estampille les fichiers de licence. Lors de la construction d'un crate de fournisseur,
définissez `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers JSON afin que le script de construction puisse les intégrer
via `include_bytes!`.

## Génération BSP STM32CubeMX 🆕

`rlvgl-creator` 🆕 convertit les projets STM32 CubeMX `.ioc` en stubs de support de carte. Les modules générés sont expédiés dans
[`rlvgl-bsps-stm` 🆕](./chips/stm/bsps/README.md). L'ancien support de superposition `board` demeure, mais est déprécié.

## Générateur BSP (`rlvgl-creator` 🆕)

`rlvgl-creator` 🆕 offre un pipeline en deux étapes pour les packages de support de carte:

1. **Importation** des fichiers de projet du fournisseur (par exemple, STM32CubeMX `.ioc`, NXP `.mex`,
   RP2040 YAML). Chaque adaptateur extrait les données du fournisseur et émet un petit **IR**
   YAML neutre vis-à-vis du fournisseur décrivant les horloges, les broches, le DMA et les périphériques.
2. **Générer** le code d'initialisation Rust en rendant les modèles MiniJinja
   par rapport à l'IR. Les utilisateurs peuvent choisir parmi des packs de modèles intégrés ou fournir
   les leurs.

L'adaptateur STM32CubeMX analyse également les multiplicateurs de PLL et les sélections d'horloge de noyau périphérique
afin que la configuration de l'horloge puisse être générée en même temps que la configuration des broches.

Aucune table par puce n'est maintenue. Les règles de niveau de classe sont réutilisées
entre les instances et les fournisseurs. Les fonctions alternatives sont dérivées des bases de données
de fournisseurs intégrées générées à partir des sources XML officielles ; aucun JSON externe n'est requis au moment de la génération.
Les broches SWD réservées (`PA13`, `PA14`) sont rejetées à moins d'être explicitement autorisées.

Flux typique:

```bash
rlvgl-creator platform import --vendor st --input board.ioc --out board.yaml
rlvgl-creator platform gen --spec board.yaml --templates templates/stm32h7 \
  --out src/generated.rs
```

Les numéros de fonction alternative sont calculés à partir de la base de données intégrée au moment de l'exécution
par `rlvgl-creator`, il n'est donc pas nécessaire de générer ou de passer un fichier JSON.

Pour empaqueter les bases de données de puces du fournisseur pour les tests ou la publication, exécutez:

```bash
tools/build_vendor.sh
RLVGL_CHIP_SRC=chipdb/rlvgl-chips-stm/generated cargo build -p rlvgl-chips-stm
```

Pour un aperçu complet du flux de travail des actifs, voir le [README de rlvgl-creator 🆕](./README-CREATOR.md).
Les détails des commandes se trouvent dans [docs/CREATOR-CLI.md](./docs/CREATOR-CLI.md).

### Schéma IR

L'étape d'importation émet une spécification YAML concise décrivant la carte:

```yaml
mcu: STM32H747XIHx
package: LQFP176
power: { supply: smps, vos: scale1 }
clocks:
  sources: { hse_hz: 25000000 }
  pll:
    pll1: { m: 5, n: 400, p: 2, q: 4, r: 2 }
  kernels: { usart1: pclk2 }
pinctrl:
  - group: usart1-default
    signals:
      - { pin: PA9,  func: USART1_TX, af: 7, pull: none, speed: veryhigh }
      - { pin: PA10, func: USART1_RX, af: 7, pull: up,   speed: veryhigh }
peripherals:
  usart1:
    class: serial
    params: { baud: 115200, parity: none, stop_bits: 1 }
    pinctrl: [ usart1-default ]
reserved_pins: [ PA13, PA14 ]
```

Résumé des champs:

- `mcu`, `package` – identifiants du projet du fournisseur.
- `power` – configuration d'alimentation ; les valeurs correspondent directement aux appels HAL.
- `clocks` – fréquences d'entrée (`sources`), multiplicateurs PLL (`pll`), et
  sélections de noyau par périphérique (`kernels`).
- `pinctrl` – groupes de broches avec leurs fonctions, fonctions alternatives,
  pulls, et vitesses.
- `peripherals` – carte des instances de périphériques par nom (`usart1`),
  chacun avec une `class` (par exemple `serial`) et des `params` facultatifs.
- `dma`, `interrupts` – tableaux facultatifs décrivant les requêtes DMA et les priorités IRQ.
- `reserved_pins` – broches qui ne doivent pas être reconfigurées (par exemple SWD).

### Aides aux modèles

Les modèles MiniJinja peuvent utiliser les filtres suivants:

- `pin_var` – convertir une broche comme `PA9` en nom de variable `pa9`.
- `periph_num` – extraire les chiffres de fin d'un nom de périphérique
  (`usart12` → `12`).
- `af_alt` – afficher un numéro de fonction alternative pour
  `into_alternate::<AF>()` (`7` → `<7>`).

Les utilisateurs peuvent fournir des modèles personnalisés en pointant `--templates` vers n'importe quel
répertoire; les filtres ci-dessus sont toujours disponibles.

Voir `docs/TODO-CREATOR-BSP.md` pour le travail restant.

## État

Tel que construit. Voir [docs](./docs/README.md) pour la progression composant par composant et les tâches en suspens.

À partir de la version 0.1.0, de nombreuses fonctionnalités sont implémentées et une couverture des tests unitaires de 87% est atteinte, mais les tests fonctionnels et les tests sur matériel nu n'ont pas encore eu lieu.

## Exemple rapide

```rust
use rlvgl_core::widget::Rect;
use rlvgl_widgets::label::Label;

fn main() {
    let mut label = Label::new(
        "hello",
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 20,
        },
    );
    label.style.bg_color = rlvgl_core::widget::Color(0, 0, 255, 255);
    // Rendering would use a DisplayDriver implementation.
}
```

## Tests

Exécutez les tests basés sur l'hôte avec la chaîne d'outils par défaut:

```bash
cargo test --workspace
```

Les tests croisés (par exemple, `thumbv7em-none-eabihf`) nécessitent un lieur. Cargo
utilise par défaut `arm-none-eabi-gcc`, mais vous pouvez éviter d'installer GCC en ajoutant
le composant `rust-lld` et en configurant:

```bash
rustup component add rust-lld
```

```toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

Voir [docs/CROSS-TESTING.md](docs/CROSS-TESTING.md) pour des conseils de dépannage.

## Couverture

L'instrumentation de la couverture LLVM est configurée via `.cargo/config.toml` et la cible `coverage` dans le `Makefile`. Exécutez `make coverage` pour exécuter les tests avec instrumentation et générer un rapport HTML sous `./coverage/`.

## [rlvgl crate](https://crates.io/crates/rlvgl)
- Le lien ci-dessus est pour le crate principal, qui regroupe les autres et inclut le simulateur.
- [rlvgl-core crate](https://crates.io/crates/rlvgl-core)
- [rlvgl-widgets crate](https://crates.io/crates/rlvgl-widgets)
- [rlvgl-platform crate](https://crates.io/crates/rlvgl-platform)

Exécutez la commande Cargo suivante dans le répertoire de votre projet:
```bash
cargo add rlvgl
```
Ou ajoutez la ligne suivante à votre Cargo.toml:
```toml
rlvgl = "0.1.5"
```

## Communauté
- [Code de conduite](./CODE_OF_CONDUCT.md)
- [Notes du contributeur](./AGENTS.md)

## Docker Hub
L'image de build utilisée par le workflow GitHub pour ce dépôt est publiquement disponible sur [Docker Hub](https://hub.docker.com/r/iraa/rlvgl).
```bash
docker pull iraa/rlvgl:latest
```

Consultez le [Dockerfile](https://github.com/SoftOboros/rlvgl/blob/main/Dockerfile) pour plus de détails sur l'environnement de build.

D'autres scripts d'aide utiles peuvent être trouvés dans [`/scripts`](https://github.com/SoftOboros/rlvgl/blob/main/scripts).

## Licence
rlvgl est sous licence MIT. Voir [LICENSE](./LICENSE) pour plus de détails.
Les avis de licence de tiers sont résumés dans [NOTICES.md](./NOTICES.md).
```
