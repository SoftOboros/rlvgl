```markdown
<!--
docs/TODO-DECOMP.md - Work plan for rlvgl-decomp (palette + RLE codec)
-->

<p align="centre">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Tâches à faire rlvgl-decomp

Ce document répertorie les tâches restantes pour la crate `rlvgl-decomp` : un
format d'image compressé compact avec palette + RLE, incluant des répétitions
courtes/longues et des échappements de pixels en ligne. La crate cible
no_std avec `alloc` et opère sur des cadres RGBA, convertissant vers/depuis
RGB565 en interne pour correspondre aux pipelines d'affichage embarqués.

## Objectifs

- Fournir un format d'image compressé stable et documenté pour les ressources rlvgl.
- Décoder rapidement en RGBA sur les cibles embarquées avec un minimum de mémoire.
- Encoder efficacement les entrées RGBA pour les outils de création; prendre en charge les cadres uniques et les séquences.
- Rester no_std; supporter `alloc` uniquement.

## Format (Récapitulatif)

- Palette : jusqu'à `MAX_PALETTE` entrées RGB565 (par défaut 192) dérivées de l'histogramme du cadre.
- Octets du flux :
  - `0xFF` (simple en ligne) : 2 octets suivants RGB565; émettre une fois.
  - `0xFE` (double en ligne) : 2 octets suivants RGB565; émettre deux fois.
  - `0xFD` (répétition longue) : répéter la couleur de l'index de palette le plus récent pour `61 + next_byte` pixels (jusqu'à 316).
  - `0..(palette_len-1)` : index de palette; émettre une fois; définit l'index récent.
  - `(palette_len)..(palette_len+60)` : répétition courte; émettre l'index récent `(byte - palette_len + 1)` fois.

Notes :
- L'encodeur limite la palette pour que les codes de répétition courte n'entrent jamais en collision avec `0xFD`..`0xFF`.
- Le décodeur valide les longueurs et les limites de la palette; retourne `Error::Truncated`/`SizeMismatch`.

## Éléments de travail

- Polissage du décodeur
  - [ ] Ajouter une API de décodage en continu (ligne par ligne) pour limiter la mémoire de pointe.
  - [ ] Exposer l'option de sortie RGB565 pour éviter l'expansion RGBA sur l'embarqué.
  - [ ] Valider les débordements/cas limites (palette vide, images de taille nulle).

- Améliorations de l'encodeur
  - [ ] Stratégies de sélection de palette : median-cut / repli k-means pour améliorer la qualité.
  - [ ] Détection de séquences sur plusieurs lignes (permettre aux séquences de continuer au-delà des limites de la ligne de balayage facultativement).
  - [ ] Stratégie mixte pour les couleurs non-palette : petite palette locale vs heuristique de pixels en ligne.
  - [ ] Ajuster les seuils de répétition longue/courte; diviser automatiquement les très longues séquences.
  - [ ] Ajouter un encodage conscient des régions (tuiles) pour une meilleure réutilisation locale sur les images complexes.

- Compression basée sur un dictionnaire (prochaine phase)
  - [ ] Construire un dictionnaire de premier ordre : tuples fréquents de 2 à 4 pixels (RGB565) → codes.
  - [ ] Étendre le flux avec une section de dictionnaire et des clés d'échappement (réserver en dessous de `0xF0`).
  - [ ] Heuristique de l'encodeur pour choisir RLE vs correspondances de dictionnaire par segment.
  - [ ] Indicateur de compatibilité descendante dans l'en-tête pour signaler la présence du dictionnaire.

- Conteneur/en-tête
  - [ ] Définir un en-tête minimal : magic, version, largeur, hauteur, drapeaux de format, longueur de palette.
  - [ ] Regrouper la palette + le flux (+ dictionnaire optionnel) en un seul blob.
  - [ ] En-tête de taille fixe, petit-boutiste, pour une analyse facile.

- Intégration du créateur
  - [ ] Ajouter une sous-commande CLI rlvgl-creator : `creator assets encode --format rle`.
  - [ ] Supporter les séquences (APNG/Lottie) : émettre des cadres numérotés ou un simple conteneur multi-cadres.
  - [ ] Option pour la cible RGB565 directement pour sauter l'aller-retour RGBA.

- Tests et CI
  - [ ] Tests unitaires : aller-retour de petits motifs (solide, damier, dégradés, longues séquences).
  - [ ] Décodage de flux flou (longueurs, clés, limites de palette) sous `std`.
  - [ ] Échantillons de référence sous `tests/` avec des images de fixture.
  - [ ] Benchmarks (hôte) : débit d'encodage/décodage et taille vs PNG (vérification).

- Performance et mémoire
  - [ ] Éviter les allocations intermédiaires pendant le décodage (fournir une API de tampon appartenant à l'appelant).
  - [ ] Chemin SIMD optionnel pour les conversions RGBA<->RGB565 sur les builds hôtes.
  - [ ] Encodeur basé sur un itérateur pour réduire les histogrammes temporaires pour les grands cadres.

- Documentation
  - [ ] Documentation de l'API publique avec des exemples.
  - [ ] Page de spécification du format (stable), inclure les diagrammes d'octets.
  - [ ] Documentation d'utilisation du créateur et dépannage (bandes de couleurs, taille de la palette, seuils).

## Agréables à avoir

- [ ] Boutons de quantification de palette avec perte (options de dithering, limite de taille de palette).
- [ ] Encodage par tuiles/bandes pour accélérer les redessins partiels.
- [ ] Encodage delta par cadre optionnel pour les séquences.

## Acceptation

- Décodeur : passe les tests unitaires et décode les ressources d'échantillon sans erreur.
- Encodeur : produit des blobs plus petits que RGBA sur les ressources d'interface utilisateur typiques; taille de palette configurable.
- Créateur : peut ingérer PNG/APNG/Lottie et émettre le conteneur; documentation de base dans `docs/`.
- CI : se construit sur stable; `cargo fmt`, `clippy` propre; vérificateur de liens OK.
```
