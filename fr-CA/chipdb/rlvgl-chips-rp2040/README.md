```markdown
<!--
README.md - Notes sur l'utilisation et le format du crate du fournisseur rlvgl-chips-rp2040.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-rp2040
Paquet: `rlvgl-chips-rp2040`

Fournit une base de données de cartes pour les dispositifs RP2040 génériques utilisés par `rlvgl-creator`.

## Utilisation

Ce crate s'attend à ce que les fichiers de définition de carte soient extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Lors de la construction, définissez la variable d'environnement `RLVGL_CHIP_SRC` au répertoire contenant ces fichiers extraits:

```sh
RLVGL_CHIP_SRC=build/chipdb/rp2040 cargo build -p rlvgl-chips-rp2040
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs:

- `vendor()` – retourne `"rp2040"`.
- `boards()` – liste les cartes prises en charge comme entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour peupler les menus déroulants du fournisseur et de la carte. Les autres crates du fournisseur suivent la même disposition et API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les futures versions pourraient inclure des informations sur le paquet et les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes l'exige.
```
