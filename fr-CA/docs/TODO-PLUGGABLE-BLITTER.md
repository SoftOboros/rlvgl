```markdown
<!--
docs/TODO-PLUGGABLE-BLITTER.md - Epic: Pluggable Rendering/Display Backends (CPU, DMA2D, winit/wgpu).
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Épopée : Moteurs de rendu/affichage enfichables (CPU, DMA2D, winit/wgpu)

**Description** : Introduction d'un trait de stratégie `Blitter` et de multiples implémentations (repli CPU, STM32H7 DMA2D, bureau wgpu). Celles-ci sont reliées sous `platform/` afin que le même code de widget/rendu cible les systèmes embarqués et de bureau. Ajoute LTDC/DSI + OTM8009A (DISCO) et le toucher FT5336. Met à jour le simulateur pour utiliser `winit + wgpu` (fenêtre + GPU) pour la vitesse.
**Résultat** : Chemins de vidage accélérés par le matériel sur H7 ; simulateur à haute fréquence d'images ; tests unifiés.

---

## A) Abstraction de Blitter (plateforme)

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Définir le trait `Blitter` : `caps()`, `fill()`, `blit()`, `blend()`, support PFC | `bitflags` (caps) | Les types de rect + surface vivent dans `platform::blit`. |
| [x] | Ajouter `Surface` (buf/stride/fmt/w,h) + énumération `PixelFmt` | aucune | Inclure ARGB8888, RGB565, L8/A8/A4. |
| [x] | Ajouter `BlitPlanner` pour regrouper les rects sales par image | aucune | Optionnel : fusionner les rects adjacents. |
| [x] | Acheminer via le moteur de rendu → blitter (pas de fuite d'API vers les widgets) | moteur de rendu de plateforme | Le moteur de rendu possède un `&mut dyn Blitter`. |

---

## B) Blitter de repli CPU

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Implémenter `CpuBlitter` (boucles scalaires) | aucune | Référence de correction, utilisée dans les tests. |
| [x] | Chemins rapides pour les formats courants (ARGB8888→RGB565, remplissages) | aucune | Envisager `bytemuck` pour les casts. |
| [x] | Tests unitaires (tampons dorés) | `proptest` optionnel | Réutiliser les mêmes tests sur tous les moteurs. |

---

## C) Blitter STM32H7 DMA2D (“GPU”)

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Créer `Dma2dBlitter` avec accès au registre PAC | `stm32h7` PAC, `cortex-m` | HAL manque de DMA2D complet ; utiliser PAC. |
| [x] | Init : horloge, configuration de la couche avant/arrière, décalage de ligne | PAC | Garder un wrapper sécurisé ; pas de `unsafe` dans l'API. |
| [x] | Implémenter R2M (remplissage) | PAC | Blocage d'abord ; ajouter IRQ plus tard. |
| [x] | Implémenter M2M/PFC (copie + conversion) | PAC | Chemin ARGB8888→RGB565 courant. |
| [x] | Implémenter le mélange M2M (FG sur BG, alpha constant/par pixel) | PAC | Hypothèse alpha direct ; le documenter. |
| [x] | Optionnel : non bloquant avec interruption/achèvement | EXTI/IRQ | Mettre en file d'attente les opérations ; clôturer avant VSYNC. |
| [ ] | Réutiliser les tests CPU pour affirmer des pixels identiques | `std` test via la construction hôte | Utiliser de petites images de test, des recadrages. |

---

## D) Affichage STM32H747I‑DISCO (LTDC/DSI + OTM8009A)

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Démarrage des horloges pour LTDC/DSI (config RCC) | `stm32h7xx-hal` (RCC) | Faire correspondre le timing du panneau. |
| [x] | SDRAM (FMC) si FB en RAM externe | HAL FMC ou PAC | AXI SRAM ok pour les petits tests. |
| [x] | Séquence d'initialisation de l'hôte DSI + OTM8009A (mode vidéo) | PAC | Porter du C BSP ; factoriser `otm8009a.rs`. |
| [x] | Configuration de la couche LTDC (adresse FB, stride, fmt) | PAC | Commencer avec FB RGB565 pour économiser de la RAM. |
| [x] | Rétroéclairage PWM + GPIO de réinitialisation du panneau | HAL TIM/GPIO | Ligne TE optionnelle pour la synchronisation verticale. |
| [x] | Colle `Stm32h747iDiscoDisplay<B: Blitter>` | sections A/C | Composer le blitter sélectionné. |
| [x] | Drapeau de fonctionnalité : `stm32h747i_disco` | Fonctionnalités Cargo | Protéger les dépendances no-std/gestionnaire de panique. |

---

## E) Touche FT5336 (I²C + EXTI)

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Initialisation I²C à 400 kHz | `stm32h7xx-hal` I2C | Utiliser les broches de la carte. |
| [x] | EXTI sur la ligne INT (optionnel) | HAL EXTI | Ou interroger dans `poll()`. |
| [x] | Pilote FT5336 minimal : lire les points | aucun | Convertir en `Event` (vers le bas/déplacer/vers le haut). |
| [x] | Intégration `Stm32h747iDiscoInput` | entrée de plateforme | Configuration de basculement/rotation des coordonnées. |

---

## F) Simulateur de bureau : Moteur **winit + wgpu**

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Remplacer/minimiser l'utilisation de `pixels/minifb` | `winit`, `wgpu` | Fenêtre `winit` + swapchain `wgpu`. |
| [x] | `WgpuBlitter` implémentant `Blitter` | `wgpu` | Utiliser le pass de rendu + les quads texturés ou le calcul. |
| [x] | Télécharger la tuile/rect vers la texture ; blit/blend dans le shader | `wgpu` | Textures mises à jour et mélangées via les pipelines de rendu. |
| [x] | Présenter à la synchronisation verticale ; mapper clavier/souris → `InputDevice` | `winit` | Mise à l'échelle DPI ; swapchain sRGB. |
| [x] | Mode sans tête pour vider les PNG pour CI | `image` | Tests de régression d'image dorée. |

---

## G) Exemple de panneau SPI (ST7789) pour prouver la portabilité

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Pilote `st7789` via `embedded-hal` | `embedded-hal` | Réutiliser `CpuBlitter`. |
| [ ] | Chemin de vidage SPI DMA | HAL DMA | Optionnel : lignes à double tampon. |

---

## H) Intégration & CI

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | Matrice de fonctionnalités Cargo (`cpu`, `dma2d`, `wgpu`) | Cargo | Rendre les moteurs interchangeables. |
| [ ] | Tâches CI : tests hôtes + wgpu hors écran + rapport de taille | GitHub Actions | Garder les vérifications de taille actuelles. |
| [ ] | Exemple : `examples/sim` utilise `wgpu` | F) | Raccourcis clavier : activer le débogage du rect sale. |
| [ ] | Exemple : `examples/STM32H747I-DISCO` utilise DMA2D | C/D/E | Partage le code de l'application avec le simulateur (refactorisation). |

---

## I) Documentation et Différences

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [ ] | `#![doc = include_str!(…)]` pour les API publiques | aucune | Reflète le style du projet. |
| [ ] | Doc développeur : « Choisir un blitter/backend » | aucune | Quand choisir lequel, compromis de mémoire. |
| [ ] | Outil de comparaison d'images (sortie sim vs dorée) | `image`, `assert_cmd` | Delta RGBA seuillé. |

---

## J) Plugins & Widgets – Intégration de Blitter

| Fait | Description | Dépendances | Notes |
|---|---|---|---|
| [x] | Intégrer le rastériseur de texte `fontdue` dans `BlitterRenderer` | `fontdue` | Mettre en cache les glyphes comme `Surface`s ; prendre en charge les chemins CPU/WGPU/DMA2D. |
| [x] | Connecter les décodeurs d'image (`png`, `jpeg`, `gif`, `apng`) pour produire des surfaces de blitter | `png`, `jpeg`, `gif`, `apng` | Décodage vers `Surface` et appel de `blit()`/`blend()` ; gérer les images d'animation. |
| [x] | Rendre `QrWidget` via le pipeline du blitter | `qrcode` | Générer le bitmap QR, télécharger comme `Surface`, éviter les écritures directes dans le framebuffer. |
| [x] | Relier les images `rlottie` aux surfaces du blitter | `rlottie` | Convertir les images vectorielles en `Surface` ; permettre l'accélération GPU. |
| [x] | Traiter les tampons `CanvasWidget` comme des surfaces de blitter | `embedded-canvas` | Vider progressivement les régions sales via le blitter. |
| [x] | Acheminer les widgets de niveau supérieur (IME pinyin, sélecteur de fichiers FATFS, démo NES) via la pile canvas/blitter | `pinyin`, `fatfs-embedded`, `yane` | S'assurer que leurs chemins de rendu restent agnostiques au moteur. |
```
