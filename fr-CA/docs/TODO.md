```markdown
<!--
docs/TODO.md - Projet TODO.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Projet TODO

Ce document répertorie les grandes lignes des travaux et des tâches de développement de rlvgl.

## 0 Initialisation du dépôt et du conteneur
 - [x] Créer le squelette du monorepo rlvgl (`core/`, `widgets/`, `platform/`, `lvgl/` submodule)
- [x] Finaliser les ajustements du Dockerfile (LLVM/clang, rustup, arm-gcc, bindgen, SDL)
- [x] Ajouter `.cargo/config.toml` avec le profil intégré et le triple cible
- [x] Ébauche de GitHub Actions / GitLab CI (build, tests unitaires, rapport de taille)

## 1 Squelette du runtime principal
- [x] Trait de widget (limites, hooks de cycle de vie, signatures de dessin/événement)
- [x] Arbre WidgetNode (Rc<RefCell<_>>, ou index de dalle)
- [x] Enum d'événement + répartiteur (ordre de bubbling/capture)
- [x] Structure de style + modèle de constructeur
- [x] Trait Minimal Renderer (agnostique de la cible)

## 2 Outils de build et CI
- [x] Ajouter les paramètres `profile.release` (lto=true, opt-level=z, etc.)
- [x] Script de vérification de la taille (`arm-none-eabi-size`)
- [x] Gating Clippy + Rust-fmt
- [x] Modèle de hook de pré-commit
- [x] Script d'initialisation de l'environnement CI + intégration du workflow
## 3 Couche HAL d'affichage et d'entrée

- [x] Définir le trait `DisplayDriver` (`flush(Rect, &[Color])`)
- [x] Définir le trait `InputDevice` (`poll() -> Option<InputEvent>`)
- [x] Fournir un pilote de stub factice pour les tests sans interface graphique
- [x] Pilote d'exemple basé sur SPI (`st7789`) utilisant `embedded-hal`

## 4 Traductions de widgets de niveau 1
- [x] Étiquette (texte uniquement)
- [x] Bouton (étend les propriétés de l'étiquette)
- [x] Conteneur (mise en page de type flex)
- [x] Tests unitaires + captures d'écran dorées via un pilote factice

## 5 Backend de simulation
- [x] Ajouter un drapeau de fonctionnalité de simulateur (`std`, pixels)
- [x] Mapper `DisplayDriver` à la fenêtre du bureau
- [x] Connecter le clavier/la souris au trait `InputDevice`
- [ ] Étape CI : exécuter l'exemple, vider le PNG pour le diff

## 6 Traductions de widgets de niveau 2
- [x] Case à cocher
- [x] Curseur
- [x] Arc / Barre de progression
- [x] Liste
- [x] Image (backend embedded-graphics)

## 7 Thèmes et animations
 - [x] Trait de thème global (schéma de couleurs, cascade de styles)
 - [x] Gestionnaire d'animations (hook `tick()` → interpolation de style/position)
 - [x] Porter le fondu/glissement de base de LVGL comme preuve

## 8 Docs et exemples
- [x] Documenter automatiquement chaque API publique avec `#![doc = include_str!(…)]`
- [ ] Générer un site mdBook ou Docusaurus
- [ ] Galerie d'exemples : intégrer des GIF générés par le simulateur

## 9 Tests avancés et harnais de régression
- [ ] Tirer la démo C de LVGL → rendre en bitmap (sim)
- [ ] Rendre la même interface utilisateur dans rlvgl, image-diff en CI
- [ ] Fuzz les événements (taps rapides, glissements) pour détecter les paniques d'emprunt/de durée de vie

## 10 Exemples de simulateur
- [x] Mettre à jour samples/sim (rlvds-sim) pour exécuter rlvds dans une fenêtre en utilisant std et la fonctionnalité "simulateur"
- [x] Démontrer les fonctionnalités de base / widget dans examples/sim (utiliser des assets de placeholder) en utilisant une hiérarchie correspondant au paquet supérieur
- [x] Démontrer les fonctionnalités du plugin (utiliser des assets de placeholder) en utilisant une hiérarchie correspondant au paquet supérieur ; ajouter un bouton `plugins` à la démo pour lancer des éléments facultatifs et définir toutes les options de build
```
