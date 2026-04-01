```markdown
<!--
docs/TODO-CREATOR.md - rlvgl-creator — Épopée et tableaux sectionnés.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator — Épopée et tableaux sectionnés

_Un seul fichier markdown qui structure le travail en une seule **Épopée** avec des tableaux d'histoires d'utilisateurs sectionnés. Chaque section commence par une brève description (histoire d'utilisateur) et un tableau de contrôle._

---

## Vue d'ensemble de l'épopée
**Épopée :** Construire **rlvgl-creator**, un outil UI + CLI qui importe, normalise, prévisualise et fournit des actifs pour les projets rlvgl tout en échafaudant des caisses d'actifs bi-mode et en minimisant l'empreinte sur les cibles `no_std + alloc`.

**Résultats :**
- Des pipelines reproductibles pour les séquences d'images RGBA brutes, les polices et Lottie.
- Des politiques de nommage/chemin strictes avec des conseils de correction automatique.
- Double livraison (intégrer ou vendre) pour les packs d'actifs.
- Une interface utilisateur de bureau pour la prévisualisation, le dimensionnement et l'empaquetage.

---

## 0) Décisions et politiques verrouillées
_Histoire d'utilisateur : En tant que mainteneur, je veux des garde-fous pour que les équipes puissent faire évoluer les actifs en toute sécurité sans dérive._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Appliquer les racines de dossiers `icons/`, `fonts/`, `media/`; rejeter les autres avec des conseils de correction. | créateur core | Vérificateur de politiques + `--fix` pour renommer.
| [x] | Générer des noms de constantes/fonctionnalités; interdire les modifications manuelles (SCREAMING_SNAKE; `ICON_`, `FONT_`, `MEDIA_`). | créateur core | Carte de noms déterministe; sortie de diff.
| [x] | Le créateur est `std`; les cibles sont compatibles `no_std + alloc`; pré-dimensionner/empaqueter les actifs. | N/A | Contrainte de conception pour toutes les fonctionnalités.
| [x] | Actifs de base stockés sous forme d'images/séquences RGBA brutes; pas de PNG/APNG à l'exécution. | interne | Remplace les formats dépendants de std. |
| [x] | Supporte à la fois la lecture directe de Lottie et la conversion Lottie→APNG. | rlottie (FFI) ou Conan CLI | Choix par actif enregistré dans le manifeste.
| [x] | Compression de bundle optionnelle utilisant RLE + table de jetons; le chemin de base décode avec un petit décodeur gâté. | interne | Compatible no_std; préférer la compression au moment de la construction dans le chemin du fournisseur. |

---

## 1) Interface et UX de la CLI
_Histoire d'utilisateur : En tant que développeur, je peux gérer les actifs via des commandes claires avec une validation utile et des exemples._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | `init` — démarrer les dossiers et le `manifest.yml` par défaut. | clap, anyhow | Idempotent; affiche les prochaines étapes.
| [x] | `scan <path>` — découvrir les nouveaux actifs/modifiés et mettre à jour le manifeste. | blake3, walkdir | Basé sur le hachage; respecte la politique des racines.
| [x] | `convert` — normaliser en séquences RGBA brutes; empaqueter les polices; écrire les métadonnées. | image, fontdue/ab_glyph | Sorties déterministes. |
| [x] | `vendor` — copier dans `$OUT_DIR`/repo et générer `rlvgl_assets.rs`. | std fs, tera | Prend en charge le préréglage par cible.
| [x] | `scaffold assets-crate` — générer une caisse bi-mode. | tera | Fonctionnalités d'intégration et de vente.
| [x] | `preview` — vignettes/feuilles de sprites. | image | Stocke dans `assets/thumbs/`.
| [x] | `add-target` — enregistrer la caisse locale + `vendor_dir` et les préréglages. | serde_yaml | Met à jour le manifeste.
| [x] | `sync` — régénérer les fonctionnalités Cargo, les constantes, l'index à partir du manifeste. | tera | Le mode "dry-run" affiche le diff.
| [x] | `apng` — construire un APNG à partir de groupes d'images brutes; définir le timing/les boucles. | apng | Exportation PNG de la première image. |
| [x] | `lottie import` — Lottie→images/APNG; exporter la carte de timing. | rlottie/CLI | Enregistre le chemin choisi.
| [x] | `fonts pack` — tailles, ensembles de glyphes, empaquetage/métriques. | fontdue/ab_glyph | Sous-ensemble optionnel.
| [x] | `check` — validation stricte de la politique; `--fix` normalisation automatique. | créateur core | Sortie non nulle en cas de violations.
| [ ] | `ui` — lancer l'interface utilisateur de bureau. | Tauri ou eframe/wgpu | Partage les bibliothèques de base.
| [x] | Fournir des drapeaux globaux et une aide riche avec des exemples. | clap | Codes de sortie standardisés.
| [x] | Diviser l'implémentation de la CLI en modules. | interne | Maintient la maintenabilité des binaires.

---

## 2) Manifeste et conventions
_Histoire d'utilisateur : En tant que mainteneur, je veux un manifeste géré par la machine qui encode la politique et les cibles._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Définir `manifest.yml` v1 (`packages`, `groups`, `features`, `expose`, `targets`). | serde_yaml, schemars | Émet un schéma JSON pour les outils d'édition.
| [x] | Appliquer la politique de chemin : chemins publics sous `icons/`, `fonts/`, ou `media/`. | créateur core | Erreurs exploitables + `--fix`.
| [x] | Générer des noms de fonctionnalités à partir de groupes; émettre des agrégats `*_all`. | créateur core | Ordre stable.
| [x] | Générer des noms de constantes à partir des entrées du manifeste; rejeter les renommages manuels. | créateur core | Le diff affiche la correspondance ancien→nouveau.
| [x] | Métadonnées de licence par actif/groupe avec liste d'autorisation/refus. | Tableau SPDX | Bloquer le fournisseur si manquant.
| [x] | Configuration de `naming` (carte de préfixes + politique de casse) pour les documents; le générateur est la source de vérité. | N/A | Maintient la politique explicite.
| [x] | Préréglages par cible (taille d'écran, profondeur, stockage) pour le dimensionnement automatique. | fichier de préréglages | Connecté à `vendor`.

---

## 3) Échafaudage de la caisse d'actifs (bi-mode)
_Histoire d'utilisateur : En tant qu'utilisateur, je peux consommer des actifs en intégrant des octets ou en fournissant des fichiers sans dépendances d'exécution._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Générer `Cargo.toml` avec les fonctionnalités `embed`, `vendor` et de groupe. | tera | Pas de fonctionnalités par défaut.
| [x] | Générer `src/lib.rs` — intégrer : constantes `include_bytes!`. | tera | Une constante par actif exposé.
| [x] | Générer `src/lib.rs` — fournisseur : `vendor_api::{copy_all, generate_rust_module}`. | std fs, tera | Chemins sûrs pour `$OUT_DIR`.
| [x] | Autotest `build.rs` optionnel pour la caisse. | std | Test rapide en CI.
| [x] | Générer un README avec l'utilisation de l'intégration vs le fournisseur. | tera | Extraits copier-coller.
| [x] | Tests d'instantanés pour les fichiers générés. | insta | Garde contre les régressions.
| [x] | `cargo publish --dry-run` passe. | cargo | Porte de CI.

---

## 4) Pipelines de conversion
_Histoire d'utilisateur : En tant que concepteur, je peux déposer des formats courants et obtenir des sorties normalisées et rapides à charger._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Format de séquence RGBA brute avec en-tête d'image maximale; taille/position par image; les images uniques abandonnent les en-têtes d'image. | interne | Remplace la base PNG/APNG. |
| [x] | Encoder des fichiers `.raw` à partir d'entrées courantes. | créateur core | Normalise les actifs raster. |
| [x] | Ingérer des fichiers `.raw` dans le pipeline. | créateur core | Analyser l'en-tête et les images. |
| [x] | SVG→images brutes dimensionnées (liste DPI; seuils monochrome/e-ink). | resvg/usvg (opt.) | Repli sur externe si nécessaire. |
| [x] | Générateur APNG à partir d'images brutes avec délai par image et nombre de boucles; PNG de la première image. | apng | Vérifications de l'ordre des images. |
| [x] | Lottie via FFI (`lottie-ffi`) utilisant `rlottie`. | rlottie, Conan | Porte de fonctionnalité; notes de plateforme.
| [x] | Lottie via CLI externe (`lottie-cli`) vers images/APNG. | recette Conan | Enregistrer le chemin dans le manifeste.
| [x] | Polices : TTF/OTF→packs bitmap (`.bin`) + métriques (`.json`); sous-ensemble optionnel. | fontdue/ab_glyph | Ensemble de glyphes par cible.
| [x] | Compression simple RLE + table de jetons pour les fichiers bruts. | interne | Petit décodeur pour les cibles no_std. |

---

## 5) Application UI (Interface utilisateur du créateur)
_Histoire d'utilisateur : En tant que développeur/concepteur, je peux prévisualiser, zoomer/déplacer, grouper et exporter des actifs visuellement._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Choisir la pile et démarrer le projet UI. | eframe/egui | Fenêtre initiale et chargement du manifeste.
| [x] | Panneau du navigateur d'actifs (arbre, filtres, recherche, badges de licence). | kit UI | Reflète les groupes du manifeste.
| [x] | Visionneuse de toile (zoom/déplacement, grille de pixels, fond en damier). | wgpu/pixels | Défilement APNG/Lottie en attente.
| [x] | Inspecteur : Méta (taille/DPI/hachage/licence/balises/groupes). | serde | L'édition en direct écrit le manifeste.
| [x] | Inspecteur : Exportation (tailles, espace couleur, alpha prémultiplié, compression). | créateur core | S'applique par actif.
| [x] | Inspecteur : Animation (timing/boucles; options Lottie→APNG). | apng/rlottie | Interface utilisateur de défilement.
| [x] | Inspecteur : Polices (ensemble de glyphes, tailles, hinting, empaquetage). | fontdue | Prévisualiser les pangrammes.
| [x] | Glisser-déposer vers `assets/raw/` avec `scan` immédiat. | notify | Affiche des toasts.
| [x] | Préréglages de taille d'écran (par exemple, `stm32h7‑480x272`) avec prévisualisation en direct. | préréglages | Rend les cadres de délimitation.
| [x] | Actions : "Créer APNG à partir de la sélection", "Ajouter au groupe", "Révéler dans le manifeste". | kit UI | Prise en charge de la sélection multiple.
| [x] | Pipeline de vignettes + rechargement à chaud. | image, notify | Invalidation du cache via hachage.
| [x] | Prévisualisation/éditeur de mise en page pour un prototypage rapide de l'interface utilisateur. | plus tard | Canevas de mise en page de base par glisser-déposer.

---

## 6) Intégration du fournisseur et de l'intégration
_Histoire d'utilisateur : En tant qu'auteur d'application, je peux choisir d'intégrer ou de fournir et obtenir des octets identiques._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Exemples d'intégration (`default-features=false`, fonctionnalités par groupe; utilisation de const). | exemples | La CI les construit.
| [ ] | Exemples de fournisseur (consommateur `build.rs` + `include!(.../rlvgl_assets.rs)`). | exemples | `$OUT_DIR` sûr.
| [x] | API `get(path)` optionnelle en mode intégré (path→bytes). | phf/lite map | Index généré.
| [x] | Test d'égalité d'octets : intégration vs fournisseur pour les mêmes ID d'actif. | tests | Assertion CI.

---

## 7) Mise en cache et builds incrémentiels
_Histoire d'utilisateur : En tant qu'utilisateur, je veux des réexécutions rapides avec des sorties déterministes._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Cache de hachage de contenu dans `assets/.cache` (hachage→sorties/horodatages/tailles). | blake3, serde | Stockage JSON/CBOR.
| [x] | Invalidation `--force` et reconstruction intelligente par hachage/mtime. | créateur core | Message clair.
| [x] | Paralléliser les conversions avec un ordre stable. | rayon (opt.) | Protéger les conditions de concurrence.
| [x] | Émettre des indices `cargo:rerun-if-changed` pour les étapes de fournisseur/build. | API build.rs | Bonne expérience développeur pour les consommateurs.

---

## 8) Validation, lints et CI
_Histoire d'utilisateur : En tant que mainteneur, je peux faire confiance à chaque PR pour appliquer la politique et rester "vert"._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | `creator check` couvre les chemins, les noms, la licence, les doublons, les seuils de taille. | créateur core | Sortie non nulle.
| [x] | Modèle de hook de pré-commit (scan/convert/check). | hooks git | Facultatif mais encouragé.
| [x] | La tâche CI exécute de bout en bout : `scan → convert → sync → scaffold → vendor`. | Actions GH | Met en cache les toolchains.
| [x] | Tests golden pour le timing APNG et les échantillons de polices. | apng, fontdue | Fixtures déterministes.
| [x] | Tests d'instantanés pour `Cargo.toml`, `lib.rs`, `rlvgl_assets.rs` générés. | insta | Stocké dans le dépôt.

---

## 9) Critères d'acceptation (MVP)
_Histoire d'utilisateur : En tant que partie prenante, je peux vérifier la valeur rapidement avec une tranche verticale fonctionnelle._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | La caisse d'actifs bi-mode compile à partir de l'échafaudage. | cargo | test rapide.
| [ ] | `scan + convert + sync` correspondent au manifeste; pas de fichiers égarés. | créateur core | Vérification CI.
| [x] | Le fournisseur et l'intégration donnent des octets identiques pour les mêmes ID d'actif. | tests | Comparaison d'octets.
| [ ] | APNG à partir d'images simples joue avec un timing correct dans un visualiseur de référence. | apng | Visualiseur en CI (headless).
| [ ] | `cargo publish --dry-run` pour la caisse générée réussit. | cargo | Règles de versionnage.
| [ ] | Les entrées non conformes donnent des erreurs exploitables et `--fix` les résout. | créateur core | Sortie conviviale.

---

## 10) Feuille de route / Phases
_Histoire d'utilisateur : En tant que planificateur, je peux échelonner la livraison pour apporter de la valeur rapidement et souvent._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Phase 1 – MVP : scan/convert/vendor; caisse d'échafaudage; vérification stricte. | éléments principaux | Version de base.
| [x] | Phase 2 – Polices : sous-ensemble/empaquetage/métriques; groupes de fonctionnalités par taille/famille. | fontdue | Améliore les performances de chargement.
| [x] | Phase 3 – Lottie : importation + APNG; feuilles de sprites + méta de timing. | rlottie/apng | Support d'animation plus large.
| [x] | Phase 4 – Prévisualisation : vignettes + visionneuse CLI/GUI; profilage de taille/chemin critique. | UI + image | Vitesse du développeur.
| [x] | Phase 5 – GUI : UI complète avec prévisualisation/éditeur de mise en page et préréglages. | pile UI | Vitesse du concepteur.
| [ ] | Phase 6 – Avancé : pipelines wasm; catalogues à distance; empaquetage CDN. | wasm-bindgen | Étirement.

---

## 11) Étirement et fonctionnalités appréciables
_Histoire d'utilisateur : En tant qu'utilisateur avancé, je peux optimiser davantage les pipelines et l'empaquetage._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Constructeur de feuilles/atlas de sprites (+ atlas JSON/RON). | image, serde | Option pour les particules/UI.
| [ ] | Préréglages et assistants par cible (contraintes d'affichage/bpp/stockage). | préréglages | UX de l'assistant.
| [ ] | Porte de licence sur le fournisseur (bloque les actifs incompatibles). | SPDX | Sécurité juridique.
| [ ] | Télémétrie locale : octets économisés, temps de chargement et estimations RAM/flash. | module stats | Opt-in.
| [ ] | Points d'extension pour les convertisseurs/optimisations personnalisés. | API de trait | Chargement depuis TOML.

---

## 12) Livrables et documents
_Histoire d'utilisateur : En tant que nouvel arrivant, je peux devenir productif avec des exemples et des guides clairs._

| Terminé | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Pack d'actifs exemple (icônes/polices/médias) avec manifeste. | données du dépôt | Utilisé dans les tests.
| [ ] | Deux exemples de consommateurs : modèles **embed** et **vendor**. | exemples | La CI construit et exécute.
| [x] | Guide de l'utilisateur (README) avec un flux de travail de bout en bout. | mdbook/README | Captures d'écran/gifs.
| [x] | Documentation développeur pour les modèles (Tera) et les hooks de pipeline. | rustdoc | API + répertoire des modèles.
```
