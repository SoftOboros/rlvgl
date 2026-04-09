```markdown
<!--
README.md - Usage and format notes for the rlvgl-chips-nxp vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-nxp
Paquet : `rlvgl-chips-nxp`

Fournit une base de données de cartes pour les appareils NXP utilisés par `rlvgl-creator`.

## Utilisation

Ce crate s'attend à ce que les fichiers de définition de carte soient extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Lors de la construction, définissez la variable d'environnement `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers extraits :

```sh
RLVGL_CHIP_SRC=build/chipdb/nxp cargo build -p rlvgl-chips-nxp
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs :

- `vendor()` – retourne `"nxp"`.
- `boards()` – liste les cartes prises en charge sous forme d'entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour peupler les listes déroulantes des fournisseurs et des cartes. Les autres crates de fournisseurs suivent la même structure et API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les futures versions pourraient inclure des informations sur le boîtier et les décalages de configuration des broches.

## Fonctionnalités

- Support `serde` optionnel pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes l'exige.
```
