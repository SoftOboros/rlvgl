```
<!--
README.md - Usage and format notes for the rlvgl-chips-silabs vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-silabs
Paquet: `rlvgl-chips-silabs`

Fournit une base de données de cartes pour les appareils Silicon Labs utilisés par `rlvgl-creator`.

## Utilisation

Cette caisse s'attend à ce que les fichiers de définition de carte soient extraits par [`tools/st_extract_af.py`](../../tools/st_extract_af.py). Lors de la construction, définissez la variable d'environnement `RLVGL_CHIP_SRC` sur le répertoire contenant ces fichiers extraits:

```sh
RLVGL_CHIP_SRC=build/chipdb/silabs cargo build -p rlvgl-chips-silabs
```

La bibliothèque expose des fonctions d'assistance pour les consommateurs:

- `vendor()` – renvoie `"silabs"`.
- `boards()` – répertorie les cartes prises en charge sous forme d'entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre cette caisse pour remplir les listes déroulantes de fournisseurs et de cartes. D'autres caisses de fournisseurs suivent la même disposition et la même API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial et la puce associée. Les versions futures pourront inclure des informations sur le boîtier et les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données de cartes : activez la fonctionnalité `serde` si l'intégration avec des outils externes l'exige.
```
