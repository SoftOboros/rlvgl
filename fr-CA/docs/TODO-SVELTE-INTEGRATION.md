```markdown
<!--
docs/TODO-SVELTE-INTEGRATION.md - alignement Svelte de rlvgl-creator et pipeline d'interface utilisateur axée sur les jetons TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator — Intégration Svelte TODO

_Un seul fichier markdown qui structure le travail en un seul **Épic** avec des tableaux d'histoires utilisateur sectionnés. Chaque section commence par une brève description (histoire utilisateur) et un tableau de vérification._

---

## Vue d'ensemble de l'Épic
**Épic :** Aligner Svelte (jetons de conception, création de composants et état des runes) avec `rlvgl` en étendant `rlvgl-creator` pour générer des fichiers pour les cibles web et embarquées à partir de sources d'interface utilisateur partagées.

**Résultats :**
- La source de jetons partagée produit des sorties CSS/Tailwind web et des sorties de thème `rlvgl-ui`.
- La création de composants Svelte se mappe aux arbres de widgets `rlvgl` (sous-ensemble avec des contraintes claires).
- Les runes Svelte 5 se mappent aux liaisons d'état embarquées et aux mises à jour dérivées.
- Une future double construction (simulateur web + embarqué) est activée par un IR partagé et des hooks de générateur.

---

## 0) Décisions et contraintes verrouillées
_Histoire utilisateur : En tant que mainteneur, je souhaite des limites claires afin que l'intégration reste uniquement la génération de fichiers et alignée sur les caisses existantes._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Le créateur reste uniquement la génération de fichiers (pas d'exécution à l'exécution). | politique | Sorties Rust/JS/CSS/config uniquement. |
| [x] | Pas de nouvelles caisses pour cette phase d'alignement. | espace de travail | Ajouter des modules sous `src/bin/creator/`. |
| [x] | La direction principale est le **Système de conception partagé** (B), avec le prototypage (A) plus tard. | produit | Les jetons d'abord, la création d'interface utilisateur ensuite. |
| [x] | Commencer par la conception de l'Option 5 (Svelte → WASM → rlvgl renderer), mais ne générer que des fichiers et des hooks. | architecture | Travaux d'exécution différés. |
| [x] | Fournir des hooks pour la double construction (Option 4) tôt; le livrer plus tard. | architecture | L'IR et les manifestes doivent prendre en charge les deux. |

---

## 1) Surface CLI : Nouvelle commande `svelte`
_Histoire utilisateur : En tant que développeur, je peux exécuter des commandes explicites du créateur pour générer des sorties de jetons, des cibles de composants et du code de liaison à partir de sources Svelte._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Ajouter la commande de haut niveau `rlvgl-creator svelte` avec des sous-commandes. | clap | Nouvelle famille de commandes. |
| [ ] | `svelte tokens` — lire le YAML des jetons et émettre des sorties web + rlvgl. | serde_yaml | Émet du CSS/Tailwind + Rust. |
| [ ] | `svelte compile` — compiler `.svelte` en IR et émettre le widget rlvgl Rust. | Analyseur/CLI Svelte | Sortie de fichier uniquement. |
| [ ] | `svelte wasm` — émettre la liaison du moteur de rendu et les configurations de construction pour Svelte→WASM→rlvgl. | modèles | Génère des shims uniquement. |
| [ ] | `svelte schema` — émettre le schéma JSON pour les jetons et l'IR de l'interface utilisateur. | schemars | Support de l'éditeur. |
| [ ] | `svelte check` — valider les jetons + les contraintes du sous-ensemble Svelte. | cœur du créateur | Sortie non nulle en cas de violation. |

---

## 2) Couche de jetons partagés
_Histoire utilisateur : En tant que concepteur, je définis les jetons une seule fois et les consomme de manière cohérente sur les cibles web et embarquées._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Définir le schéma `shared-tokens.yaml` (couleurs, espacement, rayons, typographie, mouvement). | schemars | Les couleurs acceptent hex/rgb/rgba ; l'espacement/les rayons sont en px ; le mouvement est en ms + jetons d'assouplissement. |
| [ ] | Ajouter des couches de jetons de base et sémantiques avec des modes optionnels (clair/sombre/contraste élevé). | cœur du créateur | V1 utilise un seul mode ; si plusieurs existent, utiliser par défaut/premier ou exiger une sélection explicite. |
| [ ] | Autoriser les alias de jetons avec détection de cycle. | cœur du créateur | Erreur sur les références circulaires. |
| [ ] | Normaliser les noms de jetons en identifiants déterministes. | cœur du créateur | Politique de casse + carte de préfixes. |
| [ ] | Définir la syntaxe de référence des jetons pour les sources d'interface utilisateur et le code généré. | docs | Utiliser `token("colors.primary")` dans les sources Svelte. |
| [ ] | Émettre `tokens.json` normalisé pour les consommateurs d'IR. | serde_json | Carte de jetons canonique pour les compilateurs. |
| [ ] | Générer la sortie des propriétés personnalisées CSS (`tokens.css`). | modèles | Sortie pour Svelte/web. |
| [ ] | Générer un extrait de configuration Tailwind (`tailwind.tokens.cjs`). | modèles | Intégration optionnelle. |
| [ ] | Générer le module Rust de thème `rlvgl-ui` (`theme.rs`). | modèles | Structures `Theme`/`Palette`. |
| [ ] | Ajouter une section de manifeste pour la provenance et le versionnement des jetons. | manifeste | Suivre la source + hachage. |

---

## 3) IR de composant Svelte (sous-ensemble)
_Histoire utilisateur : En tant que développeur, je peux créer un composant Svelte contraint qui se mappe proprement à la sortie d'interface utilisateur embarquée._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Définir les règles du sous-ensemble Svelte (pas d'API DOM, pas de `{ @backend/django-rolodex/rolodex/templates/rolodex/base.html}`, slots limités). | docs | Autoriser le slot par défaut uniquement ; valider dans `svelte check`. |
| [ ] | Définir les blocs/directives autorisés (`{#if}`, `{#each}` avec clé, `on:` événements, `bind:`). | docs | Pas de `{#await}`, pas de `use:`, pas de `transition:` pour l'instant. |
| [ ] | Définir les balises/composants autorisés (balises rlvgl uniquement, pas de HTML brut). | docs | Commencer par Button/Text/Image/Stack/Row/Column. |
| [ ] | Implémenter l'analyse `.svelte` vers un IR de créateur (composants, props, enfants, styles). | analyseur/CLI | Préférer un analyseur Svelte externe si nécessaire. |
| [ ] | Définir les champs IR pour les liaisons dynamiques (références de jetons vs références d'état). | IR | Distinguer les valeurs statiques des valeurs dérivées. |
| [ ] | Mapper les liaisons `style:` de Svelte aux références de jetons et aux styles rlvgl. | cœur du créateur | Les jetons comme source de vérité. |
| [ ] | Normaliser les événements (`on:click`, etc.) aux rappels rlvgl. | IR | Définir les règles de signature du gestionnaire. |
| [ ] | Sérialiser l'IR en JSON pour les futurs outils. | serde_json | Permet une double construction plus tard. |

---

## 4) Svelte → Cible rlvgl (Direction B)
_Histoire utilisateur : En tant que développeur, je peux compiler un composant Svelte en un arbre de widgets rlvgl avec des styles et des événements._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Construire le tableau de mappage de widgets (balise Svelte → widget rlvgl). | docs | Commencer par Button, Text, Image, Stack, Row, Column. |
| [ ] | Définir le mappage des propriétés de disposition (taille, rembourrage, espacement, alignement, justification). | rlvgl-ui | Assurer des valeurs par défaut déterministes. |
| [ ] | Générer le code Rust du constructeur pour les arbres de widgets. | modèles | Sortie uniquement. |
| [ ] | Prend en charge le mappage de styles (arrière-plan, rembourrage, rayon, police, couleur). | rlvgl-ui | Liaison à la sortie du jeton. |
| [ ] | Émettre des modules de composants avec des API publiques stables. | modèles | Correspondre aux conventions `rlvgl`. |
| [ ] | Ajouter des tests qui compilent un exemple de fichier Svelte en sortie Rust. | tests | Instantanés d'or. |

---

## 5) Svelte 5 Runes → Modèle d'état rlvgl
_Histoire utilisateur : En tant que développeur, je peux mapper `$state`, `$derived`, `$effect` de Svelte aux primitives d'état embarquées._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Définir un IR d'état minimal (`State`, `Derived`, `Effect`). | cœur du créateur | Sortie de fichier uniquement. |
| [ ] | Mapper `$state` à `State<T>` et `$derived` aux rappels calculés. | rlvgl-ui | Ajouter ou réutiliser des assistants d'état. |
| [ ] | Définir les modèles de script autorisés (pas d'async, pas de DOM, pas de stores externes). | docs | Uniquement les runes + fonctions locales. |
| [ ] | Définir les règles de liaison pour `bind:` (par exemple, `bind:value`, `bind:checked`). | docs | Mapper aux accesseurs/mutateurs d'état. |
| [ ] | Définir les contraintes de planification des effets pour les cibles embarquées. | docs | Pas d'effets secondaires asynchrones ; s'exécutent au changement d'état. |
| [ ] | Générer des modules Rust pour le câblage d'état et les rappels. | modèles | Liaison aux événements des widgets. |
| [ ] | Ajouter des erreurs de validation pour les modèles de réactivité Svelte non pris en charge. | cœur du créateur | Messages utiles. |

---

## 6) Hooks de l'Option 5 : Svelte → WASM → Moteur de rendu rlvgl
_Histoire utilisateur : En tant que développeur, je peux générer le code de liaison nécessaire pour connecter l'environnement d'exécution de Svelte à un moteur de rendu rlvgl, sans que le créateur n'exécute quoi que ce soit._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Définir une surface d'API de moteur de rendu pour les liaisons d'exécution Svelte. | docs | Créer/mettre à jour/supprimer des nœuds, définir des propriétés, définir des styles, envoyer des événements. |
| [ ] | Générer des shims Rust `wasm-bindgen` pour les points d'entrée du moteur de rendu. | modèles | Sortie de fichier uniquement. |
| [ ] | Émettre une colle JS qui transmet les opérations DOM aux liaisons du moteur de rendu. | modèles | Adaptateur d'exécution Svelte. |
| [ ] | Générer des extraits de configuration de construction (`Cargo.toml`, `package.json`) sous forme de modèles uniquement. | modèles | Pas d'exécution. |
| [ ] | Documenter les fonctionnalités Svelte prises en charge en mode WASM. | docs | Sous-ensemble minimal viable. |

---

## 7) Double construction (Option 4) — Prévue plus tard
_Histoire utilisateur : En tant que développeur, je peux construire un aperçu web et une cible embarquée à partir de la même source d'interface utilisateur._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Définir un IR partagé qui peut émettre des sorties web et rlvgl. | IR | Réutiliser de la section 3. |
| [ ] | Émettre la sortie de prévisualisation web (Svelte + jetons) dans un bundle `preview/`. | modèles | Fichiers statiques uniquement. |
| [ ] | Ajouter la commande `svelte preview` pour générer le bundle de prévisualisation. | CLI du créateur | Pas de serveur de développement. |
| [ ] | Ajouter des sections de manifeste pour les bundles de prévisualisation et les chemins de sortie. | manifeste | Suivre les hachages pour les reconstructions. |

---

## 8) Points d'intégration dans le créateur
_Histoire utilisateur : En tant que mainteneur, je peux intégrer l'alignement Svelte sans nouvelles caisses et garder le code modulaire._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Ajouter le module `src/bin/creator/svelte.rs` pour l'entrée CLI + l'orchestration. | cœur du créateur | Reflète les autres commandes. |
| [ ] | Ajouter les sous-modules `src/bin/creator/svelte/` : tokens, ir, compile, wasm, check. | interne | Garder les modules petits. |
| [ ] | Étendre le manifeste avec la configuration `svelte` (chemin des jetons, racines de l'interface utilisateur, sorties). | manifeste | Suivre les hachages pour les reconstructions. |
| [ ] | Câbler les commandes Svelte dans les menus de l'interface utilisateur plus tard (après la parité CLI). | creator_ui | Suivi optionnel. |

---

## 9) Validation, tests et documentation
_Histoire utilisateur : En tant que mainteneur, je peux faire confiance aux sorties générées et comprendre clairement le sous-ensemble._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Ajouter des instantanés d'or pour les sorties de jetons (CSS/Tailwind/Rust). | insta | Formatage déterministe. |
| [ ] | Ajouter des fixtures `.svelte` d'exemple pour les tests de compilation. | tests | Garder un sous-ensemble minimal. |
| [ ] | Documenter le sous-ensemble Svelte et le tableau de mappage. | docs | Contraintes + exemples. |
| [ ] | Ajouter des entrées de référence CLI du créateur pour les sous-commandes `svelte`. | docs | Mettre à jour `docs/CREATOR-CLI.md`. |

---

## 10) Feuille de route / Phases
_Histoire utilisateur : En tant que planificateur, je peux échelonner la livraison pour obtenir de la valeur rapidement et en toute sécurité._

| Complet | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Phase 1 – Pipeline de jetons + schéma + `svelte tokens`. | jetons | Valeur B immédiate. |
| [ ] | Phase 2 – Sous-ensemble Svelte + IR + `svelte compile` vers rlvgl. | analyseur | Direction B. |
| [ ] | Phase 3 – Mappage des runes et génération d'état. | rlvgl-ui | Direction B. |
| [ ] | Phase 4 – Hooks de l'Option 5 (colle du moteur de rendu WASM). | modèles | Sortie de fichier uniquement. |
| [ ] | Phase 5 – Bundle de prévisualisation de la double construction (Option 4). | prévisualisation | Direction A. |
```
```
