<!--
platform/README-VENDOR.md - Policy for vendor-specific platform support.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Politique de soutien des fournisseurs pour rlvgl

Ce document décrit la gestion du soutien spécifique aux fournisseurs dans les crates `rlvgl-platform`.
Il clarifie la distinction entre les contributions de la communauté et les plateformes officiellement prises en charge.

---

## Principes fondamentaux

- La **bibliothèque `rlvgl` de base** est neutre vis-à-vis des fournisseurs et open source.
- Les **crates `rlvgl-platform`** fournissent des couches d'intégration spécifiques aux fournisseurs et aux cartes.
- Le soutien aux plateformes est échelonné pour refléter différents niveaux de maintenance et de garanties.

---

## Niveaux de soutien

### Soutien officiel
- Maintenu directement dans le dépôt `rlvgl`.
- Inclus dans les builds et les tests d'intégration continue (CI).
- Documenté dans les exemples et la galerie officiels.
- Compatibilité garantie avec chaque version de `rlvgl`.
- Nécessite un parrainage du fournisseur ou un accord de partenariat équivalent.

### Soutien communautaire
- Peut être développé et maintenu par des contributeurs de la communauté.
- Accepté dans le dépôt s'il passe une révision de base et compile.
- Construit en CI pour les vérifications de compilation uniquement.
- Non garanti d'être inclus dans la documentation ou les exemples.
- Aucune garantie de compatibilité entre les versions de `rlvgl`.

### Soutien externe
- Développé et maintenu en dehors du dépôt `rlvgl`.
- Peut être lié à partir de la documentation en tant que ressource externe.
- Aucune garantie ni responsabilité de la part des mainteneurs de `rlvgl`.

---

## Participation des fournisseurs

Les fournisseurs intéressés par le **Soutien officiel** doivent fournir:
1. Un parrainage ou un partenariat pour couvrir la maintenance continue.
2. Du matériel de référence (kits d'évaluation, cartes ou modules).
3. La documentation et le matériel de test si nécessaire.

Cela garantit que le matériel des fournisseurs est représenté avec la même stabilité, documentation et qualité que le simulateur et les autres plateformes officiellement prises en charge.

---

## Résumé

- **N'importe qui** peut construire sur `rlvgl` et contribuer du code de plateforme.
- Le **statut officiel** est réservé aux plateformes parrainées par les fournisseurs, avec une couverture CI complète, des exemples et de la documentation.
- Cette politique maintient le cœur ouvert tout en assurant un soutien durable pour les écosystèmes des fournisseurs.
