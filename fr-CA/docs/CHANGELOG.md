```markdown
<!--
CHANGELOG.md - Notes sur les versions de la base de données des puces et des cartes.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Journal des modifications

## Non publié
- DISCO : Ajout de l'adaptateur FATFS `no_std` (`platform::sd_fatfs_adapter`) et câblage d'exemple
  facultatif pour monter et lister `/assets` sur STM32H747I-DISCO (`fatfs_nostd` +
  `sd_assets_demo`).
- Documentation DISCO : Traitement du script de liaison terminé, initialisation I2C tactile terminée, ajout
  de notes sur la rampe de rétroéclairage, liste de contrôle de la mise en service SDMMC et section de dépannage.
- README de l'exemple : Clarification des drapeaux de construction et des indicateurs à l'écran pour le succès/l'échec du montage SD.
- Crates de fournisseurs initiales pour les cartes STM, Nordic, Espressif, NXP, Silicon Labs, Microchip, Renesas, Texas Instruments et RP2040.
- Ajout de `tools/bump_vendor_versions.py` pour augmenter les versions des crates après la régénération des données de broches.
- Intégration du créateur documentée avec les crates de fournisseurs afin que les sélections de cartes reflètent les bases de données regroupées.
- Ajout de `scripts/gen_ioc_bsps.sh` pour convertir par lots des fichiers CubeMX `.ioc` à l'aide de `rlvgl-creator`.
- `rlvgl-creator` peut maintenant charger des définitions MCU canoniques ainsi que des superpositions de cartes à partir des archives des fournisseurs.
- Ajout de `rlvgl-creator board from-ioc` pour convertir les projets CubeMX des utilisateurs en superpositions de cartes.
- Ajout de l'indicateur `--allow-reserved` à `rlvgl-creator bsp from-ioc` pour autoriser les broches SWD `PA13`/`PA14`.
```
