```markdown
<!--
platform/README.md - Traits et utilitaires pour l'intégration de matériel et de simulateurs.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-platform
Paquet : `rlvgl-platform`

Traits et types utilitaires pour connecter rlvgl à du matériel réel ou à des simulateurs.

Se combine avec les caisses [core](../core/README.md) et
[widgets](../widgets/README.md).

Consultez [README-VENDOR.md](./README-VENDOR.md) pour la politique de support des fournisseurs.

Éléments actuellement fournis :

- Trait `DisplayDriver` pour pousser les données de pixels vers un framebuffer ou un écran LCD
- Trait `InputDevice` pour lire les événements de pointeur ou de touche
- Implémentations factices utilisées pour les tests sans interface graphique

## Backend stm32h747i_disco

La fonctionnalité optionnelle `stm32h747i_disco` active les pilotes d'affichage et de tactile de remplacement
pour l'écran MIPI-DSI et le contrôleur capacitif FT5336 de la carte STM32H747I-DISCO.
Ces stubs établissent la structure du module pour une future intégration matérielle.
```
