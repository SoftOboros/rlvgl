<!--
CREATOR-CLI.md - Référence en ligne de commande et flux de travail pour rlvgl-creator.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# CLI rlvgl-creator

## Aperçu
`rlvgl-creator` est un utilitaire en ligne de commande qui prépare les ressources et les progiciels de support de carte (BSP) pour les projets `rlvgl`. Il convertit les fichiers bruts en formats adaptés aux cibles embarquées, gère les manifestes qui suivent les métadonnées des ressources et peut traduire les fichiers de configuration du fournisseur en code Rust via le rendu de modèles.

Un flux de travail typique initialise un pack de ressources, importe des ressources, les convertit pour une cible et échafaude une caisse qui expose les ressources au moment de la construction. Les flux de travail matériels analysent les fichiers du fournisseur tels que les descriptions `.ioc` de STM32CubeMX et rendent le code BSP à l'aide de modèles MiniJinja.

## Flux de travail de démarrage rapide
```bash
rlvgl-creator init
rlvgl-creator add-target host vendor
rlvgl-creator scan assets/
rlvgl-creator convert assets/
rlvgl-creator preview assets/
rlvgl-creator scaffold assets-pack
```
Cette séquence crée un nouveau pack de ressources, enregistre une cible `host` dont les ressources converties sont écrites sous `vendor/`, analyse les répertoires de ressources brutes, convertit les ressources en formes normalisées, génère des vignettes pour un examen rapide, et enfin échafaude une caisse à double mode nommée `assets-pack` pour l'intégration ou la vente de ressources.

## Référence des commandes
### init
Initialise les répertoires de ressources (`icons/`, `fonts/`, `media/`) et écrit un `manifest.yml` vide.

```
rlvgl-creator init
```

### scan
Analyse une arborescence de répertoires pour les ressources, calcule les hachages et met à jour le manifeste.

```
rlvgl-creator scan <path>
```
* `path` – répertoire racine contenant les ressources brutes.

### check
Valide les entrées du manifeste par rapport aux fichiers de ressources.

```
rlvgl-creator check <path> [--fix]
```
* `path` – répertoire racine contenant les ressources.
* `--fix` – écrit des corrections dans le manifeste lorsque des divergences sont trouvées.

### vendor
Copie les ressources traitées dans un répertoire de sortie et émet un module d'assistance `rlvgl_assets.rs`.

```
rlvgl-creator vendor <path> <out> [--allow LICENSE] [--deny LICENSE]
```
* `path` – répertoire racine contenant les ressources.
* `out` – répertoire où les ressources vendues sont écrites.
* `--allow` – liste blanche des licences autorisées.
* `--deny` – liste noire des licences non autorisées.

### convert
Normalise les ressources (polices, images, médias) et actualise les métadonnées du manifeste.

```
rlvgl-creator convert <path> [--force]
```
* `path` – répertoire racine contenant les ressources.
* `--force` – reconstruit toutes les ressources même si des sorties mises en cache existent.

### preview
Génère des vignettes sous `thumbs/` pour une inspection visuelle rapide.

```
rlvgl-creator preview <path>
```
* `path` – répertoire racine contenant les ressources.

### add-target
Enregistre une cible nommée et le répertoire où ses ressources vendues seront placées.

```
rlvgl-creator add-target <name> <vendor_dir>
```
* `name` – identifiant utilisé dans `manifest.yml`.
* `vendor_dir` – chemin où les ressources converties sont vendues.

### sync
Regénère les listes de fonctionnalités Cargo et un index de ressources à partir du manifeste.

```
rlvgl-creator sync <out> [--dry-run]
```
* `out` – répertoire où écrire les fichiers générés.
* `--dry-run` – affiche les modifications sans les écrire sur le disque.

### scaffold
Crée une caisse de ressources à double mode qui peut soit intégrer des ressources, soit les vendre au moment de la construction.

```
rlvgl-creator scaffold <path>
```
* `path` – répertoire de destination de la caisse générée.

### apng
Construit un PNG animé à partir d'une séquence de cadres.

```
rlvgl-creator apng <frames> <out> [--delay MS] [--loops N]
```
* `frames` – répertoire contenant des cadres PNG séquentiels.
* `out` – fichier APNG de sortie.
* `--delay` – délai d'image en millisecondes (par défaut 100).
* `--loops` – nombre de boucles (`0` pour l'infini).

### schema
Imprime le schéma JSON de `manifest.yml` sur la sortie standard.

```
rlvgl-creator schema
```

### fonts pack
Rasterise les polices TTF/OTF en données bitmap et fichiers de métriques.

```
rlvgl-creator fonts pack <path> [--size PX] [--chars STRING]
```
* `path` – répertoire contenant les fichiers de polices.
* `--size` – taille de point pour la rastérisation (par défaut `32`).
* `--chars` – chaîne de caractères à inclure dans le pack.

### lottie import
Importe une animation JSON Lottie en cadres PNG et éventuellement un APNG.

```
rlvgl-creator lottie import <json> <out> [--apng FILE]
```
* `json` – chemin d'accès au fichier JSON Lottie.
* `out` – répertoire où les cadres sont écrits.
* `--apng` – fichier APNG facultatif à générer.

### lottie cli
Utilise un CLI externe pour convertir une animation JSON Lottie.

```
rlvgl-creator lottie cli [--bin PATH] <json> <out> [--apng FILE]
```
* `--bin` – binaire externe (par défaut `lottie-cli`).
* `json` – chemin d'accès au fichier JSON Lottie.
* `out` – répertoire où les cadres sont écrits.
* `--apng` – fichier APNG facultatif à générer.

### svg
Rend un SVG en fichiers image bruts.

```
rlvgl-creator svg <svg> <out> [--dpi DPI...] [--threshold VAL]
```
* `svg` – chemin d'accès au fichier SVG.
* `out` – répertoire où les images brutes sont écrites.
* `--dpi` – une ou plusieurs valeurs DPI à rendre (par défaut `96`).
* `--threshold` – seuil monochrome (0–255).

### bsp from-ioc
Rend la source Rust d'un projet CubeMX à l'aide d'un modèle MiniJinja.

```
rlvgl-creator bsp from-ioc <ioc> [--emit-hal] [--emit-pac] [--template <template>]
    --out <dir> [--grouped-writes] [--one-file | --per-peripheral] [--with-deinit]
    [--allow-reserved]
```
* `ioc` – fichier CubeMX `.ioc` d'entrée.
* `--emit-hal` – rendu à l'aide du modèle HAL intégré.
* `--emit-pac` – rendu à l'aide du modèle PAC intégré.
* `--template` – chemin d'accès à un modèle MiniJinja personnalisé.
* `--out` – répertoire où placer le fichier source généré.
* `--grouped-writes` – regroupe les écritures RCC par registre.
  Sélectionne automatiquement les noms de bus spécifiques à la famille pour les familles F0, F1, F2,
  F3, F4, F7, G0, G4, H5, H7, L0, L1, L4, L5, U5, WB et WL.
* `--one-file` – émet un seul fichier source consolidé.
* `--per-peripheral` – émet un fichier par périphérique avec une gestion des fonctionnalités.
* `--with-deinit` – inclut des fonctions d'assistance de désinitialisation facultatives.
* `--allow-reserved` – autorise la configuration des broches SWD réservées (`PA13`, `PA14`).
  Les assistants gèrent les horloges, masquent les IRQ et réinitialisent les registres de configuration DMA/BDMA/MDMA,
  y compris le routage DMAMUX et les cas limites de flux/canaux.
  Couvre les contrôleurs des variantes F0, F1, F2, F3, F4, F7, H5, H7, L0, L1, L4,
  L5, G0, G4, U5, WB et WL.

Voir aussi : comportement de génération de BSP STM32, drapeaux et feuille de route dans
[STM_BSP_GENERATION.md](./STM_BSP_GENERATION.md) — inclut le découpage double cœur,
l'alimentation (SCUEN/VOS) et les détails de l'horloge (PLL1/prescalers).

#### Exemples de configuration avancée

Générer des BSP HAL et PAC avec des écritures RCC groupées, une disposition par périphérique et des crochets de désinitialisation :

```bash
rlvgl-creator bsp from-ioc f769.ioc \
    --emit-hal --emit-pac --grouped-writes \
    --per-peripheral --with-deinit --out bsp
```

Rendre un BSP PAC uniquement minimal dans un seul fichier pour une mise en service rapide :

```bash
rlvgl-creator bsp from-ioc bringup.ioc \
    --emit-pac --one-file --out bsp
```

Générer un BSP HAL uniquement avec des écritures RCC non groupées dans un seul fichier :

```bash
rlvgl-creator bsp from-ioc minimal.ioc \
    --emit-hal --one-file --out bsp
```

Parcourir un BSP STM32F769I-DISCO sensible au bus avec un nettoyage DMA complet :

1. Générer le code HAL et PAC avec des écritures groupées, une disposition par périphérique et des crochets de désinitialisation :
   ```bash
   rlvgl-creator bsp from-ioc f769.ioc \
       --emit-hal --emit-pac --grouped-writes \
       --per-peripheral --with-deinit --out bsp
   ```
2. Appeler `board::deinit()` pendant l'arrêt pour gérer les horloges, masquer les interruptions et réinitialiser l'état DMA/BDMA/MDMA.

Parcourir un BSP STM32H573I-DISCO sensible au bus avec des écritures non groupées :

 1. Générer le code HAL dans un seul fichier sans écritures RCC groupées :
  ```bash
  rlvgl-creator bsp from-ioc h573.ioc \
      --emit-hal --one-file --out bsp
  ```
2. Appeler `board::deinit()` pendant l'arrêt pour gérer les horloges et réinitialiser l'état des broches.

### Cas limites et pièges

* Les registres d'horloge des périphériques varient selon les familles à faible consommation telles que L0 et
  L1. Examiner les écritures RCC générées lors du ciblage de nouvelles pièces.
* Le nettoyage DMA efface les canaux DMAMUX et les registres de flux, mais ne gère pas encore
  les modes de liste chaînée ou de double tampon.
* Certains périphériques nécessitent des étapes de réinitialisation supplémentaires au-delà de la gestion de l'horloge ; vérifier
  les crochets de désinitialisation pour les blocs IP personnalisés ou rares.

## Flux de travail : du `.ioc` STM32 au BSP
1. Convertir le fichier `.ioc` en représentation intermédiaire et rendre une caisse BSP (les AF sont dérivées des données du fournisseur intégré) :
   ```bash
   rlvgl-creator bsp from-ioc simple.ioc --emit-hal --out bsp
   ```
2. Utiliser le BSP généré dans un projet :
   ```rust
   // Cargo.toml
   // [dependencies]
   // board = { path = "bsp" }

   // main.rs
   board::init();
   ```

## Flux de travail : créer et finaliser une bibliothèque de ressources
1. Initialiser un nouveau pack et enregistrer une cible `host` :
   ```bash
   rlvgl-creator init
   rlvgl-creator add-target host vendor
   ```
2. Ajouter des ressources brutes :
   * Placer les fichiers image sous `icons/` ou `media/`.
   * Copier les polices (`.ttf`, `.otf`) dans `fonts/`.
3. Analyser et convertir les ressources :
   ```bash
   rlvgl-creator scan assets/
   rlvgl-creator convert assets/
   ```
4. Générer des aperçus et synchroniser les listes de fonctionnalités :
   ```bash
   rlvgl-creator preview assets/
   rlvgl-creator sync vendor
   ```
5. Échafauder une caisse exposant les ressources :
   ```bash
   rlvgl-creator scaffold assets-pack
   ```
6. Utiliser la caisse de ressources :
   ```rust
   // Cargo.toml
   // [dependencies]
   // assets_pack = { path = "assets-pack" }

   // main.rs
   use assets_pack::fonts::PRIMARY_FONT;
   use assets_pack::images::LOGO;
   ```
   La caisse fournit des accesseurs fortement typés pour les polices et les graphiques qui peuvent être intégrés ou vendus selon les fonctionnalités de construction.

## Exemples d'utilisation
* **BSP** – Inclure la caisse de carte générée dans un projet de micrologiciel et appeler `board::init()` pour configurer les horloges et le multiplexage des broches.
* **Bibliothèque de ressources** – Dépendre de la caisse de ressources échafaudée et référencer les éléments exportés tels que `assets_pack::images::LOGO` lors de la construction de widgets.
```
