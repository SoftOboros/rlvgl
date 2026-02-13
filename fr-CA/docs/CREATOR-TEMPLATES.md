```markdown
<!--
docs/CREATOR-TEMPLATES.md - rlvgl-creator Templates and Hooks.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator Modèles et Hooks

Documentation pour les développeurs décrivant comment le créateur utilise les modèles Tera intégrés pour l'échafaudage des caisses d'actifs et où étendre le pipeline de conversion.

## Modèles
La commande `scaffold` construit une caisse d'actifs en utilisant des modèles Minjinja qui sont intégrés en tant que constantes de chaîne dans [`src/bin/creator/scaffold.rs`](../src/bin/creator/scaffold.rs). Ces modèles couvrent des fichiers tels que `Cargo.toml`, `lib.rs`, `build.rs` et `README.md`. Modifiez les constantes correspondantes pour modifier la disposition de la caisse générée ou ajouter de nouveaux fichiers.

## Hooks de Pipeline
La logique de conversion réside dans des fichiers Rust modulaires comme [`convert.rs`](../src/bin/creator/convert.rs), [`fonts.rs`](../src/bin/creator/fonts.rs) et [`lottie.rs`](../src/bin/creator/lottie.rs). Les nouvelles étapes du pipeline peuvent s'intégrer au processus en ajoutant un module et en l'invoquant à partir de `convert.rs`. Chaque étape reçoit les métadonnées de l'actif et peut émettre des sorties dans `.cache` pour réutilisation.
```
