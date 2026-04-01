```markdown
<!--
docs/STM32H747I-DISCO.md - STM32H747I-DISCO Notes sur le matériel.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# STM32H747I-DISCO Notes sur le matériel

Ce document contient les mappages de broches et les détails de configuration des périphériques pour l'utilisation de la carte STM32H747I-DISCO avec rlvgl.

## Écran

- TFT 4" 800×480 piloté par l'hôte DSI en mode vidéo
- Contrôleur OTM8009A configuré pour les pixels RGB888 et l'orientation paysage
- `BSP_LCD_Init()` connecte les horloges, LTDC et DSI pour activer le panneau

## Tactile

- Contrôleur capacitif FT5336 sur I2C4 à l'adresse 7 bits 0x38 (8 bits 0x70)
- I2C4 SCL: PD12, SDA: PD13 (AF4), interruption: PK7
- Fréquence de bus recommandée de 400 kHz (l'assistant HAL configure ceci); prend en charge deux
  points de contact simultanés

## Carte SD

Le logement microSD embarqué est connecté au périphérique SDMMC1 en mode 4 bits
de large.

### Affectations de broches CubeMX

| Broche | Fonction     | Fonction alternative |
| ---- | ------------ | ------------------ |
| PC8  | SDMMC1_D0    | AF12               |
| PC9  | SDMMC1_D1    | AF12               |
| PC10 | SDMMC1_D2    | AF12               |
| PC11 | SDMMC1_D3    | AF12               |
| PC12 | SDMMC1_CK    | AF12               |
| PD2  | SDMMC1_CMD   | AF12               |

Activez les horloges GPIOC et GPIOD et configurez toutes les broches en très haute vitesse avec
des pull-ups internes. SDMMC1 devrait alimenter son horloge de noyau à partir du PLL2 avec une
sortie de 200 MHz. Les flux DMA2 3 (RX) et 6 (TX) utilisant le canal 4 sont
recommandés pour les transferts de données.

## Rétroéclairage et réinitialisation

- Le rétroéclairage utilise TIM8 (par exemple, CH1/CH2) sur `PJ6` (complémentaire optionnel `CH2N`
  sur `PJ7`) pour le contrôle de la luminosité par PWM. Pour un démarrage rapide, un repli GPIO haut/bas
  sur `PJ6` est acceptable.
- La réinitialisation du panneau est mappée sur `PG3` (LCD_RESET). Appliquez des délais conformes à la fiche technique
  entre l'état bas/haut de la réinitialisation et l'initialisation du lien DSI.
```
```
