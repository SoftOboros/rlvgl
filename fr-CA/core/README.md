```markdown
<!--
core/README.md - Aperçu des abstractions fondamentales de rlvgl.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-core

Paquet: `rlvgl-core`.

Ce "crate" contient les abstractions d'exécution qui sous-tendent chaque widget et
chaque "backend" utilisé dans **rlvgl**.

Éléments actuellement implémentés:

- Le trait `Widget` définissant les rappels de dessin et d'événements
- L'arbre `WidgetNode` pour la composition hiérarchique
- L'énumération `Event` pour l'entrée de base
- Le trait `Renderer` pour le dessin indépendant de la cible
- La structure `Style` avec constructeur pour l'apparence des widgets

Ces API sont à leurs débuts et évolueront à mesure que de nouveaux widgets et "backends" seront mis en service.
```
