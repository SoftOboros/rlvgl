<!--
examples/sim/README.md - Desktop simulator example.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Démo rlvgl
---
Démontre les widgets de base ainsi que les fonctionnalités des plugins telles que la génération de codes QR
et le décodage d'images PNG/JPEG.

## Utilisation

Lancez le simulateur avec une résolution d'écran personnalisée en utilisant :

```bash
cargo run --bin rlvgl-sim -- --screen=800x480
```

Omettez `--screen` pour utiliser la résolution par défaut de 320x240. Par défaut, le simulateur
utilise le blitter de secours du CPU pour le rendu. Passez `--wgpi` pour activer le blitter accéléré
par wgpu à la place. Fournissez un chemin de fichier comme argument supplémentaire pour
exporter une seule image vers un PNG au lieu de lancer la fenêtre interactive.

Pour les flux de travail de gestion des ressources utilisant `rlvgl-creator`, voir
[`README-CREATOR.md`](../../README-CREATOR.md).

## Limitations

Sur les écrans qui dépassent la taille de texture maximale du GPU, le simulateur
rend sur un framebuffer interne plus petit et redimensionne le résultat pour l'adapter à la
fenêtre. Ce redimensionnement peut introduire un letterboxing ou une netteté réduite sur
les moniteurs à ultra-haute résolution.

## Exigences
La démo rlvgl nécessite libgtk-3-dev et librlottie-dev pour l'affichage et le support de la création Lottie (non implémenté).

voir [Dockerfile](../../Dockerfile) et [setup-ci-env.sh](../../scripts/setup-ci-env.sh) pour comprendre l'ensemble complet des packages utilisés pour l'exécution.

Si non disponible, rlottie peut être construit à partir des sources comme suit :
```bash
# Définir le chemin du préfixe d'installation (modifier si nécessaire)
INSTALL_PREFIX="/opt/rlottie"

# Construire et installer rlottie localement
git clone https://github.com/Samsung/rlottie
cd rlottie && mkdir build && cd build
cmake .. \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DLIB_INSTALL_DIR=lib \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5
make -j"$(nproc)"
make install
cd ../..

# Exporter les variables d'environnement pour les étapes futures
export PKG_CONFIG_PATH="$INSTALL_PREFIX/lib/pkgconfig"
export BINDGEN_EXTRA_CLANG_ARGS="-I$INSTALL_PREFIX/include"

```

---

## Configuration de VS Code

### Extensions
- [CodeLLDB](https://github.com/vadimcn/codelldb)
- [rust-analyzer](https://rust-analyzer.github.io)
- Even Better TOML

### Configuration de lancement
(.vscode/launch.json)[../../../.vscode/launch.json] contient les paramètres d'exécution sous macOS sur x86

```json
{
  "version": "0.2.0",
  "configurations": [

    {
      "name": "Debug sim",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/x86_64-apple-darwin/debug/rlvgl-sim",
      "args": [],
      "cwd": "${workspaceFolder}",
      "cargo": {
        "args": ["build", "--package=rlvgl-sim", "--target=x86_64-apple-darwin"]
      },
      "sourceLanguages": ["rust"]
    },
  ]
}
```

Modifiez la chaîne cible pour votre plateforme hôte.
