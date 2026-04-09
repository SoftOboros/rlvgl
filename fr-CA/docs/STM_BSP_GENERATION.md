<!--
  STM_BSP_GENERATION.md — Comportement et drapeaux du générateur BSP STM32
  Couvre les entrées, les sorties, les surcharges d'environnement, le support multi-cœur,
  l'ensemble des fonctionnalités actuelles et une feuille de route pour les améliorations.
-->

# Génération du BSP STM32

Ce document explique comment le générateur BSP rlvgl‑creator consomme les fichiers `.ioc` de CubeMX pour produire du code de support de carte STM32 (styles PAC et HAL), comment fonctionnent les surcharges d'environnement, quels drapeaux sont pris en charge et le comportement actuel pour les pièces double-cœur comme le STM32H747. Il décrit également un plan pour améliorer le générateur afin de couvrir des fonctionnalités STM supplémentaires, couramment utilisées.

## Entrées

- Fichier `.ioc` CubeMX pour la carte/MCU cible. Le générateur analyse :
  - MCU, boîtier, broches (fonction/AF), étiquettes utilisateur
  - Instances de périphériques et sélections d'horloge de noyau
  - Paramètres d'alimentation : alimentation (SMPS/LDO), VOS/SDLEVEL
  - Intention d'horloge : source SYSCLK, source PLL, valeur HSE, PLL1..3 M/N/P/Q/R, pré-diviseurs (D1/D2/D3)

- Surcharges d'environnement (majuscules "STM32_SECTION_KEY") :
  - `STM32_PWR_SUPPLY=SMPS|LDO` → surcharge PWR.Supply du fichier `.ioc`
  - `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3` → surcharge PWR.SDLEVEL (VOS) du fichier `.ioc`
  - Réservé pour usage futur (pas encore appliqué) :
    - `STM32_RCC_HSE_HZ=<Hz>` → surcharge/définit la fréquence HSE si absente
    - Mappage générique "Camel→SCREAM" : les clés de la forme `STM32_<SECTION>_<KEY>` seront reconnues à mesure que nous ajouterons du support

## Sorties

- BSP de style PAC (niveau registre) : `pac.rs`
- BSP de style HAL (stm32xx‑hal) : `hal.rs`
- Un seul fichier par défaut, ou divisé en sorties par cœur sous `cm7/` et `cm4/` pour les pièces double-cœur
- Module de constantes d'étiquettes optionnel basé sur les entrées `GPIO_Label`

## Comportement du générateur (focalisation H7)

- Broches
  - Configure le mode GPIO/AF/pull/OD/speed en fonction du `.ioc`
  - Émet des écritures de registres groupées sur demande
  - Émet optionnellement des constantes d'étiquettes et/ou utilise des identifiants basés sur les étiquettes

- Alimentation (PWR) sur STM32H7
  - Active les mises à jour de l'alimentation/VOS via le bit SCUEN (écritures cadencées)
  - Sélectionne l'alimentation (SMPS/LDO) et, si SMPS, définit SDLEVEL à partir de VOS
  - Programme le VOS cible dans `PWR.D3CR.VOS[15:14]` via des bits bruts
  - Attend `ACTVOSRDY` avant de continuer

- Horloges (RCC)
  - Analyse l'intention d'horloge du `.ioc` : source SYSCLK, source PLL, HSE, paramètres PLL1, pré-diviseurs D1/D2/D3
  - Hook `init_clocks(&dp)` pour CM7 uniquement :
    - HSI/HSE SYSCLK : active la source et bascule `CFGR.SW`
    - PLL1 SYSCLK : configure `PLLCKSELR` (PLLSRC/DIVM1), `PLL1DIVR` (N/P/Q/R), active DIVP1, active PLL1, bascule SYSCLK sur PLL1 et attend
    - Applique le pré-diviseur CPU D1 (D1CFGR.D1CPRE) et les pré-diviseurs APB (D1PPRE/APB3, D2PPRE1/APB1, D2PPRE2/APB2, D3PPRE/APB4) à partir des jetons `.ioc`
  - Journalisation légère (fonction `bsp_log`) : émet un résumé de SYSCLK/PLLSRC/HSE/pré-diviseurs via un sink `_bsp_log` faible

- Double-cœur (H747)
  - Détecte automatiquement la séparation lorsque les projets CM7 et CM4 sont présents dans le `.ioc`
  - Émet PAC/HAL par cœur avec les modules PAC corrects (`stm32h747cm7` / `stm32h747cm4`)
  - Boîte aux lettres réservée (1 Ko à `0x3004_7000` dans D2 SRAM3) pour la synchronisation/le transfert inter-cœurs
  - Assistants :
    - `signal_clocks_ready()` (cœur principal) définit le sémaphore et les SEV
    - `wait_for_clocks()` (cœur secondaire) attend sur le sémaphore via WFE

- Mémoire/éditeur de liens (exemple de projet)
  - `memory.x` pour CM7 : DTCM `RAM`, régions divisées D1 pour un placement futur, `MAILBOX` partagée, région de rétention D3 déclarée
  - `memory_cm4.x` pour CM4 : D2 `RAM`, même `MAILBOX`, D1 divisée et D3 rétention déclarées
  - `build.rs` de niveau supérieur met en scène le `memory*.x` correct en fonction du nom du binaire et passe `-Tlink.x`

## Drapeaux CLI (bsp from-ioc)

- Disposition et contenu
  - `--emit-pac` / `--emit-hal` — rend un ou les deux styles BSP
  - `--grouped-writes` — regroupe les écritures GPIO/RCC par registre pour la compacité
  - `--with-deinit` — émet des assistants de désinitialisation de base
  - `--use-label-names` — préfère les identifiants basés sur les étiquettes dans le BSP HAL
  - `--emit-label-consts` — émet le module `pins` avec des constantes d'étiquettes dans le BSP PAC
  - `--label-prefix <str>` — préfixe pour les étiquettes qui commencent par des chiffres/tirets bas

- Propriété du cœur
  - `--split-cores` — émet `cm7/` et `cm4/` en mode double-cœur ; activé automatiquement si les deux cœurs sont présents dans le `.ioc`
  - `--core cm7|cm4` — restreint la sortie unifiée à un seul cœur
  - `--clock-init-core cm7|cm4` — surcharge le cœur qui gère l'initialisation de l'horloge système (par défaut : CM7 pour H7x7)
  - `--periph-core name=core,...` — attribue la propriété d'un périphérique spécifique

## Drapeaux d'environnement (affectent le contexte du template)

- `STM32_PWR_SUPPLY=SMPS|LDO`
- `STM32_PWR_SDLEVEL=VOS0|VOS1|VOS2|VOS3`
- Réservé (prévu) : `STM32_RCC_HSE_HZ=<Hz>`

## Exemple d'assistant

Le script `examples/stm32h747i-disco/gen-bsp.sh` définit les valeurs par défaut pour SMPS/VOS1 et ne se régénère qu'en cas de besoin. Il tient compte de :

- `STM32_PWR_SUPPLY`, `STM32_PWR_SDLEVEL` — par défaut `SMPS`, `VOS1`
- `FORCE_BSP=1` — force la régénération

## Limitations actuelles

- La configuration PLL est adaptée au H7 et suppose des N/P/Q/R entiers. Les configurations fractionnaires et le réglage de la plage VCO ne sont pas encore émis.
- Le mappage des jetons du pré-diviseur d'horloge couvre les encodages CubeMX courants mais peut manquer des chaînes variantes.
- La synchronisation double-cœur utilise une simple boîte aux lettres ; pas encore de HSEM ou EXTI wake.
- Les horloges de noyau (multiplexeurs périphériques) sont partiellement émises ; l'expansion est en cours.

## Plan d'amélioration (Feuille de route)

1) Horloge
   - Ajouter le support PLL fractionnaire (FRACN) et la sélection de la plage VCO à partir du `.ioc`
   - Émettre le mappage complet des pré-diviseurs D1/D2/D3 pour les jetons variants
   - Étendre les mappages d'horloge de noyau (CCIPx) pour plus de périphériques
   - Fournir un chemin d'initialisation d'horloge HAL reflétant l'intention PAC pour les utilisateurs privilégiant HAL

2) Infrastructure double-cœur
   - Handshake optionnel basé sur HSEM comme alternative à la boîte aux lettres
   - Étendre la boîte aux lettres à un protocole structuré (en-tête versionné, commandes, acquittements)
   - Fournir un exemple de flux de démarrage CM4 qui active les horloges de domaine après le signal

3) Alimentation et basse consommation
   - Émettre des assistants de gestion du domaine de basse consommation (rétention D3, entrée/sortie STOP/Standby)
   - Supporter la configuration BOR/PVD/moniteur de tension à partir du `.ioc`

4) Broches et périphériques
   - Émettre des résumés/rapports de broches avec des références croisées d'étiquettes
   - Générer des assistants d'initialisation de périphériques pour les blocs courants (I2C/SPI/UART) à partir des rôles du `.ioc`

5) Couverture multi-familles
   - Étendre les modèles PWR/RCC pour les familles STM32H5, G4/L4/L5 avec des registres spécifiques à la famille
   - Sélection automatique des modules PAC par sous-famille (déjà fait pour H747 cm7/cm4)

6) DX et validation
   - Ajouter un mode `--verbose` qui journalise les paramètres d'alimentation/horloge appliqués via `bsp_log!`
   - Tests unitaires pour les mappages jeton → champ de bits à travers les familles

## Contribuer

Veuillez ouvrir des issues ou des PR avec des exemples de `.ioc` et les comportements souhaités. Incluez le MCU, le boîtier, la source d'horloge externe (HSE) et tout multiplexage de périphérique requis afin que nous puissions étendre le générateur en toute sécurité.
