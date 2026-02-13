```markdown
<!--
examples/stm32h747i-disco/DEBUG-SETUP.md - Configuration de débogage STM32 (OpenOCD, GDB), LaunchAgent macOS et recettes de récupération.
-->

# Configuration de débogage

Ce guide documente un flux de travail fiable OpenOCD + GDB pour les cartes STM32H7 (par exemple, STM32H747I-DISCO), y compris un LaunchAgent macOS pour maintenir OpenOCD en cours d'exécution, un flux de reconnexion automatique GDB résilient, et des commandes de récupération lorsque SWD/JTAG se trouve dans un état incorrect.

## LaunchAgent macOS pour OpenOCD

Créez un LaunchAgent pour démarrer OpenOCD à la connexion et le redémarrer s'il plante.

1) Créez `~/Library/LaunchAgents/com.softoboros.openocd.plist` avec le contenu ci-dessous.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.softoboros.openocd</string>

  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/openocd</string>
    <string>-f</string><string>interface/stlink.cfg</string>
    <string>-f</string><string>target/stm32h7x.cfg</string>
    <string>-c</string>
    <string>transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init</string>
  </array>

  <!-- redémarre s'il meurt -->
  <key>KeepAlive</key>
  <true/>

  <!-- fichiers journaux -->
  <key>StandardOutPath</key>
  <string>/tmp/openocd.out.log</string>
  <key>StandardErrorPath</string>
  <string>/tmp/openocd.err.log</string>

  <!-- démarrer à la connexion -->
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
```

- Ajustez `/usr/local/bin/openocd` à l'emplacement où Homebrew l'a installé : `$(brew --prefix open-ocd)/bin/openocd`.

2) Charger et décharger :

```bash
launchctl load ~/Library/LaunchAgents/com.softoboros.openocd.plist
launchctl unload ~/Library/LaunchAgents/com.softoboros.openocd.plist
```

3) Statut et journaux :

```bash
launchctl list | grep openocd
tail -f /tmp/openocd.out.log /tmp/openocd.err.log
```

## Configuration du serveur GDB (reconnexion automatique)

Deux approches résilientes pour maintenir GDB attaché à OpenOCD même si le fil SWD tombe.

### Option A — Wrapper Shell qui relance GDB

`debug-gdb.sh` :

```bash
#!/usr/bin/env bash
set -euo pipefail
while :; do
  gdb -q --command=gdb_init.gdb || true
  echo "[gdb] déconnecté; nouvelle tentative dans 1s"; sleep 1
done
```

`gdb_init.gdb` (léger, résilient) :

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 2

# connecter; si OpenOCD n'est pas encore prêt, GDB se ferme et le wrapper le redémarre
target extended-remote :3333

# réveil typique de session STM32H7
monitor reset halt
monitor arm semihosting enable

# rester attaché; si le fil tombe, GDB se ferme → le wrapper relance
```

### Option B — Boucle de "reconnexion" intégrée à GDB (sans wrapper)

```gdb
set pagination off
set confirm off
set target-async on
set remotetimeout 1

define _reconnect
  while 1
    echo [gdb] tentative sur :3333...\n
    if $_target_connected
      monitor reset halt
      break
    end
    set timeout 1
    target extended-remote :3333
    sleep 1
  end
end

_reconnect

## OpenOCD double cœur

- Utilisez `openocd/stm32h747_dual_core.cfg` (SWD lent + connexion sous réinitialisation) :
  - Ports GDB : CM7 sur 3333, CM4 sur 3334
  - Exécuter : `make openocd-dual`
- Ordre de lancement : attacher CM7 d'abord, puis CM4.

## SVDs

- Placez les SVD du périphérique dans `.svd/` et pointez les configurations de lancement vers :
  - CM7 : `.svd/STM32H747_CM7.svd`
  - CM4 : `.svd/STM32H747_CM4.svd`
```

Si votre GDB ne dispose pas de `$_target_connected`, préférez le wrapper shell.

Fonctionnalités souhaitables :

- Maintenir les ports stables : configurez `gdb_port 3333`, `telnet_port 4444`, `tcl_port 6666` dans votre commande OpenOCD ou votre fichier `.cfg`.
- Backoff : si la sonde est débranchée, ajoutez un délai exponentiel pour éviter le spam de logs.
- Vérification de l'état : un petit watchdog peut `nc -z localhost 3333` et redémarrer si nécessaire (principalement redondant avec LaunchAgent/systemd).
- Quirks de la cible (H7) : ralentir et se connecter sous réinitialisation aide souvent :
  `-c "transport select hla_swd; adapter speed 100; reset_config srst_only srst_nogate connect_assert_srst; init"`

## Récupération du débogage (STM32H7)

Si OpenOCD échoue pendant l'"examen" (par exemple, ne peut pas lire `DBGMCU` à `0x5C001004`) et que SWD/JTAG devient invalide, c'est généralement un problème de synchronisation de la connexion, d'horloge SWD trop rapide ou d d'une cible protégée/basse consommation. Essayez, dans l'ordre :

1) SWD lent + connexion sous réinitialisation (solution la plus courante) :

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; flash erase_address 0x08000000 0x200000; exit"
```

2) Utilisez l'effacement de masse du pilote (banque 0 ; ajoutez la banque 1 si double banque) :

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 0; exit"
```

Pour les pièces à double banque, exécutez également :

```bash
openocd -f interface/stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x mass_erase 1; exit"
```

3) Si la pièce est verrouillée pour le débogage (RDP), déverrouillez (cela efface en masse) :

```bash
openocd -f interface/stlink.stlink.cfg -f target/stm32h7x.cfg \
  -c "transport select hla_swd; adapter speed 100; \
      reset_config srst_only srst_nogate connect_assert_srst; \
      init; reset halt; stm32h7x unlock 0; exit"
```
```
