<!--
README.md - Usage and format notes for the rlvgl-chips-nrf vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-nrf
Paquet : `rlvgl-chips-nrf`

Fournit une base de données de cartes pour les appareils Nordic Semiconductor utilisés par `rlvgl-creator`.

## Utilisation

Ce crate s'attend à ce que les fichiers de définition de carte soient extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Pendant la construction, définissez la variable d'environnement `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers extraits :

```sh
RLVGL_CHIP_SRC=build/chipdb/nrf cargo build -p rlvgl-chips-nrf
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs :

- `vendor()` – retourne `"nrf"`.
- `boards()` – liste les cartes prises en charge comme des entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre ce crate pour peupler les listes déroulantes de fournisseurs et de cartes. D'autres crates de fournisseurs suivent la même structure et API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les versions futures pourront inclure des informations sur le package et les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes le nécessite.
