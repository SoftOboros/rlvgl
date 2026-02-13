```markdown
<!--
docs/TODO-CREATOR-UI-FULL.md - rlvgl-creator – TODO de la fonctionnalité complète de l'interface utilisateur.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator – TODO de la fonctionnalité complète de l'interface utilisateur

Ce fichier suit le travail restant pour mettre à jour l'interface utilisateur de bureau de `rlvgl-creator` afin qu'elle corresponde à son interface de ligne de commande et offre une gestion complète des actifs.

## Surface de commande
- [x] Ajouter un menu de commandes global listant toutes les actions CLI avec des gestionnaires dédiés et un retour d'information par toast.
- [x] Exposer la commande `init` via une boîte de dialogue pour créer des racines d'actifs et un manifeste par défaut.
- [x] Ajouter une action `scan` avec un sélecteur de répertoire et un rafraîchissement du manifeste.
- [x] Ajouter une commande `check` avec un sélecteur de racine et une option de correction facultative.
- [x] Implémenter l'interface utilisateur de l'opération `vendor` pour copier des actifs et générer des modules d'intégration.
- [x] Exposer la commande `convert` avec un sélecteur de racine et un drapeau de forçage.
- [x] Ajouter la commande `preview` pour régénérer les vignettes à la demande.
- [x] Fournir une boîte de dialogue d'enregistrement `add-target` pour le nom et le répertoire du fournisseur.
- [x] Exposer la commande `sync` avec un répertoire de sortie et une option de simulation.
- [x] Implémenter l'interface utilisateur de `scaffold` pour générer une caisse d'actifs en mode double.

## Outils de conversion et d'exportation
- [x] Étendre le constructeur APNG pour permettre de définir le délai et le nombre de boucles; le répertoire des images,
      le chemin de sortie, le délai et les boucles sont configurables.
- [x] Ajouter une option d'exportation de schéma de manifeste exécutant `schema::run()`.
- [x] Exposer l'interface utilisateur de l'emballeur de polices pour la taille et le jeu de caractères; le chemin racine,
      la taille et les glyphes sont configurables.
- [x] Intégrer l'importateur Lottie (chemins CLI intégrés et externes).
 - [x] Ajouter une boîte de dialogue de rendu SVG avec une liste de DPI configurable et un seuil; les deux paramètres sont configurables par l'utilisateur avant le rendu.

## Navigateur d'actifs
- [x] Remplacer la liste plate par une arborescence hiérarchique reflétant `assets/raw`; les répertoires reflètent la hiérarchie sur disque.
- [x] Ajouter l'action "Ajouter un actif" à l'aide d'une boîte de dialogue de fichier pour copier des fichiers et mettre à jour le manifeste
      (aucun flux de travail d'importation pour l'instant).
- [x] Permettre la suppression des actifs sélectionnés avec une boîte de dialogue de confirmation et une persistance du manifeste.
- [x] Afficher le contenu complet de l'archive avec un rafraîchissement automatique lorsque des fichiers sont ajoutés en externe.

## Améliorations du flux de travail et de l'expérience utilisateur
- [x] Regrouper les commandes associées dans des menus de niveau supérieur (Actifs, Construire, Déployer) pour remplacer l'encombrement des boutons.
  - **Actifs**: init, scan, check, vendor, convert, preview.
  - **Construire**: add-target, scaffold, exportation de schéma, pack de polices, rendu SVG.
  - **Déployer**: sync, préréglages d'automatisation.
- [x] Introduire des assistants qui guident l'utilisateur à travers des séquences courantes comme scan → convert → preview avec indication de la progression.
  - Étapes de l'assistant: sélectionner la racine → scanner les actifs → convertir les formats → prévisualiser les résultats → résumé.
- [x] Prendre en charge les préréglages d'automatisation ou les macros pour enchaîner les commandes et rejouer les flux de travail fréquents.
  - Permettre de sauvegarder les séquences de commandes en tant que préréglages nommés dans un fichier JSON et d'exposer une boîte de dialogue "Exécuter le préréglage".
```
