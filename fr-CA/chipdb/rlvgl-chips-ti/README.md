```markdown
<!--
README.md - Usage and format notes for the rlvgl-chips-ti vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-ti
Paquet : `rlvgl-chips-ti`

Fournit une base de données de cartes pour les appareils Texas Instruments utilisés par `rlvgl-creator`.

## Utilisation

Ce crate attend des fichiers de définition de carte extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Lors de la construction, définissez la
variable d'environnement `RLVGL_CHIP_SRC` au répertoire contenant ces
fichiers extraits :

```sh
RLVGL_CHIP_SRC=build/chipdb/ti cargo build -p rlvgl-chips-ti
```

La bibliothèque expose des fonctions d'aide pour les consommateurs :

- `vendor()` – retourne `"ti"`.
- `boards()` – liste les cartes prises en charge comme entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour remplir les listes déroulantes de fournisseurs et de cartes.
D'autres crates de fournisseurs suivent la même structure et API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial
et une puce associée. Les versions futures pourraient inclure des informations sur le boîtier et
les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge `serde` optionnelle pour la sérialisation de la base de données de cartes : activez la
  fonctionnalité `serde` si l'intégration avec des outils externes le requiert.
```
