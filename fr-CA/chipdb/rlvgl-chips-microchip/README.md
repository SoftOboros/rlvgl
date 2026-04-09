```
<!--
README.md - Usage and format notes for the rlvgl-chips-microchip vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-microchip
Paquet: `rlvgl-chips-microchip`

Fournit une base de données de cartes pour les dispositifs Microchip utilisés par `rlvgl-creator`.

## Utilisation

Cette caisse s'attend à des fichiers de définition de carte extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Lors de la compilation, définissez la variable d'environnement `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers extraits:

```sh
RLVGL_CHIP_SRC=build/chipdb/microchip cargo build -p rlvgl-chips-microchip
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs:

- `vendor()` – retourne `"microchip"`.
- `boards()` – liste les cartes prises en charge comme entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre cette caisse pour remplir les listes déroulantes des fournisseurs et des cartes. Les autres caisses de fournisseurs suivent la même disposition et la même API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les futures versions pourront inclure des informations de boîtier et des décalages de configuration de broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes: activez la fonctionnalité `serde` si l'intégration avec des outils externes l'exige.
```
