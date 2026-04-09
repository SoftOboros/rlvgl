```markdown
<!--
ui/README.md - Documentation unifiée pour rlvgl-ui.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-ui ─ Documentation unifiée
Paquet : `rlvgl-ui`
*(Copiez-collez ce fichier unique dans `ui/README.md` ou où vous le souhaitez.)*

---

## 1 ▸ Aperçu

**rlvgl-ui** est une caisse de seconde couche qui repose sur les liaisons `rlvgl` de bas niveau
(et donc le moteur **LVGL** basé sur C).

Il offre une **API inspirée de Chakra / React** — thèmes, jetons, styles fluides, et
composants composables — sans sacrifier la vitesse brute et l'empreinte minuscule qui
font de LVGL le GUI de choix pour les microcontrôleurs et les petits MPU.

┌─────────────┐ Votre application (Button::new().on_click(save))
├─────────────┤ rlvgl-ui (Thème, Style, VStack …)
├─────────────┤ rlvgl (wrappers Rust sûrs pour LVGL)
├─────────────┤ lvgl-sys (FFI C brut)
└─────────────┘

### Pourquoi une autre couche?

| Avantage      | Détails                                                             |
|---------------|---------------------------------------------------------------------|
| Familiarité   | Les développeurs React / Chakra se sentent chez eux.               |
| Productivité  | `Style::new().bg(...)` remplace des dizaines d'appels `lv_obj_set_style_*()`. |
| Interopérable | 100 % compatible avec les thèmes et styles LVGL ; C et Rust peuvent se mélanger. |
| Empreinte Minuscuse | Ajoute de l'ergonomie, **pas** un moteur JS ou un GC.       |

---

## 2 ▸ Démarrage rapide

#### `Cargo.toml`
```toml
[dependencies]
rlvgl     = "0.2"
rlvgl-ui  = { path = "ui" }   # chemin local pendant le piratage
```

Code minimal

```rust
use rlvgl_ui::{Theme, Style, Button, VStack};

fn ui() {
    let theme = Theme::material_light();
    theme.apply_global();               // pousse les jetons vers LVGL

    VStack::new()
        .spacing(theme.spacing.md)
        .child(
            Button::new("Sauvegarder")
                .icon("save")           // police d'icônes intégrée
                .style(
                    Style::new()
                        .bg(theme.colors.primary)
                        .radius(theme.radii.md)
                )
                .on_click(|| { println!("Sauvegardé !"); })
        )
        .mount(lv_scr_act());
}
```

Construire et exécuter

Simulateur de bureau :

```
cargo run --example demo -p rlvgl-ui
```

Cible MCU (par exemple STM32-H723) :

```
cargo build --release --target thumbv7em-none-eabihf -p rlvgl-ui
```

## 3 ▸ Feuille de route / À faire

### Phase 1 · Style et thème compatibles LVGL
- [x] Auditer les API de style LVGL
- [x] StyleBuilder (rembourrage, marge, arrière-plan, texte, bordure, rayon)
- [x] Aides pour les parties/états
- [x] Structures de jetons (espacement, couleurs, rayons, polices)
- [x] Pont de thème hérité (matériel, mono)
- [x] Démo + tests CI
- [x] Étiqueter v0.1.0

### Phase 2 · Noyau rlvgl-ui
- [x] Aides à la mise en page (HStack, VStack, Grid, Box)
- [x] Hooks d'événements (on_click, on_change)
- [x] Intégration de la police d'icônes
- [x] Macro DSL optionnelle (view!) derrière un drapeau de fonctionnalité
- [x] Publier rlvgl-ui v0.1

### Phase 3 · Composants inspirés de Chakra
 - [x] Bouton / IconButton
 - [x] Texte / Titre
 - [x] Entrée / Zone de texte
 - [x] Case à cocher
 - [x] Interrupteur
 - [x] Radio
 - [x] Badge / Étiquette / Alerte
 - [x] Modale / Tiroir / Toast
 - [ ] Application de démonstration de style Storybook
 - [ ] Publier la version 0.2 et rédiger la version 1.0

## 4 ▸ Spécification de l'agent (température = 0 %)

Instructions déterministes pour tout LLM ou outil générant ou refactorisant du code
dans ui/.
Modifier les fichiers uniquement dans ui/ sauf instruction explicite.
Préserver les signatures d'API publiques à moins que le numéro de version ne soit modifié.
Tous les styles générés doivent compiler en des données `lv_style_t` valides.
Les espaces de noms des jetons sont fixes : espacement, couleurs, rayons, polices.
Longueur maximale de la ligne source : 100 colonnes.
En-tête de licence MIT : MIT / Apache-2.0.

## 5 ▸ Exemple (ui/examples/demo.rs)

```rust
use rlvgl_ui::{Theme, Style, Button, VStack};

pub fn build() {
    let theme = Theme::material_light();
    theme.apply_global();

    VStack::new()
        .spacing(theme.spacing.md)
        .child(
            Button::new("Sauvegarder")
                .icon("save")
                .style(
                    Style::new()
                        .bg(theme.colors.primary)
                        .radius(theme.radii.md)
                )
                .on_click(|| { println!("Sauvegardé !"); })
        )
        .mount(lv_scr_act());
}
```

## 6 ▸ Licence

Sous licence MIT : MIT.

« Les petits écrans méritent aussi une excellente UX. »
```
