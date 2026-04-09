<!--
docs/TODO-PLUGINS.md - rlvgl - TODO du flux de travail des plugins.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl - TODO du flux de travail des plugins

> **Objectif** Suivre le portage incrémentiel des add-ons LVGL basés sur C vers des crates Rust pour `rlvgl`. Les tâches sont ordonnées pour respecter les dépendances techniques afin que chaque couche s'appuie sur la précédente.

---

## 🛠️ Instructions de pré-configuration de Codex

Avant d'aborder les TODOs des plugins, Codex doit configurer l'espace de travail `rlvgl` pour prendre en charge le développement modulaire de plugins à l'aide des fonctionnalités de Cargo.

### 1. Mettre à jour `Cargo.toml` avec les fonctionnalités des plugins

Ajoutez ce qui suit à la section `[features]` :

```toml
[features]
default = []

# Niveau 1
png = ["dep:png"]
jpeg = ["dep:jpeg-decoder"]
gif = ["dep:gif"]
qrcode = ["dep:qrcode"]
fontdue = ["dep:fontdue"]

# Niveau 2
lottie = ["dep:rlottie"]
canvas = ["dep:embedded-canvas"]
pinyin = []
fatfs = ["dep:fatfs-embedded"]
nes = ["dep:yane"]
```

Déclarez également les entrées `[dependencies]` avec `optional = true`, par exemple :

```toml
[dependencies.png]
version = "*"
optional = true
```

### 2. Structure des crates

Assurez-vous que chaque plugin réside dans son propre fichier `src/plugins/<name>.rs` :

```rust
#[cfg(feature = "png")]
pub mod png;
```

Puis dans `lib.rs` :

```rust
#[cfg(feature = "png")]
pub use plugins::png;
```

### 3. Tests

Chaque plugin devrait avoir :

- Des tests unitaires `#[cfg(test)]` dans son propre fichier.
- Des tests d'intégration optionnels sous `tests/plugins_png.rs`, etc.

Utilisez des drapeaux de fonctionnalités dans les tests :

```rust
#[cfg(feature = "png")]
#[test]
fn test_png_decode() { /* … */ }
```

### 4. Fragment de matrice CI

Prend en charge `cargo test --features gif,fontdue`, etc. Exemple de matrice de tâche CI :

```yaml
matrix:
  include:
    - features: "png jpeg gif"
    - features: "qrcode fontdue"
    - features: "lottie canvas"
```

---

## ⬛ Niveau 1 – Pipeline multimédia et texte de base

*Composants fondamentaux nécessaires avant
