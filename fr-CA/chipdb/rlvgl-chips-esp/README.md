```markdown
<!--
README.md - Usage and format notes for the rlvgl-chips-esp vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-esp
Paquet : `rlvgl-chips-esp`

Fournit une base de données de cartes pour les appareils Espressif utilisée par `rlvgl-creator`.

## Utilisation

Ce crate s'attend à des fichiers de définition de carte extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Pendant la construction, définissez la variable d'environnement `RLVGL_CHIP_SRC` au répertoire contenant ces fichiers extraits :

```sh
RLVGL_CHIP_SRC=build/chipdb/esp cargo build -p rlvgl-chips-esp
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs :

- `vendor()` – retourne `"esp"`.
- `boards()` – liste les cartes prises en charge en tant qu'entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour peupler les menus déroulants des fournisseurs et des cartes.
D'autres crates de fournisseurs suivent la même structure et API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les futures versions pourront inclure des informations sur les paquets et les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes le requiert.
```
