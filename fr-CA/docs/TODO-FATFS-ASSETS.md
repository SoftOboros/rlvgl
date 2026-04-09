```markdown
<!--
docs/TODO-FATFS-ASSETS.md - TODO – Chargement d'actifs basé sur FATFS pour rlvgl (fonctionnalité de base optionnelle).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# À FAIRE – Chargement d'actifs basé sur FATFS pour rlvgl (fonctionnalité de base optionnelle)

> **Épopée :** Ajouter le chargement d'actifs basé sur un système de fichiers optionnel à rlvgl en utilisant une implémentation FAT portable. Le cœur expose une API `AssetSource` petite et stable ; les crates de plate-forme fournissent des pilotes de périphérique de bloc (carte SD sur H747I-DISCO) ou un bouchon de simulateur. Lorsqu'elle est désactivée, le cœur supporte toujours les actifs intégrés.

---

## Objectifs et non-objectifs

- **Objectifs**
  - Fonctionnalité de base `` optionnelle permettant les actifs basés sur FATFS.
  - Liaison de plate-forme via un trait `BlockDevice` implémenté par chaque cible (SD sur DISCO ; image basée sur fichier sur simulateur).
  - Zéro `std` dans le cœur ; `std` uniquement dans le backend du simulateur.
  - Lecture seule v0 (monter, lister, ouvrir, lire). Écriture/vidange sont pour l'avenir.
  - Gestion sécurisée du DMA et du D-Cache sur H7 pour SDMMC.
- **Non-objectifs (v0)**
  - Pas de journalisation ou de systèmes de fichiers exotiques.
  - Pas d'outils de partitionnement dynamique.

---

## Fonctionnalités et disposition des Crates

| ✓   | Description                                       | Dépendances            | Notes                                   |
| --- | ------------------------------------------------- | ---------------------- | --------------------------------------- |
| [x] | Ajouter la fonctionnalité `fs` à `rlvgl/core`       | `alloc`                | Tout le code FS derrière un drapeau de fonctionnalité |
| [x] | Traits FS (`BlockDevice`, `FsError`) dans le cœur | —                     | Déplacé d'un crate autonome            |
| [x] | Nouveau crate : `rlvgl-fs-sim` (std)              | `fatfs`, `std`         | Simulateur : périphérique de bloc basé sur fichier |
| [x] | Module de plate-forme : `platform/stm32h747i_disco_sd` | HAL + DMA              | SDMMC + DMA + maintenance du cache     |

> **Choix de l'implémentation FAT :** Préférer le crate Rust `fatfs` en mode `no_std` pour une API cohérente sur toutes les cibles. `embedded-sdmmc` est une alternative ; garder l'abstraction mince pour que l'un ou l'autre puisse être inséré plus tard.

---

## API publique (côté cœur)

**Dans **``

```rust
/// Secteurs logiques de 512 octets recommandés ; exposer la taille réelle via `block_size()`.
pub trait BlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError>;
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError>; // v1: peut être bouchonné pour RO
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn flush(&mut self) -> Result<(), FsError>;
}

/// Handle de système de fichiers (volume FAT) construit sur un BlockDevice.
pub struct FatVolume<'a, B: BlockDevice> { /* ... */ }

pub trait AssetSource {
    /// Ouvre un actif par chemin logique, ex: "fonts/regular.bin".
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, FsError>;
    fn exists(&self, path: &str) -> bool;
    fn list(&self, dir: &str) -> Result<AssetIter, FsError>;
}

pub trait AssetRead {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, FsError>;
    fn len(&self) -> usize;
    fn seek(&mut self, pos: u64) -> Result<u64, FsError>;
}
```

**Dans **``** (derrière **``**)**

```rust
pub struct AssetManager<S: AssetSource> { /* ... */ }
impl<S: AssetSource> AssetManager<S> {
    pub fn load_font(&self, path: &str) -> Result<Font, AssetError>;
    pub fn load_image(&self, path: &str) -> Result<Image, AssetError>;
    // helper générique
    pub fn open(&self, path: &str) -> Result<Box<dyn AssetRead + '_>, AssetError>;
}
```

---

## Simulateur (std) – Image disque de fichier unique

| ✓   | Description                          | Dépendances         | Notes                                                |
| --- | ------------------------------------ | ------------------- | ---------------------------------------------------- |
| [x] | Implémenter `SimBlockDevice`         | `std::fs::File`     | Un gros fichier **image disque**, pré-dimensionné (ex: 32MB) |
| [x] | Mappage mémoire optionnel pour la vitesse | `memmap2` (fonctionnalité) | Repli sur pread/pwrite si non disponible             |
| [x] | Outil : créer/peupler l'image        | CLI Rust            | `mkfatimg --size 32M --from ./assets/`               |
| [ ] | Monter et test rapide                | rlvgl sim           | Lire un PNG/police, rendre une étiquette            |

**Justification :** Garder la logique FAT intacte en laissant FATFS gérer la disposition sur disque. Le simulateur fournit juste des lectures/écritures de secteur dans un seul fichier hôte.

---

## Pilote de carte SD STM32H747I‑DISCO (SDMMC + DMA)
```
