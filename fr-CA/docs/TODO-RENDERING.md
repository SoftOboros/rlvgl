<!--
docs/TODO-RENDERING.md - rlvgl – Liste de tâches pour le flux de travail de rendu.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl – Liste de tâches pour le flux de travail de rendu

Ce fichier suit les tâches visant à améliorer le pipeline de rendu afin que les widgets puissent dessiner plusieurs couches avec fusion alpha. Toutes les valeurs de couleur doivent contenir des données RGBA de la source à l'affichage ; si chaque couche est transparente à un pixel, la couleur de la couche la plus basse reste visible.

## Rendu compatible Alpha
- [x] Étendre `Colour` de RGB à RGBA afin que les widgets puissent exprimer l'opacité.
- [ ] Ajouter des méthodes de fusion conscientes de l'alpha à `Renderer` et mettre à jour les backends.
- [ ] Définir la sémantique de superposition/composition des widgets afin que les couches supérieures se fondent sur les couches inférieures.
- [ ] Propager les couleurs RGBA à travers les API de style et de remplissage à travers les widgets et les backends.

---

*Dernière mise à jour : 2025-08-06*
