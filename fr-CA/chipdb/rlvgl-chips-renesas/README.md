<!--
README.md - Usage and format notes for the rlvgl-chips-renesas vendor crate.
-->
<p align="centre">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-renesas
Paquet : `rlvgl-chips-renesas`

Fournit une base de données de cartes pour les appareils Renesas utilisés par `rlvgl-creator`.

## Utilisation

Ce crate s'attend à ce que les fichiers de définition de carte soient extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Pendant la compilation, définissez la variable d'environnement `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers extraits :

```sh
RLVGL_CHIP_SRC=build/chipdb/renesas cargo build -p rlvgl-chips-renesas
```

La bibliothèque expose des fonctions d'aide pour les consommateurs :

- `vendor()` – renvoie `"renesas"`.
- `boards()` – liste les cartes prises en charge sous forme d'entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour peupler les menus déroulants des fournisseurs et des cartes. Les autres crates de fournisseurs suivent la même structure et la même API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les versions futures pourront inclure des informations sur les boîtiers et des décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes le nécessite.
```
