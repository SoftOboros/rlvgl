```markdown
<!--
AGENTS.md - Guide du contributeur et conventions de projet.
-->
<p align="center">
  <img src="rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl

Une réimplémentation Rust modulaire et idiomatique de LVGL (Light and Versatile Graphics Library).

rlvgl préserve le paradigme d'interface utilisateur basé sur des widgets de LVGL tout en éliminant la gestion de la mémoire non sécurisée de style C et l'état global. Cette bibliothèque est structurée pour prendre en charge les environnements `no_std`, les cibles embarquées (par exemple, STM32H7) et les backends de simulateur pour un prototypage rapide.

La version C de LVGL est incluse en tant que sous-module git pour référence et extraction de vecteurs de test, mais elle n'est ni liée ni compilée dans cette bibliothèque.

## Objectifs

*   Préserver l'architecture et le système de mise en page de LVGL
*   Remplacer la gestion de la mémoire en C par une appropriation idiomatique de Rust
*   Prendre en charge l'affichage embarqué / l'entrée via `embedded-hal`
*   Activer la hiérarchie des widgets, les styles et les événements à l'aide des traits Rust
*   Utiliser les caisses Rust existantes lorsque cela est possible (par exemple, `embedded-graphics`, `heapless`, `tinybmp`)

## Fonctionnalités

*   Support `no_std` + allocateur
*   Disposition modulaire basée sur les composants (noyau, widgets, plateforme)
*   Simulable via le drapeau de fonctionnalité `std`-activé
*   Backends d'affichage et d'entrée enfichables

## Structure du projet

*   `core/` – Trait de base du widget, mise en page, distribution des événements
*   `widgets/` – Réimplémentations Rust-natives des widgets LVGL
*   `platform/` – Traits d'affichage/entrée et adaptateurs HAL
*   `lvgl/` – Sous-module C (référence uniquement)

## Statut

Tel que construit. Voir `./docs/TODO.md` pour la progression composant par composant.

## Lignes directrices du générateur BSP

L'outil `rlvgl-creator` convertit les fichiers de configuration du fournisseur en un IR neutre vis-à-vis du fournisseur et rend le code Rust BSP via des modèles MiniJinja.

*   Éviter les tables par puce. Les règles de classe de périphérique devraient être
    réutilisables entre les instances et les fournisseurs.
*   Les numéros de fonction alternatifs sont résolus par programme à partir de données JSON
    générées par des scripts Python spécifiques au fournisseur.
*   Garder les broches réservées (par exemple, SWD sur `PA13`/`PA14`) hors du code généré
    sauf autorisation explicite.
*   Documenter toute aide de modèle dans `README.md` et suivre les travaux en suspens dans
    `docs/TODO-CREATOR-BSP.md`.

## Notes sur la couverture

La couverture LLVM est disponible en utilisant `grcov`. Exécutez `make coverage` pour compiler les tests
avec instrumentation et générer un rapport HTML dans `./coverage`. Lors de la
collecte de la couverture, assurez-vous que les variables d'environnement suivantes sont définies (elles sont
également présentes dans `.cargo/config.toml`) :

```
CARGO_INCREMENTAL=0
RUSTFLAGS="-Zinstrument-coverage"
LLVM_PROFILE_FILE="coverage-%p-%m.profraw"
```

Les futures exécutions de Codex devraient se concentrer sur une couverture mesurable et utiliser ces variables
lors de la génération des tests.

Exécutez toujours `cargo fmt --all` et corrigez les erreurs de formatage avant de préparer une
demande de tirage. Vérifiez le formatage avec `cargo fmt --all -- --check`.

Les API publiques doivent être documentées. Le lint `#![deny(missing_docs)]` est activé dans
toutes les caisses, de sorte que la compilation échouera si un élément public manque d'un
docstring significatif. Ces caisses sont publiées sur crates.io et nécessitent une
documentation claire pour les utilisateurs.
Tous les fichiers doivent inclure un en-tête descriptif résumant leur objectif.

Exécutez `./scripts/pre-commit.sh` et assurez-vous qu'il réussit avant d'ouvrir une demande de tirage. Ce script applique le formatage, exécute clippy, compile avec toutes les fonctionnalités et vérifie la génération de documentation en utilisant nightly.

Utilisez `scripts/check-links.sh` pour valider les liens Markdown avant de soumettre des modifications de documentation.

## Exemples de scripts d'édition de liens

Chaque projet d'exemple qui fournit un script d'édition de liens `memory.x` doit inclure un fichier
`build.rs` qui :

*   copie le `memory.x` local dans le répertoire de sortie de construction de Cargo,
*   émet `cargo:rustc-link-search` pour ce répertoire, et
*   émet `cargo:rustc-link-arg=-Tmemory.x`.

Ceci évite de dépendre d'un fichier global `.cargo/config.toml` pour la configuration du
script d'édition de liens.
```
