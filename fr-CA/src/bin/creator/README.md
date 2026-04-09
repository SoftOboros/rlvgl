<!--
src/bin/creator/README.md - Guide des flux de travail binaires de rlvgl-creator.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator

Un outil combiné UI et ligne de commande pour normaliser les actifs et générer des caisses d'actifs bi-mode pour les projets rlvgl. L'exécution sans arguments lance l'interface utilisateur de bureau; la fourniture d'arguments exécute le CLI. Ce guide couvre le flux de travail de bout en bout, de l'initialisation à la consommation.

## Flux de travail

1.  **Initialiser les dossiers et le manifeste**
    ```sh
    cargo run --bin rlgvl-creator --features creator,creator_ui -- init
    ```
    Crée `icons/`, `fonts/`, `media/`, et un `manifest.yml` dans le répertoire de travail.

2.  **Analyser les actifs nouveaux ou modifiés**
    ```sh
    cargo run --bin rlgvl-creator --features creator,creator_ui -- scan .
    ```
    Met à jour les hachages dans le manifeste pour les actifs sous les racines autorisées.

3.  **Convertir les actifs en séquences brutes et en packs de polices**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- convert
    ```
    Les images raster deviennent des séquences RGBA brutes, et les polices sont compressées en binaires bitmap et en métriques. Les conversions s'exécutent en parallèle
    avec un ordre stable. Utilisez `--force` pour reconstruire tous les actifs, quel que soit le cache.

    Pour rendre les actifs vectoriels, la commande `svg` convertit un SVG en une ou plusieurs images brutes aux valeurs DPI choisies :
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- svg logo.svg out/ --dpi 96 --dpi 192
    ```
    Fournissez `--threshold <VAL>` pour appliquer un seuil monochrome adapté aux écrans e-ink.

4.  **Synchroniser les drapeaux de fonctionnalité, les constantes et l'index**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- sync
    ```
    Regénère le code piloté par le manifeste sans toucher aux octets des actifs.

5.  **Échafauder une caisse d'actifs consommateur**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- scaffold assets-crate
    ```
    Génère une caisse avec les fonctionnalités `embed` et `vendor` qui expose vos actifs traités.

6.  **Vendre des actifs pour la sortie de build**
    ```sh
    cargo run --bin rlvgl-creator --features creator,creator_ui -- vendor
    ```
    Copie les actifs traités dans `$OUT_DIR` et émet un module `rlvgl_assets.rs` pour inclusion.

La caisse résultante peut être construite avec `--features embed` pour inclure des octets bruts ou `--features vendor` pour copier des fichiers au moment de la construction tout en important le module généré.

## Interface utilisateur de bureau et émulateur

Lancez l'interface utilisateur de bureau explicitement :

```sh
cargo run --bin rlvgl-creator --features creator,creator_ui -- ui
```

Exécutez le simulateur à partir du même binaire :

```sh
cargo run --bin rlvgl-creator --features creator,creator_ui -- sim --screen=800x480 --png --qrcode
```

## Notes du développeur

Pour plus de détails sur la personnalisation des modèles d'échafaudage et l'extension du pipeline de conversion, consultez
[`docs/CREATOR-TEMPLATES.md`](../../../docs/CREATOR-TEMPLATES.md).
