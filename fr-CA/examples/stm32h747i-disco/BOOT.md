```markdown
# Notes de démarrage du STM32H747I-DISCO (CM7)

Ce fichier résume l'état essentiel, les décisions et les prochaines étapes pour le démarrage du noyau CM7 de la cible STM32H747I-DISCO dans ce dépôt.

## Cible, Compilation, Débogage
- Cible : STM32H747I-DISCO (noyau CM7)
- Tâche de compilation : `build-disco (cm7)` avec les fonctionnalités :
  - `stm32h747i_disco_cm7,dma2d,backlight_pwm,pac_sdram_init,sdram_ramtest`
- Lancement VSCode : « CM7 attach (external OpenOCD) »
  - `runToEntryPoint: main`
  - Aucune commande de semihosting post-lancement

## Instantané de la séquence de démarrage
Le chemin CM7 exécute désormais systématiquement les étapes de démarrage initiales :

1. Courte attente active pour que les débogueurs puissent s'attacher avant les effets secondaires périphériques.
2. Les régions MPU (DTCM/ITCM/SRAM/SRAM4/SRAM3/SRAM1 + SDRAM) sont programmées avant tout accès aux registres RCC ou FMC.
3. Les GPIO SDRAM sont forcés en AF12/Très Haute et l'horloge du noyau FMC est activée ; la séquence de commandes SDRAM s'exécute directement via le PAC pendant que l'appareil est toujours sur l'arbre d'horloge de réinitialisation.
4. Le PWR SMPS + VOS1 sont configurés une fois que la SDRAM est stable.
5. La configuration HAL RCC configure PLL1/PLL3 et réactive le FMC pour un fonctionnement normal, suivie de l'initialisation du panneau/rétroéclairage.

## Horloges (Actuelles)
- HSE = 25 MHz ; SYSCLK = 400 MHz (PLL1)
- LTDC : PLL3R ≈ 32 MHz (chemin d'horloge pixel)
- Horloge noyau FMC (post-HAL) = PLL2R 150 MHz → SDCLK ≈ 75 MHz (diviseur /2)
- AHB (HCLK) = 200 MHz via HPRE=/2

## État de la SDRAM (FMC)
- Profil de l'appareil : IS42S32800G (32 MiB, x32) avec SDCLK maintenant cadencé à ~75 MHz (noyau FMC 150 MHz / 2).
- L'initialisation pilotée par le PAC se produit maintenant avant que le HAL ne touche RCC/PWR :
  - GPIO D/E/F/G/H/I acheminés vers AF12 + vitesse Très Haute avant tout changement d'horloge.
  - Horloges FMC activées via `AHB3ENR` et `C1_AHB3ENR`.
  - SDCR1/SDTR1/SDCMR/SDRTR écrits directement ; le sondage BUSY vérifie que chaque commande se termine au taux d'horloge le plus élevé.
- Vérifié : l'instrumentation MPU enregistre toujours l'état de la région (`MPU_TRACE`/`MPU_DUMP`) ; avec le SDCLK de 75 MHz (PLL2R 150 MHz avec diviseur /2), l'allocateur/les tests SDRAM restent stables après le démarrage.
- Journalisation : le semihosting reste désactivé sur le chemin critique de la SDRAM pour éviter les blocages SWD.

## Optimisation / Étapes
- Profil de développement : `opt-level = 0` pour l'espace de travail ; `stm32h7xx-hal` reste non optimisé pour le débogage.
- La rotation d'attachement au démarrage a été supprimée (pas de délai initial long).

## Observations connues
- Le pas doit atterrir après chaque écriture PAC `SDCMR` ; l'arrêt avant que BUSY ne s'efface peut laisser le contrôleur attendre indéfiniment.
- Le sondage de la visionneuse périphérique peut toujours interférer avec l'initialisation de la SDRAM ; gardez-la fermée pendant le déroulement pas à pas de la séquence.
- Le bouton de pause de VS Code peut émettre `reset halt` ; utilisez plutôt `monitor halt` et fiez-vous aux "miettes de pain" `.noinit` pour la reconstruction post-réinitialisation.
- La vue désassemblage reste inestimable pour vérifier les écritures brutes des registres PAC.

## Stratégie de pas minimale et fiable
1) Placez des points d'arrêt sur l'assistant PAC juste après chaque écriture `SDCMR` dans `configure_fmc_sdram`.
2) Après que le point d'arrêt est atteint, lisez :
   - `SDCMR @ 0x5200_4140` pour confirmer MODE/CTBx.
   - `SDSR @ 0x5200_4158` et attendez BUSY=0 avant de continuer.
3) Continuez à travers ClkEnable → délai → PALL → AR(8) → LoadMode → programmation SDRTR.
4) Si des erreurs MPU apparaissent toujours, compilez temporairement avec `--features skip_sdram_mpu` ; une fois que le MPU est terminé, inspectez `MPU_TRACE`/`MPU_DUMP` pour l'état final de la région.

## Propositions pour aller de l'avant
- Piégeage des pannes : ajoutez des gestionnaires d'exceptions CM7 pour s'arrêter sur place en cas de panne
  - Implémentez `HardFault`, `BusFault`, `UsageFault` avec des boucles `bkpt()` afin que les pannes ne retombent jamais en réinitialisation
- Délai du compteur de cycles DWT (optionnel) : utilisez CYCCNT pour des délais déterministes et insensibles au débogueur au lieu de la rotation asm (les deux sont OK ; CYCCNT facilite le réglage)
- Attentes entre les commandes : si un `SDSR.BUSY` ne s'efface pas, prolongez la boucle d'attente ou ajoutez un court délai entre les écritures `SDCMR` dans `configure_fmc_sdram()`
- Vérification croisée CubeIDE : générez l'initialisation FMC/SDRAM pour H747I-DISCO et reproduisez les valeurs exactes SDCR/SDTR/SDCMR/SDRTR pour une comparaison équitable
- Vérification de l'oscilloscope (matériel) : vérifiez le SDCLK sur PG8 et que SDCKE1 est affirmé avant PALL (PH7)
- Allocation de mémoire : gardez le tas/la pile par défaut dans le DTCM et introduisez un deuxième allocateur adossé à la `SDRAM`
  - Définissez une section de sortie `.sdram_heap` dans `memory.x`
  - Soutenez-la avec un statique Rust utilisant `#[link_section = ".sdram_heap"]` et initialisez un `Heap`/allocateur de bloc dédié
  - Protégez l'allocateur SDRAM par un mutex afin que les composants à forte empreinte optent explicitement pour cela

### Gestionnaires d'exceptions
Toujours recommandé : ajoutez des gestionnaires `HardFault`, `BusFault`, `UsageFault` qui bouclent sur `bkpt()` afin que les pannes ne réinitialisent pas le CM7. Lorsqu'elles se déclenchent, lisez `0xE000_ED28`, `0xE000_ED2C`, `0xE000_ED34`, `0xE000_ED38` pour en identifier la cause.

## Référence rapide (Adresses)
- Base FMC : `0x5200_4000`
  - `BCR1..` : `0x5200_4000`
  - `SDCR1/2` : `0x5200_4080`
  - `SDTR1/2` : `0x5200_4104`
- `SDCMR` : `0x5200_4140`
- `SDSR` : `0x5200_4158`
- `MPU_TRACE` : `0x2001_0030`
- `MPU_DUMP` : `0x2001_0034` (paires de RBAR/RASR écrites lors du démarrage MPU)

Vérifications typiques lors de l'exécution pas à pas :
- Après chaque écriture `SDCMR`, lisez `SDSR` et confirmez que BUSY s'efface (commande terminée) avant d'émettre la suivante.
- Vérifiez que `BCR1.FMCEN` est défini par le HAL (`memory_controller_enable()`).
- Confirmez que les champs `SDCR` correspondent aux CAS/largeur/banques/col/ligne souhaités ; `SDTR` respecte le timing du SDCLK.

## Paramètres par défaut actuels dans le code
- FMCSEL : `PLL2R` @ `100 MHz`
- `max_sd_clock_hz: 75_000_000`
- `hclk: 200_000_000` (HPRE=/2)
- Vitesses des broches : Très Haute pour toutes les broches FMC
- Pas de journalisation semihost SDRAM ; pas de rotation de démarrage initiale

## Ce dont Codex a besoin (essentiels)
- Confirmation du stade `configure_fmc_sdram` où `SDSR.BUSY` pourrait rester bloqué (après ClkEnable, PALL, AR(8) ou LoadMode).
- Instantanés de registre uniques autour de chaque commande :
  - `p/x *(u32*)0x52004140` (SDCMR) et `p/x *(u32*)0x52004158` (SDSR).
  - `p/x *(u32*)0x52004080`/`0x52004084` (SDCR1/2) et `0x52004104`/`0x52004108` (SDTR1/2).
- Contexte de panne si quelque chose se déclenche : CFSR/HFSR/MMFAR/BFAR + "miettes de pain" `.noinit`.
- Les captures d'oscilloscope de PG8 (SDCLK) et PH7 (SDCKE1) restent utiles pour la validation du timing.
- Vidage des registres CubeIDE (si disponible) pour la vérification croisée des valeurs SDCR/SDTR/SDCMR/SDRTR.

## Aide-mémoire GDB (Pas à Pas)
- Interrompez juste après chaque écriture `SDCMR` à l'intérieur de `configure_fmc_sdram`.
  1) Laissez le STR s'exécuter, puis `p/x $lr` / `tbreak *$lr` au besoin pour revenir à l'appelant.
  2) Inspectez `SDCMR`/`SDSR` ; attendez que BUSY s'efface avant de continuer.
- Si vous devez exécuter le STR pas à pas, `x/8i $pc`, `ni` à travers la mémoire, puis `finish` pour revenir à l'assistant.
- Préférez `monitor halt` pour mettre en pause ; `interrupt` émet souvent une réinitialisation. Si une réinitialisation se produit, récupérez le contexte à partir de `MPU_TRACE`/`MPU_DUMP`.

## Résumé des broches matérielles (FMC SDRAM)
- Horloge/Activation : PG8 (SDCLK), PH7 (SDCKE1), PH6 (SDNE1)
- Contrôle : PF11 (SDNRAS), PG15 (SDNCAS), PH5 (SDNWE)
- Adresse de banque : PG4 (BA0), PG5 (BA1)
- Adresse : PF0..PF5 (A0..A5), PF12..PF15 (A6..A9), PG0..PG2 (A10..A12)
- Pistes d'octets : PE0 (NBL0), PE1 (NBL1), PI4 (NBL2), PI5 (NBL3)
- Données : PD14..PD15, PD0..PD1, PE7..PE15, PD8..PD10, PH8..PH15, PI0..PI3, PI6, PI7, PI9, PI10
- Tous les éléments ci-dessus sont définis en AF12 + Vitesse::Très Haute dans le code

## Prochaines actions (minimales)
1) Ajoutez des gestionnaires d'exceptions CM7 pour piéger immédiatement les pannes.
2) Continuez à valider chaque commande SDRAM avec des points d'arrêt après `SDCMR` et ajustez le sondage BUSY si nécessaire.
3) Reproduisez les timings CubeIDE si des différences matérielles apparaissent ; prolongez le délai inter-commandes si nécessaire.
4) Envisagez de restaurer un `opt-level` modeste une fois que le démarrage reste stable.
5) Continuez à capturer `MPU_TRACE`/`MPU_DUMP` ; ils survivent aux réinitialisations et confirment l'état MPU.

## Démarrage SDRAM (Initialisation PAC déroulée)

Contexte : l'exécution pas à pas de l'assistant HAL était fragile sous le débogueur, de sorte que la fonctionnalité `pac_sdram_init` déroule maintenant la séquence d'initialisation avec des écritures PAC explicites.

- Fonctionnalité : `pac_sdram_init` (par défaut dans la compilation CM7).
- Séquence (base FMC 0x5200_4000) :
  - Active les horloges FMC via `AHB3ENR.FMCEN` et `C1_AHB3ENR.FMCEN` (avant HAL RCC).
  - `BCR1.FMCEN = 1` (activation du contrôleur).
  - Programme `SDCR1` pour IS42S32800G : NC=9, NR=12, MWID=32 bits, NB=4, CAS=3, SDCLK=/2, RBURST=1, RPIPE=0.
- Programme les timings `SDTR1` (@ ~75 MHz SDCLK) : TMRD=2 cycles (valeur d'écriture 1), TXSR=7 cycles (valeur 6), TRAS=5 cycles (valeur 4), TRC=7 cycles (valeur 6), TWR=2 cycles (valeur 1), TRP=2 cycles (valeur 1), TRCD=2 cycles (valeur 1).
  - Émet les commandes via `SDCMR` avec sondage BUSY entre chaque :
    1) Activation de l'horloge (MODE=1, CTB1=1).
    2) Précharge Tout (MODE=2, CTB1=1).
    3) Auto-Rafraîchissement ×8 (MODE=3, NRFS=7 encodant 8 cycles).
    4) Registre de mode de chargement (MODE=4, MRD=0x0230, CTB1=1).
- Programme `SDRTR` pour ~7,81 µs à 75 MHz : COUNT ≈ 566 (écrire `COUNT<<1`).

Observé (bon) : `SDCMR.MODE=4` (LoadMode) et `SDSR.BUSY=0` après la séquence. Le chemin PAC revient à l'appelant de manière fiable.

### Région MPU pour SDRAM (M7)
Une fois que le HAL active le MPU, la SDRAM externe doit être couverte par une région MPU ou la première lecture peut provoquer un MemManage/BusFault. Nous installons une région MPU SDRAM immédiatement après l'initialisation du PAC :

- Base de la région : `0xC000_0000` ; taille : 32 MiB (champ SIZE = 24)
- Attributs : Mémoire normale, non-cacheable (TEX=1, C=0, B=0), Partageable=1, AP=Accès complet
- MPU activé avec PRIVDEFENA, plus barrières DSB/ISB

Résultat : les premières lectures de SDRAM ne provoquent plus de pannes ; les sondes rapides réussissent.

## Configuration de débogage VS Code (Cortex-Debug)

Principaux enseignements pour stabiliser l'attachement/l'exécution sur H7 avec OpenOCD :
- Évitez d'émettre `monitor reset halt` après l'attachement — cela force une nouvelle analyse et atterrit à Reset (PC=0x08000298), effaçant l'état FMC.
- Préférez un attachement pur qui s'exécute immédiatement :
  - Ajoutez une configuration avec `request: "attach"`, `servertype: "external"` et `postAttachCommands: ["continue"]`.
- Chargez les macros GDB en toute sécurité :
  - `preLaunchCommands` : `set mem inaccessible-by-default off`, `add-auto-load-safe-path ${workspaceFolder}/.gdbinit`, `source ${workspaceFolder}/.gdbinit`
- Gardez la vue Registres Périphériques fermée pendant le démarrage du FMC/SDRAM ; les sondages fréquents peuvent provoquer des erreurs SWD "Occupé".
- Les configurations de lancement définissent `objdumpPath` + `showDisassembly: "always"` ; la vue Désassemblage affichera les instructions même en l'absence de symboles.

Entrée de lancement minimale attachement+exécution (extrait) :

```
{
  "name": "CM7 attach + run (no reset)",
  "type": "cortex-debug",
  "request": "attach",
  "servertype": "external",
  "gdbTarget": "localhost:3333",
  "preLaunchCommands": [
    "set mem inaccessible-by-default off",
    "add-auto-load-safe-path ${workspaceFolder}/.gdbinit",
    "source ${workspaceFolder}/.gdbinit"
  ],
  "postAttachCommands": ["continue"]
}
```

### Invocation OpenOCD (recommandée)
- Utilisez SWD lent + connexion sous réinitialisation pendant le démarrage :
  - `transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init`

## Macros et tactiques GDB

- Assistants `.gdbinit` (à la racine du dépôt) : `faultregs`, `sdramregs`, `wait_busy_clear`, `lrtrap_safe`.
- lrtrap vs BKPT en ligne :
  - `lrtrap` est efficace aux vrais sites d'appel ; `bkpt` en ligne n'a pas de LR que vous pouvez `tbreak` — utilisez `set $pc = $pc + 2` pour passer.
- Les points de surveillance sur `SDCMR @ 0x5200_4140` sont utiles, mais peuvent provoquer une agitation ; préférez le chemin PAC + les dumps `sdramregs` à la place.

## Étapes de validation (Rapide)
- Après l'exécution de l'initialisation du PAC, vérifiez rapidement la SDRAM via la console de débogage :
  - `set {unsigned int}0xC0000000 = 0xDEADBEEF`
  - `x/wx 0xC0000000` → `0xDEADBEEF`
- Vérifications de l'état du FMC :
  - `x/wx 0x52004140` (SDCMR) → MODE=4 après LoadMode
  - `x/wx 0x52004158` (SDSR) → bit 5 BUSY = 0
  - `x/wx 0x52004080` (SDCR1), `0x52004104` (SDTR1) → valeurs non par défaut

## Chemins HAL vs PAC

- `pac_sdram_init` est le chemin par défaut et s'exécute maintenant avant tout changement d'horloge/alimentation HAL ; il a été validé de bout en bout.
- L'ancien assistant HAL (`hal_sdram`) reste disponible pour l'expérimentation mais ne fait plus partie de la compilation CM7 standard.

## Quirks connus (OpenOCD/H7)
- « read_memory … 0x5C001004 … examine-end failed » : bruit d'attachement inoffensif ; évitez les réinitialisations/examens fréquents de GDB.
- « target not halted … resume was requested » : se produit si Continuer est pressé alors que l'exécution est déjà en cours ; il suffit de faire Pause → Continuer.
- Si la connexion est instable, redémarrez OpenOCD avec SWD lent et connexion sous réinitialisation, puis attachez avec la configuration sans réinitialisation ci-dessus.
``````markdown
# Notes de démarrage du STM32H747I-DISCO (CM7)

Ce fichier résume l'état essentiel, les décisions et les prochaines étapes pour le démarrage du noyau CM7 de la cible STM32H747I-DISCO dans ce dépôt.

## Cible, Compilation, Débogage
- Cible : STM32H747I-DISCO (noyau CM7)
- Tâche de compilation : `build-disco (cm7)` avec les fonctionnalités :
  - `stm32h747i_disco_cm7,dma2d,backlight_pwm,pac_sdram_init,sdram_ramtest`
- Lancement VSCode : « CM7 attach (external OpenOCD) »
  - `runToEntryPoint: main`
  - Aucune commande de semihosting post-lancement

## Instantané de la séquence de démarrage
Le chemin CM7 exécute désormais systématiquement les étapes de démarrage initiales :

1. Courte attente active pour que les débogueurs puissent s'attacher avant les effets secondaires périphériques.
2. Les régions MPU (DTCM/ITCM/SRAM/SRAM4/SRAM3/SRAM1 + SDRAM) sont programmées avant tout accès aux registres RCC ou FMC.
3. Les GPIO SDRAM sont forcés en AF12/Très Haute et l'horloge du noyau FMC est activée ; la séquence de commandes SDRAM s'exécute directement via le PAC pendant que l'appareil est toujours sur l'arbre d'horloge de réinitialisation.
4. Le PWR SMPS + VOS1 sont configurés une fois que la SDRAM est stable.
5. La configuration HAL RCC configure PLL1/PLL3 et réactive le FMC pour un fonctionnement normal, suivie de l'initialisation du panneau/rétroéclairage.

## Horloges (Actuelles)
- HSE = 25 MHz ; SYSCLK = 400 MHz (PLL1)
- LTDC : PLL3R ≈ 32 MHz (chemin d'horloge pixel)
- Horloge noyau FMC (post-HAL) = PLL2R 150 MHz → SDCLK ≈ 75 MHz (diviseur /2)
- AHB (HCLK) = 200 MHz via HPRE=/2

## État de la SDRAM (FMC)
- Profil de l'appareil : IS42S32800G (32 MiB, x32) avec SDCLK maintenant cadencé à ~75 MHz (noyau FMC 150 MHz / 2).
- L'initialisation pilotée par le PAC se produit maintenant avant que le HAL ne touche RCC/PWR :
  - GPIO D/E/F/G/H/I acheminés vers AF12 + vitesse Très Haute avant tout changement d'horloge.
  - Horloges FMC activées via `AHB3ENR` et `C1_AHB3ENR`.
  - SDCR1/SDTR1/SDCMR/SDRTR écrits directement ; le sondage BUSY vérifie que chaque commande se termine au taux d'horloge le plus élevé.
- Vérifié : l'instrumentation MPU enregistre toujours l'état de la région (`MPU_TRACE`/`MPU_DUMP`) ; avec le SDCLK de 75 MHz (PLL2R 150 MHz avec diviseur /2), l'allocateur/les tests SDRAM restent stables après le démarrage.
- Journalisation : le semihosting reste désactivé sur le chemin critique de la SDRAM pour éviter les blocages SWD.

## Optimisation / Étapes
- Profil de développement : `opt-level = 0` pour l'espace de travail ; `stm32h7xx-hal` reste non optimisé pour le débogage.
- La rotation d'attachement au démarrage a été supprimée (pas de délai initial long).

## Observations connues
- Le pas doit atterrir après chaque écriture PAC `SDCMR` ; l'arrêt avant que BUSY ne s'efface peut laisser le contrôleur attendre indéfiniment.
- Le sondage de la visionneuse périphérique peut toujours interférer avec l'initialisation de la SDRAM ; gardez-la fermée pendant le déroulement pas à pas de la séquence.
- Le bouton de pause de VS Code peut émettre `reset halt` ; utilisez plutôt `monitor halt` et fiez-vous aux "miettes de pain" `.noinit` pour la reconstruction post-réinitialisation.
- La vue désassemblage reste inestimable pour vérifier les écritures brutes des registres PAC.

## Stratégie de pas minimale et fiable
1) Placez des points d'arrêt sur l'assistant PAC juste après chaque écriture `SDCMR` dans `configure_fmc_sdram`.
2) Après que le point d'arrêt est atteint, lisez :
   - `SDCMR @ 0x5200_4140` pour confirmer MODE/CTBx.
   - `SDSR @ 0x5200_4158` et attendez BUSY=0 avant de continuer.
3) Continuez à travers ClkEnable → délai → PALL → AR(8) → LoadMode → programmation SDRTR.
4) Si des erreurs MPU apparaissent toujours, compilez temporairement avec `--features skip_sdram_mpu` ; une fois que le MPU est terminé, inspectez `MPU_TRACE`/`MPU_DUMP` pour l'état final de la région.

## Propositions pour aller de l'avant
- Piégeage des pannes : ajoutez des gestionnaires d'exceptions CM7 pour s'arrêter sur place en cas de panne
  - Implémentez `HardFault`, `BusFault`, `UsageFault` avec des boucles `bkpt()` afin que les pannes ne retombent jamais en réinitialisation
- Délai du compteur de cycles DWT (optionnel) : utilisez CYCCNT pour des délais déterministes et insensibles au débogueur au lieu de la rotation asm (les deux sont OK ; CYCCNT facilite le réglage)
- Attentes entre les commandes : si un `SDSR.BUSY` ne s'efface pas, prolongez la boucle d'attente ou ajoutez un court délai entre les écritures `SDCMR` dans `configure_fmc_sdram()`
- Vérification croisée CubeIDE : générez l'initialisation FMC/SDRAM pour H747I-DISCO et reproduisez les valeurs exactes SDCR/SDTR/SDCMR/SDRTR pour une comparaison équitable
- Vérification de l'oscilloscope (matériel) : vérifiez le SDCLK sur PG8 et que SDCKE1 est affirmé avant PALL (PH7)
- Allocation de mémoire : gardez le tas/la pile par défaut dans le DTCM et introduisez un deuxième allocateur adossé à la `SDRAM`
  - Définissez une section de sortie `.sdram_heap` dans `memory.x`
  - Soutenez-la avec un statique Rust utilisant `#[link_section = ".sdram_heap"]` et initialisez un `Heap`/allocateur de bloc dédié
  - Protégez l'allocateur SDRAM par un mutex afin que les composants à forte empreinte optent explicitement pour cela

### Gestionnaires d'exceptions
Toujours recommandé : ajoutez des gestionnaires `HardFault`, `BusFault`, `UsageFault` qui bouclent sur `bkpt()` afin que les pannes ne réinitialisent pas le CM7. Lorsqu'elles se déclenchent, lisez `0xE000_ED28`, `0xE000_ED2C`, `0xE000_ED34`, `0xE000_ED38` pour en identifier la cause.

## Référence rapide (Adresses)
- Base FMC : `0x5200_4000`
  - `BCR1..` : `0x5200_4000`
  - `SDCR1/2` : `0x5200_4080`
  - `SDTR1/2` : `0x5200_4104`
- `SDCMR` : `0x5200_4140`
- `SDSR` : `0x5200_4158`
- `MPU_TRACE` : `0x2001_0030`
- `MPU_DUMP` : `0x2001_0034` (paires de RBAR/RASR écrites lors du démarrage MPU)

Vérifications typiques lors de l'exécution pas à pas :
- Après chaque écriture `SDCMR`, lisez `SDSR` et confirmez que BUSY s'efface (commande terminée) avant d'émettre la suivante.
- Vérifiez que `BCR1.FMCEN` est défini par le HAL (`memory_controller_enable()`).
- Confirmez que les champs `SDCR` correspondent aux CAS/largeur/banques/col/ligne souhaités ; `SDTR` respecte le timing du SDCLK.

## Paramètres par défaut actuels dans le code
- FMCSEL : `PLL2R` @ `100 MHz`
- `max_sd_clock_hz: 75_000_000`
- `hclk: 200_000_000` (HPRE=/2)
- Vitesses des broches : Très Haute pour toutes les broches FMC
- Pas de journalisation semihost SDRAM ; pas de rotation de démarrage initiale

## Ce dont Codex a besoin (essentiels)
- Confirmation du stade `configure_fmc_sdram` où `SDSR.BUSY` pourrait rester bloqué (après ClkEnable, PALL, AR(8) ou LoadMode).
- Instantanés de registre uniques autour de chaque commande :
  - `p/x *(u32*)0x52004140` (SDCMR) et `p/x *(u32*)0x52004158` (SDSR).
  - `p/x *(u32*)0x52004080`/`0x52004084` (SDCR1/2) et `0x52004104`/`0x52004108` (SDTR1/2).
- Contexte de panne si quelque chose se déclenche : CFSR/HFSR/MMFAR/BFAR + "miettes de pain" `.noinit`.
- Les captures d'oscilloscope de PG8 (SDCLK) et PH7 (SDCKE1) restent utiles pour la validation du timing.
- Vidage des registres CubeIDE (si disponible) pour la vérification croisée des valeurs SDCR/SDTR/SDCMR/SDRTR.

## Aide-mémoire GDB (Pas à Pas)
- Interrompez juste après chaque écriture `SDCMR` à l'intérieur de `configure_fmc_sdram`.
  1) Laissez le STR s'exécuter, puis `p/x $lr` / `tbreak *$lr` au besoin pour revenir à l'appelant.
  2) Inspectez `SDCMR`/`SDSR` ; attendez que BUSY s'efface avant de continuer.
- Si vous devez exécuter le STR pas à pas, `x/8i $pc`, `ni` à travers la mémoire, puis `finish` pour revenir à l'assistant.
- Préférez `monitor halt` pour mettre en pause ; `interrupt` émet souvent une réinitialisation. Si une réinitialisation se produit, récupérez le contexte à partir de `MPU_TRACE`/`MPU_DUMP`.

## Résumé des broches matérielles (FMC SDRAM)
- Horloge/Activation : PG8 (SDCLK), PH7 (SDCKE1), PH6 (SDNE1)
- Contrôle : PF11 (SDNRAS), PG15 (SDNCAS), PH5 (SDNWE)
- Adresse de banque : PG4 (BA0), PG5 (BA1)
- Adresse : PF0..PF5 (A0..A5), PF12..PF15 (A6..A9), PG0..PG2 (A10..A12)
- Pistes d'octets : PE0 (NBL0), PE1 (NBL1), PI4 (NBL2), PI5 (NBL3)
- Données : PD14..PD15, PD0..PD1, PE7..PE15, PD8..PD10, PH8..PH15, PI0..PI3, PI6, PI7, PI9, PI10
- Tous les éléments ci-dessus sont définis en AF12 + Vitesse::Très Haute dans le code

## Prochaines actions (minimales)
1) Ajoutez des gestionnaires d'exceptions CM7 pour piéger immédiatement les pannes.
2) Continuez à valider chaque commande SDRAM avec des points d'arrêt après `SDCMR` et ajustez le sondage BUSY si nécessaire.
3) Reproduisez les timings CubeIDE si des différences matérielles apparaissent ; prolongez le délai inter-commandes si nécessaire.
4) Envisagez de restaurer un `opt-level` modeste une fois que le démarrage reste stable.
5) Continuez à capturer `MPU_TRACE`/`MPU_DUMP` ; ils survivent aux réinitialisations et confirment l'état MPU.

## Démarrage SDRAM (Initialisation PAC déroulée)

Contexte : l'exécution pas à pas de l'assistant HAL était fragile sous le débogueur, de sorte que la fonctionnalité `pac_sdram_init` déroule maintenant la séquence d'initialisation avec des écritures PAC explicites.

- Fonctionnalité : `pac_sdram_init` (par défaut dans la compilation CM7).
- Séquence (base FMC 0x5200_4000) :
  - Active les horloges FMC via `AHB3ENR.FMCEN` et `C1_AHB3ENR.FMCEN` (avant HAL RCC).
  - `BCR1.FMCEN = 1` (activation du contrôleur).
  - Programme `SDCR1` pour IS42S32800G : NC=9, NR=12, MWID=32 bits, NB=4, CAS=3, SDCLK=/2, RBURST=1, RPIPE=0.
- Programme les timings `SDTR1` (@ ~75 MHz SDCLK) : TMRD=2 cycles (valeur d'écriture 1), TXSR=7 cycles (valeur 6), TRAS=5 cycles (valeur 4), TRC=7 cycles (valeur 6), TWR=2 cycles (valeur 1), TRP=2 cycles (valeur 1), TRCD=2 cycles (valeur 1).
  - Émet les commandes via `SDCMR` avec sondage BUSY entre chaque :
    1) Activation de l'horloge (MODE=1, CTB1=1).
    2) Précharge Tout (MODE=2, CTB1=1).
    3) Auto-Rafraîchissement ×8 (MODE=3, NRFS=7 encodant 8 cycles).
    4) Registre de mode de chargement (MODE=4, MRD=0x0230, CTB1=1).
- Programme `SDRTR` pour ~7,81 µs à 75 MHz : COUNT ≈ 566 (écrire `COUNT<<1`).

Observé (bon) : `SDCMR.MODE=4` (LoadMode) et `SDSR.BUSY=0` après la séquence. Le chemin PAC revient à l'appelant de manière fiable.

### Région MPU pour SDRAM (M7)
Une fois que le HAL active le MPU, la SDRAM externe doit être couverte par une région MPU ou la première lecture peut provoquer un MemManage/BusFault. Nous installons une région MPU SDRAM immédiatement après l'initialisation du PAC :

- Base de la région : `0xC000_0000` ; taille : 32 MiB (champ SIZE = 24)
- Attributs : Mémoire normale, non-cacheable (TEX=1, C=0, B=0), Partageable=1, AP=Accès complet
- MPU activé avec PRIVDEFENA, plus barrières DSB/ISB

Résultat : les premières lectures de SDRAM ne provoquent plus de pannes ; les sondes rapides réussissent.

## Configuration de débogage VS Code (Cortex-Debug)

Principaux enseignements pour stabiliser l'attachement/l'exécution sur H7 avec OpenOCD :
- Évitez d'émettre `monitor reset halt` après l'attachement — cela force une nouvelle analyse et atterrit à Reset (PC=0x08000298), effaçant l'état FMC.
- Préférez un attachement pur qui s'exécute immédiatement :
  - Ajoutez une configuration avec `request: "attach"`, `servertype: "external"` et `postAttachCommands: ["continue"]`.
- Chargez les macros GDB en toute sécurité :
  - `preLaunchCommands` : `set mem inaccessible-by-default off`, `add-auto-load-safe-path ${workspaceFolder}/.gdbinit`, `source ${workspaceFolder}/.gdbinit`
- Gardez la vue Registres Périphériques fermée pendant le démarrage du FMC/SDRAM ; les sondages fréquents peuvent provoquer des erreurs SWD "Occupé".
- Les configurations de lancement définissent `objdumpPath` + `showDisassembly: "always"` ; la vue Désassemblage affichera les instructions même en l'absence de symboles.

Entrée de lancement minimale attachement+exécution (extrait) :

```
{
  "name": "CM7 attach + run (no reset)",
  "type": "cortex-debug",
  "request": "attach",
  "servertype": "external",
  "gdbTarget": "localhost:3333",
  "preLaunchCommands": [
    "set mem inaccessible-by-default off",
    "add-auto-load-safe-path ${workspaceFolder}/.gdbinit",
    "source ${workspaceFolder}/.gdbinit"
  ],
  "postAttachCommands": ["continue"]
}
```

### Invocation OpenOCD (recommandée)
- Utilisez SWD lent + connexion sous réinitialisation pendant le démarrage :
  - `transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init`

## Macros et tactiques GDB

- Assistants `.gdbinit` (à la racine du dépôt) : `faultregs`, `sdramregs`, `wait_busy_clear`, `lrtrap_safe`.
- lrtrap vs BKPT en ligne :
  - `lrtrap` est efficace aux vrais sites d'appel ; `bkpt` en ligne n'a pas de LR que vous pouvez `tbreak` — utilisez `set $pc = $pc + 2` pour passer.
- Les points de surveillance sur `SDCMR @ 0x5200_4140` sont utiles, mais peuvent provoquer une agitation ; préférez le chemin PAC + les dumps `sdramregs` à la place.

## Étapes de validation (Rapide)
- Après l'exécution de l'initialisation du PAC, vérifiez rapidement la SDRAM via la console de débogage :
  - `set {unsigned int}0xC0000000 = 0xDEADBEEF`
  - `x/wx 0xC0000000` → `0xDEADBEEF`
- Vérifications de l'état du FMC :
  - `x/wx 0x52004140` (SDCMR) → MODE=4 après LoadMode
  - `x/wx 0x52004158` (SDSR) → bit 5 BUSY = 0
  - `x/wx 0x52004080` (SDCR1), `0x52004104` (SDTR1) → valeurs non par défaut

## Chemins HAL vs PAC

- `pac_sdram_init` est le chemin par défaut et s'exécute maintenant avant tout changement d'horloge/alimentation HAL ; il a été validé de bout en bout.
- L'ancien assistant HAL (`hal_sdram`) reste disponible pour l'expérimentation mais ne fait plus partie de la compilation CM7 standard.

## Quirks connus (OpenOCD/H7)
- « read_memory … 0x5C001004 … examine-end failed » : bruit d'attachement inoffensif ; évitez les réinitialisations/examens fréquents de GDB.
- « target not halted … resume was requested » : se produit si Continuer est pressé alors que l'exécution est déjà en cours ; il suffit de faire Pause → Continuer.
- Si la connexion est instable, redémarrez OpenOCD avec SWD lent et connexion sous réinitialisation, puis attachez avec la configuration sans réinitialisation ci-dessus.
```
