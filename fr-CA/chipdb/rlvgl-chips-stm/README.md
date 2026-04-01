<!--
README.md - Usage and format notes for the rlvgl-chips-stm vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-stm
Paquet : `rlvgl-chips-stm`

Fournit une base de données de cartes pour les appareils STMicroelectronics utilisés par `rlvgl-creator`.

## Utilisation

La caisse publiée intègre une base de données de puces compressée zstd générée par
`tools/build_vendor.sh`. Les consommateurs lient simplement la caisse ; l'archive est
décompressée à l'exécution par `rlvgl-creator`.

Lors de la construction à partir d'un extrait git, exécutez d'abord `tools/build_vendor.sh` pour
produire le fichier `assets/chipdb.bin.zst` utilisé par le script de construction :

```sh
VENDOR_DIR=chips/stm CRATE_DIR=chipdb/rlvgl-chips-stm OUT_DIR=build/chipdb/stm \
    bash tools/build_vendor.sh
```

Si `assets/chipdb.bin.zst` est absent, le script de construction se rabat sur la
variable d'environnement `RLVGL_CHIP_SRC` pour localiser les définitions JSON non compressées.

Voir [assets/README.md](./assets/README.md) pour plus de détails sur l'archive de la base de données compressée.

La bibliothèque expose des fonctions d'aide pour les consommateurs :

- `vendor()` – renvoie `"stm"`.
- `boards()` – liste les cartes prises en charge sous forme d'entrées `BoardInfo`.
- `find(name)` – recherche une carte par son nom exact.

`rlvgl-creator` intègre cette caisse pour remplir les menus déroulants des fournisseurs et des cartes.
Les autres caisses de fournisseurs suivent la même disposition et la même API.

## Format BoardInfo

Chaque `BoardInfo` décrit une carte avec au moins un nom de carte convivial
et la puce associée. Les futures versions pourront inclure des informations sur les paquets et
les décalages de configuration des broches.

## Fonctionnalités

- Prise en charge optionnelle de `serde` pour la sérialisation de la base de données des cartes : activez la
  fonctionnalité `serde` si l'intégration avec des outils externes le nécessite.
```
